/// Central queue routing registry — `Queue::route` + dispatch resolution.
///
/// The lock-free registry state, route insertion, payload envelope builder, and
/// driver resolution live in the [`helpers`] submodule (re-exported here) so
/// this module stays within the file-size standard; the [`Queue`] and
/// [`QueueRegistry`] facades remain in this file.
mod helpers;

use std::sync::Arc;

use crate::batch::BatchId;
use crate::driver::{
    dead_letter_sync, DatabaseDriver, QueueDriver, DATABASE_CONNECTION, SYNC_CONNECTION,
};
use crate::error::{QueueError, Result};
use crate::job::{run_erased, DispatchHandle, ErasedJob, Job, JobId, JobOutcome};
use crate::metrics::{QueueMetrics, Queues};
use crate::unique::DispatchOutcome;

pub use helpers::Route;
pub(crate) use helpers::{
    driver, insert_route, metric_targets, registry, to_payload, RegistryInner,
};

/// The `Queue` facade — typed dispatch, chain, batch, and metrics.
///
/// Mirrors the doc-shape `Queue` surface: `route` registers a job type to a
/// connection/queue once (boot-time), `dispatch` resolves the route and pushes
/// through the connection's driver, `onQueue`/`onConnection`/`delay` override
/// per dispatch via `DispatchHandle`, and the metric methods surface the Cloud
/// gauges per connection/queue (FS-M4-03).
pub struct Queue;

impl Queue {
    /// Register `J` to be dispatched to `queue` on `connection` by default.
    ///
    /// The route is keyed by `std::any::type_name::<J>()` (which matches the
    /// default `Job::queue_name`), so dispatch resolves the same key from the
    /// erased job. Duplicate registration for the same type returns
    /// `QueueError::DuplicateRoute`.
    pub fn route<J: Job + 'static>(connection: &'static str, queue: &'static str) -> Result<Route> {
        let route = Route { connection, queue };
        insert_route(std::any::type_name::<J>(), route.clone())?;
        Ok(route)
    }

    /// Register `J` on the default sync connection (registry-only testing).
    pub fn route_sync<J: Job + 'static>(queue: &'static str) -> Result<Route> {
        Self::route::<J>(SYNC_CONNECTION, queue)
    }

    /// Resolve the registered route for a job type name, or `Unrouted`.
    pub fn resolve(type_key: &str) -> Result<Route> {
        let reg = registry();
        let guard = reg.read().map_err(QueueError::from)?;
        guard
            .routes
            .get(type_key)
            .cloned()
            .ok_or_else(|| QueueError::Unrouted(type_key.to_string()))
    }

    /// Resolve the driver registered under `connection`.
    pub fn driver(connection: &str) -> Result<Arc<dyn QueueDriver>> {
        driver(connection)
    }

    /// Register a driver under `connection` (boot-time).
    pub fn register_driver(connection: impl Into<String>, driver: Arc<dyn QueueDriver>) {
        let reg = registry();
        if let Ok(mut guard) = reg.write() {
            guard.drivers.insert(connection.into(), driver);
        }
    }

    /// Register a route + driver in one call (convenience).
    pub fn connect<J: Job + 'static>(
        connection: &'static str,
        queue: &'static str,
        driver: Arc<dyn QueueDriver>,
    ) -> Result<Route> {
        let route = Self::route::<J>(connection, queue)?;
        Self::register_driver(connection, driver);
        Ok(route)
    }

    /// Enqueue an already-built dispatch handle through its resolved route.
    ///
    /// The `sync` connection executes the erased job body inline (deterministic
    /// test semantics) and records permanent failures — `Failed` and (on sync)
    /// `Retrying`, which cannot re-enqueue without a real worker — into the
    /// `failed_jobs` sink. `database`/`redis` connections push the serialized
    /// payload to their registered driver for a worker to drain.
    ///
    /// When the job type is registered as unique (see
    /// [`crate::unique::register_unique`]) and a cache store is installed, a
    /// deduplicated dispatch is a silent no-op that still yields a fresh
    /// [`JobId`] (Laravel semantics). Use [`Queue::dispatch_handle_outcome`] to
    /// observe the deduplication explicitly.
    pub async fn dispatch_handle(handle: DispatchHandle) -> Result<JobId> {
        Ok(match Self::dispatch_handle_outcome(handle).await? {
            DispatchOutcome::Enqueued(id) => id,
            DispatchOutcome::Deduplicated { .. } => JobId::new(),
        })
    }

    /// Enqueue a dispatch handle, reporting whether it was enqueued or deduped.
    ///
    /// Acquires the job's unique lease (when the type is registered unique and a
    /// store is configured) before enqueuing. A [`LeaseState::Held`] result
    /// short-circuits to [`DispatchOutcome::Deduplicated`] without touching the
    /// driver. Non-unique jobs and applications without a store behave exactly
    /// as [`Queue::dispatch_handle`] always has. See
    /// [`crate::dispatch::dispatch_outcome`] for the gate itself.
    pub async fn dispatch_handle_outcome(handle: DispatchHandle) -> Result<DispatchOutcome> {
        crate::dispatch::dispatch_outcome(handle).await
    }

    /// Enqueue a typed job through its routed connection/queue.
    pub async fn dispatch<J: Job + 'static>(job: J) -> Result<JobId> {
        Self::dispatch_handle(job.dispatch()).await
    }

    /// Execute an erased job inline on the sync connection.
    ///
    /// Runs the body and returns the `JobOutcome`. `Failed` outcomes are
    /// dead-lettered (to the `database` connection's `failed_jobs` table when
    /// one is registered, else the process-local sink); `Retrying` cannot be re-enqueued on sync (no worker to
    /// release it later), so it is dead-lettered too — a sync dispatch never
    /// silently drops a failed job. `Skipped` outcomes (missing-model
    /// suppression, FR-605) are recorded as diagnostics but never dead-lettered
    /// nor retried. The driver buffer is untouched.
    pub(crate) async fn execute_sync(handle: &DispatchHandle) -> Result<JobOutcome> {
        // Running the job consumes its body, so the payload a dead letter
        // needs is captured first.
        let body = handle.exec.as_json();
        let outcome = run_erased(handle.exec.as_ref()).await;
        // A sync dispatch has no later worker to release a retry, so both
        // `Failed` and `Retrying` are dead-lettered with the attempt's trace.
        let (trace, suffix) = match &outcome {
            JobOutcome::Failed { exception } => (exception.as_str(), String::new()),
            JobOutcome::Retrying {
                attempt, exception, ..
            } => (
                exception.as_str(),
                format!(" (sync retry {attempt} not schedulable)"),
            ),
            JobOutcome::Succeeded | JobOutcome::Skipped => return Ok(outcome),
        };
        dead_letter_sync(
            handle.exec.type_key(),
            handle.queue.as_deref(),
            body,
            format!("JobError::MaxAttemptsExceeded: {trace}{suffix}"),
        )
        .await;
        Ok(outcome)
    }

    /// Execute a typed job inline on the sync connection.
    ///
    /// Returns the execution `JobOutcome`; `Failed` and `Retrying` outcomes are
    /// dead-lettered (see `execute_sync`) before the outcome is returned.
    pub async fn dispatch_sync<J: Job + 'static>(job: J) -> Result<JobOutcome> {
        let handle = job.dispatch().on_connection(SYNC_CONNECTION);
        Self::execute_sync(&handle).await
    }

    /// Run a chain of erased jobs inline on the sync connection.
    ///
    /// Executes sequentially and stops at the first non-succeeded outcome —
    /// `Failed` and `Retrying` are both dead-lettered (sync cannot schedule a
    /// later retry), so no chain step is ever silently dropped; a `Skipped`
    /// step stops the chain without dead-lettering (FR-605 suppression).
    pub async fn chain(jobs: Vec<Arc<dyn ErasedJob>>) -> Result<Vec<JobOutcome>> {
        let mut outcomes = Vec::with_capacity(jobs.len());
        for exec in jobs {
            // Captured before the run consumes the job body (see `execute_sync`).
            let body = exec.as_json();
            let outcome = run_erased(exec.as_ref()).await;
            let stopped = !matches!(outcome, JobOutcome::Succeeded);
            if matches!(
                outcome,
                JobOutcome::Failed { .. } | JobOutcome::Retrying { .. }
            ) {
                dead_letter_sync(
                    exec.type_key(),
                    None,
                    body,
                    "JobError::MaxAttemptsExceeded".to_string(),
                )
                .await;
            }
            outcomes.push(outcome);
            if stopped {
                break;
            }
        }
        Ok(outcomes)
    }

    /// Dispatch a batch of erased jobs and return a fresh `BatchId`.
    pub async fn batch(jobs: Vec<Arc<dyn ErasedJob>>) -> Result<BatchId> {
        crate::batch::dispatch_erased_in(None, jobs).await
    }

    /// Dispatch erased jobs tagged with `batch_id`; `None` mints a fresh id.
    ///
    /// Every tagged payload carries the id so a worker decrements the batch's
    /// `pending_jobs` on each terminal outcome.
    pub async fn batch_in(
        batch_id: Option<&str>,
        jobs: Vec<Arc<dyn ErasedJob>>,
    ) -> Result<BatchId> {
        crate::batch::dispatch_erased_in(batch_id, jobs).await
    }

    /// Number of available jobs for a connection/queue pair.
    pub async fn pending_size(connection: &str, queue: &str) -> Result<usize> {
        driver(connection)?.pending_size(queue).await
    }

    /// Number of delayed jobs for a connection/queue pair.
    pub async fn delayed_size(connection: &str, queue: &str) -> Result<usize> {
        driver(connection)?.delayed_size(queue).await
    }

    /// Number of reserved jobs for a connection/queue pair.
    pub async fn reserved_size(connection: &str, queue: &str) -> Result<usize> {
        driver(connection)?.reserved_size(queue).await
    }

    /// UTC instant of the oldest pending job (RFC3339), `None` when empty.
    pub async fn creation_time_of_oldest_pending_job(
        connection: &str,
        queue: &str,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
        driver(connection)?
            .creation_time_of_oldest_pending_job(queue)
            .await
    }

    /// Aggregate metric snapshot across every configured connection/queue.
    ///
    /// Builds one [`QueueMetrics`] row per routed `(connection, queue)` pair by
    /// querying its driver, so a dashboard sees real depths without ad-hoc Redis
    /// or SQL commands. A driver/store failure is surfaced as a typed
    /// `StoreUnavailable` (or `UnknownConnection`) rather than silently reported
    /// as zero, so a down store never looks like a healthy empty queue.
    pub async fn metrics() -> Result<Queues> {
        let mut snapshot = Queues::default();
        for (connection, queue) in metric_targets() {
            let drv = driver(&connection)?;
            let pending = drv.pending_size(&queue).await?;
            let delayed = drv.delayed_size(&queue).await?;
            let reserved = drv.reserved_size(&queue).await?;
            let oldest_pending = drv.creation_time_of_oldest_pending_job(&queue).await?;
            snapshot.upsert(QueueMetrics {
                connection,
                queue,
                pending,
                delayed,
                reserved,
                oldest_pending,
            });
        }
        Ok(snapshot)
    }
}

/// Register a [`DatabaseDriver`] under the canonical `database` connection.
///
/// Boot-time helper: wraps `pool` in a driver and installs it through the same
/// registry path as [`Queue::register_driver`]. Returns the registered driver
/// so callers (e.g. a worker) can hold it directly.
pub fn register_database_driver(pool: rustasea_orm::DbPool) -> Arc<DatabaseDriver> {
    let driver = Arc::new(DatabaseDriver::new(pool));
    Queue::register_driver(DATABASE_CONNECTION, driver.clone());
    driver
}

/// Register a [`crate::driver::RedisDriver`] under the `redis` connection.
///
/// Boot-time helper gated on the `redis` feature; a `None`/empty `url` installs
/// a disabled driver so a worker can skip the connection without erroring.
#[cfg(feature = "redis")]
pub fn register_redis_driver(url: Option<&str>) -> Result<Arc<crate::driver::RedisDriver>> {
    let driver = Arc::new(crate::driver::RedisDriver::from_url(url)?);
    Queue::register_driver(crate::driver::REDIS_CONNECTION, driver.clone());
    Ok(driver)
}

/// Registry facade kept for source parity with the docs' `QueueRegistry`.
pub struct QueueRegistry;

impl QueueRegistry {
    /// Register `J` under `connection`/`queue` (see `Queue::route`).
    pub fn route<J: Job + 'static>(connection: &'static str, queue: &'static str) -> Result<Route> {
        Queue::route::<J>(connection, queue)
    }

    /// The default connection name used when none is provided.
    ///
    /// Returns the connection named by `[queue].default` once
    /// [`QueueRegistry::configure_from`] has run, falling back to the historical
    /// `sync` connection when no configuration was applied — so existing callers
    /// that never configure a queue keep their previous behaviour.
    pub fn default_connection() -> &'static str {
        crate::wiring::configured_default_connection().unwrap_or(SYNC_CONNECTION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::SyncDriver;
    use crate::job::JobPayload;

    /// Register a sync-backed route under a unique connection/queue.
    ///
    /// The driver is registered before the route so a concurrent `metrics()`
    /// snapshot never observes a route without its driver.
    fn register_sync_route(
        type_key: &'static str,
        connection: &'static str,
        queue: &'static str,
    ) -> Arc<SyncDriver> {
        let drv = Arc::new(SyncDriver::new());
        Queue::register_driver(connection, drv.clone());
        insert_route(type_key, Route { connection, queue }).expect("unique route");
        drv
    }

    /// Enqueued jobs are reflected in the aggregate snapshot.
    #[tokio::test]
    async fn metrics_aggregates_configured_queue_depths() {
        let drv = register_sync_route("test::metrics::JobA", "metrics-a", "metrics-queue-a");
        for _ in 0..2 {
            drv.push(JobPayload::new(
                "metrics-queue-a",
                "metrics-a",
                None,
                serde_json::json!({}),
            ))
            .await
            .expect("push");
        }

        let snapshot = Queue::metrics().await.expect("metrics");
        let row = snapshot
            .row("metrics-a", "metrics-queue-a")
            .expect("row present");
        assert_eq!(row.pending, 2);
        assert_eq!(row.delayed, 0);
        assert_eq!(row.reserved, 0);
        assert_eq!(row.oldest_pending, None);
    }

    /// An empty configured queue reports zeros, not a missing row.
    #[tokio::test]
    async fn metrics_empty_queue_reports_zeros() {
        register_sync_route("test::metrics::JobB", "metrics-b", "metrics-queue-b");
        let snapshot = Queue::metrics().await.expect("metrics");
        let row = snapshot
            .row("metrics-b", "metrics-queue-b")
            .expect("row present");
        assert_eq!((row.pending, row.delayed, row.reserved), (0, 0, 0));
        assert_eq!(row.oldest_pending, None);
    }

    /// Two routes to the same queue collapse into a single metric row.
    #[tokio::test]
    async fn metrics_deduplicates_shared_queue() {
        register_sync_route("test::metrics::JobC", "metrics-c", "metrics-queue-c");
        register_sync_route("test::metrics::JobD", "metrics-c", "metrics-queue-c");
        let snapshot = Queue::metrics().await.expect("metrics");
        let rows = snapshot
            .queues
            .iter()
            .filter(|q| q.queue == "metrics-queue-c")
            .count();
        assert_eq!(rows, 1, "one row per configured queue");
    }

    /// The same queue name on two connections yields two distinct aggregate rows.
    #[tokio::test]
    async fn metrics_keeps_shared_queue_name_across_connections() {
        let redis = register_sync_route("test::metrics::JobE", "metrics-r", "m-shared");
        let db = register_sync_route("test::metrics::JobF", "metrics-d", "m-shared");
        for _ in 0..3 {
            let p = JobPayload::new("m-shared", "metrics-d", None, serde_json::json!({}));
            db.push(p).await.expect("push db");
        }
        let p = JobPayload::new("m-shared", "metrics-r", None, serde_json::json!({}));
        redis.push(p).await.expect("push redis");
        let snap = Queue::metrics().await.expect("metrics");
        assert_eq!(snap.row("metrics-r", "m-shared").expect("redis").pending, 1);
        assert_eq!(snap.row("metrics-d", "m-shared").expect("db").pending, 3);
    }
}
