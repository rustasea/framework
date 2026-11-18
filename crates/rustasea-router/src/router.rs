//! Expressive router builder — Laravel-inspired registration DSL.
//!
//! The builder accumulates introspectable [`RouteEntry`] metadata and, for
//! routes registered with a concrete action, the executable handler factory.
//! Compilation into an axum router lives in [`crate::dispatch`]; reverse URL
//! resolution in [`crate::url`]; redirect helpers in [`crate::redirect`]; and
//! the named-middleware registry in [`crate::metadata`].

use axum::Router as AxumRouter;

use crate::authorize::AuthorizeRegistry;
use crate::binding::BindingRegistry;
use crate::handler::{action_factory, ActionFactory, BoundAction, Handler};
use crate::metadata::MiddlewareRegistry;
use crate::route::{
    join_prefix, normalize_prefix, parse_binding_fields, prefixed_name, AuthorizeSpec,
    ControllerRef, RouteEntry,
};
/// Expressive router builder with Laravel-inspired API.
pub struct Router {
    pub(crate) routes: Vec<RouteEntry>,
    pub(crate) prefix: String,
    pub(crate) name_prefix: String,
    pub(crate) pending_middleware: Vec<String>,
    pub(crate) pending_authorizations: Vec<AuthorizeSpec>,
    pub(crate) pending_domain: Option<String>,
    pub(crate) pending_controller: Option<String>,
    pub(crate) layers: Vec<Box<dyn FnOnce(AxumRouter) -> AxumRouter + Send>>,
    /// Routes explicitly bound to a concrete controller action.
    pub(crate) actions: Vec<BoundAction>,
    /// Controller actions keyed by `(controller, action)` for resource dispatch.
    pub(crate) controller_actions: Vec<(String, String, ActionFactory)>,
    /// Named-middleware registry consulted at build time.
    pub(crate) middleware_registry: MiddlewareRegistry,
    /// `#[authorize]` resource registry consulted at build time.
    pub(crate) authorize_registry: AuthorizeRegistry,
    /// `{param:field}` model-binding registry consulted at build time.
    pub(crate) binding_registry: BindingRegistry,
    /// Index into [`Router::routes`] where the most recent registration batch
    /// started, so [`Router::named`] can name every route the last helper
    /// produced (e.g. all six methods of an `any` route).
    batch_start: usize,
}

impl Router {
    /// Create a new empty router.
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            prefix: String::new(),
            name_prefix: String::new(),
            pending_middleware: Vec::new(),
            pending_authorizations: Vec::new(),
            pending_domain: None,
            pending_controller: None,
            layers: Vec::new(),
            actions: Vec::new(),
            controller_actions: Vec::new(),
            middleware_registry: MiddlewareRegistry::new(),
            authorize_registry: AuthorizeRegistry::new(),
            binding_registry: BindingRegistry::new(),
            batch_start: 0,
        }
    }

    /// Append a URI prefix for subsequent routes and groups (concatenates with parent).
    pub fn prefix(&mut self, prefix: &str) -> &mut Self {
        let child = normalize_prefix(prefix);
        if !child.is_empty() {
            self.prefix = join_prefix(&self.prefix, &child);
        }
        self
    }

    /// Append a name prefix for subsequent routes (concatenates with parent).
    ///
    /// The prefix composes with per-route names: a group named `settings.`
    /// around `get("/profile").named("profile.edit")` yields the full name
    /// `settings.profile.edit`.
    pub fn name(&mut self, name: &str) -> &mut Self {
        self.name_prefix = format!("{}{}", self.name_prefix, name);
        self
    }

    /// Name every route produced by the most recent registration helper.
    ///
    /// Chains directly off any method helper because those return `&mut Self`:
    /// `router.get("/home").named("home")`. The accumulated group name prefix
    /// is applied, so a route named inside a `name("settings.")` group becomes
    /// `settings.<name>`. A multi-method helper (`any`, `redirect`) names every
    /// route in the batch. A no-op when no route has been registered since the
    /// last helper call.
    pub fn named(&mut self, name: impl Into<String>) -> &mut Self {
        let name = name.into();
        let prefix = self.name_prefix.clone();
        let start = self.batch_start.min(self.routes.len());
        for entry in &mut self.routes[start..] {
            entry.name = prefixed_name(&prefix, Some(&name));
        }
        self
    }

    /// Set domain constraint for subsequent routes.
    pub fn domain(&mut self, domain: &str) -> &mut Self {
        self.pending_domain = Some(domain.to_string());
        self
    }

    /// Add middleware identifier for subsequent routes.
    pub fn middleware(&mut self, middleware: &str) -> &mut Self {
        self.pending_middleware.push(middleware.to_string());
        self
    }

    /// Mark the start of a registration batch for [`Router::named`].
    pub(crate) fn begin_batch(&mut self) {
        self.batch_start = self.routes.len();
    }

    /// Register a GET route.
    pub fn get(&mut self, path: &str) -> &mut Self {
        self.begin_batch();
        self.push("GET", path);
        self
    }

    /// Register a POST route.
    pub fn post(&mut self, path: &str) -> &mut Self {
        self.begin_batch();
        self.push("POST", path);
        self
    }

    /// Register a PUT route.
    pub fn put(&mut self, path: &str) -> &mut Self {
        self.begin_batch();
        self.push("PUT", path);
        self
    }

    /// Register a DELETE route.
    pub fn delete(&mut self, path: &str) -> &mut Self {
        self.begin_batch();
        self.push("DELETE", path);
        self
    }

    /// Register a PATCH route.
    pub fn patch(&mut self, path: &str) -> &mut Self {
        self.begin_batch();
        self.push("PATCH", path);
        self
    }

    /// Register an OPTIONS route.
    pub fn options(&mut self, path: &str) -> &mut Self {
        self.begin_batch();
        self.push("OPTIONS", path);
        self
    }

    /// Register a route matching any HTTP method.
    pub fn any(&mut self, path: &str) -> &mut Self {
        // Expand to the six explicit methods — each entry stays introspectable
        // and binding-aware; a router that is never compiled to Axum still sees
        // concrete methods instead of a synthetic ANY catch-all.
        self.begin_batch();
        for method in ["GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS"] {
            self.push(method, path);
        }
        self
    }

    /// Set the default controller for subsequent routes.
    ///
    /// Routes registered without an explicit action (plain method helpers)
    /// bind to `{controller}@handle`; resource routes bind per-action.
    pub fn controller(&mut self, controller: &str) -> &mut Self {
        self.pending_controller = Some(controller.to_string());
        self
    }

    /// Register a GET route bound to a real controller action.
    ///
    /// The equivalent of Laravel's `Route::get("/users", [UserController,
    /// "index"])`: `UserController::index` is dispatched for `GET /users`
    /// with axum performing path/query/body extraction.
    pub fn get_action<H, T>(&mut self, path: &str, handler: H) -> &mut Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.begin_batch();
        self.push_action("GET", path, handler)
    }

    /// Register a POST route bound to a real controller action.
    pub fn post_action<H, T>(&mut self, path: &str, handler: H) -> &mut Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.begin_batch();
        self.push_action("POST", path, handler)
    }

    /// Register a PUT route bound to a real controller action.
    pub fn put_action<H, T>(&mut self, path: &str, handler: H) -> &mut Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.begin_batch();
        self.push_action("PUT", path, handler)
    }

    /// Register a DELETE route bound to a real controller action.
    pub fn delete_action<H, T>(&mut self, path: &str, handler: H) -> &mut Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.begin_batch();
        self.push_action("DELETE", path, handler)
    }

    /// Register a PATCH route bound to a real controller action.
    pub fn patch_action<H, T>(&mut self, path: &str, handler: H) -> &mut Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.begin_batch();
        self.push_action("PATCH", path, handler)
    }

    /// Register an OPTIONS route bound to a real controller action.
    pub fn options_action<H, T>(&mut self, path: &str, handler: H) -> &mut Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.begin_batch();
        self.push_action("OPTIONS", path, handler)
    }

    /// Register a route bound to a real controller action for any method.
    ///
    /// Mirrors [`Router::any`] — the action is cloned across the six concrete
    /// methods so every entry stays introspectable.
    pub fn any_action<H, T>(&mut self, path: &str, handler: H) -> &mut Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.begin_batch();
        for method in ["GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS"] {
            self.push_action(method, path, handler.clone());
        }
        self
    }

    /// Register a route bound to a real action for an explicit HTTP method.
    pub fn action<H, T>(&mut self, method: &str, path: &str, handler: H) -> &mut Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.begin_batch();
        self.push_action(method, path, handler)
    }

    /// Register a route from `#[route]` metadata plus a real controller action.
    ///
    /// `meta` is the doc-hidden `__RUSTASEA_ROUTE_<Fn>` tuple emitted by the
    /// `#[route]` attribute, so the macro's method + path are consumed at
    /// registration instead of being read only by tests.
    pub fn route_meta<H, T>(&mut self, meta: (&str, &str), handler: H) -> &mut Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.begin_batch();
        self.push_action(meta.0, meta.1, handler)
    }

    /// Register a controller action resolvable by `{controller}@{action}`.
    ///
    /// The controller name comes from the preceding [`Router::controller`]
    /// call, so `resource("users", "UserController")` dispatches to the action
    /// registered here for `(UserController, index)`, `(UserController, show)`,
    /// etc.
    pub fn controller_action<H, T>(&mut self, action: &str, handler: H) -> &mut Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        let controller = self.pending_controller.clone().unwrap_or_default();
        self.controller_action_for(&controller, action, handler)
    }

    /// Register a controller action under an explicit controller name.
    pub fn controller_action_for<H, T>(
        &mut self,
        controller: &str,
        action: &str,
        handler: H,
    ) -> &mut Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.controller_actions.push((
            controller.to_string(),
            action.to_string(),
            action_factory(handler),
        ));
        self
    }

    /// Apply a tower layer to the compiled router.
    ///
    /// Bounds mirror `axum::Router::layer` — any tower layer whose service
    /// wraps axum's `Route` — so `CorsConfig::default().layer()` and
    /// `ThrottleLayer` compose directly:
    ///
    /// ```rust,ignore
    /// use rustasea_http::CorsConfig;
    /// route_table.layer(CorsConfig::default().layer());
    /// ```
    ///
    /// Layers apply in registration order after all routes are merged.
    pub fn layer<L>(&mut self, layer: L)
    where
        L: tower::Layer<axum::routing::Route> + Clone + Send + 'static,
        L::Service: tower::Service<axum::http::Request<axum::body::Body>> + Clone + Send + 'static,
        <L::Service as tower::Service<axum::http::Request<axum::body::Body>>>::Response:
            axum::response::IntoResponse + 'static,
        <L::Service as tower::Service<axum::http::Request<axum::body::Body>>>::Error:
            Into<std::convert::Infallible> + 'static,
        <L::Service as tower::Service<axum::http::Request<axum::body::Body>>>::Future:
            Send + 'static,
    {
        self.layers
            .push(Box::new(move |router: AxumRouter| router.layer(layer)));
    }

    /// Create a grouped sub-router sharing prefix, name, middleware, and domain.
    pub fn group<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut Router),
    {
        let mut sub = Router {
            routes: Vec::new(),
            prefix: self.prefix.clone(),
            name_prefix: self.name_prefix.clone(),
            pending_middleware: self.pending_middleware.clone(),
            pending_authorizations: self.pending_authorizations.clone(),
            pending_domain: self.pending_domain.clone(),
            pending_controller: self.pending_controller.clone(),
            layers: Vec::new(),
            actions: Vec::new(),
            controller_actions: Vec::new(),
            middleware_registry: MiddlewareRegistry::new(),
            authorize_registry: AuthorizeRegistry::new(),
            binding_registry: BindingRegistry::new(),
            batch_start: 0,
        };
        f(&mut sub);
        self.routes.extend(sub.routes);
        self.actions.extend(sub.actions);
        self.controller_actions.extend(sub.controller_actions);
        self.middleware_registry.merge(sub.middleware_registry);
        self.authorize_registry.merge(sub.authorize_registry);
        self.binding_registry.merge(sub.binding_registry);
        // Routes added by the group are not part of any open batch, so a
        // trailing `named` call cannot accidentally rename them.
        self.batch_start = self.routes.len();
        self
    }

    /// Register a RESTful resource (7 routes) for a given name.
    ///
    /// `controller` is the class name resolved at bind time — plain
    /// `resource("photos")` defers to the route-group controller (see
    /// [`Router::controller`]). The `update` and `destroy` actions accept
    /// both `PUT` and `PATCH` (Laravel-style), so introspection yields the
    /// full request-method surface. Resource names compose with the group
    /// name prefix like any other route.
    ///
    /// The implementation lives in [`crate::resource`].
    pub fn resource(&mut self, name: &str, controller: &str) -> &mut Self {
        self.register_resource(name, controller)
    }

    /// Return all registered routes, domain-constrained first.
    pub fn get_routes(&self) -> Vec<RouteEntry> {
        let mut routes = self.routes.clone();
        routes.sort_by_key(|r| r.domain.is_none());
        routes
    }

    /// Alias for get_routes for Laravel naming parity.
    ///
    /// Non-snake-case by design: mirrors the Laravel `getRoutes` collector name
    /// so framework docs map 1:1 onto the Rust surface.
    #[allow(non_snake_case)]
    pub fn getRoutes(&self) -> Vec<RouteEntry> {
        self.get_routes()
    }

    /// Convert into Axum router (snake_case alias).
    pub fn into_axum_router_owned(self) -> AxumRouter {
        self.into_axum_router()
    }

    /// Push an action-bound route and record its executable method router.
    fn push_action<H, T>(&mut self, method: &str, path: &str, handler: H) -> &mut Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.push(method, path);
        let label = std::any::type_name::<H>().to_string();
        if let Some(entry) = self.routes.last_mut() {
            // An explicit action is authoritative — drop the pending controller
            // ref so resolution does not fall back to `{controller}@handle`.
            entry.controller = None;
            entry.handler = Some(label);
        }
        let full = self
            .routes
            .last()
            .expect("push always appends a route entry")
            .path
            .clone();
        let router = handler.into_method_router(method);
        self.actions.push(BoundAction {
            method: method.to_ascii_uppercase(),
            path: full,
            domain: self.pending_domain.clone(),
            router,
        });
        self
    }

    /// Push a plain (unnamed) route entry carrying the pending metadata.
    pub(crate) fn push(&mut self, method: &str, path: &str) {
        self.push_route(method, path, None);
    }

    /// Push a named route entry, applying the accumulated name prefix.
    pub(crate) fn push_with_name(&mut self, method: &str, path: &str, name: &str) {
        self.push_route(method, path, Some(name));
    }

    /// Push a route entry with an optional per-route name.
    ///
    /// This is the single place route names are composed with the group name
    /// prefix; `name` is `None` for plain routes and `Some` for resource and
    /// explicitly-named routes.
    fn push_route(&mut self, method: &str, path: &str, name: Option<&str>) {
        let full = join_prefix(&self.prefix, path);
        self.routes.push(RouteEntry {
            method: method.to_ascii_uppercase(),
            path: full.clone(),
            name: prefixed_name(&self.name_prefix, name),
            middleware: self.pending_middleware.clone(),
            authorizations: self.pending_authorizations.clone(),
            domain: self.pending_domain.clone(),
            binding_fields: parse_binding_fields(&full),
            controller: self.pending_controller.as_ref().map(|c| ControllerRef {
                name: c.clone(),
                action: "handle".to_string(),
            }),
            handler: None,
        });
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}
