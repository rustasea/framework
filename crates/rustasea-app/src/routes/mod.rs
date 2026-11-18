//! Route tables for the runnable `rustasea-app` binary.
//!
//! Concern-scoped tables mirror Laravel's `routes/{web,auth,settings,console}`
//! split (ADR-0002 decision 9). Each module registers its routes onto the
//! shared [`RouteTable`] via a `register(&mut RouteTable)` function; this
//! module merges them, registers the named middleware the tables declare, and
//! compiles the result into a dispatchable axum router.
//!
//! The workspace-root `routes/web.rs` is **not** compiled (see that file's
//! shim header); the canonical route definitions live here.

pub mod auth;
/// Broadcasting authorization surface — `POST /broadcasting/auth` (ADOPT-022),
/// compiled only with the `broadcasting` feature.
#[cfg(feature = "broadcasting")]
pub mod broadcasting;
pub mod console;
#[cfg(feature = "debugbar")]
pub mod debugbar;
pub mod docs;
/// Request-error middleware — dev page + prod JSON envelope (ADOPT-010).
pub mod errors;
/// Example domain routes (`/examples/posts`) - the scaffold-style
/// [`PostController`](crate::app::http::controllers::PostController) JSON CRUD
/// surface.
pub mod examples;
mod helpers;

pub(crate) use helpers::install_user_provider;
/// Dev-only log viewer surface (`/_logs`, `/_logs/json`) — ADOPT-014, compiled
/// only with the `log-viewer` feature.
#[cfg(feature = "log-viewer")]
pub mod log_viewer;
/// Queue dashboard surface — metrics history + failed-job management
/// (ADOPT-021), compiled only with the `queue-dashboard` feature.
#[cfg(feature = "queue-dashboard")]
pub mod queue_dashboard;
pub mod settings;
pub mod web;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use tower_http::services::ServeDir;
use tower_sessions::cookie::Cookie;

use rustasea::auth::csrf::{csrf_error_response, RequestHeaders};
use rustasea::auth::{password_timeout_secs, AuthUser, Guard, PreventRequestForgery, SessionGuard};
use rustasea::http::AppState;
use rustasea::router::Router as RouteTable;

/// Middleware id: require an authenticated principal (`Extension<AuthUser>`).
pub const AUTH: &str = "auth";
/// Middleware id: require a verified email address.
pub const VERIFIED: &str = "verified";
/// Middleware id: require recent password confirmation.
pub const PASSWORD_CONFIRM: &str = "password.confirm";

/// Where unauthenticated requests to a gated route are redirected.
pub const LOGIN_PATH: &str = "/login";
/// Where authenticated-but-unverified requests to a `verified` route are
/// redirected (the kit's `verification.notice` screen).
pub const VERIFY_EMAIL_PATH: &str = "/verify-email";
/// Where authenticated-but-unconfirmed requests to a `password.confirm` route
/// are redirected (the kit's confirm-password screen).
pub const CONFIRM_PASSWORD_PATH: &str = "/confirm-password";

/// Session cookie the auth middleware reads, matching `config/session.toml`
/// (`cookie = "rustasea-session"`).
pub const SESSION_COOKIE_NAME: &str = "rustasea-session";

/// Build the full route table for the application.
///
/// Registers every concern table plus the named middleware those tables
/// declare, so [`RouteTable::try_into_axum_router`] never fails closed with
/// [`rustasea::router::RouteError::UnknownMiddleware`].
pub fn table() -> RouteTable {
    let mut table = RouteTable::new();
    register_middleware(&mut table);
    web::register(&mut table);
    auth::register(&mut table);
    settings::register(&mut table);
    console::register(&mut table);
    docs::register(&mut table);
    // Example application domain (`/examples/posts`) - the scaffold-style
    // `app/{models,http,actions}` tree, served as a JSON CRUD API.
    examples::register(&mut table);
    // Dev-only profiler surface (`/_debugbar`, `/_debugbar/json`). Registered
    // only when the feature is compiled in; the handlers answer `404` unless
    // `AppState::debug` is set (the same runtime gate `docs` uses).
    #[cfg(feature = "debugbar")]
    debugbar::register(&mut table);
    // Queue dashboard surface (`/queue`, `/queue/*.json`, retry/forget). Like
    // the debugbar surface it is compiled only with its feature; the handlers
    // additionally require `[queue.dashboard].enabled` and `AppState::debug`.
    #[cfg(feature = "queue-dashboard")]
    queue_dashboard::register(&mut table);
    // Dev-only log viewer (`/_logs`, `/_logs/json`). Like the debugbar surface
    // it is compiled only with its feature; the handlers require
    // `AppState::debug`.
    #[cfg(feature = "log-viewer")]
    log_viewer::register(&mut table);
    // Broadcasting authorization surface (`POST /broadcasting/auth`), compiled
    // only with the `broadcasting` feature (ADOPT-022).
    #[cfg(feature = "broadcasting")]
    broadcasting::register(&mut table);
    table
}

/// Compile an already-built route table into a servable axum router.
///
/// The shared [`AppState`] is injected as a request extension
/// (`Extension<Arc<AppState>>`) for every route, matching the crate-local
/// `Arc<AppState>` threading contract. A build failure (an unregistered
/// middleware id) fails closed with a `500` router rather than panicking, so
/// the served router never silently drops a declared gate.
///
/// `main` builds the table once, prints it, then calls this with the same
/// table, so the printed route table is exactly the one served. Tests use
/// [`table`] + this to build a router from a known table.
pub fn compile(mut table: RouteTable, state: Arc<AppState>) -> axum::Router {
    table.layer(axum::Extension(Arc::clone(&state)));
    match table.try_into_axum_router() {
        Ok(router) => {
            // Layer order, innermost → outermost:
            //   mount_assets → with_panic_catch → with_errors → with_session
            //   → with_csrf → with_sentry → with_debugbar
            // `with_panic_catch` is innermost so a handler panic becomes a 500
            // the error middleware can render; `with_errors` sits inside
            // `with_session` so the `Extension<AuthUser>` projection is already
            // present when it builds the dev-page context (ADOPT-010).
            let core = errors::with_errors(errors::with_panic_catch(mount_assets(router)), &state);
            let session = with_session(core, &state);
            let csrf = with_csrf(session, &state);
            with_debugbar(with_sentry(csrf), &state)
        }
        Err(error) => {
            eprintln!("route table build failed: {error}");
            axum::Router::new().fallback(|| async { StatusCode::INTERNAL_SERVER_ERROR })
        }
    }
}

/// Apply the Sentry request-context layer to a compiled router (ADOPT-004).
///
/// The middleware tags each request with its method/path/request-id and captures
/// a Sentry event for any 5xx response. It is a no-op while no Sentry client is
/// bound, so the layer is safe to apply unconditionally — but it only exists
/// when the `sentry` feature is compiled in, hence the `cfg` gate.
#[cfg(feature = "sentry")]
fn with_sentry(router: axum::Router) -> axum::Router {
    router.layer(axum::middleware::from_fn(
        rustasea_http::sentry::sentry_context_middleware,
    ))
}

/// No-op stand-in for [`with_sentry`] when the `sentry` feature is off.
#[cfg(not(feature = "sentry"))]
fn with_sentry(router: axum::Router) -> axum::Router {
    router
}

/// Apply the dev request-profiler layer to a compiled router (ADOPT-009).
///
/// The middleware builds a per-request profile context (SQL/cache/events),
/// records the finalized profile in the process-wide ring, and is a no-op when
/// `AppState::debug` is `false`. It is the outermost layer so it wraps every
/// other layer and the route handlers, giving an accurate end-to-end duration.
/// The layer only exists when the `debugbar` feature is compiled in.
#[cfg(feature = "debugbar")]
fn with_debugbar(router: axum::Router, state: &AppState) -> axum::Router {
    let debug = state.debug;
    router.layer(axum::middleware::from_fn(move |request, next| {
        rustasea_debugbar::middleware::profiler_middleware(debug, request, next)
    }))
}

/// No-op stand-in for [`with_debugbar`] when the `debugbar` feature is off.
#[cfg(not(feature = "debugbar"))]
fn with_debugbar(router: axum::Router, _state: &AppState) -> axum::Router {
    router
}

/// Apply the session → `Extension<AuthUser>` layer to a compiled router.
///
/// # Why a global layer, not the sticky `auth` middleware id
///
/// [`RouteTable::middleware`] is **sticky**: it applies to every subsequent
/// route on the table (and is why the gates are declared inside
/// [`RouteTable::group`]). Registering the session resolver as the `auth`
/// middleware id would therefore either leak the resolver onto every later
/// route or force a re-grouping of the whole table — and the resolver must run
/// for **ungated** routes too (the nav renders the logged-in user on `/`).
///
/// This function instead wraps the fully compiled axum router with a global
/// `layer(...)`, so every route receives the projection while the existing
/// `AUTH`/`VERIFIED`/`PASSWORD_CONFIRM` gate middleware keep doing the gating
/// exactly as before. The layer is applied *outside* [`mount_assets`], so the
/// static asset service is covered too — it inserts nothing unless a valid
/// session cookie is present.
///
/// # State capture, not the `State` extractor
///
/// The guard is resolved from [`AppState`] once here and captured by the
/// middleware closure. The `State<Arc<AppState>>` extractor is deliberately not
/// used: `from_fn`'s state is fixed to `()`, and `Arc<AppState>` has no
/// `FromRef<()>` impl, so the extractor cannot resolve it. Capturing avoids a
/// second `AppState` clone per request and keeps the layer construction
/// explicit. When no guard is wired the closure is a no-op and auth fails
/// closed.
pub fn with_session(router: axum::Router, state: &AppState) -> axum::Router {
    let guard = state.auth::<SessionGuard>();
    router.layer(axum::middleware::from_fn(
        move |mut request: Request, next: Next| {
            let guard = guard.clone();
            async move {
                // Read the cookie synchronously: `Request` is not `Sync`, so it
                // must not be held across the `parse` await.
                if let Some(session_id) = session_id_from_headers(request.headers()) {
                    if let Some(principal) = resolve_principal(guard.as_deref(), &session_id).await
                    {
                        request.extensions_mut().insert(principal);
                    }
                }
                next.run(request).await
            }
        },
    ))
}

/// Apply the origin-aware CSRF gate to the mutating routes only.
///
/// # Why a scoped global layer, not the sticky `middleware` id
///
/// [`RouteTable::middleware`] is **sticky**: it applies to every subsequent
/// route on the table, so registering the gate that way would leak it onto
/// unrelated routes. This wraps the already-compiled axum router with a global
/// `layer(...)` instead, and the closure decides per request whether the
/// request targets a protected `(method, path)` (see [`helpers::csrf_protected`]).
/// The allow-list is exact, so safe routes — `GET /`, `GET /health`, assets —
/// are untouched: the guard is a no-op for them.
///
/// # The decision
///
/// For a protected route the crate's [`PreventRequestForgery`] runs the full
/// token-first + `Sec-Fetch-Site`/`Origin` decision table. The expected token
/// is [`helpers::csrf_token`] and the origin allow-list is
/// [`AppState::csrf_origins`](rustasea::http::AppState). A failure returns the
/// crate's `403` JSON envelope via [`csrf_error_response`]; safe methods and
/// non-protected routes pass straight through.
///
/// # Fail-closed
///
/// The crate's [`PreventRequestForgery::check`] fails closed on a missing or
/// mismatched token and on a `cross-site` request whose `Origin` is not in the
/// allow-list. An empty allow-list therefore still rejects cross-site writes,
/// and a request with no CSRF token at all is rejected — never silently
/// allowed.
pub fn with_csrf(router: axum::Router, state: &AppState) -> axum::Router {
    let origins = state.csrf_origins().to_vec();
    let expected_token = helpers::csrf_token().to_string();
    router.layer(axum::middleware::from_fn(
        move |request: Request, next: Next| {
            let policy = PreventRequestForgery::new(origins.clone())
                .with_expected_token(expected_token.clone());
            async move {
                if helpers::csrf_protected(request.method(), request.uri().path()) {
                    let headers = RequestHeaders::from_request(&request);
                    if let Err(error) = policy.check(&headers) {
                        return csrf_error_response(&error);
                    }
                }
                next.run(request).await
            }
        },
    ))
}

/// Resolve the authenticated principal for a session id, or `None`.
///
/// Split out so the fail-closed branches are unit-testable without a full
/// middleware stack. `None` means "not authenticated" — the caller inserts
/// nothing. Every failure mode (no guard, an invalid or unknown session id, or
/// a store error) collapses to `None`.
pub async fn resolve_principal(guard: Option<&SessionGuard>, session_id: &str) -> Option<AuthUser> {
    let guard = guard?;
    guard.parse(session_id).await.ok()
}

/// Extract the session id from the request's `Cookie` header, if present.
///
/// Parses the `Cookie` header into individual cookies with
/// [`tower_sessions::cookie::Cookie`] (the same `cookie` crate `rustasea-auth`
/// builds the session cookie with) and returns the value of the one named
/// [`SESSION_COOKIE_NAME`]. A missing header, a header that is not valid UTF-8,
/// or the absence of the named cookie all yield `None`.
pub fn session_id_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    Cookie::split_parse(raw)
        .filter_map(Result::ok)
        .find(|cookie| cookie.name() == SESSION_COOKIE_NAME)
        .map(|cookie| cookie.value().to_string())
}

/// Mount the static asset service at `/assets/*`, serving the workspace
/// `resources/` directory.
///
/// [`ServeDir`] owns all path handling: it percent-decodes the request path,
/// rejects any segment containing `..` (or a backslash), and returns `404`
/// rather than escaping the root, so traversal is handled by the library and
/// never hand-rolled. The route is mounted on the already-compiled router
/// outside any [`RouteTable::group`], so it inherits no authentication gate,
/// and the `/assets` prefix cannot shadow the existing named routes (`/`,
/// `/health`, `/dashboard`, `/settings/*`, `/console`).
fn mount_assets(router: axum::Router) -> axum::Router {
    router.nest_service("/assets", ServeDir::new(resources_root()))
}

/// Locate the workspace `resources/` directory.
///
/// Prefers the process-relative `resources/` — correct for a deployed app and
/// for `cargo run` from the workspace root — and otherwise falls back to the
/// workspace root derived from `CARGO_MANIFEST_DIR` (the crate lives at
/// `crates/rustasea-app`, two levels below it). The fallback keeps `cargo test`
/// working, since tests run with the crate root as their working directory.
pub(crate) fn resources_root() -> PathBuf {
    let cwd_relative = PathBuf::from("resources");
    if cwd_relative.is_dir() {
        return cwd_relative;
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("resources")
}

/// Register the named middleware the concern tables declare.
fn register_middleware(table: &mut RouteTable) {
    table.register_middleware(AUTH, |method_router| {
        method_router.layer(axum::middleware::from_fn(require_authenticated))
    });
    table.register_middleware(VERIFIED, |method_router| {
        method_router.layer(axum::middleware::from_fn(require_verified))
    });
    table.register_middleware(PASSWORD_CONFIRM, |method_router| {
        method_router.layer(axum::middleware::from_fn(require_password_confirmed))
    });
}

/// `auth` guard — reject requests without an authenticated principal.
///
/// Identity travels as a request extension (`Extension<AuthUser>`), the
/// convention documented by the stateless `SessionGuard`/`JwtGuard` in
/// `rustasea-auth`: the guard holds no per-request state, so the HTTP layer
/// projects `Guard::parse` results into `Extension<AuthUser>`. The projection is
/// performed by [`session_middleware`], which runs globally over the compiled
/// router and inserts the extension when the request carries a resolvable
/// session cookie. A request lacking that extension (no cookie, an unknown id,
/// or no auth slot) is answered with `302 Found` → [`LOGIN_PATH`] (the web-app
/// convention, so a browser is sent to the login form) rather than `401`.
async fn require_authenticated(request: Request, next: Next) -> Response {
    if request.extensions().get::<AuthUser>().is_some() {
        next.run(request).await
    } else {
        redirect_to_login()
    }
}

/// `verified` guard — email-verification gate.
///
/// Identity travels as an `Extension<AuthUser>` (see [`require_authenticated`]).
/// The gate fails closed in two stages:
///
/// * **Unauthenticated** (no extension) → `302 Found` → [`LOGIN_PATH`], so a
///   browser is sent to the login form first.
/// * **Authenticated but unverified** (`email_verified_at` is `None`) →
///   `302 Found` → [`VERIFY_EMAIL_PATH`], the kit's `verification.notice`
///   screen where the user re-sends the verification email. Redirecting to
///   login here would wrongly discard a valid session, and returning `403`
///   would strand the user with no route forward; the notice screen is the
///   actionable destination.
///
/// Only a principal carrying an `email_verified_at` timestamp reaches the
/// handler. The check is the real one — no degradation to a plain auth check.
async fn require_verified(request: Request, next: Next) -> Response {
    match request.extensions().get::<AuthUser>() {
        None => redirect_to(LOGIN_PATH),
        Some(user) if !user.is_email_verified() => redirect_to(VERIFY_EMAIL_PATH),
        Some(_) => next.run(request).await,
    }
}

/// `password.confirm` guard — recent-password-confirmation gate.
///
/// The confirmation timestamp is read from the authenticated principal's
/// `password_confirmed_at` field (populated by the session guard from the
/// `auth.password_confirmed_at` session entry) and compared against the
/// configured window — `AUTH_PASSWORD_TIMEOUT` / `[auth] password_timeout`,
/// defaulting to 3 hours. The gate fails closed in two stages:
///
/// * **Unauthenticated** (no extension) → `302 Found` → [`LOGIN_PATH`].
/// * **Authenticated but unconfirmed/stale** → `302 Found` →
///   [`CONFIRM_PASSWORD_PATH`], the kit's confirm-password screen. Returning
///   `403` would not tell the browser where to re-confirm; the dedicated
///   screen lets the user re-authenticate their password and continue.
///
/// A principal that never confirmed (or whose confirmation is older than the
/// window) is rejected — the placeholder degradation is gone.
async fn require_password_confirmed(request: Request, next: Next) -> Response {
    let timeout_secs = password_timeout_secs();
    match request.extensions().get::<AuthUser>() {
        None => redirect_to(LOGIN_PATH),
        Some(user) if !user.is_password_confirmed(timeout_secs) => {
            redirect_to(CONFIRM_PASSWORD_PATH)
        }
        Some(_) => next.run(request).await,
    }
}

/// Build the `302 Found` → [`LOGIN_PATH`] redirect response.
fn redirect_to_login() -> Response {
    redirect_to(LOGIN_PATH)
}

/// Build a `302 Found` redirect to `location`.
///
/// Delegates to the framework helper [`rustasea::http::redirect`], which keeps
/// the invalid-location fallback (`/`) in one place.
fn redirect_to(location: &'static str) -> Response {
    rustasea::http::redirect(location)
}
