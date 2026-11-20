//! RustaSea umbrella crate — re-exports foundation, config, and M0–M4 crates.

pub use rustasea_activitylog as activitylog;
pub use rustasea_authlog as authlog;
pub use rustasea_cache as cache;
pub use rustasea_config as config;
pub use rustasea_events as events;
pub use rustasea_foundation as foundation;
pub use rustasea_i18n as i18n;
pub use rustasea_queue as queue;
pub use rustasea_schedule as schedule;
pub use rustasea_timezone as timezone;

pub mod services;

pub use config::ConfigLoader;
pub use foundation::{Application, Container, ServiceProvider};
pub use rustasea_auth as auth;
pub use rustasea_cli as cli;
pub use rustasea_http as http;
pub use rustasea_macros as macros;
pub use rustasea_openapi as openapi;
pub use rustasea_orm as orm;
pub use rustasea_router as router;
pub use rustasea_testing as testing;
pub use rustasea_validation as validation;

/// Queue re-exports for typed job dispatch ergonomics (M4).
pub use queue::{
    async_trait as queue_async_trait, default_resolver, register_job, register_job_handler,
    register_job_with_policy, run_worker, run_worker_with, ConcreteJob, DatabaseDriver,
    DispatchHandle, ErasedJob, FailedJob, Job, JobError, JobId, JobOutcome, JobPayload, JobPolicy,
    Queue, QueueDriver, QueueError, QueueRegistry, ShouldRetry, ShouldRetryUntil,
};
pub use queue::{queue_migrator, register_queue_migrations};

/// Cache re-exports for store/repository ergonomics (M4).
pub use cache::{CacheError, CacheManager, Lock, LockError, LockGuard, Store};

/// Events re-exports for dispatch ergonomics (M4).
pub use events::{
    async_trait as events_async_trait, Dispatcher, Event, EventError, JobAttempted, Listener,
    QueueBusy,
};

/// Schedule re-exports for scheduler ergonomics (M4).
pub use schedule::{
    Schedule, ScheduleBuilder, ScheduleCommand, ScheduleError, SchedulePaused, ScheduleResumed,
    ScheduleState, Scheduler, SchedulerStatus,
};

pub use i18n::__ as i18n_trans;
/// i18n re-exports for translation ergonomics (M3). The free `__` /
/// `trans_choice` helpers are aliased to keep the umbrella's root namespace
/// explicit.
pub use i18n::{
    choose_form as i18n_choose_form, clear_translator as i18n_clear_translator,
    interpolate as i18n_interpolate, set_translator as i18n_set_translator,
    trans_choice as i18n_trans_choice, translator as i18n_translator, I18nError,
    Param as I18nParam, TranslationLoader, Translations, Translator, DEFAULT_LANG_DIR,
};

/// Timezone re-exports for wall-clock scheduling / display ergonomics
/// (ADOPT-006). The resolution chain is user → session → header → app default.
pub use timezone::{
    clear_default_timezone, default_timezone, format_local as format_local_time, now_local,
    resolve as resolve_timezone, resolve_tz as resolve_timezone_tz, set_default_timezone,
    to_local as to_local_time, validate as validate_timezone, TimezoneError, TimezoneMapper, Tz,
    DEFAULT_TIMEZONE,
};

pub use orm::{
    register_migration, register_seeder, registered_migrator, slugify, Migration, MigrationError,
    MigrationRecord, Migrator, Model, OrmError, Paginator, QueryBuilder, Relation,
    Result as OrmResult, ScopeRegistry, Seeder, SlugOptions, SluggableFind, SoftDeletes,
    Timestamps, UpsertError,
};

/// ORM query-cache re-exports (ADOPT-019).
///
/// Install a [`QueryCacheStore`](orm::QueryCacheStore) at boot with
/// [`register_query_cache_store`](orm::register_cache_store); builders opt in
/// with `.cache(ttl)` / `.cache_forever()`, and table writes invalidate cached
/// queries automatically.
pub use orm::{
    bump_table_generation as bump_query_cache_generation, cache_store as query_cache_store,
    clear_cache_store as clear_query_cache_store, flush_model_cache, query_cache_key,
    register_cache_store as register_query_cache_store, table_generation, QueryCacheStore,
};

/// Activity-log re-exports for audit-trail ergonomics (M2).
pub use activitylog::{
    register as register_activity_log_migration, Activity, ActivityColumnMode, ActivityColumns,
    ActivityError, ActivityEvent, ActivityLogger, ActivityOperation, ActivityQuery,
    ActivityRecorder, CreateAuditLogTable,
};

/// Authentication-log re-exports for sign-in-history ergonomics (ADOPT-003).
///
/// The recorder is installed process-wide with [`install`](authlog::install) and
/// the HTTP layer records through the best-effort [`record_event`](authlog::record_event)
/// helper; [`logger`](authlog::logger) resolves the installed instance.
pub use authlog::{
    clear as clear_authentication_log, install as install_authentication_log,
    logger as authentication_logger, record_event as record_auth_event,
    register as register_authentication_log_migration, AuthLogError, AuthLogEvent,
    AuthLogEventKind, AuthenticationLog, AuthenticationLogLogger, CreateAuthenticationLogTable,
    NewDeviceNotification, NewDeviceNotifier, QueuedMailNotifier,
};

/// Auth re-exports for handler ergonomics (`Auth::guard`, guards, CSRF).
pub use auth::{
    AuthError, AuthManager, AuthUser, Credentials, CsrfError, CsrfLayer, DeserializationAllowList,
    Guard, HasRoles, JwtClaims, JwtConfig, JwtGuard, KeyBy, Limit, MemoryRateLimiter,
    PermissionResolver, PreventRequestForgery, RateLimiter, RbacError, RbacRegistry, Role,
    SecFetchSite, SessionGuard, SessionPolicy, ThrottleConfig, ThrottleLayer, ThrottleService,
    Token,
};

/// Validation re-exports (`#[validate]` wiring surface).
pub use validation::{ErrorBag, FormRequest, Rules, Validatable, ValidationError};

/// OpenAPI re-exports for spec-generation ergonomics (ADOPT-011).
pub use openapi::{generate, generate_with_info, OpenApiError, SpecInfo};

/// CLI re-exports (`Artisan::call`, `Command` trait, generators).
pub use cli::{
    Artisan, Command, CommandMeta, CommandOutput, CommandRegistry, Generator, GeneratorError, Io,
    Registered, Shutdownable,
};

/// Testing re-exports (`TestCase`, factories, paginator views).
pub use testing::{
    bootstrap_3, factory_registry, paginator_view, reset_factory_sequences, str_factory, TestCase,
    TestConfig, TestError,
};

/// Locale-aware fake-data generation (ADOPT-012) — only with the `faker`
/// feature.
///
/// Re-exports the [`Faker`](rustasea_testing::faker::Faker) facade, its
/// [`FakerError`](rustasea_testing::faker::FakerError), the
/// [`Locale`](rustasea_testing::faker::Locale) enum, and the fail-open
/// [`resolve_faker_locale`](rustasea_testing::faker::resolve_faker_locale)
/// resolver. Opt-in so the core build never links the `fake` tree.
#[cfg(feature = "faker")]
pub use rustasea_testing::faker::{resolve_faker_locale, Faker, FakerError, Locale};

/// M6 broadcast/storage/search/jsonapi re-exports (Sprint 07).
pub use rustasea_broadcast as broadcast;
pub use rustasea_jsonapi as jsonapi;
pub use rustasea_search as search;
pub use rustasea_storage as storage;

/// M6 mail re-export: `Mailable`/`Mailer` contracts, the `Mail` facade, and
/// queued notifications. The SMTP transport and the template hook need the
/// `mail-smtp` / `mail-templates` features.
pub use rustasea_mail as mail;

/// M6 AI SDK re-export — only with the `ai` feature (NFR-Sca-02).
#[cfg(feature = "ai")]
pub use rustasea_ai as ai;

/// Broadcast re-exports for channel/SSE ergonomics (M6).
pub use broadcast::{Authorize, BroadcastError, Channel, ShouldBroadcast};

/// Broadcast driver re-exports (ADOPT-022).
///
/// The [`BroadcastManager`](broadcast::BroadcastManager) owns the named
/// connections (the in-process hub plus the optional Pusher/Redis drivers) and
/// is the façade for publishing events and authorizing channel subscriptions;
/// [`BroadcastingConfig`](broadcast::BroadcastingConfig) resolves the
/// `[broadcasting]` config table. The `broadcast-pusher` / `broadcast-redis`
/// features expose the matching drivers.
pub use broadcast::{
    broadcast_config, broadcast_gate, broadcast_manager, clear_broadcast_config,
    set_broadcast_config, set_broadcast_gate, set_broadcast_manager, BroadcastManager,
    BroadcastPayload, Broadcaster, BroadcastingConfig,
};
#[cfg(feature = "broadcast-pusher")]
pub use broadcast::{ChannelAuth, PusherBroadcaster, PusherConfig, PusherTransport};
#[cfg(feature = "broadcast-redis")]
pub use broadcast::{RedisBroadcaster, RedisConfig, RedisSubscriber};

/// Storage re-exports for read-through disk ergonomics (M6).
pub use storage::{
    LocalDisk, ObjectDisk, ReadThrough, Storage, StorageConfig, StorageError, StorageManager,
};

/// Live SFTP disk (ADOPT-025); requires the `storage-sftp` feature.
#[cfg(feature = "storage-sftp")]
pub use storage::SftpDisk;

/// SFTP disk configuration (ADOPT-025); always available so the facade can parse
/// an `sftp` disk even when the driver is off.
pub use storage::SftpDiskConfig;

/// Search re-exports for vector ergonomics (M6).
pub use search::{
    MemoryVectorStore, Similarity as VectorSimilarity, Str, VectorDocument, VectorIndex,
    VectorIndexOps, VectorSearch, VectorSearchError,
};

/// JSON:API re-exports for resource ergonomics (M6).
pub use jsonapi::{
    content_type as jsonapi_content_type, Document as JsonApiDocument, JsonApiError,
    JsonApiResource, Link as JsonApiLink, Links as JsonApiLinks, ResourceBuilder, SparseFields,
};

/// M6 view layer re-export — only with the `view` feature (pay-for-what-you-use).
#[cfg(feature = "view")]
pub use rustasea_view as view;

/// View re-exports for handler ergonomics (`View<T>`, `ViewEngine`, errors).
#[cfg(feature = "view")]
pub use rustasea_view::{AskamaEngine, View, ViewEngine, ViewError, ViewResponse};

/// Runtime (minijinja) engine re-export — only with `view-runtime-templates`.
#[cfg(feature = "view-runtime-templates")]
pub use rustasea_view::MinijinjaEngine;

/// Inertia server protocol re-export — only with the `inertia` feature (ADR-0002 §3).
#[cfg(feature = "inertia")]
pub use rustasea_inertia as inertia;

/// Inertia server re-exports for page/response handler ergonomics.
#[cfg(feature = "inertia")]
pub use rustasea_inertia::{
    Inertia, InertiaError, InertiaRequest, InertiaResponse, Page, PartialReload, RootView,
};

/// Inertia WASM client protocol re-export — only with `inertia-client`.
#[cfg(feature = "inertia-client")]
pub use rustasea_inertia_client as inertia_client;

/// Inertia client re-exports for registry/navigation ergonomics.
#[cfg(feature = "inertia-client")]
pub use rustasea_inertia_client::{
    ClientError, ComponentRegistry, InertiaClient, NavigationOutcome, Value as InertiaValue,
};

/// Inertia WASM presentation adapters re-export — with either variant feature.
#[cfg(any(feature = "wasm-dioxus", feature = "wasm-leptos"))]
pub use rustasea_inertia_adapters as inertia_adapters;

/// Shared adapter surface for mounting pages and performing browser visits.
#[cfg(any(feature = "wasm-dioxus", feature = "wasm-leptos"))]
pub use rustasea_inertia_adapters::{
    fetch_page, hard_navigate, install_registry, mount_installed, mount_page, AdapterError,
    RouterState,
};

/// Dioxus (React variant) adapter re-exports — only with `wasm-dioxus`.
#[cfg(feature = "wasm-dioxus")]
pub use rustasea_inertia_adapters::{
    use_dioxus_router, DioxusLink, DioxusRouterContext, DioxusRouterProvider,
};

/// Leptos (Vue variant) adapter re-exports — only with `wasm-leptos`.
#[cfg(feature = "wasm-leptos")]
pub use rustasea_inertia_adapters::{
    use_leptos_router, LeptosLink, LeptosRouterContext, LeptosRouterProvider,
};

/// Livewire analogue re-export — only with the `livewire` feature (ADR-0002 §5).
#[cfg(feature = "livewire")]
pub use rustasea_livewire as livewire;

/// Livewire re-exports for component/action/HTMX/realtime ergonomics.
#[cfg(feature = "livewire")]
pub use rustasea_livewire::{
    is_htmx, sse_from_receiver, ws_route, ActionAuthorizer, ActionHandler, ActionRequest,
    ActionResult, Actor, AllowAll, Component, ComponentState, DenyAll, Livewire, LivewireError,
    StateError, StatePatch, HX_REQUEST_HEADER,
};

/// Livewire axum router factory, aliased to avoid a generic root `routes` name.
#[cfg(feature = "livewire")]
pub use rustasea_livewire::routes as livewire_routes;

/// Starter-kit scaffolder re-export — only with the `scaffold` feature (ADR-0002 §6).
#[cfg(feature = "scaffold")]
pub use rustasea_scaffold as scaffold;

/// Scaffolder re-exports for `cargo rustasea new` ergonomics.
#[cfg(feature = "scaffold")]
pub use rustasea_scaffold::{
    resolve_target, AppName, Generated, RenderedFile, Scaffold, ScaffoldError, ScaffoldResult,
    StarterKitVariant,
};

/// AI re-exports only with the `ai` feature (NFR-Sca-02).
#[cfg(feature = "ai")]
pub use ai::{
    adapters, embed, Agent, AgentError, Ai, AiChunk, AiError, AiProvider, AiResponse, Capability,
    InProcessProvider, ProviderCall, StrToEmbeddings, Tool, ToolRegistry,
};

/// M7 MongoDB document store re-export — only with the `mongo` feature (ADR-0010).
#[cfg(feature = "mongo")]
pub use rustasea_mongo as mongo;

/// Mongo re-exports for document/CRUD ergonomics (ADR-0010).
#[cfg(feature = "mongo")]
pub use rustasea_mongo::{
    Collection as MongoCollection, Document as MongoDocument, Filter as MongoFilter, MongoClient,
    MongoConfig, MongoError, MongoPoolConfig, Update as MongoUpdate,
};

/// Logging facade re-export — only with the `logging` feature (CFG-005).
///
/// Opt-in so the core build never links the tracing subscriber/appender tree
/// (NFR-Sca-02). The crate parses `config/logging.toml` and installs a global
/// `tracing` subscriber from the selected Laravel-style channel.
#[cfg(feature = "logging")]
pub use rustasea_logging as logging;

/// Logging re-exports for subscriber-install and channel ergonomics (CFG-005).
#[cfg(feature = "logging")]
pub use rustasea_logging::{
    build as logging_build, init as logging_init, init_from_config as logging_init_from_config,
    ChannelConfig as LoggingChannelConfig, Driver as LoggingDriver, LoggingConfig, LoggingError,
    LoggingGuard,
};

/// Sentry error-tracking re-exports (ADOPT-004) — only with the `sentry` feature.
///
/// [`init_sentry`](rustasea_logging::init_sentry) installs the process-global
/// Sentry client from a [`SentryConfig`](rustasea_logging::SentryConfig); the
/// returned guard must be kept alive for the process lifetime.
#[cfg(feature = "sentry")]
pub use rustasea_logging::{init_sentry, scrub_event as scrub_sentry_event, SentryConfig};

/// Dev request-profiler / debug-toolbar re-export (ADOPT-009) — only with the
/// `debugbar` feature.
///
/// [`install`](rustasea_debugbar::install) registers the process-wide SQL and
/// event hooks; [`snapshot`](rustasea_debugbar::snapshot) returns the newest-last
/// ring of captured request profiles. Opt-in so the core build never links the
/// profiler (NFR-Sca-02).
#[cfg(feature = "debugbar")]
pub use rustasea_debugbar as debugbar;

/// Debug-toolbar re-exports for hook install and profile inspection (ADOPT-009).
#[cfg(feature = "debugbar")]
pub use rustasea_debugbar::{
    install as debugbar_install, profiler_middleware as debugbar_middleware, snapshot,
    uninstall as debugbar_uninstall, CacheEntry, EventEntry, RequestProfile, SqlEntry,
};

/// Queue dashboard re-export (ADOPT-021) — only with the `queue-dashboard`
/// feature.
///
/// [`DashboardConfig`](rustasea_queue_dashboard::DashboardConfig) resolves the
/// `[queue.dashboard]` config table and
/// [`spawn_sampler`](rustasea_queue_dashboard::sampler::spawn_sampler) starts the
/// periodic metrics sampler. Opt-in so the core build never links the dashboard
/// crate (NFR-Sca-02).
#[cfg(feature = "queue-dashboard")]
pub use rustasea_queue_dashboard as queue_dashboard;

/// Excel/CSV import-export re-export (ADOPT-023) — only with the `excel`
/// feature.
///
/// [`Excel`](rustasea_excel::Excel) is the facade for
/// [`import`](rustasea_excel::Excel::import) /
/// [`export`](rustasea_excel::Excel::export), and
/// [`ExportJob`](rustasea_excel::ExportJob) queues an export off the request
/// path. Opt-in so the core build never links the spreadsheet stack
/// (NFR-Sca-02).
#[cfg(feature = "excel")]
pub use rustasea_excel as excel;

/// Image manipulation re-export (ADOPT-024) — only with the `image` feature.
///
/// [`Image`](rustasea_image::Image) is the facade for
/// [`load`](rustasea_image::Image::load) /
/// [`from_bytes`](rustasea_image::Image::from_bytes), which build an
/// [`ImageBuilder`](rustasea_image::ImageBuilder) transform pipeline, and
/// [`ImageTransformJob`](rustasea_image::ImageTransformJob) queues a transform
/// off the request path. Opt-in so the core build never links the image codec
/// stack (NFR-Sca-02).
#[cfg(feature = "image")]
pub use rustasea_image as image;

/// Action pattern re-export (ADOPT-028) — only with the `action` feature.
///
/// [`Action`](rustasea_action::Action) is one unit of work with a typed
/// input/output/error and optional `validate`/`authorize` hooks. The same
/// action is driven by
/// [`ActionController`](rustasea_action::ActionController) (HTTP),
/// [`ActionJob`](rustasea_action::ActionJob) (queue),
/// [`ActionCommand`](rustasea_action::ActionCommand) (CLI), and
/// [`ActionListener`](rustasea_action::ActionListener) (events). Opt-in so the
/// core build never links the action adapters (NFR-Sca-02).
#[cfg(feature = "action")]
pub use rustasea_action as action;

/// Google service-account auth re-export (ADOPT-026) — only with the `google`
/// feature.
///
/// [`GoogleAuthClient`](rustasea_google::GoogleAuthClient) signs an RS256
/// assertion with a [`ServiceAccount`](rustasea_google::ServiceAccount) key,
/// exchanges it for a cached [`AccessToken`](rustasea_google::AccessToken), and
/// refreshes it at 80% of its lifetime (single-flight). Opt-in so the core
/// build never links the RSA/PEM signing stack (NFR-Sca-02).
#[cfg(feature = "google")]
pub use rustasea_google as google;

/// Modular application support re-export (ADOPT-027) — only with the `modules`
/// feature.
///
/// [`Module`](rustasea_modules::Module) is the contract a `make:module` crate
/// implements, and [`ModuleRegistry`](rustasea_modules::ModuleRegistry) mounts
/// the enabled modules' routes, providers, and migrations in deterministic
/// order from the `[modules]` manifest table. Opt-in so the core build never
/// links the registry (NFR-Sca-02).
#[cfg(feature = "modules")]
pub use rustasea_modules as modules;
