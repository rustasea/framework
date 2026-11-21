//! `ModelOps::find` against a real in-memory SQLite pool.

use chrono::{DateTime, Utc};
use rustasea_orm::{DbPool, ModelOps};
use uuid::Uuid;

/// Derived model mapped to `users`, with tracked timestamps and soft deletes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, rustasea_macros::Model)]
struct User {
    id: Uuid,
    name: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
}

async fn pool() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.execute_script(
        "CREATE TABLE users (
            id BLOB PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT
        );",
    )
    .await
    .unwrap();
    pool
}

fn sample(name: &str) -> User {
    let now = Utc::now();
    User {
        id: Uuid::now_v7(),
        name: name.to_string(),
        created_at: now,
        updated_at: now,
        deleted_at: None,
    }
}

#[tokio::test]
async fn find_returns_live_rows_only() {
    let pool = pool().await;
    let ada = User::create(&pool, sample("Ada")).await.unwrap();

    let found = User::find(&pool, ada.id).await.unwrap().expect("found");
    assert_eq!(found.name, "Ada");
    assert!(User::find(&pool, Uuid::now_v7()).await.unwrap().is_none());

    // A soft-deleted row is not found, though `refresh` still reads it.
    assert!(User::delete(&pool, ada.id).await.unwrap());
    assert!(User::find(&pool, ada.id).await.unwrap().is_none());
    assert!(User::refresh(&pool, ada.id).await.unwrap().is_some());
}
