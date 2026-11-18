//! Persistent dead-letter sink for inline `sync` dispatches (M4).
//!
//! Positive: once a `database` connection is registered, a failed sync dispatch
//! lands in the `failed_jobs` table as a retryable envelope instead of the
//! process-local sink, and `retry_failed` re-enqueues it on that connection.
//!
//! One test owns this binary's process-wide registry, so the two phases
//! (before and after registering the database driver) run in a fixed order.

use rustasea_orm::DbPool;
use rustasea_queue::{
    failed_jobs, register_database_driver, Job, JobError, JobOutcome, Queue, QueueDriver,
};
use serde::{Deserialize, Serialize};

/// A job that always fails permanently.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Doomed {
    /// Marker carried through the stored payload.
    order_id: u64,
}

#[async_trait::async_trait]
impl Job for Doomed {
    /// Fail on every attempt.
    async fn handle(self) -> std::result::Result<(), JobError> {
        Err(JobError::Exception("boom".into()))
    }
}

/// Dead-letter entries in the in-memory sink for the `Doomed` job type.
fn in_memory() -> usize {
    failed_jobs()
        .into_iter()
        .filter(|f| f.queue == std::any::type_name::<Doomed>())
        .count()
}

#[tokio::test]
async fn sync_failures_persist_once_a_database_connection_exists() {
    // Without a database connection the failure stays in the in-memory sink.
    let outcome = Queue::dispatch_sync(Doomed { order_id: 1 })
        .await
        .expect("dispatch");
    assert!(!matches!(outcome, JobOutcome::Succeeded));
    assert_eq!(in_memory(), 1);
    // The dead letter keeps the job body even though running the job consumed it.
    let kept = failed_jobs()
        .into_iter()
        .find(|f| f.queue == std::any::type_name::<Doomed>())
        .expect("in-memory entry");
    assert_eq!(kept.payload["order_id"], 1);

    let pool = DbPool::connect("sqlite::memory:").await.expect("pool");
    rustasea_queue::queue_migrator()
        .run(&pool)
        .await
        .expect("queue migrations");
    let driver = register_database_driver(pool);

    Queue::dispatch_sync(Doomed { order_id: 2 })
        .await
        .expect("dispatch");

    // Persisted, and the in-memory sink did not grow.
    assert_eq!(in_memory(), 1);
    let failed = driver.failed_jobs().await.expect("failed rows");
    assert_eq!(failed.len(), 1);
    let row = &failed[0];
    assert_eq!(row.connection, "sync");
    assert_eq!(row.queue, "default");
    assert!(row.exception.contains("boom"), "{}", row.exception);
    assert_eq!(
        row.payload["job"].as_str(),
        Some(std::any::type_name::<Doomed>())
    );
    assert_eq!(row.payload["payload"]["order_id"], 2);

    // The stored envelope is retryable on the database connection.
    driver.retry_failed(row.id).await.expect("retry");
    assert!(driver.failed_jobs().await.expect("failed rows").is_empty());
    assert_eq!(driver.pending_size("default").await.expect("pending"), 1);
}
