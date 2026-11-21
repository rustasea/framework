//! Framework service providers: database, cache, and queue bindings at boot.
//!
//! Add the three providers to an application's provider list and boot it:
//!
//! - [`DatabaseServiceProvider`] binds an [`orm::ConnectionResolver`] built from
//!   `[database]` under [`DATABASE_KEY`]. It connects lazily, on the first
//!   [`orm::ConnectionResolver::resolve`], so boot does no I/O.
//! - [`CacheServiceProvider`] binds a [`cache::CacheManager`] built from
//!   `[cache]` under [`CACHE_KEY`].
//! - [`QueueServiceProvider`] registers the `[queue]` connections that need no
//!   database (`sync`, and `redis` with its feature) with the queue registry,
//!   records the default connection, and binds the parsed
//!   [`queue::QueueConfig`] under [`QUEUE_CONFIG_KEY`].
//!
//! Registering a `database` queue connection needs a live pool, which cannot
//! be opened inside a synchronous provider. After boot, inside the runtime,
//! call [`connect_database_queues`] to resolve each connection's pool through
//! the bound resolver and register those connections.
//!
//! Each provider reads the boot-time [`crate::ConfigLoader`]. A missing table
//! falls back to the component's defaults; a table that does not parse leaves
//! the binding out and prints why, so a bad config is visible at boot without
//! stopping it.

use std::sync::Arc;

use crate::foundation::Application;
use crate::{cache, orm, queue, ServiceProvider};

/// Container key of the bound [`orm::ConnectionResolver`].
pub const DATABASE_KEY: &str = "db.resolver";

/// Container key of the bound [`cache::CacheManager`].
pub const CACHE_KEY: &str = "cache.manager";

/// Container key of the bound [`queue::QueueConfig`].
pub const QUEUE_CONFIG_KEY: &str = "queue.config";

/// The bound connection resolver, if [`DatabaseServiceProvider`] registered one.
pub fn database(app: &Application) -> Option<Arc<orm::ConnectionResolver>> {
    app.container
        .get::<Arc<orm::ConnectionResolver>>(DATABASE_KEY)
        .cloned()
}

/// The bound cache manager, if [`CacheServiceProvider`] registered one.
pub fn cache(app: &Application) -> Option<Arc<cache::CacheManager>> {
    app.container
        .get::<Arc<cache::CacheManager>>(CACHE_KEY)
        .cloned()
}

/// The bound queue config, if [`QueueServiceProvider`] registered one.
pub fn queue_config(app: &Application) -> Option<Arc<queue::QueueConfig>> {
    app.container
        .get::<Arc<queue::QueueConfig>>(QUEUE_CONFIG_KEY)
        .cloned()
}

/// Binds the database connection resolver under [`DATABASE_KEY`].
pub struct DatabaseServiceProvider;

impl ServiceProvider for DatabaseServiceProvider {
    fn name(&self) -> &'static str {
        "DatabaseServiceProvider"
    }

    fn register(&self, app: &mut Application) {
        let Some(loader) = app.config().cloned() else {
            return;
        };
        match orm::ConnectionResolver::from_loader(&loader) {
            Ok(resolver) => app.container.instance(DATABASE_KEY, Arc::new(resolver)),
            Err(error) => eprintln!("database: invalid [database] config, not bound: {error}"),
        }
    }
}

/// Binds the cache manager under [`CACHE_KEY`].
pub struct CacheServiceProvider;

impl ServiceProvider for CacheServiceProvider {
    fn name(&self) -> &'static str {
        "CacheServiceProvider"
    }

    fn register(&self, app: &mut Application) {
        let Some(loader) = app.config().cloned() else {
            return;
        };
        match cache::CacheManager::configure_from(&loader) {
            Ok(manager) => app.container.instance(CACHE_KEY, Arc::new(manager)),
            Err(error) => eprintln!("cache: invalid [cache] config, not bound: {error}"),
        }
    }
}

/// Registers the queue connections that need no database and binds the config.
pub struct QueueServiceProvider;

impl ServiceProvider for QueueServiceProvider {
    fn name(&self) -> &'static str {
        "QueueServiceProvider"
    }

    fn register(&self, app: &mut Application) {
        let Some(loader) = app.config().cloned() else {
            return;
        };
        let config = match queue::QueueConfig::from_loader(&loader) {
            Ok(config) => config,
            Err(error) => {
                eprintln!("queue: invalid [queue] config, not registered: {error}");
                return;
            }
        };
        // With no connections there is nothing to register, and registering
        // would still record a default connection; leave the registry as is.
        if !config.connections.is_empty() {
            if let Err(error) = queue::register_from_config(&config, None) {
                eprintln!("queue: registering connections failed: {error}");
                return;
            }
        }
        app.container.instance(QUEUE_CONFIG_KEY, Arc::new(config));
    }
}

/// Register the `database` queue connections through the bound resolver.
///
/// Call once after boot, inside the runtime. Returns `Ok(false)` when there is
/// nothing to do: no queue config or resolver is bound, or no connection uses
/// the `database` driver.
///
/// Each `database` connection resolves the pool named by its `connection`
/// field, so two connections backed by different `[database.connections.*]`
/// entries get their own pools. A connection that names none falls back to the
/// default database connection, as before.
///
/// # Errors
///
/// A [`queue::QueueError`] when a named database connection cannot be opened
/// or the drivers cannot be registered.
pub async fn connect_database_queues(app: &Application) -> queue::Result<bool> {
    let (Some(config), Some(resolver)) = (queue_config(app), database(app)) else {
        return Ok(false);
    };
    if !config.connections.values().any(|c| c.is_database()) {
        return Ok(false);
    }
    // Register the pool-free drivers (`sync`, `redis`) and the default
    // connection first; the `database` drivers are installed below with a pool
    // resolved per declared connection.
    queue::register_from_config(&config, None)?;
    for (name, connection) in &config.connections {
        if !connection.is_database() {
            continue;
        }
        let pool = resolver.resolve(connection.connection.as_deref()).await?;
        let driver = queue::DatabaseDriver::with_tables(
            pool,
            config.jobs_table(connection),
            config.failed_table(),
        );
        queue::Queue::register_driver(name.clone(), Arc::new(driver));
    }
    Ok(true)
}
