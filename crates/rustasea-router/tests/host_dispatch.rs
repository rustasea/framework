//! Host-aware dispatch for domain-constrained routes.
//!
//! Positive: a domain route answers only its host (exact or `{placeholder}`),
//! the catch-all answers every other host, path parameters survive the host
//! dispatch, and other methods on the same path keep their own routes.
//! Negative: a domain route without a catch-all is `404` for any other host,
//! and malformed, duplicate, or non-ASCII hosts neither panic nor steal
//! another host's route.

use axum::body::Body;
use axum::extract::Path;
use axum::http::{HeaderValue, Request, StatusCode};
use rustasea_router::Router;
use tower::ServiceExt;

/// Send `method path` with an optional `Host` header; return status and body.
async fn send(
    app: &axum::Router,
    method: &str,
    path: &str,
    host: Option<&str>,
) -> (StatusCode, String) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(host) = host {
        request = request.header("host", host);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .expect("router dispatch");
    read(response).await
}

/// Send `GET path` with a raw-byte `Host` header.
///
/// Lets a test supply a header value that is not visible ASCII — those bytes
/// are accepted when building the request but rejected by `HeaderValue::to_str`
/// during host extraction, which is the malformed-value path under test.
async fn send_raw_host(app: &axum::Router, path: &str, host: &[u8]) -> (StatusCode, String) {
    let value = HeaderValue::from_bytes(host).expect("header value");
    let request = Request::builder()
        .method("GET")
        .uri(path)
        .header("host", value);
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .expect("router dispatch");
    read(response).await
}

/// Status and body of an axum response.
async fn read(response: axum::response::Response) -> (StatusCode, String) {
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn api() -> &'static str {
    "api"
}

async fn tenant() -> &'static str {
    "tenant"
}

async fn main_site() -> &'static str {
    "main"
}

async fn admin() -> &'static str {
    "admin"
}

async fn created() -> &'static str {
    "created"
}

async fn api_user(Path(id): Path<u32>) -> String {
    format!("api-user:{id}")
}

#[tokio::test]
async fn each_host_reaches_its_own_route() {
    // `domain` applies to every route registered after it, so the catch-all
    // is registered first.
    let mut router = Router::new();
    router.get_action("/home", main_site);
    router.domain("api.example.com").get_action("/home", api);
    router
        .domain("{tenant}.example.com")
        .get_action("/home", tenant);
    let app = router.into_axum_router();

    assert_eq!(
        send(&app, "GET", "/home", Some("api.example.com")).await.1,
        "api"
    );
    assert_eq!(
        send(&app, "GET", "/home", Some("API.example.com:8443"))
            .await
            .1,
        "api"
    );
    assert_eq!(
        send(&app, "GET", "/home", Some("acme.example.com")).await.1,
        "tenant"
    );
    assert_eq!(
        send(&app, "GET", "/home", Some("example.com")).await.1,
        "main"
    );
    assert_eq!(send(&app, "GET", "/home", None).await.1, "main");
}

#[tokio::test]
async fn a_domain_route_without_a_catch_all_is_not_found_elsewhere() {
    let mut router = Router::new();
    router.domain("api.example.com").get_action("/home", api);
    let app = router.into_axum_router();

    assert_eq!(
        send(&app, "GET", "/home", Some("api.example.com")).await,
        (StatusCode::OK, "api".into())
    );
    assert_eq!(
        send(&app, "GET", "/home", Some("www.example.com")).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(&app, "GET", "/home", None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn path_parameters_and_other_methods_survive_host_dispatch() {
    let mut router = Router::new();
    router.post_action("/users/{id}", created);
    router
        .domain("api.example.com")
        .get_action("/users/{id}", api_user);
    let app = router.into_axum_router();

    let (status, body) = send(&app, "GET", "/users/7", Some("api.example.com")).await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "api-user:7"));
    let (status, body) = send(&app, "POST", "/users/7", Some("www.example.com")).await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "created"));
}

#[tokio::test]
async fn an_exact_host_outranks_a_wildcard_registered_first() {
    // Finding 1 regression: `{tenant}.example.com` is registered before
    // `admin.example.com`, so a plain first-match would hand the exact host to
    // the wildcard. Specificity ranking must send `admin.example.com` to the
    // exact route while other subdomains keep the wildcard.
    let mut router = Router::new();
    router
        .domain("{tenant}.example.com")
        .get_action("/home", tenant);
    router
        .domain("admin.example.com")
        .get_action("/home", admin);
    let app = router.into_axum_router();

    assert_eq!(
        send(&app, "GET", "/home", Some("admin.example.com")).await,
        (StatusCode::OK, "admin".into())
    );
    assert_eq!(
        send(&app, "GET", "/home", Some("acme.example.com")).await,
        (StatusCode::OK, "tenant".into())
    );
}

#[tokio::test]
async fn a_trailing_dot_fqdn_host_matches_the_plain_host() {
    // Finding 2: the FQDN root dot is not a label, so `api.example.com.` must
    // reach the `api.example.com` route instead of falling through.
    let mut router = Router::new();
    router.domain("api.example.com").get_action("/home", api);
    let app = router.into_axum_router();

    assert_eq!(
        send(&app, "GET", "/home", Some("api.example.com.")).await,
        (StatusCode::OK, "api".into())
    );
    assert_eq!(
        send(&app, "GET", "/home", Some("api.example.com.:8443")).await,
        (StatusCode::OK, "api".into())
    );
}

#[tokio::test]
async fn case_folded_domains_dedup_to_one_route() {
    // Finding 3: patterns differing only in case match the same host, so the
    // second registration must not create a competing route. The first
    // registration wins, and the host still reaches it.
    let mut router = Router::new();
    router.domain("API.example.com").get_action("/home", api);
    router
        .domain("api.example.com")
        .get_action("/home", main_site);
    let app = router.into_axum_router();
    assert_eq!(
        send(&app, "GET", "/home", Some("api.example.com")).await,
        (StatusCode::OK, "api".into())
    );
    assert_eq!(
        send(&app, "GET", "/home", Some("API.EXAMPLE.COM")).await,
        (StatusCode::OK, "api".into())
    );
}

#[tokio::test]
async fn malformed_and_non_ascii_hosts_do_not_panic_or_cross_match() {
    // Finding 5 negative: malformed Host values must not panic and must not be
    // served by another domain's route. Every case below falls through to the
    // catch-all (`main`) rather than `api` (exact) or `tenant` (wildcard).
    let mut router = Router::new();
    router.get_action("/home", main_site);
    router.domain("api.example.com").get_action("/home", api);
    router
        .domain("{tenant}.example.com")
        .get_action("/home", tenant);
    let app = router.into_axum_router();

    for host in [
        "",
        ".",
        "..",
        "a..b",
        ":8080",
        "[",
        "example.com",
        "api.example.com.evil",
        "api.example.com..",
    ] {
        let (status, body) = send(&app, "GET", "/home", Some(host)).await;
        assert_eq!(status, StatusCode::OK, "malformed host {host:?}");
        assert_eq!(
            body, "main",
            "malformed host {host:?} reached a wrong route"
        );
    }

    // Non-ASCII bytes are rejected by the HTTP header parser path, so the host
    // is absent and the request serves the catch-all — and it does not panic.
    let (status, body) = send_raw_host(&app, "/home", b"ex\xc3\xa4mple.com").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "main");

    // Duplicate `Host` headers do not panic; the first value wins, so the
    // request reaches the first host's route and never the second one's.
    let duplicate = Request::builder()
        .method("GET")
        .uri("/home")
        .header("host", "api.example.com")
        .header("host", "{tenant}.example.com")
        .body(Body::empty())
        .unwrap();
    let (status, body) = read(app.oneshot(duplicate).await.expect("router dispatch")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "api");
}
