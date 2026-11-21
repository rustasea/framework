//! Crate entry-point templates for a generated app: `lib.rs` and `main.rs`.
//!
//! Split out of [`super::core`] to keep that file under the 500-line limit.

pub(super) const LIB_RS: &str = r##"//! @@app_pascal@@ — RustaSea application library.
//!
//! The module tree mirrors the Laravel layout: `app/` holds domain actions,
//! concerns, HTTP, models, and providers; `bootstrap/` wires the kernel;
//! `routes/` owns the route tables; `database/` holds migrations, factories,
//! and seeders.

pub mod app;
pub mod bootstrap;
pub mod database;
pub mod routes;
"##;

pub(super) const MAIN_RS: &str = r##"//! @@app_pascal@@ HTTP entry point.

use std::net::SocketAddr;
use std::sync::Arc;

use @@app_snake@@::{bootstrap, routes};
use rustasea::http::AppState;

/// Default bind address for the dev server (Laravel `php artisan serve` parity).
const DEFAULT_BIND: &str = "0.0.0.0:8000";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the container and run the provider boot DAG. `configure`
    // returns `Result` so a misconfigured boot aborts before the server starts
    // instead of silently ignoring the failure.
    let app = bootstrap::app::configure()?;

    // `database` queue connections need a live pool, which providers cannot
    // open; register them now through the database connection bound at boot.
    if let Err(error) = rustasea::services::connect_database_queues(&app).await {
        eprintln!("queue: database connections not registered: {error}");
    }

    // Create the shared HTTP state once; every route table receives this same
    // instance so handlers resolve one application state, not a disconnected one.
    let state = Arc::new(AppState::new("local", true));

    // Build the axum router from the generated route tables.
    let router = routes::router(state);

    // Resolve the bind address from `APP_URL` (host:port) so the advertised URL
    // and the listening socket stay in sync; fall back to the default.
    let addr: SocketAddr = bind_address();
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("@@app_pascal@@ listening on http://{addr}");

    // `ConnectInfo` hands every request its client IP (throttle keys, audit logs).
    axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(app.shutdown())
        .await?;
    Ok(())
}

/// Resolve the bind address from `APP_URL` (host:port) or the default.
fn bind_address() -> SocketAddr {
    std::env::var("APP_URL")
        .ok()
        .and_then(parse_host_port)
        .unwrap_or_else(default_bind)
}

/// The fallback bind address used when `APP_URL` is unset or carries no port.
fn default_bind() -> SocketAddr {
    match DEFAULT_BIND.parse() {
        Ok(addr) => addr,
        // `DEFAULT_BIND` is a compile-time constant and always parses; this arm
        // keeps the function panic-free without an `expect`.
        Err(_) => SocketAddr::from(([0, 0, 0, 0], 8000)),
    }
}

/// Parse an `APP_URL` value into a `SocketAddr` when it carries a port.
fn parse_host_port(url: String) -> Option<SocketAddr> {
    let authority = url.split("://").nth(1)?;
    let host_port = authority.split('/').next()?;
    host_port.parse().ok()
}
"##;
