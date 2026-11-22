//! `InsertBuilder::insert_or_ignore` against a real in-memory SQLite pool.

use rustasea_orm::{DbPool, InsertBuilder, QueryBuilder};

async fn pool() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.execute_script("CREATE TABLE subscribers (email TEXT NOT NULL UNIQUE);")
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn insert_or_ignore_skips_a_conflicting_row() {
    let pool = pool().await;
    let row = InsertBuilder::table("subscribers").column("email", "ada@example.com");

    assert!(row.insert_or_ignore(&pool).await.unwrap(), "first insert");
    assert!(
        !row.insert_or_ignore(&pool).await.unwrap(),
        "conflict ignored"
    );
    let count = QueryBuilder::table("subscribers")
        .count(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let other = InsertBuilder::table("subscribers").column("email", "grace@example.com");
    assert!(other.insert_or_ignore(&pool).await.unwrap());
}

#[tokio::test]
async fn insert_or_ignore_skips_a_not_null_violation() {
    let pool = pool().await;
    pool.execute_script("CREATE TABLE strict_rows (email TEXT, name TEXT NOT NULL);")
        .await
        .unwrap();

    // `name` is NOT NULL with no default. SQLite's `INSERT OR IGNORE` suppresses
    // the constraint failure instead of returning it, so the row is skipped and
    // the caller cannot tell this apart from a duplicate.
    let row = InsertBuilder::table("strict_rows").column("email", "ada@example.com");
    assert!(
        !row.insert_or_ignore(&pool).await.unwrap(),
        "a NOT NULL violation is suppressed, not returned"
    );

    let count = QueryBuilder::table("strict_rows")
        .count(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "no row was inserted");
}
