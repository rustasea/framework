//! Tests for the auth bootstrap wiring.

use super::*;
use rustasea::auth::NewUserRecord;

/// A unique scratch path under the system temp directory.
fn scratch(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("rustasea-app-{name}-{}", uuid::Uuid::new_v4()))
}

/// Load an `[auth]` table whose default guard uses `driver`.
fn loader_with_provider_driver(driver: &str) -> ConfigLoader {
    let base = scratch("auth");
    let toml = format!(
        "[auth.defaults]\nguard = \"web\"\n\
         [auth.guards.web]\ndriver = \"session\"\nprovider = \"users\"\n\
         [auth.providers.users]\ndriver = \"{driver}\"\n"
    );
    let file = base.with_extension("toml");
    std::fs::write(&file, toml).expect("write auth.toml");
    let loader = ConfigLoader::load_from(&[&base.to_string_lossy()]).expect("load");
    let _ = std::fs::remove_file(file);
    loader
}

#[test]
fn only_orm_backed_drivers_select_the_database_provider() {
    assert!(uses_database_provider(&loader_with_provider_driver(
        "eloquent"
    )));
    assert!(uses_database_provider(&loader_with_provider_driver(
        "Database"
    )));
    assert!(!uses_database_provider(&loader_with_provider_driver(
        "memory"
    )));
}

#[tokio::test]
async fn database_provider_is_built_only_when_the_users_table_exists() {
    let path = scratch("users").with_extension("sqlite");
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let mut config = rustasea::orm::DatabaseConfig {
        url: Some(url.clone()),
        ..Default::default()
    };
    config.synthesize();
    let resolver = rustasea::orm::ConnectionResolver::new(config);

    // No `users` table yet: stay fail-closed.
    assert!(database_user_provider(&resolver).await.is_none());

    let pool = rustasea::orm::DbPool::connect(&url).await.expect("pool");
    pool.execute_script(
        "CREATE TABLE users (id UUID PRIMARY KEY, name VARCHAR(255) NOT NULL, \
         email VARCHAR(255) NOT NULL UNIQUE, password VARCHAR(255) NOT NULL, \
         email_verified_at TIMESTAMPTZ NULL, timezone VARCHAR(64) NULL, \
         created_at TIMESTAMPTZ NOT NULL, updated_at TIMESTAMPTZ NOT NULL, \
         deleted_at TIMESTAMPTZ NULL);",
    )
    .await
    .expect("schema");

    let provider = database_user_provider(&resolver).await.expect("provider");
    let created = provider
        .create(NewUserRecord::new("Ada", "ada@example.com", "phc-hash"))
        .await
        .expect("create");
    let found = provider
        .find_by_email("ada@example.com")
        .await
        .expect("find");
    assert_eq!(found.map(|user| user.id), Some(created.id));

    pool.close().await;
    let _ = std::fs::remove_file(path);
}
