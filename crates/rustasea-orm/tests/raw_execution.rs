//! Raw-SQL execution round-trip tests (LARAVEL-007) against in-memory SQLite.
//!
//! Proves the previously display-only `raw()` / `raw_sql()` helpers are now
//! executable: a `Raw` (parameterized) and a `SqlFragment` (inline) both run
//! through `DbPool` / `Transaction`, a bound `SELECT COUNT(*)` returns the right
//! value, bind values are passed as parameters (never interpolated), and a
//! syntax error surfaces as a typed [`OrmError::Storage`]. The sqlx runtime API
//! is used throughout.

use rustasea_orm::{raw, raw_sql, DbPool, OrmError, Transaction, Value};

/// Build a fresh in-memory pool with a seeded `users` table.
async fn pool_with_users() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.execute_raw(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, status TEXT NOT NULL)",
        &[],
    )
    .await
    .unwrap();
    for (id, status) in [(1, "active"), (2, "active"), (3, "banned")] {
        pool.execute_raw(
            "INSERT INTO users (id, status) VALUES ($1, $2)",
            &[Value::Int(id), Value::Text(status.into())],
        )
        .await
        .unwrap();
    }
    pool
}

/// Positive: a `raw()` fragment with a bound value returns the correct count.
#[tokio::test]
async fn raw_query_returns_bound_count() {
    let pool = pool_with_users().await;
    let statement = raw(
        "SELECT COUNT(*) AS n FROM users WHERE status = $1",
        vec![Value::Text("active".into())],
    );
    let rows = statement.query(&pool).await.unwrap();
    let count = rows
        .first()
        .and_then(|row| row.get("n"))
        .and_then(serde_json::Value::as_u64);
    assert_eq!(count, Some(2));
    pool.close().await;
}

/// Positive: a `raw()` fragment executes as a mutation via `Raw::execute`.
#[tokio::test]
async fn raw_execute_reports_affected_rows() {
    let pool = pool_with_users().await;
    let statement = raw(
        "UPDATE users SET status = $1 WHERE status = $2",
        vec![Value::Text("banned".into()), Value::Text("active".into())],
    );
    assert_eq!(statement.execute(&pool).await.unwrap(), 2);
    pool.close().await;
}

/// Positive: an inline `raw_sql()` fragment runs as a `SELECT`.
#[tokio::test]
async fn raw_sql_fragment_query_returns_rows() {
    let pool = pool_with_users().await;
    let fragment = raw_sql("SELECT id FROM users ORDER BY id");
    let rows = fragment.query(&pool).await.unwrap();
    assert_eq!(rows.len(), 3);
    pool.close().await;
}

/// Positive: raw fragments run on the open transaction connection, so a write
/// and a read observe the same uncommitted state.
#[tokio::test]
async fn raw_executes_inside_transaction() {
    let pool = pool_with_users().await;
    let mut tx = Transaction::begin(&pool).await.unwrap();

    let insert = raw(
        "INSERT INTO users (id, status) VALUES ($1, $2)",
        vec![Value::Int(4), Value::Text("active".into())],
    );
    assert_eq!(insert.execute(&mut tx).await.unwrap(), 1);

    let count = raw(
        "SELECT COUNT(*) AS n FROM users WHERE status = $1",
        vec![Value::Text("active".into())],
    );
    let rows = count.query(&mut tx).await.unwrap();
    let seen = rows
        .first()
        .and_then(|row| row.get("n"))
        .and_then(serde_json::Value::as_u64);
    assert_eq!(seen, Some(3), "transaction sees its own uncommitted row");

    tx.commit().await.unwrap();
    pool.close().await;
}

/// Negative: a syntax error in a raw statement is a typed `OrmError::Storage`.
#[tokio::test]
async fn raw_syntax_error_is_storage_error() {
    let pool = pool_with_users().await;
    let error = raw("SELECT * FRM users", vec![])
        .query(&pool)
        .await
        .unwrap_err();
    assert!(matches!(error, OrmError::Storage(_)), "got {error:?}");
    pool.close().await;
}

/// Negative: a syntax error in an inline fragment is also a typed storage error.
#[tokio::test]
async fn raw_sql_syntax_error_is_storage_error() {
    let pool = pool_with_users().await;
    let error = raw_sql("SELCT 1").query(&pool).await.unwrap_err();
    assert!(matches!(error, OrmError::Storage(_)), "got {error:?}");
    pool.close().await;
}

/// Positive: a bound value carrying SQL metacharacters is data, not statement
/// text — the injection payload cannot alter the predicate.
#[tokio::test]
async fn raw_binds_injection_payload_as_data() {
    let pool = pool_with_users().await;
    let statement = raw(
        "SELECT COUNT(*) AS n FROM users WHERE status = $1",
        vec![Value::Text("active' OR '1'='1".into())],
    );
    let rows = statement.query(&pool).await.unwrap();
    let count = rows
        .first()
        .and_then(|row| row.get("n"))
        .and_then(serde_json::Value::as_u64);
    assert_eq!(count, Some(0), "payload must be bound, not executed");
    pool.close().await;
}

/// Positive: a UUID bound into a column declared `UUID` reads back as its
/// hyphenated string on SQLite, where that declared type has NUMERIC affinity
/// and the driver reports neither `UUID` nor `BLOB` as the column type.
#[tokio::test]
async fn uuid_column_declared_as_uuid_round_trips_on_sqlite() {
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.execute_raw("CREATE TABLE things (id UUID PRIMARY KEY)", &[])
        .await
        .unwrap();
    let id = uuid::Uuid::new_v4();
    pool.execute_raw("INSERT INTO things (id) VALUES ($1)", &[Value::Uuid(id)])
        .await
        .unwrap();

    let rows = pool
        .fetch_json("SELECT id FROM things WHERE id = $1", &[Value::Uuid(id)])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"].as_str(), Some(id.to_string().as_str()));
}
