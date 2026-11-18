//! Shared route helpers — form decoding, JSON error envelopes, config seams,
//! and the origin-aware CSRF guard primitives.
//!
//! These were previously duplicated verbatim between [`super::auth`] and
//! [`super::settings`] (F-03/F-06), which pushed both files to the 500-line
//! hard cap. They now live once here and are imported by both concern tables.
//!
//! The CSRF pieces ([`csrf_protected`], [`csrf_token`]) back the single
//! `with_csrf` gate wired onto the compiled router in [`super`]: the guard
//! matches an exact `(method, path)` allow-list, so it can never leak onto
//! unrelated routes (the sticky-middleware trap).

use std::sync::{Arc, OnceLock, RwLock};

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use rustasea::auth::{FortifyConfig, UserProvider};
use rustasea::ConfigLoader;

/// Process-wide `[fortify]` config, loaded once from the layered `config/*.toml`.
///
/// Cached in a [`OnceLock`] because handlers run per request and the config is
/// immutable after boot. A missing or malformed table falls back to
/// [`FortifyConfig::default`] (the shipped defaults), so a config failure never
/// disables throttling or email verification.
pub(crate) fn fortify_config() -> &'static FortifyConfig {
    static CONFIG: OnceLock<FortifyConfig> = OnceLock::new();
    CONFIG.get_or_init(|| {
        ConfigLoader::load()
            .ok()
            .and_then(|loader| FortifyConfig::from_loader(&loader).ok())
            .unwrap_or_default()
    })
}

/// Process-wide async [`UserProvider`] seam, built once at first use.
///
/// [`crate::bootstrap::auth::build_user_provider`] is fail-closed: unless the
/// dev-only `RUSTASEA_DEV_SEED_USER` env var is set it installs
/// [`DenyAllProvider`](rustasea::auth::DenyAllProvider), so no credential
/// resolves and login always fails with bad credentials.
static PROVIDER: OnceLock<RwLock<Arc<dyn UserProvider>>> = OnceLock::new();

/// The process-wide provider cell, initialized to the fail-closed build.
fn provider_cell() -> &'static RwLock<Arc<dyn UserProvider>> {
    PROVIDER.get_or_init(|| RwLock::new(crate::bootstrap::auth::build_user_provider()))
}

/// Resolve the shared [`UserProvider`] seam.
///
/// Returns a clone of the installed provider so a caller can await it without
/// holding the read lock across the await. A poisoned lock falls back to a
/// fresh fail-closed provider rather than panicking.
pub(crate) fn user_provider() -> Arc<dyn UserProvider> {
    match provider_cell().read() {
        Ok(provider) => Arc::clone(&provider),
        Err(_) => crate::bootstrap::auth::build_user_provider(),
    }
}

/// Replace the process-wide [`UserProvider`] seam.
///
/// The handlers resolve the provider through [`user_provider`]. Boot installs
/// the database-backed provider here (see
/// [`crate::bootstrap::auth::install_database_user_provider`]), and a test that
/// needs a real credential match seeds it here first.
pub(crate) fn install_user_provider(provider: Arc<dyn UserProvider>) {
    if let Ok(mut slot) = provider_cell().write() {
        *slot = provider;
    }
}

/// Parse an `application/x-www-form-urlencoded` body into ordered key/value
/// pairs.
///
/// Parsed by hand (rather than via axum's `Form` extractor) so the username
/// field name can come from `[fortify].username` instead of a fixed struct
/// field, and so the handler keeps full control of the response envelope. Both
/// `%XX` escapes and `+`-for-space are decoded.
pub(crate) fn parse_form(body: &[u8]) -> Vec<(String, String)> {
    String::from_utf8_lossy(body)
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (percent_decode(key), percent_decode(value))
        })
        .collect()
}

/// Return the value for `name` from parsed form `fields`.
pub(crate) fn field<'a>(fields: &'a [(String, String)], name: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// Percent-decode an urlencoded component (`%40` → `@`, `+` → space).
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                match (hex_value(bytes[index + 1]), hex_value(bytes[index + 2])) {
                    (Some(high), Some(low)) => {
                        out.push(high * 16 + low);
                        index += 3;
                    }
                    // Malformed escape: keep the literal `%`.
                    _ => {
                        out.push(b'%');
                        index += 1;
                    }
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Decode a single ASCII hex digit.
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Build a `303 See Other` redirect to `location`.
///
/// `303` (not `302`) converts the POST into a GET, so the browser does not
/// re-submit the credentials when it follows the redirect. An invalid location
/// falls back to `/` rather than panicking.
pub(crate) fn see_other(location: &str) -> Response {
    rustasea::http::see_other(location)
}

/// One item in the app's `{"errors":[...]}` JSON error envelope
/// (`status`/`code`/`title`/`detail`).
#[derive(serde::Serialize)]
pub(crate) struct ErrorItem {
    status: String,
    code: String,
    title: String,
    detail: String,
}

/// The `{"errors":[...]}` envelope shared by the JSON error responses.
#[derive(serde::Serialize)]
pub(crate) struct ErrorEnvelope {
    errors: Vec<ErrorItem>,
}

/// Build a JSON error response in the app's `{"errors":[...]}` envelope.
pub(crate) fn json_error(status: StatusCode, code: &str, detail: &str) -> Response {
    let item = ErrorItem {
        status: status.as_u16().to_string(),
        code: code.to_string(),
        title: status.canonical_reason().unwrap_or("Error").to_string(),
        detail: detail.to_string(),
    };
    (status, axum::Json(ErrorEnvelope { errors: vec![item] })).into_response()
}

/// Whether `(method, path)` is a mutating route gated by the CSRF guard.
///
/// Matched by exact method + path so the guard is scoped to the known write
/// routes and can never leak onto unrelated routes — the guard runs as a global
/// axum layer (see [`super::with_csrf`]), so this predicate *is* the scope.
/// Covers the implemented writes (`/login`, `/logout`, the settings
/// `PATCH`/`PUT`, the password-reset writes), the still-`501` POST stubs, and
/// the example posts resource (`/examples/posts`) for consistency.
pub(crate) fn csrf_protected(method: &axum::http::Method, path: &str) -> bool {
    // The passkey delete route is parameterised (`/user/passkeys/{id}`), so it
    // is matched by prefix rather than the exact `(method, path)` allow-list.
    if method == axum::http::Method::DELETE && path.starts_with("/user/passkeys/") {
        return true;
    }
    // The queue-dashboard failed-job actions are parameterised
    // (`/queue/failed/{id}/retry`, `/queue/failed/{id}`), so they are matched by
    // prefix too (ADOPT-021).
    if method == axum::http::Method::POST
        && path.starts_with("/queue/failed/")
        && path.ends_with("/retry")
    {
        return true;
    }
    if method == axum::http::Method::DELETE && path.starts_with("/queue/failed/") {
        return true;
    }
    // The example posts resource (`/examples/posts/{id}`) is parameterised, so
    // the mutating verbs are matched by prefix too.
    if path.starts_with("/examples/posts") {
        return matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE");
    }
    matches!(
        (method.as_str(), path),
        ("POST", "/login")
            | ("POST", "/logout")
            | ("POST", "/register")
            | ("POST", "/confirm-password")
            | ("POST", "/email/verification-notification")
            | ("POST", "/forgot-password")
            | ("POST", "/reset-password")
            | ("POST", "/two-factor-challenge")
            | ("POST", "/user/two-factor-authentication")
            | ("POST", "/user/confirmed-two-factor-authentication")
            | ("DELETE", "/user/two-factor-authentication")
            | ("POST", "/user/two-factor-recovery-codes")
            | ("POST", "/user/passkeys")
            | ("POST", "/passkeys/login")
            | ("PATCH", "/settings/profile")
            | ("PUT", "/settings/password")
            // Broadcasting channel authorization (ADOPT-022). A Pusher client
            // posts `socket_id`/`channel_name` here; the write is CSRF-gated.
            | ("POST", "/broadcasting/auth")
    )
}

/// Process-wide expected CSRF token for the app's mutating routes.
///
/// The app has no per-session token store yet, so the expected token is a
/// single process-wide value, overridable with `RUSTASEA_CSRF_TOKEN` (the
/// deployment seam). The origin / `Sec-Fetch-Site` allow-list remains the
/// primary gate; this token satisfies the crate's token-first spec guard, which
/// fails closed when the token is missing or does not match. Generated once at
/// first use.
pub(crate) fn csrf_token() -> &'static str {
    static TOKEN: OnceLock<String> = OnceLock::new();
    TOKEN.get_or_init(|| std::env::var("RUSTASEA_CSRF_TOKEN").unwrap_or_else(|_| generate_token()))
}

/// Generate a non-guessable process token without a new dependency.
///
/// Seeds from the wall clock, the process id, and a monotonic counter; the
/// result is opaque and unique per process. A production deployment should pin
/// `RUSTASEA_CSRF_TOKEN` (or wire a session-bound provider) instead.
fn generate_token() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:032x}{:08x}{:08x}", nanos, std::process::id(), sequence)
}
