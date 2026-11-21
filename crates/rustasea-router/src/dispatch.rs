//! Axum compilation — resolve bound actions and emit a dispatchable router.
//!
//! [`Router::try_into_axum_router`] is the fallible build path: it resolves
//! each route's declared middleware through the [`MiddlewareRegistry`] and
//! fails with [`RouteError::UnknownMiddleware`] when one is missing.
//! [`Router::into_axum_router`] wraps it and fails closed — an unknown
//! middleware yields a `500` fallback router rather than a silently
//! under-protected route table.

use std::sync::Arc;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::{any_service, on_service, MethodFilter, MethodRouter};
use axum::Router as AxumRouter;

use crate::authorize::{AuthorizeRegistry, AuthorizeResource};
use crate::binding::{extract_param, selector_pairs, BindingRegistry, ModelBinder};
use crate::handler::{stub_handler, ActionFactory, BoundAction, Handler};
use crate::host::HostDispatch;
use crate::metadata::{MiddlewareApply, RouteError};
use crate::route::{AuthorizeSpec, RouteEntry};
use crate::router::Router;

impl Router {
    /// Convert into an Axum router, failing on unknown middleware.
    ///
    /// Iterates domain-first ordering via [`Router::get_routes`] and groups
    /// routes by method+path. A slot with domain-constrained routes is served
    /// by a [`HostDispatch`], which prefers an exact domain pattern over any
    /// placeholder pattern, then falls back to the route without a domain; two
    /// routes with the same method, path, and domain keep the first. Domain
    /// patterns are case-folded to ASCII lowercase so patterns differing only
    /// in case dedup to a single route. Each route resolves to an explicitly
    /// bound action, a controller-registry action, or the stub handler, then
    /// has its declared middleware applied — first-declared outermost.
    ///
    /// Slot bookkeeping stays on the Laravel-style [`RouteEntry::path`], while
    /// the string handed to `axum::Router::route` is translated by
    /// [`to_axum_path`] — Axum 0.7 (matchit 0.7) understands `:param`, not
    /// `{param}`.
    ///
    /// # Errors
    ///
    /// Returns [`RouteError::UnknownMiddleware`] when a route declares a
    /// middleware identifier that was never registered with
    /// [`Router::register_middleware`].
    pub fn try_into_axum_router(self) -> Result<AxumRouter, RouteError> {
        let entries = self.get_routes();
        let mut actions = self.actions;
        let controller_actions = self.controller_actions;
        let layers = self.layers;
        let registry = self.middleware_registry;
        let authorize_registry = self.authorize_registry;
        let binding_registry = self.binding_registry;

        let mut slots: Vec<Slot> = Vec::new();
        for entry in entries {
            // Domain patterns are matched case-insensitively, so fold them to
            // ASCII lowercase here: `API.example.com` and `api.example.com`
            // then dedup to one route instead of racing for the same host.
            // Only the pattern is folded — captured placeholder values and the
            // host seen by handlers are untouched.
            let domain = entry.domain.as_deref().map(str::to_ascii_lowercase);
            let index = match slots
                .iter()
                .position(|slot| slot.path == entry.path && slot.method == entry.method)
            {
                Some(index) => index,
                None => {
                    slots.push(Slot::new(&entry));
                    slots.len() - 1
                }
            };
            let slot = &mut slots[index];
            let taken = match &domain {
                Some(domain) => slot.domains.iter().any(|(d, _)| d == domain),
                None => slot.fallback.is_some(),
            };
            if taken {
                continue;
            }
            let mut method_router = resolve(&entry, &mut actions, &controller_actions);
            let axum_path = to_axum_path(&entry.path);
            // Authorization runs *innermost* — after the route's middleware
            // (which populates the principal and the resolved resource) and
            // before the handler, so a denial short-circuits the body.
            method_router = apply_authorize(method_router, &entry, &authorize_registry)?;
            // Model binding wraps authorization so the bound model is in the
            // request extensions before the authorizer reads it; it sits inside
            // the route middleware (which populates the principal). The selector
            // scan reads the Laravel-style `entry.path` (the axum path has
            // already dropped the `:field` selector), while the runtime value
            // extraction uses the axum `template`.
            method_router =
                apply_binding(method_router, &entry.path, &axum_path, &binding_registry);
            method_router = apply_middleware(method_router, &entry, &registry)?;
            match domain {
                Some(domain) => slots[index].domains.push((domain, method_router)),
                None => slots[index].fallback = Some(method_router),
            }
        }
        let mut router = AxumRouter::new();
        for slot in slots {
            let axum_path = to_axum_path(&slot.path);
            let method_router = match (slot.domains.is_empty(), slot.fallback) {
                (true, Some(fallback)) => fallback,
                (_, fallback) => host_service(&slot.method, slot.domains, fallback),
            };
            router = router.merge(AxumRouter::new().route(&axum_path, method_router));
        }
        for apply in layers {
            router = apply(router);
        }
        Ok(router)
    }

    /// Convert into an Axum router, failing closed on build errors.
    ///
    /// Delegates to [`Router::try_into_axum_router`]; when that fails (an
    /// unregistered middleware identifier), the returned router answers every
    /// request with `500 Internal Server Error` so a misconfigured table never
    /// silently serves routes without their declared middleware.
    pub fn into_axum_router(self) -> AxumRouter {
        match self.try_into_axum_router() {
            Ok(router) => router,
            Err(_) => AxumRouter::new().fallback(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
        }
    }
}

/// Routes that share one method+path, before they are compiled.
struct Slot {
    method: String,
    path: String,
    domains: Vec<(String, MethodRouter<()>)>,
    fallback: Option<MethodRouter<()>>,
}

impl Slot {
    fn new(entry: &RouteEntry) -> Self {
        Self {
            method: entry.method.clone(),
            path: entry.path.clone(),
            domains: Vec::new(),
            fallback: None,
        }
    }
}

/// Serve a method+path slot that has domain routes through [`HostDispatch`].
///
/// The dispatcher is registered for the slot's method only, so other methods
/// on the same path keep their own routes; an unknown verb answers any method,
/// mirroring [`Handler::into_method_router`].
fn host_service(
    method: &str,
    domains: Vec<(String, MethodRouter<()>)>,
    fallback: Option<MethodRouter<()>>,
) -> MethodRouter<()> {
    let dispatch = HostDispatch::new(domains, fallback);
    let filter = match method.to_ascii_uppercase().as_str() {
        "GET" => MethodFilter::GET,
        "POST" => MethodFilter::POST,
        "PUT" => MethodFilter::PUT,
        "DELETE" => MethodFilter::DELETE,
        "PATCH" => MethodFilter::PATCH,
        "OPTIONS" => MethodFilter::OPTIONS,
        _ => return any_service(dispatch),
    };
    on_service(filter, dispatch)
}

/// Apply a route's declared middleware, first-declared outermost.
///
/// Middleware is resolved through `registry`; a missing identifier — or a
/// parameterized spec whose factory rejects its parameters — is a build error.
/// Because `MethodRouter::layer` makes each successive layer the outermost one,
/// the identifiers are applied in reverse declaration order so the first
/// identifier declared ends up wrapping the route on the outside.
fn apply_middleware(
    method_router: MethodRouter<()>,
    entry: &RouteEntry,
    registry: &crate::metadata::MiddlewareRegistry,
) -> Result<MethodRouter<()>, RouteError> {
    let mut resolved: Vec<MiddlewareApply> = Vec::with_capacity(entry.middleware.len());
    for spec in &entry.middleware {
        resolved.push(registry.resolve(spec)?);
    }
    let mut router = method_router;
    for apply in resolved.into_iter().rev() {
        router = apply(router);
    }
    Ok(router)
}

/// Apply a route's declared `#[authorize]` checks as the innermost layer.
///
/// Each [`AuthorizeSpec`] resolves to an [`AuthorizeResource`] through
/// `registry`; an unregistered resource id (or a missing ability gate for the
/// ability-only form) is a fail-closed build error
/// ([`RouteError::UnknownAuthorization`]). The checks run inside a single
/// `from_fn` layer placed *after* the route's middleware (so the principal and
/// resolved resource are already in the request extensions) and *before* the
/// handler — the first denial returns its `403` response without executing the
/// body. A route with no `#[authorize]` declarations is returned untouched.
fn apply_authorize(
    method_router: MethodRouter<()>,
    entry: &RouteEntry,
    registry: &AuthorizeRegistry,
) -> Result<MethodRouter<()>, RouteError> {
    if entry.authorizations.is_empty() {
        return Ok(method_router);
    }
    let mut checks: Vec<(Arc<dyn AuthorizeResource>, String)> =
        Vec::with_capacity(entry.authorizations.len());
    for spec in &entry.authorizations {
        let (resource, ability) = match spec {
            AuthorizeSpec::Resource { ability, resource } => {
                // An omitted resource id resolves to the sole registered
                // authorizer; with zero or several registrations the
                // declaration is ambiguous and fails the build closed.
                let resolved = if resource.is_empty() {
                    registry
                        .sole()
                        .ok_or_else(|| RouteError::UnknownAuthorization {
                            name: resource.clone(),
                        })?
                } else {
                    registry.resolve(resource)?
                };
                (resolved, ability.clone())
            }
            AuthorizeSpec::Ability { name } => {
                // Ability-only checks resolve through the single ability gate.
                let gate = registry
                    .ability()
                    .ok_or_else(|| RouteError::UnknownAuthorization { name: name.clone() })?;
                (gate, name.clone())
            }
        };
        checks.push((resource, ability));
    }
    let checks = Arc::new(checks);
    Ok(method_router.layer(axum::middleware::from_fn(
        move |request: Request, next: Next| {
            let checks = Arc::clone(&checks);
            async move {
                for (resource, ability) in checks.iter() {
                    if let Err(response) = resource.authorize(&request, ability) {
                        return response;
                    }
                }
                next.run(request).await
            }
        },
    )))
}

/// Apply a route's `{param:field}` model bindings as an outer layer.
///
/// Binding is *opt-in*: only selectors whose field resolves to a registered
/// [`ModelBinder`] through `registry` are wrapped. A selector with no registered
/// binder is skipped entirely — it stays metadata-only (consumed by `route:list`
/// and the OpenAPI document) and the route keeps serving that segment as a plain
/// `:param` path parameter, preserving the pre-binding contract. The resolved
/// binders run inside a single `from_fn` layer placed *outside* the authorization
/// layer (so the bound model is in the request extensions before an authorizer
/// reads it) and *inside* the route middleware (so a preceding middleware may
/// have populated the principal). The path value is read by aligning the axum
/// `template` segments with the request URI segments; a missing value or a
/// binder miss returns its response without executing the handler. A route with
/// no resolvable selectors is returned untouched.
fn apply_binding(
    method_router: MethodRouter<()>,
    laravel_path: &str,
    axum_path: &str,
    registry: &BindingRegistry,
) -> MethodRouter<()> {
    let mut checks: Vec<(Arc<dyn ModelBinder>, String)> = Vec::new();
    for (field, param) in selector_pairs(laravel_path) {
        if let Some(binder) = registry.resolve(&field) {
            checks.push((binder, param));
        }
    }
    if checks.is_empty() {
        return method_router;
    }
    let template = axum_path.to_string();
    let checks = Arc::new(checks);
    method_router.layer(axum::middleware::from_fn(
        move |mut request: Request, next: Next| {
            let checks = Arc::clone(&checks);
            let template = template.clone();
            async move {
                let uri_path = request.uri().path().to_string();
                for (binder, param) in checks.iter() {
                    let Some(value) = extract_param(&uri_path, &template, param) else {
                        return StatusCode::NOT_FOUND.into_response();
                    };
                    if let Err(response) = binder.bind(&mut request, &value) {
                        return response;
                    }
                }
                next.run(request).await
            }
        },
    ))
}

/// Resolve the executable method router for a route entry.
///
/// Precedence: an explicit action bound to the same method+path+domain, then a
/// controller action registered for the entry's `{controller}@{action}` ref,
/// then the stub handler. Resolution never fails — an unbound route still
/// compiles so partial route tables remain servable.
///
/// A route only consumes a bound action when it is itself bound
/// (`entry.handler.is_some()`) and the action's domain matches the entry's
/// domain exactly. This prevents a domain-constrained entry, sorted ahead of
/// the catch-all it overlaps, from stealing the catch-all's action, and stops
/// an unbound route from consuming an action bound to another route.
fn resolve(
    entry: &RouteEntry,
    actions: &mut Vec<BoundAction>,
    controller_actions: &[(String, String, ActionFactory)],
) -> MethodRouter<()> {
    if entry.handler.is_some() {
        let bound = actions.iter().position(|a| {
            a.method == entry.method && a.path == entry.path && a.domain == entry.domain
        });
        if let Some(index) = bound {
            return actions.swap_remove(index).router;
        }
    }
    if let Some(controller) = &entry.controller {
        let found = controller_actions
            .iter()
            .find(|(name, action, _)| name == &controller.name && action == &controller.action);
        if let Some((_, _, factory)) = found {
            return factory(&entry.method);
        }
    }
    stub_handler.into_method_router(&entry.method)
}

/// Translate a Laravel-style route path into Axum (matchit) syntax.
///
/// Laravel placeholders become Axum parameters: `{id}` → `:id` and the
/// route-model-bound `{user:slug}` → `:user` (the `:slug` selector is a
/// Laravel-only model-binding hint and is dropped from the match pattern).
/// Text outside placeholders — including paths already written in `:param`
/// form — passes through verbatim, so the translation is idempotent and the
/// introspectable [`RouteEntry::path`] stays the single source of truth. An
/// unterminated `{` is emitted unchanged rather than silently swallowed.
fn to_axum_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut rest = path;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let tail = &rest[open + 1..];
        let Some(close) = tail.find('}') else {
            // Unterminated placeholder — emit the remainder verbatim.
            out.push_str(&rest[open..]);
            return out;
        };
        let inner = &tail[..close];
        let param = inner.split(':').next().unwrap_or(inner);
        if !param.is_empty() {
            out.push(':');
            out.push_str(param);
        }
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::to_axum_path;

    /// Simple, typed, and multi-segment placeholders translate to `:param`.
    #[test]
    fn to_axum_path_translates_laravel_placeholders() {
        assert_eq!(to_axum_path("/users/{id}"), "/users/:id");
        assert_eq!(to_axum_path("/users/{user:slug}"), "/users/:user");
        assert_eq!(
            to_axum_path("/teams/{team}/members/{member:slug}"),
            "/teams/:team/members/:member"
        );
    }

    /// Paths without placeholders — and already-normalized `:param` paths —
    /// pass through unchanged; an unterminated brace is not swallowed.
    #[test]
    fn to_axum_path_leaves_plain_and_normalized_paths_untouched() {
        assert_eq!(to_axum_path("/"), "/");
        assert_eq!(to_axum_path("/users/create"), "/users/create");
        assert_eq!(to_axum_path("/users/:id/edit"), "/users/:id/edit");
        assert_eq!(to_axum_path("/users/{id"), "/users/{id");
    }
}
