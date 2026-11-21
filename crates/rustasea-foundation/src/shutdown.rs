//! Graceful shutdown utilities — SIGTERM/SIGINT drain with a timeout.
//!
//! [`graceful`] resolves when the process receives SIGTERM or SIGINT (Ctrl-C
//! elsewhere), which is the moment a server handed it as its graceful-shutdown
//! signal stops accepting connections and starts draining. From then on the
//! drain has `timeout` to finish: if the process is still running when it
//! elapses, it exits with status 1, so a stuck request or connection cannot
//! hold shutdown open indefinitely.
//!
//! The deadline must be cancellable, or a drain that completes inside the
//! timeout would still be force-exited if the caller keeps the runtime alive
//! afterwards. [`graceful`] therefore returns a [`GracefulShutdown`] whose
//! [`GracefulShutdown::guard`] the caller holds for the whole drain; dropping
//! that guard aborts the pending timer.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::task::JoinHandle;

/// Exit status used when the drain outlives its timeout.
const DRAIN_TIMEOUT_EXIT_CODE: i32 = 1;

/// Drain budget the framework's default shutdown path uses.
pub const DEFAULT_DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

/// State of the drain deadline.
enum DeadlineState {
    /// The signal has not arrived, so no timer exists yet.
    Unarmed,
    /// The signal arrived; the deadline task is running.
    Armed(JoinHandle<()>),
    /// A guard was dropped, so the deadline must never arm.
    Cancelled,
}

/// Shared handle to the deadline state.
type Deadline = Arc<Mutex<DeadlineState>>;

/// Lock the deadline state, recovering from a poisoned mutex.
fn lock(deadline: &Deadline) -> MutexGuard<'_, DeadlineState> {
    deadline
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Guard that cancels the drain deadline when dropped.
///
/// Hold it for as long as the drain runs, then drop it. Dropping it aborts the
/// timer that the signal armed, so a drain that finished inside the timeout is
/// never exited with status 1 afterwards. Dropping it before the signal
/// prevents the timer from arming at all.
#[must_use = "dropping the guard is what cancels the shutdown deadline"]
pub struct ShutdownGuard {
    /// State shared with the shutdown future.
    deadline: Deadline,
}

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        // Swap under the lock so a concurrent arm cannot leave an unowned
        // timer behind; `Cancelled` keeps a later poll from re-arming it.
        let armed = std::mem::replace(&mut *lock(&self.deadline), DeadlineState::Cancelled);
        if let DeadlineState::Armed(handle) = armed {
            handle.abort();
        }
    }
}

/// Shutdown signal future paired with the guard that cancels its deadline.
///
/// Awaiting it resolves when SIGTERM/SIGINT arrives and arms the deadline.
/// Call [`GracefulShutdown::guard`] before awaiting and hold that guard until
/// the drain completes.
pub struct GracefulShutdown {
    /// Drain budget measured from the signal.
    timeout: Duration,
    /// State shared with the guard.
    deadline: Deadline,
    /// The underlying signal wait.
    signal: Pin<Box<dyn Future<Output = ()> + Send>>,
}

impl GracefulShutdown {
    /// Cancellation handle for the deadline; hold it until the drain completes.
    pub fn guard(&self) -> ShutdownGuard {
        ShutdownGuard {
            deadline: Arc::clone(&self.deadline),
        }
    }
}

impl Future for GracefulShutdown {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        match this.signal.as_mut().poll(cx) {
            Poll::Ready(()) => {
                let timeout = this.timeout;
                arm_with(&this.deadline, timeout, move || {
                    eprintln!("shutdown: drain did not finish within {timeout:?}; exiting");
                    std::process::exit(DRAIN_TIMEOUT_EXIT_CODE);
                });
                Poll::Ready(())
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Wait for SIGTERM/SIGINT, then bound the drain that follows by `timeout`.
///
/// Resolves as soon as the signal arrives, so the caller can begin draining.
/// Await it together with [`GracefulShutdown::guard`]: hold the guard until the
/// drain ends so the armed timer is aborted and a clean drain never exits with
/// status 1 afterwards. A dropped guard also keeps a later signal from arming
/// the timer, so shutdown completes without any forced exit.
///
/// A `timeout` of [`Duration::ZERO`] grants the drain no grace period: the
/// armed timer's sleep has already elapsed, so the process exits with status 1
/// as soon as the runtime polls that timer after the signal. Any non-zero
/// timeout is measured from the signal. Dropping the guard before the signal
/// skips the timer entirely, zero timeout included.
pub fn graceful(timeout: Duration) -> GracefulShutdown {
    GracefulShutdown {
        timeout,
        deadline: Arc::new(Mutex::new(DeadlineState::Unarmed)),
        signal: Box::pin(wait_for_signal()),
    }
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

/// Arm the deadline unless another poll already armed it.
///
/// Spawning inside the critical section keeps check-and-store atomic with
/// respect to a guard drop; the spawned task cannot run before the lock is
/// released.
fn arm_with(deadline: &Deadline, timeout: Duration, on_timeout: impl FnOnce() + Send + 'static) {
    let mut state = lock(deadline);
    if !matches!(*state, DeadlineState::Unarmed) {
        return;
    }
    *state = DeadlineState::Armed(tokio::spawn(deadline_timer(timeout, on_timeout)));
}

/// Run `on_timeout` once `timeout` has elapsed.
///
/// Split out of [`arm_with`] so the deadline can be tested without exiting.
async fn deadline_timer(timeout: Duration, on_timeout: impl FnOnce() + Send + 'static) {
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
        let task = tokio::spawn(deadline_timer(Duration::from_millis(200), move || {
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

    #[tokio::test]
    async fn dropping_the_guard_cancels_the_deadline() {
        // Mirrors a drain that finishes inside the timeout: the guard is held
        // while the timer is armed, then dropped before the timer can fire.
        let deadline: Deadline = Arc::new(Mutex::new(DeadlineState::Unarmed));
        let guard = ShutdownGuard {
            deadline: Arc::clone(&deadline),
        };
        let fired = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&fired);
        arm_with(&deadline, Duration::from_millis(10), move || {
            flag.store(true, Ordering::SeqCst);
        });
        assert!(
            matches!(*lock(&deadline), DeadlineState::Armed(_)),
            "deadline was not armed"
        );

        drop(guard);
        tokio::time::sleep(Duration::from_millis(40)).await;

        assert!(
            matches!(*lock(&deadline), DeadlineState::Cancelled),
            "deadline was not cancelled"
        );
        assert!(
            !fired.load(Ordering::SeqCst),
            "a clean drain still hit the deadline exit"
        );
    }
}
