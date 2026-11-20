//! Host-aware dispatch for routes that share a method+path across domains.
//!
//! A route declared under [`crate::Router::domain`] only answers requests whose
//! host matches its domain pattern. When several routes claim the same
//! method+path, [`HostDispatch`] picks the first domain route whose pattern
//! matches the request host and otherwise falls back to the route without a
//! domain (the catch-all). With no match and no catch-all the request gets
//! `404 Not Found`, the same answer as an unrouted path.

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::MethodRouter;
use tower::{Service, ServiceExt};

/// Whether `host` matches the domain `pattern`.
///
/// Labels are compared case-insensitively. A `{name}` label in the pattern
/// matches any single non-empty label (`{tenant}.example.com` matches
/// `acme.example.com` but not `example.com` or `a.b.example.com`). Captured
/// labels are not exposed as route parameters.
pub(crate) fn host_matches(pattern: &str, host: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('.').collect();
    let host: Vec<&str> = host.split('.').collect();
    pattern.len() == host.len()
        && pattern.iter().zip(&host).all(|(expected, actual)| {
            let placeholder = expected.starts_with('{') && expected.ends_with('}');
            if placeholder {
                !actual.is_empty()
            } else {
                expected.eq_ignore_ascii_case(actual)
            }
        })
}

/// The request host without its port, from `Host` or the URI authority.
fn request_host(request: &Request) -> Option<String> {
    let raw = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .or_else(|| request.uri().host().map(str::to_string))?;
    // `[::1]:8080` keeps its brackets; `example.com:8080` drops the port.
    let host = match raw.rfind(':') {
        Some(colon) if !raw[colon..].contains(']') => &raw[..colon],
        _ => raw.as_str(),
    };
    Some(host.to_ascii_lowercase())
}

/// Routes sharing one method+path, chosen per request by host.
#[derive(Clone)]
pub(crate) struct HostDispatch {
    /// Domain-constrained routes in registration (precedence) order.
    domains: Arc<Vec<(String, MethodRouter<()>)>>,
    /// The route without a domain, if one was registered.
    fallback: Option<MethodRouter<()>>,
}

impl HostDispatch {
    /// Build a dispatcher from domain routes and an optional catch-all.
    pub(crate) fn new(
        domains: Vec<(String, MethodRouter<()>)>,
        fallback: Option<MethodRouter<()>>,
    ) -> Self {
        Self {
            domains: Arc::new(domains),
            fallback,
        }
    }

    /// The route that should answer a request for `host`.
    fn select(&self, host: Option<&str>) -> Option<MethodRouter<()>> {
        host.and_then(|host| {
            self.domains
                .iter()
                .find(|(pattern, _)| host_matches(pattern, host))
                .map(|(_, route)| route.clone())
        })
        .or_else(|| self.fallback.clone())
    }
}

impl Service<Request> for HostDispatch {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let route = self.select(request_host(&request).as_deref());
        Box::pin(async move {
            match route {
                Some(route) => route.oneshot(request).await,
                None => Ok(StatusCode::NOT_FOUND.into_response()),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_domains_match_case_insensitively() {
        assert!(host_matches("api.example.com", "API.Example.com"));
        assert!(!host_matches("api.example.com", "www.example.com"));
        assert!(!host_matches("api.example.com", "example.com"));
    }

    #[test]
    fn a_placeholder_matches_exactly_one_label() {
        assert!(host_matches("{tenant}.example.com", "acme.example.com"));
        assert!(!host_matches("{tenant}.example.com", "example.com"));
        assert!(!host_matches("{tenant}.example.com", "a.b.example.com"));
        assert!(!host_matches("{tenant}.example.com", ".example.com"));
    }

    #[test]
    fn the_port_is_ignored() {
        let request = Request::builder()
            .header(header::HOST, "Api.Example.com:8080")
            .body(axum::body::Body::empty())
            .expect("request");
        assert_eq!(request_host(&request).as_deref(), Some("api.example.com"));

        let ipv6 = Request::builder()
            .header(header::HOST, "[::1]")
            .body(axum::body::Body::empty())
            .expect("request");
        assert_eq!(request_host(&ipv6).as_deref(), Some("[::1]"));
    }
}
