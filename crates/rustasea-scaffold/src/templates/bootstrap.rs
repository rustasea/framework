//! Application bootstrap templates - the kernel wiring under `bootstrap/`.
//!
//! Emits `bootstrap/mod.rs`, `bootstrap/app.rs`, `bootstrap/providers.rs`, and
//! `bootstrap/commands.rs`. The generated `commands.rs` owns boot-time
//! registration: it installs the framework's built-in command surface (and the
//! framework queue migrations behind it) and registers the application's own
//! migrations into the process-wide ORM migrator, so `cargo artisan migrate`
//! sees the real schema instead of reporting an empty registry.

use super::TemplateFile;

/// Bootstrap templates (shared by every variant).
pub fn entries() -> Vec<TemplateFile> {
    vec![
        ("bootstrap/mod.rs", BOOTSTRAP_MOD),
        ("bootstrap/app.rs", BOOTSTRAP_APP),
        ("bootstrap/providers.rs", BOOTSTRAP_PROVIDERS),
        ("bootstrap/commands.rs", BOOTSTRAP_COMMANDS),
    ]
}

const BOOTSTRAP_MOD: &str = r##"//! Application bootstrap - providers, commands, and kernel configuration.

pub mod app;
pub mod commands;
pub mod providers;
"##;

const BOOTSTRAP_APP: &str = r##"//! Application bootstrap - `Application::configure` for @@app_pascal@@.
//!
//! Registers the generated service providers, registers the console command
//! surface and the application migrations, and runs the register -> boot DAG
//! before the HTTP kernel starts serving.

use rustasea::foundation::BootError;
use rustasea::Application;

use crate::bootstrap::{commands, providers};

/// Build and boot the application container.
///
/// Returns [`BootError`] when the provider graph contains a cycle or an
/// unresolved dependency, so a misconfigured boot never starts the server.
/// Registering the command surface also registers the framework's queue
/// migrations and the application's own migrations, so `cargo artisan migrate`
/// runs the real schema in this binary.
pub fn configure() -> Result<Application, BootError> {
    let mut app = Application::configure(|_| {});
    for provider in providers::providers() {
        app.provider(provider);
    }
    commands::register_default();
    app.boot()?;
    Ok(app)
}
"##;

const BOOTSTRAP_PROVIDERS: &str = r##"//! Provider registry - service providers registered by the application.
//!
//! This registry is populated by the starter kit (previously empty) and is the
//! registration site for providers generated with `cargo rustasea make:provider`.

use rustasea::services::{CacheServiceProvider, DatabaseServiceProvider, QueueServiceProvider};
use rustasea::ServiceProvider;

use crate::app::providers::{AppServiceProvider, AuthServiceProvider};

/// Providers wired into the boot DAG, in registration order.
///
/// The framework providers bind the database connection resolver, the cache
/// manager, and the queue connections (see `rustasea::services`) ahead of the
/// application's own. Order is the tie-breaker for providers without
/// `dependencies()`; the foundation `Application::boot` topologically sorts
/// them regardless.
pub fn providers() -> Vec<Box<dyn ServiceProvider>> {
    vec![
        Box::new(DatabaseServiceProvider),
        Box::new(CacheServiceProvider),
        Box::new(QueueServiceProvider),
        Box::new(AppServiceProvider),
        Box::new(AuthServiceProvider),
    ]
}
"##;

const BOOTSTRAP_COMMANDS: &str = r##"//! CLI command registry - `cargo artisan` console commands.
//!
//! This registry gives the previously-empty command site a real home; commands
//! generated into `app/console/commands/*` are appended here. [`register_default`]
//! is the boot-time entry point: it installs the framework's built-in command
//! surface and registers the application migrations into the process-wide ORM
//! migrator so `cargo artisan migrate` never reports an empty registry.

use std::sync::OnceLock;

use rustasea::orm::register_migration;

use crate::database::migrations::{
    create_audit_log::CreateAuditLog,
    create_authentication_log::CreateAuthenticationLog,
    create_password_reset_tokens::CreatePasswordResetTokens,
    create_permission_role::CreatePermissionRole,
    create_permissions::CreatePermissions,
    create_role_user::CreateRoleUser,
    create_roles::CreateRoles,
    create_sessions::CreateSessions,
    create_users::CreateUsers,
};

/// Names of the console commands registered for the application.
pub fn commands() -> Vec<&'static str> {
    vec![]
}

/// Guards the application-migration registration so a second `configure()`
/// call (tests, embedded boots) never registers a migration twice.
static MIGRATIONS_REGISTERED: OnceLock<()> = OnceLock::new();

/// Register the framework command surface and the application migrations.
///
/// `rustasea::cli::load_default_commands()` installs the built-in commands and
/// the framework queue migrations (`jobs`, `failed_jobs`, `job_batches`); it is
/// idempotent. The application's own nine migrations are then registered into
/// the process-wide migrator in execution order (foreign keys depend on the
/// `users`/`roles`/`permissions` tables existing first), guarded so repeated
/// boots register them exactly once.
pub fn register_default() {
    rustasea::cli::load_default_commands();
    MIGRATIONS_REGISTERED.get_or_init(|| {
        register_migration(CreateUsers);
        register_migration(CreateSessions);
        register_migration(CreatePasswordResetTokens);
        register_migration(CreateRoles);
        register_migration(CreatePermissions);
        register_migration(CreateRoleUser);
        register_migration(CreatePermissionRole);
        register_migration(CreateAuditLog);
        register_migration(CreateAuthenticationLog);
    });
}
"##;
