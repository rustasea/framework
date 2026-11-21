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
