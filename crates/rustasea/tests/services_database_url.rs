//! `DATABASE_URL` overrides the default database connection at boot.
//!
//! Its own test binary: the variable is process-global, so it must not leak
//! into the other service-provider tests.

use rustasea::services::{database, DatabaseServiceProvider};
use rustasea::Application;

#[tokio::test]
async fn database_url_replaces_the_default_connection_only() {
    let tag = std::process::id();
    let tmp = std::env::temp_dir();
    let configured = tmp.join(format!("rustasea-dburl-configured-{tag}.sqlite"));
    let from_env = tmp.join(format!("rustasea-dburl-env-{tag}.sqlite"));
    let dir = tmp.join(format!("rustasea-dburl-config-{tag}"));
    std::fs::create_dir_all(&dir).expect("config dir");
    std::fs::write(
        dir.join("database.toml"),
        format!(
            "[database]\ndefault = \"main\"\n\n\
             [database.connections.main]\ndriver = \"sqlite\"\nurl = \"sqlite://{}?mode=rwc\"\n\n\
             [database.connections.jobs]\ndriver = \"sqlite\"\nurl = \"sqlite://{}?mode=rwc\"\n",
            configured.display(),
            configured.display()
        ),
    )
    .expect("database.toml");
    let env_url = format!("sqlite://{}?mode=rwc", from_env.display());
    std::env::set_var("DATABASE_URL", &env_url);

    let mut app = Application::configure(|app| {
        app.set_config_dir(&dir);
    });
    app.provider(DatabaseServiceProvider);
    app.boot().expect("boot");
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

    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_file(from_env);
}
