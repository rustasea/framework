//! Host-aware dispatch for domain-constrained routes.
//!
//! Positive: a domain route answers only its host (exact or `{placeholder}`),
//! the catch-all answers every other host, path parameters survive the host
//! dispatch, and other methods on the same path keep their own routes.
//! Negative: a domain route without a catch-all is `404` for any other host.

use axum::body::Body;
use axum::extract::Path;
use axum::http::{Request, StatusCode};
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
