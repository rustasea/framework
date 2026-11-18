//! Integration tests for the database-backed [`DatabaseUserProvider`] (M3).
//!
//! Exercises the provider over a real in-memory SQLite pool and the `users`
//! table shape from the scaffold migration (`create_users_table`):
//!
//! * **positive** — register, resolve by email and id, rotate the password,
//!   verify/unverify the email, update the profile and timezone;
//! * **negative** — duplicate email, unknown / malformed / soft-deleted user,
//!   an unparsable timestamp, and a missing `users` table (typed
//!   `AuthError::StoreUnavailable`, never a panic or a silent `Ok(None)`).

use rustasea_auth::users::{DatabaseUserProvider, NewUserRecord, UserProvider};
use rustasea_auth::AuthError;
use rustasea_orm::DbPool;

/// The scaffold `create_users_table` DDL.
const CREATE_USERS: &str = "CREATE TABLE users (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL UNIQUE,
    password VARCHAR(255) NOT NULL,
    email_verified_at TIMESTAMPTZ NULL,
    two_factor_secret TEXT NULL,
    two_factor_recovery_codes TEXT NULL,
    two_factor_confirmed_at TIMESTAMPTZ NULL,
    remember_token VARCHAR(100) NULL,
    timezone VARCHAR(64) NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    deleted_at TIMESTAMPTZ NULL
);";

/// A provider over a fresh in-memory database with the `users` table.
async fn provider() -> DatabaseUserProvider {
    let pool = DbPool::connect("sqlite::memory:").await.expect("pool");
    pool.execute_script(CREATE_USERS).await.expect("schema");
    DatabaseUserProvider::new(pool)
}

/// Register a user and return the stored record.
async fn register(
    provider: &DatabaseUserProvider,
    email: &str,
) -> rustasea_auth::users::AuthUserRecord {
    provider
        .create(NewUserRecord::new("Ada Lovelace", email, "phc-hash"))
        .await
        .expect("create")
}

#[tokio::test]
async fn create_then_resolve_by_email_and_id() {
    let provider = provider().await;
    let created = register(&provider, "ada@example.com").await;
    assert_eq!(created.email, "ada@example.com");
    assert_eq!(created.password_hash, "phc-hash");
    assert_eq!(created.email_verified_at, None);
    assert_eq!(created.timezone, None);

    let by_email = provider
        .find_by_email("ada@example.com")
        .await
        .expect("find");
    assert_eq!(by_email.as_ref(), Some(&created));
    let by_id = provider.find_by_id(&created.id).await.expect("find");
    assert_eq!(by_id, Some(created));

    assert_eq!(
        provider
            .find_by_email("nobody@example.com")
            .await
            .expect("find"),
        None
    );
}

#[tokio::test]
async fn create_persists_verification_and_timezone() {
    let provider = provider().await;
    let created = provider
        .create(
            NewUserRecord::new("Ada", "ada@example.com", "phc-hash")
                .with_email_verified_at(Some("2026-01-02T03:04:05Z"))
                .with_timezone(Some("Asia/Jakarta")),
        )
        .await
        .expect("create");
    let verified = created.email_verified_at.expect("verified");
    assert!(verified.starts_with("2026-01-02T03:04:05"), "{verified}");
    assert_eq!(created.timezone.as_deref(), Some("Asia/Jakarta"));
}

#[tokio::test]
async fn duplicate_email_is_a_typed_collision() {
    let provider = provider().await;
    register(&provider, "ada@example.com").await;
    let error = provider
        .create(NewUserRecord::new("Other", "ada@example.com", "phc-hash"))
        .await
        .expect_err("duplicate");
    assert!(matches!(error, AuthError::UserExists { email } if email == "ada@example.com"));
}

#[tokio::test]
async fn writes_update_the_stored_record() {
    let provider = provider().await;
    let user = register(&provider, "ada@example.com").await;

    provider
        .update_password(&user.id, "new-hash")
        .await
        .expect("password");
    provider
        .set_email_verified_at(&user.id, Some("2026-05-06T07:08:09Z"))
        .await
        .expect("verify");
    provider
        .update_timezone(&user.id, Some("Europe/London".to_string()))
        .await
        .expect("timezone");
    provider
        .update_profile(&user.id, "Ada King", "countess@example.com")
        .await
        .expect("profile");

    let stored = provider
        .find_by_id(&user.id)
        .await
        .expect("find")
        .expect("user");
    assert_eq!(stored.password_hash, "new-hash");
    assert_eq!(stored.email, "countess@example.com");
    assert_eq!(stored.timezone.as_deref(), Some("Europe/London"));
    assert!(stored.email_verified_at.is_some());

    // Clearing writes a NULL back.
    provider
        .set_email_verified_at(&user.id, None)
        .await
        .expect("unverify");
    provider
        .update_timezone(&user.id, None)
        .await
        .expect("clear timezone");
    let stored = provider
        .find_by_id(&user.id)
        .await
        .expect("find")
        .expect("user");
    assert_eq!(stored.email_verified_at, None);
    assert_eq!(stored.timezone, None);
}

#[tokio::test]
async fn update_profile_rejects_an_email_owned_by_another_user() {
    let provider = provider().await;
    let ada = register(&provider, "ada@example.com").await;
    register(&provider, "grace@example.com").await;

    let error = provider
        .update_profile(&ada.id, "Ada", "grace@example.com")
        .await
        .expect_err("taken");
    assert!(matches!(error, AuthError::UserExists { .. }));

    // Re-asserting the caller's own email is not a collision.
    provider
        .update_profile(&ada.id, "Ada K.", "ada@example.com")
        .await
        .expect("own email");
}

#[tokio::test]
async fn unknown_and_malformed_ids_are_not_found() {
    let provider = provider().await;
    for id in ["0190a1b2-0000-7000-8000-000000000000", "not-a-uuid"] {
        assert_eq!(provider.find_by_id(id).await.expect("find"), None);
        let error = provider
            .update_password(id, "hash")
            .await
            .expect_err("missing");
        assert!(
            matches!(error, AuthError::UserNotFound { .. }),
            "{id}: {error}"
        );
        let error = provider
            .update_profile(id, "n", "e@example.com")
            .await
            .expect_err("missing");
        assert!(
            matches!(error, AuthError::UserNotFound { .. }),
            "{id}: {error}"
        );
    }
}

#[tokio::test]
async fn soft_deleted_users_do_not_resolve() {
    let provider = provider().await;
    let user = register(&provider, "ada@example.com").await;
    provider
        .pool()
        .execute_script("UPDATE users SET deleted_at = '2026-01-01T00:00:00Z';")
        .await
        .expect("soft delete");

    assert_eq!(
        provider
            .find_by_email("ada@example.com")
            .await
            .expect("find"),
        None
    );
    assert_eq!(provider.find_by_id(&user.id).await.expect("find"), None);
    let error = provider
        .update_password(&user.id, "hash")
        .await
        .expect_err("deleted");
    assert!(matches!(error, AuthError::UserNotFound { .. }));
    // The address stays reserved by the unique index.
    let error = provider
        .create(NewUserRecord::new("Again", "ada@example.com", "phc-hash"))
        .await
        .expect_err("reserved");
    assert!(matches!(error, AuthError::UserExists { .. }));
}

#[tokio::test]
async fn unparsable_verification_timestamp_is_rejected_before_writing() {
    let provider = provider().await;
    let user = register(&provider, "ada@example.com").await;
    let error = provider
        .set_email_verified_at(&user.id, Some("yesterday"))
        .await
        .expect_err("bad timestamp");
    assert!(matches!(error, AuthError::Disabled(_)));
    let stored = provider
        .find_by_id(&user.id)
        .await
        .expect("find")
        .expect("user");
    assert_eq!(stored.email_verified_at, None);
}

#[tokio::test]
async fn missing_table_is_store_unavailable() {
    let pool = DbPool::connect("sqlite::memory:").await.expect("pool");
    let provider = DatabaseUserProvider::new(pool);
    let error = provider
        .find_by_email("ada@example.com")
        .await
        .expect_err("no table");
    assert!(matches!(error, AuthError::StoreUnavailable));
    let error = provider
        .create(NewUserRecord::new("Ada", "ada@example.com", "phc-hash"))
        .await
        .expect_err("no table");
    assert!(matches!(error, AuthError::StoreUnavailable));
}

#[tokio::test]
async fn invalid_table_name_fails_closed() {
    let pool = DbPool::connect("sqlite::memory:").await.expect("pool");
    let provider = DatabaseUserProvider::new(pool).with_table("users; DROP TABLE users");
    let error = provider
        .find_by_email("ada@example.com")
        .await
        .expect_err("bad table");
    assert!(matches!(error, AuthError::StoreUnavailable));
}
