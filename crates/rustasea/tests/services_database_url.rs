//! `DATABASE_URL` overrides the default database connection at boot.
//!
//! Its own test binary: the variable is process-global, so it must not leak
//! into the other service-provider tests. Every case runs inside one
//! `#[tokio::test]` so the process-wide variables cannot race between tests.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use rustasea::services::{database, DatabaseServiceProvider};

static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Restore process-wide database URL variables after the test, including on panic.
struct DatabaseUrlEnv {
    database_url: Option<std::ffi::OsString>,
    database_nested_url: Option<std::ffi::OsString>,
}

impl DatabaseUrlEnv {
    /// Save the caller's environment before the test changes either URL key.
    fn capture() -> Self {
        Self {
            database_url: std::env::var_os("DATABASE_URL"),
            database_nested_url: std::env::var_os("DATABASE__URL"),
        }
    }
}

impl Drop for DatabaseUrlEnv {
    fn drop(&mut self) {
        match self.database_url.take() {
            Some(value) => std::env::set_var("DATABASE_URL", value),
            None => std::env::remove_var("DATABASE_URL"),
        }
        match self.database_nested_url.take() {
            Some(value) => std::env::set_var("DATABASE__URL", value),
            None => std::env::remove_var("DATABASE__URL"),
        }
    }
}
use rustasea::Application;

/// A scratch tag unique to this process, call, and moment.
///
/// The PID separates concurrent test binaries; the counter separates repeated
/// calls even when the clock is coarse; the nanosecond stamp separates this run
/// from a stale file left by a crashed run that reused the PID.
fn scratch_tag() -> String {
    let sequence = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}-{sequence}", std::process::id())
}

/// Write `database.toml` whose default connection points at `db_path`.
fn write_config(dir: &Path, db_path: &Path) {
    std::fs::create_dir_all(dir).expect("config dir");
    std::fs::write(
        dir.join("database.toml"),
        format!(
            "[database]\ndefault = \"main\"\n\n\
             [database.connections.main]\ndriver = \"sqlite\"\nurl = \"sqlite://{}?mode=rwc\"\n\n\
             [database.connections.jobs]\ndriver = \"sqlite\"\nurl = \"sqlite://{}?mode=rwc\"\n",
            db_path.display(),
            db_path.display()
        ),
    )
    .expect("database.toml");
}

/// Boot a fresh application whose config directory is `dir`.
fn boot_app(dir: &Path) -> Application {
    let mut app = Application::configure(|app| {
        app.set_config_dir(dir);
    });
    app.provider(DatabaseServiceProvider);
    app.boot().expect("boot");
    app
}

/// Remove a scratch config directory and database file, ignoring absence.
fn cleanup(dir: &Path, db: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_file(db);
}

#[tokio::test]
async fn database_url_replaces_the_default_connection_only() {
    let _env = DatabaseUrlEnv::capture();
    let tmp = std::env::temp_dir();

    // Case 1: `DATABASE_URL` replaces the default connection, others are kept.
    {
        let tag = scratch_tag();
        let configured = tmp.join(format!("rustasea-dburl-configured-{tag}.sqlite"));
        let from_env = tmp.join(format!("rustasea-dburl-env-{tag}.sqlite"));
        let dir = tmp.join(format!("rustasea-dburl-config-{tag}"));
        write_config(&dir, &configured);
        let env_url = format!("sqlite://{}?mode=rwc", from_env.display());
        std::env::remove_var("DATABASE__URL");
        std::env::set_var("DATABASE_URL", &env_url);

        let app = boot_app(&dir);
        std::env::remove_var("DATABASE_URL");

        let resolver = database(&app).expect("resolver bound");
        let config = resolver.config();
        assert_eq!(config.default.as_deref(), Some("main"));
        assert_eq!(
            config.connections["main"].url.as_deref(),
            Some(env_url.as_str())
        );
        assert!(
            config.connections.contains_key("jobs"),
            "other connections are kept"
        );

        // The default connection now opens the database named by the variable.
        resolver.resolve(None).await.expect("connect");
        assert!(from_env.exists(), "DATABASE_URL database was opened");
        assert!(!configured.exists(), "configured database was not opened");

        cleanup(&dir, &from_env);
    }

    // Case 2: `DATABASE__URL` is the fallback when `DATABASE_URL` is absent.
    {
        let tag = scratch_tag();
        let configured = tmp.join(format!("rustasea-dburl-configured-nested-{tag}.sqlite"));
        let from_env = tmp.join(format!("rustasea-dburl-env-nested-{tag}.sqlite"));
        let dir = tmp.join(format!("rustasea-dburl-config-nested-{tag}"));
        write_config(&dir, &configured);
        let env_url = format!("sqlite://{}?mode=rwc", from_env.display());
        std::env::remove_var("DATABASE_URL");
        std::env::set_var("DATABASE__URL", &env_url);

        let app = boot_app(&dir);
        std::env::remove_var("DATABASE__URL");

        let resolver = database(&app).expect("resolver bound");
        let config = resolver.config();
        assert_eq!(
            config.connections["main"].url.as_deref(),
            Some(env_url.as_str()),
            "DATABASE__URL replaces the default connection"
        );
        resolver.resolve(None).await.expect("connect");
        assert!(from_env.exists(), "DATABASE__URL database was opened");
        assert!(!configured.exists(), "configured database was not opened");

        cleanup(&dir, &from_env);
    }

    // Case 3: a blank `DATABASE_URL` falls back to the configured default.
    {
        let tag = scratch_tag();
        let configured = tmp.join(format!("rustasea-dburl-configured-blank-{tag}.sqlite"));
        let dir = tmp.join(format!("rustasea-dburl-config-blank-{tag}"));
        write_config(&dir, &configured);
        let configured_url = format!("sqlite://{}?mode=rwc", configured.display());
        std::env::remove_var("DATABASE__URL");
        std::env::set_var("DATABASE_URL", "  \t ");

        let app = boot_app(&dir);
        std::env::remove_var("DATABASE_URL");

        let resolver = database(&app).expect("resolver bound");
        let config = resolver.config();
        assert_eq!(
            config.connections["main"].url.as_deref(),
            Some(configured_url.as_str()),
            "a blank DATABASE_URL leaves the configured default in place"
        );
        resolver.resolve(None).await.expect("connect");
        assert!(configured.exists(), "configured database was opened");

        cleanup(&dir, &configured);
    }

    // Case 4: no variable at all leaves the configured default unchanged.
    {
        let tag = scratch_tag();
        let configured = tmp.join(format!("rustasea-dburl-configured-unset-{tag}.sqlite"));
        let dir = tmp.join(format!("rustasea-dburl-config-unset-{tag}"));
        write_config(&dir, &configured);
        let configured_url = format!("sqlite://{}?mode=rwc", configured.display());
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("DATABASE__URL");

        let app = boot_app(&dir);

        let resolver = database(&app).expect("resolver bound");
        let config = resolver.config();
        assert_eq!(config.default.as_deref(), Some("main"));
        assert_eq!(
            config.connections["main"].url.as_deref(),
            Some(configured_url.as_str()),
            "the configured default connection is unchanged"
        );
        resolver.resolve(None).await.expect("connect");
        assert!(configured.exists(), "configured database was opened");

        cleanup(&dir, &configured);
    }

    // Case 5: a malformed `[database]` table does not hide `DATABASE_URL`.
    {
        let tag = scratch_tag();
        let from_env = tmp.join(format!("rustasea-dburl-env-malformed-{tag}.sqlite"));
        let dir = tmp.join(format!("rustasea-dburl-config-malformed-{tag}"));
        std::fs::create_dir_all(&dir).expect("config dir");
        std::fs::write(dir.join("database.toml"), "[database]\ndefault = 123\n")
            .expect("database.toml");
        let env_url = format!("sqlite://{}?mode=rwc", from_env.display());
        std::env::remove_var("DATABASE__URL");
        std::env::set_var("DATABASE_URL", &env_url);

        let app = boot_app(&dir);
        std::env::remove_var("DATABASE_URL");

        let resolver = database(&app).expect("resolver bound despite malformed config");
        let config = resolver.config();
        assert_eq!(
            config.connections["default"].url.as_deref(),
            Some(env_url.as_str()),
            "DATABASE_URL wins over a malformed [database] config"
        );
        resolver.resolve(None).await.expect("connect");
        assert!(from_env.exists(), "DATABASE_URL database was opened");

        cleanup(&dir, &from_env);
    }
}
