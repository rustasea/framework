//! Graceful shutdown utilities — SIGTERM/SIGINT drain with a timeout.
//!
//! [`graceful`] resolves when the process receives SIGTERM or SIGINT (Ctrl-C
//! elsewhere), which is the moment a server handed it as its graceful-shutdown
//! signal stops accepting connections and starts draining. From then on the
//! drain has `timeout` to finish: if the process is still running when it
//! elapses, it exits with status 1, so a stuck request or connection cannot
//! hold shutdown open indefinitely.

use std::time::Duration;

/// Exit status used when the drain outlives its timeout.
const DRAIN_TIMEOUT_EXIT_CODE: i32 = 1;

/// Wait for SIGTERM/SIGINT, then bound the drain that follows by `timeout`.
///
/// Returns as soon as the signal arrives, so the caller can begin draining. A
/// background deadline then exits the process with status 1 if it has not
/// exited on its own within `timeout`.
pub async fn graceful(timeout: Duration) {
    wait_for_signal().await;
    tokio::spawn(deadline(timeout, move || {
        eprintln!("shutdown: drain did not finish within {timeout:?}; exiting");
        std::process::exit(DRAIN_TIMEOUT_EXIT_CODE);
    }));
}

/// Resolve on SIGTERM or SIGINT (Ctrl-C on non-Unix targets).
async fn wait_for_signal() {
    #[cfg(unix)]
    {
        let mut term =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        let mut int =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).ok();
        tokio::select! {
            _ = async { if let Some(s) = term.as_mut() { s.recv().await; } } => {},
            _ = async { if let Some(s) = int.as_mut() { s.recv().await; } } => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Run `on_timeout` once `timeout` has elapsed.
///
/// Split out of [`graceful`] so the deadline can be tested without exiting.
async fn deadline(timeout: Duration, on_timeout: impl FnOnce() + Send + 'static) {
    tokio::time::sleep(timeout).await;
    on_timeout();
}

/// Shutdown handle wrapping a timeout.
pub struct ShutdownHandle {
    /// Timeout for drain.
    pub timeout: Duration,
}

impl ShutdownHandle {
    /// Create a new handle.
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Wait for the shutdown signal and bound the drain by the handle's timeout.
    pub async fn drain(self) {
        graceful(self.timeout).await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use super::*;

    #[tokio::test]
    async fn the_deadline_fires_only_after_the_timeout() {
        let fired = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&fired);
        let task = tokio::spawn(deadline(Duration::from_millis(200), move || {
            flag.store(true, Ordering::SeqCst);
        }));

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!fired.load(Ordering::SeqCst), "fired before the timeout");

        task.await.expect("deadline task");
        assert!(
            fired.load(Ordering::SeqCst),
            "did not fire after the timeout"
        );
    }
}
