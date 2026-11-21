//! Embedded starter-kit templates.
//!
//! Every generated file is a compile-time `&'static str` so scaffolding never
//! performs network or filesystem template lookups. [`render`] substitutes the
//! `@@…@@` placeholders derived from the requested application name; the
//! `@@` delimiters are chosen so generated askama `{{ }}` expressions survive
//! substitution untouched.

mod app_auth;
mod app_domain;
mod app_http;
mod blade;
mod bootstrap;
mod config;
mod core;
mod database;
mod docker;
mod entry;
mod inertia;
mod inertia_react;
mod inertia_svelte;
mod inertia_vue;
mod livewire;
mod manifest;
mod routes;
mod tests;
mod tooling;

use crate::name::AppName;
use crate::variant::StarterKitVariant;

/// A template entry: application-relative path plus raw template contents.
pub type TemplateFile = (&'static str, &'static str);

/// Placeholder values substituted into every template.
pub struct Placeholders<'a> {
    /// Kebab-case application name (`my-app`).
    pub app_name: &'a str,
    /// Snake-case application name (`my_app`).
    pub app_snake: &'a str,
    /// PascalCase application name (`MyApp`).
    pub app_pascal: &'a str,
    /// Lowercase variant token (`blade`, `react`, `vue`, `svelte`, `livewire`).
    pub variant: &'a str,
}

impl<'a> Placeholders<'a> {
    /// Derive the placeholder set from a parsed name and variant.
    pub fn new(name: &'a AppName, variant: StarterKitVariant) -> Self {
        Self {
            app_name: &name.kebab,
            app_snake: &name.snake,
            app_pascal: &name.pascal,
            variant: variant.as_str(),
        }
    }
}

/// Substitute every placeholder in `template`.
pub fn render(template: &str, vars: &Placeholders<'_>) -> String {
    template
        .replace("@@app_name@@", vars.app_name)
        .replace("@@app_snake@@", vars.app_snake)
        .replace("@@app_pascal@@", vars.app_pascal)
        .replace("@@variant@@", vars.variant)
}

/// Application-relative path of the generated package manifest.
pub const CARGO_MANIFEST_PATH: &str = "Cargo.toml";

/// Application-relative path of the modular-layout marker file.
pub const MODULES_MARKER_PATH: &str = "modules/.gitkeep";

/// Marker keeping an (initially empty) `modules/` directory in version control.
pub const MODULES_MARKER: &str =
    "# Module crates live under `modules/<name>/`; run\n# `cargo artisan make:module <Name>` to create one.\n";

/// Rustasea dependency prefix shared by every generated manifest.
const RUSTASEA_FEATURES_PREFIX: &str = r#"rustasea = { version = "0.1", features = ["#;

/// Workspace stanza appended by `cargo rustasea new --modular` (ADOPT-027).
///
/// The application becomes the workspace root and every `modules/*` crate is a
/// member, so `make:module` output compiles with the application.
const MODULAR_WORKSPACE: &str = r#"
# Modular application layout (ADOPT-027): module crates under `modules/*` are
# workspace members built together with the application.
[workspace]
members = ["modules/*"]
resolver = "2"
"#;

/// Rewrite a rendered `Cargo.toml` into the modular application layout.
///
/// Enables the umbrella `modules` feature and appends the workspace stanza that
/// admits `modules/*` crates as members. Non-modular generation never calls
/// this, so the default manifest is byte-identical to previous releases.
pub fn apply_modular_layout(cargo: &str) -> String {
    let replacement = format!("{RUSTASEA_FEATURES_PREFIX}\"modules\", ");
    let mut out = cargo.replacen(RUSTASEA_FEATURES_PREFIX, &replacement, 1);
    out.push_str(MODULAR_WORKSPACE);
    out
}

/// All template entries for `variant`, in deterministic write order.
///
/// Shared core templates come first, then the variant-specific `resources/`
/// layer. Duplicate paths are a programming error and are debug-asserted.
pub fn entries(variant: StarterKitVariant) -> Vec<TemplateFile> {
    let mut files: Vec<TemplateFile> = Vec::new();
    files.extend(manifest::entries(variant));
    files.extend(core::entries(variant));
    files.extend(bootstrap::entries());
    files.extend(tooling::entries());
    files.extend(app_domain::entries());
    files.extend(app_auth::entries());
    files.extend(app_http::entries(variant));
    files.extend(routes::entries());
    files.extend(config::entries(variant));
    files.extend(database::entries());
    files.extend(docker::entries());
    files.extend(tests::entries());
    match variant {
        StarterKitVariant::Blade => files.extend(blade::entries()),
        StarterKitVariant::Livewire => files.extend(livewire::entries()),
        StarterKitVariant::React | StarterKitVariant::Vue | StarterKitVariant::Svelte => {
            files.extend(inertia::entries(variant))
        }
    }
    debug_assert_unique(&files);
    files
}

/// Debug-only guard against two templates targeting the same path.
fn debug_assert_unique(files: &[TemplateFile]) {
    #[cfg(debug_assertions)]
    {
        let mut seen = std::collections::HashSet::new();
        for (path, _) in files {
            debug_assert!(seen.insert(*path), "duplicate template path: {path}");
        }
    }
}
