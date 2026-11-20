//! Framework service providers bind database, cache, and queue at boot.
//!
//! Positive: with `[database]`, `[cache]`, and `[queue]` configured, booting the
//! three providers binds the resolver, the cache manager, and the queue config,
//! and `connect_database_queues` registers the `database` queue connection.
//! With no config the components fall back to their defaults.
//! Negative: a `[cache]` table that does not validate leaves the cache unbound
//! without stopping boot.

use std::path::{Path, PathBuf};

use rustasea::services::{
    cache, connect_database_queues, database, queue_config, CacheServiceProvider,
    DatabaseServiceProvider, QueueServiceProvider,
};
use rustasea::Application;

/// A fresh config directory holding `files` (name, contents).
fn config_dir(tag: &str, files: &[(&str, String)]) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "rustasea-services-{tag}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("config dir");
    for (name, contents) in files {
        std::fs::write(dir.join(name), contents).expect("config file");
    }
    dir
}

/// Boot an application over `dir` with the three framework providers.
fn boot(dir: &Path) -> Application {
    let mut app = Application::configure(|app| {
        app.set_config_dir(dir);
    });
    app.provider(DatabaseServiceProvider);
    app.provider(CacheServiceProvider);
    app.provider(QueueServiceProvider);
    app.boot().expect("boot");
    app
}

#[tokio::test]
async fn configured_services_are_bound_and_database_queues_connect() {
    let db = std::env::temp_dir().join(format!("rustasea-services-{}.sqlite", std::process::id()));
    let dir = config_dir(
        "full",
        &[
            (
                "database.toml",
                format!(
                    "[database]\ndefault = \"sqlite\"\n\n[database.connections.sqlite]\ndriver = \"sqlite\"\nurl = \"sqlite://{}?mode=rwc\"\n",
                    db.display()
                ),
            ),
            ("cache.toml", "[cache]\ndefault = \"memory\"\n\n[cache.stores.memory]\ndriver = \"memory\"\n".to_string()),
            (
                "queue.toml",
                "[queue]\ndefault = \"database\"\n\n[queue.connections.sync]\ndriver = \"sync\"\n\n[queue.connections.database]\ndriver = \"database\"\ntable = \"jobs\"\n".to_string(),
            ),
        ],
    );
    let app = boot(&dir);

    let resolver = database(&app).expect("resolver bound");
    assert_eq!(resolver.config().default.as_deref(), Some("sqlite"));
    assert!(cache(&app).is_some(), "cache manager bound");
    assert_eq!(
        queue_config(&app).expect("queue config bound").default,
        "database"
    );

    // `sync` is registered at boot; `database` only once a pool is connected.
    assert!(rustasea::queue::Queue::driver("sync").is_ok());
    assert!(rustasea::queue::Queue::driver("database").is_err());
    assert!(connect_database_queues(&app).await.expect("connect"));
    assert!(rustasea::queue::Queue::driver("database").is_ok());

    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_file(db);
}

#[tokio::test]
async fn missing_config_falls_back_to_defaults() {
    let dir = config_dir("empty", &[]);
    let app = boot(&dir);

    assert!(database(&app).is_some());
    assert!(cache(&app).is_some());
    assert!(queue_config(&app)
        .expect("queue config bound")
        .connections
        .is_empty());
    assert!(!connect_database_queues(&app)
        .await
        .expect("nothing to connect"));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_invalid_cache_table_leaves_the_cache_unbound() {
    let dir = config_dir(
        "bad-cache",
        &[("cache.toml", "[cache]\ndefault = \"missing\"\n".to_string())],
    );
    let app = boot(&dir);

    assert!(cache(&app).is_none());
    assert!(database(&app).is_some());

    let _ = std::fs::remove_dir_all(dir);
}
