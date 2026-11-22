//! A missing table surfaces as `OrmError::MissingTable` on a real SQLite pool.

use rustasea_orm::{DbPool, OrmError, QueryBuilder, Value};

async fn pool() -> DbPool {
    DbPool::connect("sqlite::memory:").await.unwrap()
}

#[tokio::test]
async fn a_missing_table_is_a_typed_error() {
    let pool = pool().await;

    let read = QueryBuilder::table("widgets")
        .first(&pool)
        .await
        .unwrap_err();
    assert!(
        matches!(&read, OrmError::MissingTable { table } if table == "widgets"),
        "got {read:?}"
    );
    let write = pool
        .execute_bind(
            "INSERT INTO widgets (name) VALUES ($1)",
            &[Value::Text("x".into())],
        )
        .await
        .unwrap_err();
    assert!(
        matches!(write, OrmError::MissingTable { .. }),
        "got {write:?}"
    );

    // Other driver errors stay storage errors.
    let syntax = pool.execute_bind("SELEC 1", &[]).await.unwrap_err();
    assert!(matches!(syntax, OrmError::Storage(_)), "got {syntax:?}");
}
