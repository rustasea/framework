//! RustaSea router — expressive Axum-backed routing with groups and domains.
//!
//! Registration yields introspectable [`RouteEntry`] metadata plus, for routes
//! bound to a concrete action, an executable axum handler. `into_axum_router`
//! compiles the table into a dispatching axum router: bound actions run for
//! real, unbound routes fall back to the stub handler, and axum supplies
//! path/query/body extraction, 404 and 405 semantics.
//!
//! Beyond dispatch the crate provides named routes with reverse URL resolution
//! ([`Router::named`] / [`Router::url`] / [`NamedRoutes`]), redirect helpers
//! ([`Router::redirect`]), and build-time enforcement of per-route and
//! per-group middleware declared through [`Router::middleware`] and registered
//! with [`Router::register_middleware`].

mod authorize;
mod binding;
mod dispatch;
mod handler;
mod host;
mod metadata;
mod redirect;
mod resource;
mod route;
mod router;
#[cfg(test)]
mod tests;
mod url;

pub use authorize::{AuthorizeRegistry, AuthorizeResource};
pub use binding::{BindingRegistry, ModelBinder};
pub use handler::Handler;
pub use metadata::{MiddlewareApply, MiddlewareRegistry, RouteError};
pub use route::{AuthorizeSpec, ControllerRef, RouteEntry};
pub use router::Router;
pub use url::NamedRoutes;
