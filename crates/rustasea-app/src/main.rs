//! RustaSea runnable app scaffold — served by `cargo run -p rustasea-app`.
//!
//! Mirrors the canonical Laravel-style layout described in `README.md`:
//! `crates/rustasea-app/src/bootstrap/app.rs` configures the [`Application`],
//! the workspace-root `config/*.toml` + `.env` provide typed settings, and
//! `crates/rustasea-app/src/routes/` owns the concern-scoped route tables
//! (`web`, `auth`, `settings`, `console`). This binary boots the framework,
//! compiles the route tables into a real dispatch router, serves it over HTTP,
//! and shuts down gracefully on SIGINT/SIGTERM.

use std::net::SocketAddr;
use std::sync::Arc;

use rustasea::http::AppState;

mod app;
mod bootstrap;
mod routes;

/// Default bind address for the dev server.
const DEFAULT_BIND: &str = "0.0.0.0:8000";

/// Program entry point: configure, boot, register routes, and serve.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Build + boot the foundation application (providers, bindings) from
    // `bootstrap/app.rs` — the README-mandated `Application::configure` home.
    let app = bootstrap::app::configure()?;

    // Read users from the database when auth is configured for it and the
    // `users` table exists; otherwise the fail-closed provider stays in place.
    bootstrap::auth::install_database_user_provider(&app).await;

    // Build the real route table from the concern-scoped tables
    // (`routes::{web,auth,settings,console}`) and print what is actually
    // served — the print reflects the compiled table, not a placeholder.
    let table = routes::table();
    println!("registered {} routes:", table.get_routes().len());
    for entry in table.get_routes() {
        let name = entry.name.as_deref().unwrap_or("-");
        println!("  {:7} {:30} {}", entry.method, entry.path, name);
    }

    // Compile that exact table into a dispatch router with the shared state.
    // The session guard was installed into the container by
    // `AuthServiceProvider::register`; seed it into `AppState` so the global
    // session middleware can project `Extension<AuthUser>` for logged-in
    // requests. When wiring failed the slot stays absent and auth fails closed.
    let mut state = AppState::new("local", true);
    if let Some(guard) = bootstrap::auth::session_guard(&app.container) {
        state = state.with_auth(guard);
    }
    let router = routes::compile(table, Arc::new(state));

    // Bind and serve with graceful shutdown.
    let addr: SocketAddr = bind_address();
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("RustaSea dev server listening on http://{addr}");

    serve(listener, router, app.shutdown()).await?;
    Ok(())
}

/// Serve `router` on `listener` until `shutdown` resolves.
///
/// Every request carries the connection's peer address as
/// `ConnectInfo<SocketAddr>`: the login throttle and the authentication log key
/// on the client IP, and without it every client falls back to `0.0.0.0` and
/// shares one throttle bucket.
pub(crate) async fn serve(
    listener: tokio::net::TcpListener,
    router: axum::Router,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
}

/// Resolve the bind address from `APP_URL` (host:port) or the default.
fn bind_address() -> SocketAddr {
    std::env::var("APP_URL")
        .ok()
        .and_then(parse_host_port)
        .unwrap_or_else(|| DEFAULT_BIND.parse().expect("static default bind address"))
}

/// Parse an `APP_URL` value into a `SocketAddr` when it carries a port.
fn parse_host_port(url: String) -> Option<SocketAddr> {
    let authority = url.split("://").nth(1)?;
    let host_port = authority.split('/').next()?;
    host_port.parse().ok()
}
