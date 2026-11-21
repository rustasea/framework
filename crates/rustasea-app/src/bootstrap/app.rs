//! Application bootstrap — `Application::configure` for the runnable app.
//!
//! Mirrors Laravel's `bootstrap/app.php` (README "Proposed Directory
//! Structure"): configures the foundation container, registers the providers
//! from [`crate::bootstrap::providers`], registers the console command
//! surface, and boots the register→boot DAG before the HTTP kernel takes
//! over. `src/main.rs` calls [`configure`].

use rustasea::foundation::BootError;
use rustasea::Application;

use crate::bootstrap::{commands, providers, tinker};

/// Process-wide logging guard.
///
/// [`rustasea_logging::init_from_config`] returns a guard that owns the
/// non-blocking file-appender workers; dropping it flushes pending lines and
/// shuts the workers down. [`configure`] cannot return it to a caller, so it is
/// parked here for the process lifetime. [`configure`] is idempotent, so at most
/// one real guard is ever stored (later calls store their empty, non-installed
/// guard, which is harmless).
static LOGGING_GUARD: std::sync::OnceLock<rustasea_logging::LoggingGuard> =
    std::sync::OnceLock::new();

/// Process-wide Sentry client guard (ADOPT-004).
///
/// Dropping the guard flushes the send queue and shuts the transport down, so —
/// exactly like [`LOGGING_GUARD`] — it is parked here for the process lifetime.
#[cfg(feature = "sentry")]
static SENTRY_GUARD: std::sync::OnceLock<rustasea_logging::ClientInitGuard> =
    std::sync::OnceLock::new();

/// Build and boot the application.
///
/// Returns [`BootError`] when the config loader cannot be built or the provider
/// graph contains a cycle or an unresolved dependency, so a misconfigured boot
/// never starts the server. [`Application::boot`] mounts the layered
/// [`ConfigLoader`](rustasea::ConfigLoader) (from `config/*.toml` + environment)
/// into the container before providers register, so they resolve configuration
/// from `app.container` instead of loading it themselves. Registering the
/// default command surface also registers the framework's queue migrations.
///
/// After boot, [`install_observability`] installs the global `tracing`
/// subscriber (and the Sentry client when the `sentry` feature is on) from the
/// same loader. This is best-effort: an observability failure is reported but
/// never aborts boot.
pub fn configure() -> Result<Application, BootError> {
    let mut app = Application::configure(|_| {});
    for provider in providers::providers() {
        app.provider(provider);
    }
    commands::register_default();
    // Publish the live route table so `route:list` introspects the real routes.
    // `rustasea-cli` is framework-generic and cannot depend on this app crate,
    // so the app pushes its table into the CLI's process-wide route registry at
    // boot; the command reads it back (see `rustasea::cli::routes`).
    rustasea::cli::routes::set_route_source(|| crate::routes::table().get_routes());
    app.boot()?;
    // Publish the application introspection source for `tinker`. Like the route
    // source, the CLI cannot depend on this app crate, so the app pushes a
    // snapshot into the CLI's process-wide tinker registry after boot (once the
    // container bindings + config loader are populated).
    publish_tinker_source(&app);
    install_observability(&app);
    install_debugbar();
    install_queue_dashboard(&app);
    install_broadcasting(&app);
    install_error_pages(&app);
    Ok(app)
}

/// Install the logging subscriber and (optionally) the Sentry client.
///
/// Both are driven by the boot-time [`ConfigLoader`](rustasea::ConfigLoader)
/// mounted by [`Application::boot`]. This closes the CFG-005 gap where the
/// `logging_init_from_config` re-export was never called at runtime, and adds
/// the ADOPT-004 Sentry client on top.
///
/// Failure is non-fatal: a configuration or initialisation error is logged to
/// stderr and boot continues, so observability can never take the app down. The
/// guards are parked in process-wide [`std::sync::OnceLock`]s so the workers and
/// the Sentry transport stay alive for the whole process.
///
/// # Idempotency
///
/// [`configure`] may run more than once (tests, embedded boots). Each init is
/// skipped once its guard is already parked: re-initialising Sentry would bind a
/// *second* client and, when the redundant guard loses the [`OnceLock::set`]
/// race and is dropped, `ClientInitGuard::drop` would call `client.close(None)`
/// and kill the process-wide transport. The early-return avoids building that
/// redundant client at all; [`park_guard`] covers the residual concurrent case.
fn install_observability(app: &Application) {
    let Some(loader) = app.config() else {
        return;
    };

    if LOGGING_GUARD.get().is_none() {
        match rustasea_logging::init_from_config(loader) {
            Ok(guard) => park_guard(&LOGGING_GUARD, guard),
            Err(error) => eprintln!("logging init failed: {error}"),
        }
    }

    #[cfg(feature = "sentry")]
    {
        if SENTRY_GUARD.get().is_none() {
            match rustasea_logging::SentryConfig::from_loader(loader) {
                Ok(config) => {
                    if let Some(guard) = rustasea_logging::init_sentry(&config) {
                        park_guard(&SENTRY_GUARD, guard);
                    }
                }
                Err(error) => eprintln!("sentry config failed: {error}"),
            }
        }
    }
}

/// Install the dev request profiler hooks (ADOPT-009) when the `debugbar`
/// feature is compiled in.
///
/// Registers the process-wide SQL recorder and event observer so any request
/// served by the profiler middleware accumulates SQL/cache/event entries.
/// Best-effort and idempotent: [`rustasea_debugbar::install`] replaces the hooks
/// on every call, so a second `configure()` is harmless and boot can never fail
/// because of profiling. The cache hook needs a live `CacheManager` and is not
/// installed here (no manager is bound in this scaffold).
#[cfg(feature = "debugbar")]
fn install_debugbar() {
    rustasea_debugbar::install(rustasea_debugbar::DEFAULT_CAPACITY);
}

/// No-op stand-in for [`install_debugbar`] when the `debugbar` feature is off.
#[cfg(not(feature = "debugbar"))]
fn install_debugbar() {}

/// Process-wide sampler-task guard (ADOPT-021).
///
/// The sampler is a detached tokio task; parking its [`tokio::task::JoinHandle`]
/// keeps it alive for the process lifetime and makes a second `configure()` a
/// no-op. See [`park_guard`].
#[cfg(feature = "queue-dashboard")]
static SAMPLER_GUARD: std::sync::OnceLock<tokio::task::JoinHandle<()>> = std::sync::OnceLock::new();

/// Install the queue dashboard pool + metrics sampler (ADOPT-021).
///
/// Reads `[queue.dashboard]` from the boot-time config loader and always records
/// it (so the route handlers can gate on `enabled`). When the dashboard is
/// enabled *and* a tokio runtime is current, it spawns a task that opens the
/// default connection through the bound resolver, installs the pool, and starts
/// the sampler.
///
/// Best-effort and idempotent: a config or connection failure is logged and boot
/// continues; the [`SAMPLER_GUARD`] ensures at most one sampler task is spawned
/// across repeated `configure()` calls. Outside a tokio runtime (e.g. a sync
/// test calling `configure()`) the pool/sampler step is skipped, leaving the
/// config recorded so the route surface still reports its enabled state.
#[cfg(feature = "queue-dashboard")]
fn install_queue_dashboard(app: &Application) {
    let Some(loader) = app.config().cloned() else {
        return;
    };
    let config = match rustasea_queue_dashboard::DashboardConfig::from_loader(&loader) {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "queue dashboard config failed");
            return;
        }
    };
    rustasea_queue_dashboard::set_config(config.clone());
    if !config.enabled {
        return;
    }
    // The sampler needs a runtime to connect the pool and spawn; skip cleanly
    // when `configure()` runs outside one (the sync test path).
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let Some(resolver) = rustasea::services::database(app) else {
        tracing::error!("queue dashboard: no database is bound");
        return;
    };
    SAMPLER_GUARD.get_or_init(|| {
        handle.spawn(async move {
            match resolver.resolve(None).await {
                Ok(pool) => {
                    rustasea_queue_dashboard::set_pool(pool.clone());
                    rustasea_queue_dashboard::sampler::spawn_sampler(
                        pool,
                        config.sample_interval(),
                        config.retention(),
                    );
                }
                Err(error) => tracing::error!(%error, "queue dashboard pool failed"),
            }
        })
    });
}

/// No-op stand-in for [`install_queue_dashboard`] when the feature is off.
#[cfg(not(feature = "queue-dashboard"))]
fn install_queue_dashboard(_app: &Application) {}

/// Install the broadcast config, gate, and manager (ADOPT-022).
///
/// Reads `[broadcasting]` from the boot-time config loader and installs the
/// parsed config plus a built [`BroadcastManager`](rustasea::broadcast::BroadcastManager)
/// into the process-wide slots so the `POST /broadcasting/auth` handler can
/// authorize and sign channel subscriptions.
///
/// No authorization gate is installed here (the scaffold has no channel
/// policy registry yet), so private/presence subscriptions fail closed as
/// unauthenticated until an app installs one with
/// [`set_broadcast_gate`](rustasea::broadcast::set_broadcast_gate).
///
/// Best-effort: a config or build failure is logged and boot continues. The
/// facade enables `rustasea-broadcast`'s default `ws` feature, so the
/// in-process `hub` connection always builds; a config selecting an
/// unconfigured external driver is reported and skipped rather than aborting
/// boot.
#[cfg(feature = "broadcasting")]
fn install_broadcasting(app: &Application) {
    let Some(loader) = app.config().cloned() else {
        return;
    };
    let config = match rustasea::broadcast::BroadcastingConfig::from_loader(&loader) {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "broadcast config failed");
            return;
        }
    };
    match rustasea::broadcast::BroadcastManager::from_config(&config, None) {
        Ok(manager) => {
            rustasea::broadcast::set_broadcast_config(config);
            rustasea::broadcast::set_broadcast_manager(std::sync::Arc::new(manager));
        }
        Err(error) => tracing::warn!(%error, "broadcast manager build failed"),
    }
}

/// No-op stand-in for [`install_broadcasting`] when the feature is off.
#[cfg(not(feature = "broadcasting"))]
fn install_broadcasting(_app: &Application) {}

/// Install the dev panic-location hook (ADOPT-010) when the app is in debug.
///
/// The hook records `(message, file, line)` for the most recent panic so the dev
/// error page can render a source snippet. It is installed **only** in debug
/// mode — a production process keeps the default panic hook untouched. The
/// install is idempotent (a process-wide [`OnceLock`] guard inside
/// [`rustasea_http::panic::install_panic_location_hook`]), so a second
/// `configure()` cannot double-chain the hook.
///
/// The debug flag is read from the boot-time [`ConfigLoader`](rustasea::ConfigLoader)
/// (`app_debug`), falling back to the config-file default. This is the same
/// source the served [`AppState`](rustasea::http::AppState) is seeded from.
fn install_error_pages(app: &Application) {
    if app_debug(app) {
        rustasea_http::panic::install_panic_location_hook();
    }
}

/// Whether the app runs in debug mode, per the boot-time config loader.
///
/// Reads `app_debug` (overlaid by `APP_DEBUG`) from the mounted loader; when no
/// loader is bound the conservative default is `false`, so the hook is not
/// installed on an unbooted app.
fn app_debug(app: &Application) -> bool {
    app.config()
        .and_then(|loader| loader.get_key::<bool>("app_debug").ok())
        .unwrap_or(false)
}

/// Park `guard` in `slot` for the process lifetime, never dropping it.
///
/// Dropping a [`rustasea_logging::LoggingGuard`] flushes and shuts down the
/// appender workers, and dropping a [`rustasea_logging::ClientInitGuard`] closes
/// the Sentry transport — either would kill observability for the whole process.
/// A guard that loses the [`OnceLock::set`] race is therefore `mem::forget`-ed
/// rather than dropped. The `get().is_none()` checks in [`install_observability`]
/// make that path unreachable in the sequential case; the forget covers the
/// residual concurrent case.
fn park_guard<T>(slot: &std::sync::OnceLock<T>, guard: T) {
    if let Err(guard) = slot.set(guard) {
        std::mem::forget(guard);
    }
}

/// Publish the booted application's [`tinker`] source into the CLI registry.
///
/// Best-effort: without a boot-time config loader or environment binding the
/// source is skipped, leaving the REPL's framework-generic surfaces usable.
fn publish_tinker_source(app: &Application) {
    let Some(loader) = app.config().cloned() else {
        return;
    };
    let environment = app
        .container
        .get::<String>(providers::ENVIRONMENT_KEY)
        .cloned()
        .unwrap_or_else(|| providers::app_environment_from(app));
    let source = tinker::AppTinkerSource::from_booted(loader, environment, &app.container);
    rustasea::cli::set_tinker_source(source);
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::*;

    /// Booting publishes the live route table `route:list` reads back.
    #[test]
    fn configure_publishes_route_table() {
        configure().expect("app boots");

        let routes = rustasea::cli::routes::routes();
        let paths: Vec<&str> = routes.iter().map(|route| route.path.as_str()).collect();
        for expected in ["/login", "/dashboard", "/settings/profile"] {
            assert!(paths.contains(&expected), "missing {expected} in {paths:?}");
        }
    }

    /// A value that records its own drop, used to prove a losing guard is kept.
    struct DropCounter(Arc<AtomicUsize>);

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// `park_guard` must never drop a guard whose `OnceLock::set` loses the race.
    ///
    /// The first value stays in the slot; the second is `mem::forget`-ed (its
    /// drop counter never fires). This is the invariant that keeps the Sentry
    /// transport and the logging workers alive across a double `configure()`.
    #[test]
    fn park_guard_forgets_the_losing_guard() {
        let drops = Arc::new(AtomicUsize::new(0));
        let slot: std::sync::OnceLock<DropCounter> = std::sync::OnceLock::new();

        assert!(slot.set(DropCounter(Arc::clone(&drops))).is_ok());
        park_guard(&slot, DropCounter(Arc::clone(&drops)));

        assert_eq!(
            drops.load(Ordering::SeqCst),
            0,
            "the losing guard must be forgotten, not dropped"
        );
        assert!(slot.get().is_some(), "the first guard stays parked");

        drop(slot);
        assert_eq!(
            drops.load(Ordering::SeqCst),
            1,
            "dropping the slot drops exactly the parked guard"
        );
    }

    /// A second `configure()` must be safe (no panic) and leave observability up.
    ///
    /// Whether logging/Sentry actually initialise depends on the test working
    /// directory (`config/*.toml` is discovered relative to it), so the test
    /// asserts the *invariant* rather than a specific install outcome: whatever
    /// state the first boot reached, the second boot neither panics nor clears a
    /// parked guard.
    #[test]
    fn configure_twice_is_idempotent() {
        configure().expect("first boot");
        let logging_installed = LOGGING_GUARD.get().is_some();
        #[cfg(feature = "sentry")]
        let sentry_installed = SENTRY_GUARD.get().is_some();

        configure().expect("second boot is a no-op");

        assert_eq!(
            LOGGING_GUARD.get().is_some(),
            logging_installed,
            "a second configure must not drop the parked logging guard"
        );
        #[cfg(feature = "sentry")]
        assert_eq!(
            SENTRY_GUARD.get().is_some(),
            sentry_installed,
            "a second configure must not drop the parked Sentry guard"
        );
    }
}
