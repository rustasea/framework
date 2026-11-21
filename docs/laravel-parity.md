# RustaSea × Laravel 13.x — API-Surface Parity Map

> **Last updated:** 2026-11-20
> **Scope:** Maps the Laravel 13.x **API surface** (namespaces, contracts/interfaces, traits, notable classes) to RustaSea crates/modules, with an adoption status per row.
> **Companion docs:** [`docs/laravel-13-research.md`](laravel-13-research.md) (feature-level research) · [`docs/milestones.md`](milestones.md) (M0–M6 implementation status).

## 1. Purpose

`docs/laravel-13-research.md` answers *what Laravel 13 ships* at the feature level.
This document answers a different question: **which Laravel 13.x namespaces, interfaces, traits, and classes already have a RustaSea counterpart, and how complete is it?**

It is the API-surface parity layer. It is intentionally **not** a 1:1 inventory of every symbol on `api.laravel.com` (that index contains ~1000 classes). Per the agreed scope, it covers the **13 core surfaces** — Container, Contracts, Database/Eloquent, Routing, Http, Cache, Queue, Events, Auth, Validation, Support, Console, Filesystem — plus the advanced surfaces already present in the workspace (Broadcasting, Search, Storage, JSON:API, Testing, AI).

## 2. Methodology

1. Fetched the five authoritative Laravel 13.x API reference pages (see §3) and extracted the namespace list, the full interface list, the full trait list, and the notable-class list.
2. Enumerated RustaSea's real public API from the workspace source under `crates/` (traits, structs, enums, and re-exports), rather than from aspirational docs.
3. Mapped each Laravel surface to the closest RustaSea crate/module and assigned a status from §4.
4. Recorded gaps and an adoption order aligned to milestones **M0–M6** (as defined in [`docs/milestones.md`](milestones.md)).

**Name-integrity rule:** every Laravel name below is taken from a fetched reference page (or the raw `doc-index`); every RustaSea name is taken from the source tree. No symbol is invented. Because Laravel's class-level names for Routing/Database are only partially present in the truncated `classes.html` fetch, class-level rows use the fully-fetched `interfaces.html` / `traits.html` names and the raw `doc-index.html` cross-check.

## 3. Sources

| # | URL | Used for |
|---|---|---|
| 1 | https://api.laravel.com/docs/13.x/namespaces.html | Complete `Illuminate\*` namespace list |
| 2 | https://api.laravel.com/docs/13.x/interfaces.html | Complete interface/contract list |
| 3 | https://api.laravel.com/docs/13.x/traits.html | Complete trait list |
| 4 | https://api.laravel.com/docs/13.x/classes.html | Notable classes (fetch truncated; used for the first ~half of the A–Z class list) |
| 5 | https://api.laravel.com/docs/13.x/doc-index.html | Master A–Z symbol index (page is 8.7 MB; fetched raw and grepped for cross-verification, see §7) |

> Laravel's first-party **AI SDK** (`laravel/ai`) is a separate Composer package and is **not** part of the `api.laravel.com/docs/13.x` core namespace set; its Rust counterpart (`rustasea-ai`) is therefore covered as an advanced surface, not as an `Illuminate\*` row.
>
> Third-party Composer packages with a RustaSea counterpart (`nwidart/laravel-modules`, `league/flysystem-sftp-v3`) are listed as rows in Table B instead of the `Illuminate\*` namespace tables.

## 4. Status Legend

| Status | Meaning |
|---|---|
| **Adopted** | A working RustaSea equivalent exists and is reachable from a real code path. |
| **Partial** | An equivalent exists but is a stub, unwired, or covers only part of the Laravel contract. |
| **Planned** | No equivalent yet; listed on the gap roadmap (M0–M6). |
| **N-A** | Not applicable in Rust by design (e.g. global facades, PHP magic methods) or explicitly out of scope. |

> **ADOPT adoption is a first-class input to this matrix.** The `ADOPT-001`..`ADOPT-031` adoption wave (tracked per crate and command in [`docs/milestones.md`](milestones.md) M0-M6) is the source of the third-party Composer-package rows below (Spatie, `laravel/*`, and the wider ecosystem). Every row that corresponds to an ADOPT item cites its id in the Notes column, and the two documents cross-reference the same ADOPT id set. Two ADOPT items are workspace tooling rather than package parity, so they have no Table A/B row: `ADOPT-030` (quality gate: `rustfmt.toml`/`clippy.toml`/`deny.toml`/`cargo xtask ci`) and `ADOPT-031` (workspace dependency management: `[workspace.dependencies]` + `cargo xtask deps:check`).

## 5. Table A — Laravel Namespace → RustaSea Crate/Module

| Laravel namespace | RustaSea crate / module | Status | Notes |
|---|---|---|---|
| `Illuminate\Container` | `rustasea-foundation` (`Container`, `Application`) | **Adopted** | `bind` / `singleton` / `instance` / `get` are real (`crates/rustasea-foundation/src/lib.rs:63-104`). |
| `Illuminate\Contracts\Container` | `rustasea-foundation` (`Container`) | **Partial** | No separate contracts crate; contextual binding / `SelfBuilding` auto-wiring absent. |
| `Illuminate\Contracts` (general) | per-crate traits (no `rustasea-contracts` crate) | **Partial** | Contracts are expressed as Rust traits co-located with each crate (ADR-driven), not a mirrored namespace tree. |
| `Illuminate\Support` | `rustasea` facade re-exports; `rustasea-search::Str` | **Partial** | `Arrayable`/`Jsonable` map to `serde`; collections map to `Vec`/`Iterator`. |
| `Illuminate\Support\Facades` | — | **N-A** | ADR-0007: no global facades; managers live in `AppState` and flow through `axum::extract::State`. |
| `Illuminate\Config` | `rustasea-config` (`ConfigLoader`) | **Partial** | TOML + env overlay real; only `config/app` is auto-loaded today. |
| `Illuminate\Console` | `rustasea-cli` (`Artisan`, `Command`, `CommandRegistry`) | **Adopted** | `cargo artisan` registry + generators are real. |
| `Illuminate\Console\Scheduling` | `rustasea-schedule` (`Schedule`, `Scheduler`, `ScheduleCommand`) | **Partial** | Frequency builder, pause/resume, and `schedule:run` / `schedule:list` exist, but a due command cannot run: `dispatch_command` returns an error because no queue dispatch path is wired (`crates/rustasea-schedule/src/scheduler.rs:146`). Pause state is per-process (`crates/rustasea-schedule/src/pause.rs:28`); `onOneServer` takes no lock; `schedule:run` runs a single tick. |
| `Illuminate\Database` | `rustasea-orm` (`DbPool`, `Executor`, `Migrator`) | **Adopted** | Real sqlx pool + async query execution + transactions + migrations/seeders/factories (`crates/rustasea-orm/src/db.rs:24`, `db/exec.rs:70`, `migration.rs:190`). |
| `Illuminate\Database\Eloquent` | `rustasea-orm` (`Model`, `QueryBuilder`, `Relation`, `SoftDeletes`, `Timestamps`) | **Partial** | `#[derive(Model)]` + eager loading and relation serde round-trip real (`crates/rustasea-orm/src/eager.rs:59`); `show:model` source-level introspection (attributes/casts/soft-delete/relations, both `#[derive(Model)]` and hand-written `impl Model`) real (`crates/rustasea-cli/src/model_inspect.rs:123`); live DB column reflection pending. |
| `Illuminate\Database\Migrations` | `rustasea-orm` (`Migration`, `Migrator`, `MigrationRecord`) | **Adopted** | Runner executes against the live pool — `run`/`rollback`/`fresh`/`seed` (`crates/rustasea-orm/src/migration.rs:190`). |
| `Illuminate\Database\Query` | `rustasea-orm` (`QueryBuilder`, `Value`, `JsonFilter`) | **Adopted** | Fluent builder + async execution real (`crates/rustasea-orm/src/builder/exec.rs:105`). |
| `Illuminate\Events` | `rustasea-events` (`Dispatcher`, `Event`, `Listener`) | **Partial** | Inline dispatch real; queue-enabled listeners are serialized into a `ListenerJob` and pushed through the queue (`crates/rustasea-events/src/dispatcher.rs:137`, `:145`). `dispatchAfterResponse` only buffers into `EventSink` (`:254`) and nothing flushes it. |
| `Illuminate\Routing` | `rustasea-router` (`Router`, `RouteEntry`, `ControllerRef`) | **Partial** | DSL + controller dispatch real; `route:list` introspection live (6-column table published at boot via `RouteSource`: `crates/rustasea-cli/src/routes.rs:19`, `:33`; `crates/rustasea-app/src/bootstrap/app.rs:57`; `crates/rustasea-cli/src/commands/inspect.rs:44`). |
| `Illuminate\Http` | `rustasea-http` (`AppState`, `JsonResponse`, `HttpError`) | **Partial** | Request/response + CORS real; idle (inter-chunk) timeout enforced for streamed bodies (`crates/rustasea-http/src/idle.rs:23`). |
| `Illuminate\Http\Client` | `rustasea-http` (`HttpClient`) | **Adopted** | reqwest wrapper with `throw` / `try_throw` semantics. |
| `Illuminate\Http\Resources\JsonApi` | `rustasea-jsonapi` (`JsonApiResource`, `Document`, `ResourceBuilder`) | **Adopted** | Sparse fieldsets, links, JSON:API content type real. |
| `Illuminate\Cache` | `rustasea-cache` (`Store`, `CacheManager`, `Repository`, `Lock`) | **Partial** | Memory store real; Redis store real behind the opt-in `redis` feature (`GAP-004`; `crates/rustasea-cache/src/redis.rs:155`; `crates/rustasea-cache/Cargo.toml:12`) — typed `StoreUnavailable` only when the feature is off or Redis is unreachable; driver matrix incomplete. |
| `Illuminate\Queue` | `rustasea-queue` (`Queue`, `QueueDriver`, `Job`, `QueueRegistry`) | **Adopted** | Sync + real database/Redis drivers + worker loop (`crates/rustasea-queue/src/driver/{database,redis,worker}.rs`). |
| `Illuminate\Bus` | `rustasea-queue` (`BatchHandle`, `BatchId`) | **Partial** | Batch handles plus a durable `DatabaseBatchRepository` over the `job_batches` table (`crates/rustasea-queue/src/batch_db.rs:77`). |
| `lorisleiva/laravel-actions` | `rustasea-action` (`Action`, `ActionController`, `ActionJob`, `ActionCommand`, `ActionListener`) | **Partial** | One action unit invoked from HTTP/queue/CLI/events with validate/authorize hooks; `make:action` generator; scaffold auth actions demonstrate the pattern (ADOPT-028). |
| `Laravel\Horizon` | `rustasea-queue-dashboard` (`DashboardConfig`, `sampler`, `QueueMetricsHistory`) | **Partial** | Queue dashboard (`/queue`): live depth/age, `queue_metrics` history + retention, failed-job retry/forget, worker heartbeats; feature `queue-dashboard` (ADOPT-021). No supervisor/balancing/auto-scaling yet. |
| `Illuminate\Auth` | `rustasea-auth` (`AuthManager`, `Guard`, `JwtGuard`, `SessionGuard`) | **Partial** | JWT/CSRF/throttle real; session guard real over `tower-sessions` (`crates/rustasea-auth/src/session.rs:99`) with an in-memory default store and a database store (`crates/rustasea-auth/src/session/database.rs:74`); users resolve through the database-backed `DatabaseUserProvider` (`crates/rustasea-auth/src/users/provider/database.rs:49`), which the runnable app installs at boot when the `users` table exists (`crates/rustasea-app/src/bootstrap/auth.rs:337`). |
| `Illuminate\Auth\Access` | `rustasea-router` (`AuthorizeRegistry`, `AuthorizeResource`, `#[authorize]` metadata) | **Adopted** | `#[authorize]` metadata is resolved and enforced at dispatch: `Router::authorize_meta` consumes the macro-emitted tuple (`crates/rustasea-router/src/authorize.rs:216`), and `apply_authorize` wraps the route in a fail-closed authorization layer (`crates/rustasea-router/src/dispatch.rs:209`). |
| `Illuminate\Validation` | `rustasea-validation` (`Validatable`, `Rules`, `ErrorBag`, `FormRequest`) | **Partial** | Rules + ErrorBag + form requests real; DB presence verifier absent. |
| `Illuminate\Hashing` | `rustasea-auth` (`PasswordVerifier`, `Argon2Verifier`) | **Partial** | Argon2 verifier real; no hasher manager / rehash policy. |
| `Illuminate\Session` | `rustasea-auth` (`SessionGuard`, `SessionPolicy`) | **Partial** | `tower-sessions`-backed session guard real (`crates/rustasea-auth/src/session.rs:99`); the default store is in-memory and `driver = "database"` in `config/session.toml` selects the persistent `DatabaseSessionStore` (`crates/rustasea-auth/src/session/database.rs:74`, built by `from_config_database` `crates/rustasea-auth/src/session.rs:155`). No Redis/file/cookie store. |
| `Illuminate\Cookie` | `rustasea-http` (CORS + `SecurityConfig`) | **Partial** | No queued-cookie jar / cookie encryption layer yet. |
| `Illuminate\Filesystem` | `rustasea-storage` (`Storage`, `StorageManager`, `LocalDisk`, `ObjectDisk`, `ReadThrough`) | **Adopted** | `object_store`-backed disks + read-through with path confinement. |
| `Maatwebsite\Excel` | `rustasea-excel` (`Excel`, `ImportBuilder`, `ExportBuilder`, `ExportJob`) | **Partial** | Streaming CSV/xlsx import with row-level validation reports; chunked export to storage; queued `ExportJob` + signed download URLs (ADOPT-023). Formula/style/format parity absent. |
| `Illuminate\Broadcasting` | `rustasea-broadcast` (`ShouldBroadcast`, `BroadcastEvent`, `Channel`, `BroadcastHub`, `BroadcastManager`) | **Partial** | WS + SSE real; Pusher HTTP driver + Redis Pub/Sub fan-out real (ADOPT-022); Ably absent. |
| `Illuminate\Pagination` | `rustasea-orm` (`Paginator`, `PageMeta`) | **Adopted** | Paginator wired into query execution (`crates/rustasea-orm/src/builder/exec.rs:145`). |
| `Illuminate\Pipeline` | — | **N-A** | Tower middleware chains replace the PHP pipeline; no `Illuminate\Pipeline` analogue required. |
| `Illuminate\Encryption` | — | **Planned** | No encrypter / key-rotation service. |
| `Illuminate\Translation` | `rustasea-i18n` (`TranslationLoader`, `Translator`, `Message`, global `__`/`trans_choice`) + `lang:check` | **Adopted** | Loads `resources/lang/{locale}/*.toml`/`*.json` with dot-namespaced keys, active-locale-then-fallback resolution, interpolation, and pluralization (`crates/rustasea-i18n/src/{loader,translator,message}.rs`); global helpers in `crates/rustasea-i18n/src/global.rs`; `lang:check` reports missing/unused/duplicate entries (ADOPT-005; `crates/rustasea-cli/src/commands/langcheck.rs:106`). |
| `Illuminate\View` | `rustasea-view` (`ViewEngine`, `ViewResponse`) | **Partial** | Engine-agnostic rendering with askama (default) and minijinja (`runtime-templates`), re-exported behind the `view` feature; the runnable app renders `resources/views` (`crates/rustasea-view/src/{askama_engine,minijinja_engine,response}.rs`). No Blade syntax or component system. |
| `Illuminate\Mail` | `rustasea-mail` (`Mailable`, `Mailer`, `ArrayMailer`, `LogMailer`, `SmtpMailer`, `QueuedNotification`) | **Partial** | Mailable/Mailer contracts + array/log transports and a feature-gated SMTP transport (`smtp`) with queued notifications are real (`crates/rustasea-mail/src/`); re-exported from the umbrella as `rustasea::mail` with the `mail-smtp` / `mail-templates` features (`crates/rustasea/src/lib.rs:158`). Mailables render from minijinja templates behind the `templates` feature (`crates/rustasea-mail/src/template.rs:39`); no Markdown mail components. |
| `Illuminate\Notifications` | `rustasea-queue` (`NotificationGuard`), `rustasea-mail` (`MailNotification`, `QueuedNotification`) | **Partial** | Missing-model skip guard plus queued mail notifications (`crates/rustasea-mail/src/notification.rs:22`, `:41`); no multi-channel dispatcher. |
| `Illuminate\Log` | `tracing` (workspace dependency) | **N-A** | Structured logging is handled by `tracing`, not a Laravel-style `Log` facade. |
| `opcodesio/log-viewer` | `rustasea-logging` reader + `cargo artisan log:show` + `/_logs` dev viewer | **Partial** | On-disk log reader (`rustasea-logging::reader`: parse/filter/resolve/tail), `log:show` CLI (`--level`/`--since`/`--grep`/`--limit`/`--follow`/`--json`), and a dev-only `/_logs` web surface behind the `log-viewer` feature (ADOPT-014). Rotation-aware file resolution; no per-entry stack-trace grouping or multi-file index. |
| `Illuminate\Redis` | `rustasea-cache` (`RedisStore`) | **Partial** | Real `deadpool-redis` store behind the opt-in `redis` feature (`GAP-004`; `crates/rustasea-cache/src/redis.rs:155`), built via `from_url`/`from_pool` (`:91`, `:105`); atomic `SET NX` insert-if-absent + Lua compare-and-delete. No general-purpose command/pub-sub surface. |
| `Illuminate\Process` | — | **N-A** | Process spawning handled by `tokio::process` directly. |
| `Illuminate\Concurrency` | `tokio` (workspace dependency) | **N-A** | Concurrency is native `tokio`; no `Concurrency` facade. |
| `Illuminate\Foundation` | `rustasea-foundation` + `bootstrap/` | **Partial** | App boot + graceful shutdown real; provider/command registries empty. |
| `Illuminate\Testing` | `rustasea-testing` (`TestCase`, `TestConfig`) | **Partial** | `TestCase` real; `testcontainers` unused; no DB refresh traits. |
| `pestphp/pest-plugin-browser` | `rustasea-testing` (`Browser`, `ServerHandle`, `BrowserError`) | **Partial** | Feature-gated WebDriver harness (feature `browser`, `fantoccini`) with Dusk-style `visit`/`fill`/`click`/`select`/`check`/`press`/`wait_for`/`assert_see` helpers, an ephemeral axum `ServerHandle`, and failure screenshots. Auto-skips when `WEBDRIVER_URL` is unreachable rather than failing the default run; `make:test --browser` scaffolds a test (ADOPT-029). No DevTools protocol, parallel browser pool, or Vite integration. |
| `Illuminate\Image` | rustasea-image (Image, ImageBuilder, ImageTransformJob) | Partial | Fluent pipeline (resize/fit/cover/thumbnail/crop/rotate/watermark text+image), EXIF auto-orient, jpeg/png/webp/gif/bmp/tiff encode, storage round-trip + queued transforms (ADOPT-024). No GD/Imagick drivers or animated webp/gif transforms. |
| `Illuminate\JsonSchema` | — | **Planned** | No JSON-schema contract. |
| AI SDK (`laravel/ai`, separate package) | `rustasea-ai` (`AiProvider`, `Agent`, `Tool`) | **Partial** | Provider-agnostic traits real; the agent runtime echoes the prompt and never calls a provider (`crates/rustasea-ai/src/agent.rs:247`); real HTTP-backed OpenAI-style and Anthropic adapters plus a deterministic in-process provider kept for tests (feature `ai`; `crates/rustasea-ai/src/providers/mod.rs:28`, `adapters.rs:50`). Gemini and Bedrock are served through their OpenAI-compatible endpoints (`crates/rustasea-ai/src/providers/client.rs:225`, `:382`); the provider-native APIs are not implemented. Provider-backed embeddings send a dedicated embedding model (`crates/rustasea-ai/src/embed.rs:18`, `providers/client.rs:246`), and Bedrock requires `BEDROCK_EMBEDDING_MODEL`; the engine-agnostic `rustasea-search::Str` keeps a deterministic stub fallback (`crates/rustasea-search/src/embeddings.rs:97`). |
| `spatie/laravel-permission` | `rustasea-auth::rbac` (`Role`, `RbacRegistry`, `HasRoles`, `PermissionResolver`) | **Adopted** | Role/permission assignment with cached permission lookups and a `PermissionResolver` the ability `Gate` consults after its defined abilities (ADOPT-001; `crates/rustasea-auth/src/rbac/{mod,role,registry,has_roles}.rs`). |
| `spatie/laravel-activitylog` | `rustasea-activitylog` (`ActivityLogger`, `ActivityEvent`) | **Adopted** | Model-change audit trail with JSON property diffs and an optional `batch_uuid`, built on the ORM `#[logs_activity]` seam and persisted to the `audit_log` table (ADOPT-002; `crates/rustasea-activitylog/src/{lib,recorder,model}.rs`). |
| `rappasoft/laravel-authentication-log` | `rustasea-authlog` (`AuthenticationLogLogger`, `NewDeviceNotifier`) | **Adopted** | One row per login/failed/lockout/logout with client IP and `User-Agent`; an unknown-IP login triggers a queued new-device notification (ADOPT-003; `crates/rustasea-authlog/src/{lib,recorder,notifier}.rs`). |
| `getsentry/sentry-laravel` | `rustasea-http::sentry` + `rustasea-logging::sentry` | **Partial** | Opt-in request-context middleware (method/path/status/request-id, 5xx capture) plus a logging layer with a `before_send` secret-scrubbing hook; inert until a DSN is bound (ADOPT-004; `crates/rustasea-http/src/sentry.rs`, `crates/rustasea-logging/src/sentry/mod.rs`). |
| `glhd/laravel-timezone-mapper` | `rustasea-timezone` (`resolve`, `mapper`) | **Adopted** | IANA validation plus a deterministic user-timezone resolution chain (stored preference, session, request header, app default) with UTC/local formatting helpers (ADOPT-006; `crates/rustasea-timezone/src/{lib,mapper}.rs`). |
| `laravel/sail` | `xtask docker:*` + `rustasea-scaffold` docker templates | **Partial** | `cargo xtask docker:up`/`docker:down`/`docker:logs` wrap the compose CLI, and `cargo rustasea new` emits a multi-stage `Dockerfile` + `docker-compose.yml` dev stack (ADOPT-007; `xtask/src/docker.rs`, `crates/rustasea-scaffold/src/templates/docker.rs`). |
| `laravel/react-starter-kit` / `laravel/vue-starter-kit` / `laravel/svelte-starter-kit` / `laravel/livewire-starter-kit` (first-party starter kits) | `rustasea/<x>-starter-kit` repositories + `rustasea-scaffold` | **Partial** | Starter-kit model parity (ADR-0002): the variant set is `{blade\|react\|vue\|svelte\|livewire}` (`crates/rustasea-scaffold/src/variant.rs:13`), each variant ships as its own repository mirroring the Laravel split (`rustasea/react-starter-kit`, `rustasea/vue-starter-kit`, `rustasea/svelte-starter-kit`, `rustasea/livewire-starter-kit`; the Blade kit is the `rustasea/rustasea` skeleton), and every kit shares one generated auth/domain core: react/vue/svelte map to Dioxus/Leptos/Sycamore WASM on the Inertia contract, livewire to askama + HTMX. The react/vue/svelte kits carry full 11-page auth/settings parity (`crates/rustasea-scaffold/src/templates/inertia_{react,vue,svelte}/`). No `composer`/`node_modules` bootstrap step, and no kit installer (`install:features`) yet. |
| `laravel/tinker` | `cargo artisan tinker` | **Adopted** | Interactive REPL over a booted application (`config`/container/route/command inspection, `help`); piped stdin scripts the session (ADOPT-008; `crates/rustasea-cli/src/commands/tinker.rs:43`). |
| `barryvdh/laravel-debugbar` | `rustasea-debugbar` (`Profiler`, recorders, middleware) | **Partial** | Dev-only per-request profiler capturing SQL, cache, and events into an in-memory ring buffer with a JSON/HTML surface behind the `debugbar` feature (ADOPT-009; `crates/rustasea-debugbar/src/{lib,middleware,recorders}.rs`). |
| `spatie/laravel-ignition` | `rustasea-http` dev/prod error renderers | **Partial** | Application error type plus dev page and prod JSON envelope, with panic catching and dev panic-location capture; no rich stack-frame/editor integration (ADOPT-010; `crates/rustasea-http/src/{error,panic}.rs`). |
| `dedoc/scramble` | `rustasea-openapi` + `cargo artisan openapi:generate` | **Adopted** | OpenAPI 3.1 document generated from the live route table (the same registry `route:list` renders) so the documented and served surfaces cannot diverge (ADOPT-011; `crates/rustasea-openapi/src/lib.rs`, `crates/rustasea-cli/src/commands/openapi.rs`). |
| `fakerphp/faker` | `rustasea-testing::faker` (`Faker`, `unique_*`) | **Partial** | Locale-aware fake-data generation (`fake` 5.1) behind the `faker` feature: seeded determinism, 14 locales, and `unique_*` helpers; the `WithFaker` trait-style mixin is not reproduced (ADOPT-012; `crates/rustasea-testing/src/faker/`). |
| `Illuminate\Support\Testing\Fakes` | `rustasea-testing::fakes` (`FakeQueue`, `FakeMailer`, `FakeDispatcher`, `FakeCache`) | **Partial** | Recording doubles that intercept queue/mail/event/cache side effects with `assert_*` helpers; each installs through the closest existing seam (ADOPT-013; `crates/rustasea-testing/src/fakes.rs`, `fakes/`). |
| `laravel/boost` | `cargo artisan mcp:serve` | **Partial** | Serves project knowledge (routes, docs, commands, redacted config) over MCP stdio behind the `mcp` feature (ADOPT-015; `crates/rustasea-cli/src/commands/mcp_serve.rs`). |
| `bottelet/translation-checker` | `cargo artisan lang:check` | **Adopted** | Loads every locale under `resources/lang` through `rustasea-i18n::TranslationLoader` and reports missing (fails), duplicate (fails), and unused (informational) keys against a reference locale (ADOPT-005; `crates/rustasea-cli/src/commands/langcheck.rs:106`). |
| `spatie/laravel-cascade-soft-deletes` | `rustasea-orm` cascade soft delete/restore/force delete | **Adopted** | Models opt in with `#[cascade_soft_deletes("posts", ...)]`; the delete/restore path fans the operation out to the named relations in chunks (single level) (ADOPT-018; `crates/rustasea-orm/src/model_ops/cascade.rs`). |
| `spatie/laravel-model-caching` | `rustasea-orm` query/model result caching | **Partial** | `QueryBuilder::cache(ttl)`/`cache_forever()` store decoded rows through a process-wide `QueryCacheStore`, with O(1) per-table generation invalidation on write (ADOPT-019; `crates/rustasea-orm/src/cache.rs`, `builder/cached.rs`). |
| `pusher/pusher-php-server` | `rustasea-broadcast` Pusher HTTP driver | **Partial** | Pusher HTTP transport with HMAC-SHA256 request signing and the MD5 body digest, plus `pusher`/`private-`/`presence-` client-auth channel authorization behind the `pusher` feature (ADOPT-022; `crates/rustasea-broadcast/src/`, `crates/rustasea-app/src/routes/broadcasting.rs`). |
| `hypervel/hypervel` | `rustasea-http` base `Controller` + `rustasea-scaffold` app skeleton + `make:controller` | **Partial** | App-skeleton parity: a base `Controller` trait with shared response/redirect helpers (`crates/rustasea-http/src/controller.rs`), scaffold-emitted `app/http/controllers/controller.rs` + struct controllers implementing it (`crates/rustasea-scaffold/src/templates/app_http.rs`), and `make:controller` output that conforms to the base trait (`crates/rustasea-cli/src/generators/kinds/controller.rs`) (TASK-091, TASK-092, TASK-093). Project-structure parity: generated apps ship Hypervel-style root hygiene and directory placeholders (`public/`, `bootstrap/cache/`, `storage/` subdirectories; `crates/rustasea-scaffold/src/templates/core.rs`), broadcasting/CORS config templates (`crates/rustasea-scaffold/src/templates/config.rs`), boot-time migration registration in `bootstrap/commands.rs` (framework queue migrations plus the app's own migrations; `crates/rustasea-scaffold/src/templates/bootstrap.rs`), and a `rustfmt.toml` + minimal CI workflow (`crates/rustasea-scaffold/src/templates/tooling.rs`) (TASK-094, TASK-095, TASK-096). The `User` model and `AppServiceProvider` conventions are adopted by the generated app skeleton; full Hypervel coroutine/runtime parity is out of scope. |

## 6. Table B — Key Interface / Trait → RustaSea Equivalent

> Rows are grouped by the 13 focus surfaces. "Rationale" states why the status is what it is.

### Container & Contracts

| Laravel interface/trait | RustaSea equivalent | Status | Rationale |
|---|---|---|---|
| `Illuminate\Contracts\Container\Container` | `rustasea-foundation::Container` | **Partial** | Resolve/bind/singleton/instance present; contextual bindings and auto-construction (`SelfBuilding`) missing. |
| `Illuminate\Contracts\Container\ContextualBindingBuilder` | — | **Planned** | No per-consumer binding surface. |
| `Illuminate\Contracts\Container\SelfBuilding` | — | **Planned** | No reflection-based auto-wiring. |
| `Illuminate\Contracts\Support\Arrayable` | `serde::Serialize` | **N-A** | Serialization is idiomatic `serde`, not a bespoke interface. |
| `Illuminate\Contracts\Support\Jsonable` | `serde_json` | **N-A** | JSON encoding is handled by `serde_json`. |
| `Illuminate\Contracts\Support\DeferrableProvider` | `rustasea-foundation::ServiceProvider` | **Partial** | `register`/`boot` lifecycle real; no deferred-provider optimization. |
| `Illuminate\Contracts\Support\MessageBag` / `MessageProvider` | `rustasea-validation::ErrorBag` | **Adopted** | Field-keyed aggregation implemented. |
| `Illuminate\Contracts\Support\ValidatedData` | `rustasea-validation::FormRequest` | **Partial** | `validated()` payload exists; contract breadth smaller. |
| `Illuminate\Contracts\Support\Responsable` | `rustasea-http::JsonResponse` | **Partial** | JSON/status helpers real; not a generic response contract. |
| `Illuminate\Contracts\Debug\ExceptionHandler` | — | **Planned** | Error handling delegated to axum/tower; no handler contract. |
| `Illuminate\Container\Attributes\Singleton` | `rustasea-foundation::Container::singleton` | **Partial** | Programmatic singleton binding; no attribute-driven binding. |

### Database / Eloquent

| Laravel interface/trait | RustaSea equivalent | Status | Rationale |
|---|---|---|---|
| `Illuminate\Contracts\Database\Eloquent\Builder` | `rustasea-orm::QueryBuilder` | **Partial** | Fluent builder real; no Eloquent-level model hydration. |
| `Illuminate\Contracts\Database\Query\Builder` | `rustasea-orm::QueryBuilder` | **Adopted** | Clause compilation + async execution real (`crates/rustasea-orm/src/builder/exec.rs`). |
| `Illuminate\Contracts\Database\Eloquent\CastsAttributes` | — | **Planned** | Attribute casting not implemented (serde only). |
| `Illuminate\Contracts\Database\Eloquent\Castable` | — | **Planned** | No castable type contract. |
| `Illuminate\Contracts\Database\Eloquent\SupportsPartialRelations` | `rustasea-orm::Relation` | **Partial** | Relation declarations exist; partial-relation loading pending. |
| `Illuminate\Database\Eloquent\Scope` | `rustasea-orm::ScopeRegistry` | **Partial** | Global-scope registry real; not applied during execution. |
| `Illuminate\Database\Eloquent\SoftDeletes` (trait) | `rustasea-orm::SoftDeletes` | **Adopted** | Marker/inference via `#[derive(Model)]`; soft-delete query filtering enforced (`crates/rustasea-orm/src/model_ops.rs`). |
| `Illuminate\Database\Eloquent\Concerns\HasTimestamps` | `rustasea-orm::Timestamps` | **Partial** | Timestamp inference real; write-path population pending. |
| `Illuminate\Database\Eloquent\Concerns\HasRelationships` | `rustasea-orm::{Relation, RelationKind}` | **Adopted** | Relation declarations + eager-loading loader real (`crates/rustasea-orm/src/eager.rs:59`). |
| `Illuminate\Database\Eloquent\Concerns\HasUuids` | `#[derive(Model)]` + `uuid::Uuid` `id` | **Partial** | UUID `id` enforced at derive time; no ULID variant. |
| `cviebrock/eloquent-sluggable` (`SlugOptions` / `HasSlug`) | `rustasea_orm::sluggable` (`SlugOptions`, `slugify`, `SluggableFind`) | **Partial** | Unicode-aware slug generation, deterministic collision suffixing (`-2`, `-3`, …), `#[sluggable(...)]` derive opt-in, write-path hooks (`on_update`), `find_by_slug`/`find_by_slug_or_fail`, and `{post:slug}` route binding real (ADOPT-016; `crates/rustasea-orm/src/sluggable.rs`, `crates/rustasea-router/src/binding.rs`); no slug-history/redirect table. |
| `Illuminate\Database\Eloquent\Factories\HasFactory` | `rustasea-orm::Factory` / `SqlSeeder` | **Adopted** | Factory + seeder traits execute against the pool (`crates/rustasea-orm/src/factory.rs`, `migration.rs:289`). |
| `Illuminate\Database\ConnectionInterface` | `rustasea-orm::DbPool` | **Adopted** | Pool connect/ping + model CRUD round-trip real (`crates/rustasea-orm/src/db.rs:24`, `model_ops.rs:25`). |
| `Illuminate\Database\ConnectionResolverInterface` | `rustasea-orm::DbPool` | **Partial** | Single-pool resolution; multi-connection resolver thinner. |
| `Illuminate\Database\Migrations\MigrationRepositoryInterface` | `rustasea-orm::{Migrator, MigrationRecord}` | **Adopted** | Repository records + migration runner real (`crates/rustasea-orm/src/migration.rs:190`). |
| `Illuminate\Database\Eloquent\Relations\Concerns\InteractsWithPivotTable` | `rustasea-orm::Relation` | **Planned** | Pivot-table interaction not implemented. |
| `awobaz/compoships` (composite-key relations) | `rustasea_orm::{Relation, Model}` | **Adopted** | Composite-key `has_many`/`belongs_to`/`many_to_many` builders (`Relation::has_many_composite` etc., arity-validated with a typed error), tuple `IN` eager-loading in one batched query (`crates/rustasea-orm/src/eager.rs`), and composite-primary-key models via `#[model(primary_key = ["a", "b"])]` with a full create/update/delete round-trip (ADOPT-017; `crates/rustasea-orm/src/relation.rs`, `model_ops/composite.rs`, `rustasea-macros/src/model_primary_key.rs`). |
| `staudenmeir/eloquent-json-relations` | `rustasea_orm::{Relation, JsonSpec}` | **Adopted** | JSON-embedded relations: `Relation::belongs_to_json`/`has_many_json`/`belongs_to_many_json` carry a `JsonSpec { column, path }` (`crates/rustasea-orm/src/relation.rs`), dialect predicates `QueryBuilder::where_json_in` (scalar `IN`) and `where_json_contains_any` (array overlap; Postgres `?\|`, MySQL `JSON_OVERLAPS`, SQLite `json_each`) via `JsonRelationFilter` (`crates/rustasea-orm/src/types.rs`, `builder/json.rs`), batched eager loaders in `crates/rustasea-orm/src/eager/json.rs` (one `IN (…)` query, malformed/missing cells skipped), and a `Blueprint::json_index` expression-index helper (ADOPT-020; `crates/rustasea-orm/src/schema/emit.rs`). |

### Routing / Http

| Laravel interface/trait | RustaSea equivalent | Status | Rationale |
|---|---|---|---|
| `Illuminate\Contracts\Routing\Registrar` | `rustasea-router::Router` | **Partial** | Registration DSL real; full registrar contract (bindings, fallbacks) thinner. |
| `Illuminate\Contracts\Routing\ResponseFactory` | `rustasea-http::JsonResponse` | **Partial** | JSON/status helpers only. |
| `Illuminate\Contracts\Routing\UrlGenerator` | — | **Planned** | No named-route URL generator. |
| `Illuminate\Contracts\Routing\UrlRoutable` | `rustasea-router::{ModelBinder, BindingRegistry}` | **Partial** | Implicit model binding for `{post:slug}` selector routes via registered binders (`crates/rustasea-router/src/binding.rs`); no named-route URL generation. |
| `Illuminate\Routing\Contracts\ControllerDispatcher` | `rustasea-router::dispatch` | **Adopted** | Real controller dispatch landed in GAP-002. |
| `Illuminate\Routing\Contracts\CallableDispatcher` | `rustasea-router::Handler` | **Partial** | Handler trait real; closure/callable dispatch breadth narrower. |
| `Illuminate\Routing\Controllers\HasMiddleware` | `rustasea-router::MiddlewareRegistry` + `rustasea-macros::#[middleware]` | **Adopted** | `#[middleware]` emits a doc-hidden spec list consumed by `Router::middleware_meta` (`crates/rustasea-router/src/metadata.rs:237`) and enforced by `apply_middleware` at dispatch (`crates/rustasea-router/src/dispatch.rs:183`). |
| `Illuminate\Contracts\Http\Kernel` | `rustasea-http::AppState` | **Partial** | Middleware stack assembled via axum/tower; no single Kernel contract. |
| `Illuminate\Http\Client\Factory` | `rustasea-http::HttpClient` | **Partial** | Client real; no fake/record-replay factory. |
| `Illuminate\Http\Resources\JsonApi\Concerns\ResolvesJsonApiElements` | `rustasea-jsonapi::ResourceBuilder` | **Adopted** | Element resolution + document shaping implemented. |

### Cache / Queue / Events

| Laravel interface/trait | RustaSea equivalent | Status | Rationale |
|---|---|---|---|
| `Illuminate\Contracts\Cache\Store` | `rustasea-cache::Store` | **Adopted** | `get`/`put`/`forget` store contract implemented by memory store. |
| `Illuminate\Contracts\Cache\Repository` | `rustasea-cache::{Repository, RepositoryLike}` | **Adopted** | Repository wrapper + trait implemented. |
| `Illuminate\Contracts\Cache\Lock` | `rustasea-cache::{Lock, LockGuard}` | **Partial** | Atomic lock/guard over any `Store` (`put_if_absent` + compare-and-delete release, `crates/rustasea-cache/src/lock.rs:47`, `:124`); distributed when backed by the feature-gated `RedisStore` (`SET NX` + Lua). No owner/force-release surface. |
| `Illuminate\Contracts\Cache\Factory` | `rustasea-cache::CacheManager` | **Partial** | Manager selects stores; driver matrix incomplete. |
| `Illuminate\Contracts\Queue\Queue` | `rustasea-queue::Queue` | **Adopted** | Push/dispatch surface real; sync + database + Redis drivers wired. |
| `Illuminate\Contracts\Queue\Job` | `rustasea-queue::{Job, ErasedJob}` | **Adopted** | Job + erased-job traits real; worker loop real (`crates/rustasea-queue/src/driver/worker.rs:78`). |
| `Illuminate\Contracts\Queue\ShouldQueue` | `rustasea-queue::Job` | **Adopted** | Implemented via trait; queue-enabled listeners enqueue a `ListenerJob` (`crates/rustasea-events/src/dispatcher.rs:145`). |
| `Illuminate\Contracts\Queue\ShouldBeUnique` | `rustasea-queue::ShouldBeUnique` | **Adopted** | Unique-job lease over a cache lock: `register_unique::<J>()` at boot, lease acquired on dispatch and released on a terminal outcome (`crates/rustasea-queue/src/unique.rs:66`, `:169`, `:233`). Locking disengages when no cache store is installed. |
| `Illuminate\Contracts\Queue\Factory` | `rustasea-queue::QueueRegistry` | **Partial** | Registry + routing real; connection factory thinner. |
| `Illuminate\Queue\Connectors\ConnectorInterface` | `rustasea-queue::QueueDriver` | **Partial** | Driver trait real; database + Redis drivers implemented (`crates/rustasea-queue/src/driver/{database,redis}.rs`). |
| `Illuminate\Contracts\Events\Dispatcher` | `rustasea-events::Dispatcher` | **Partial** | Inline dispatch real; queue-enabled listeners enqueue a `ListenerJob` (`crates/rustasea-events/src/dispatcher.rs:145`); `dispatchAfterResponse` buffers but is never flushed (`:254`). |
| `Illuminate\Events\Dispatcher` (class) | `rustasea-events::Dispatcher` | **Partial** | Concrete dispatcher present with the same caveat. |

### Auth / Validation / Session

| Laravel interface/trait | RustaSea equivalent | Status | Rationale |
|---|---|---|---|
| `Illuminate\Contracts\Auth\Guard` | `rustasea-auth::Guard` | **Adopted** | Guard trait + JWT implementation real; JWT `logout` revokes the token pair and refresh tokens are single-use (`crates/rustasea-auth/src/jwt.rs:314`, `:295`). |
| `Illuminate\Contracts\Auth\StatefulGuard` | `rustasea-auth::SessionGuard` | **Partial** | `tower-sessions`-backed session guard real (`crates/rustasea-auth/src/session.rs:99`); default store is in-memory, with a database store selectable by config (`crates/rustasea-auth/src/session/database.rs:74`). |
| `Illuminate\Contracts\Auth\Authenticatable` | `rustasea-auth::AuthUser` | **Adopted** | User identity type implemented. |
| `Illuminate\Contracts\Auth\UserProvider` | `rustasea-auth::{UserProvider, UserLookup}` | **Adopted** | Async write+read `UserProvider` (`crates/rustasea-auth/src/users/provider.rs:110`) with a database implementation over the `users` table (`DatabaseUserProvider`, `crates/rustasea-auth/src/users/provider/database.rs:49`), an in-memory one, and a fail-closed default; the sync `UserLookup` remains for guard reads. The database provider is exercised on SQLite only. |
| `Illuminate\Contracts\Auth\Factory` | `rustasea-auth::AuthManager` | **Adopted** | Named guard registration + `Auth::extend` real. |
| `Illuminate\Contracts\Auth\Access\Gate` | `rustasea-router::AuthorizeRegistry` | **Partial** | Record-level authorization enforced at dispatch: `#[authorize]` metadata resolves through `Router::authorize_meta` (`crates/rustasea-router/src/authorize.rs:216`) and `apply_authorize` fails closed on an unregistered resource (`crates/rustasea-router/src/dispatch.rs:209`). A general-purpose ability `Gate` (named abilities with `before`/`after` hooks) lives in `rustasea-auth` (`crates/rustasea-auth/src/gate/mod.rs:176`). |
| `Illuminate\Contracts\Auth\Access\Authorizable` | `rustasea-router::{AuthorizeRegistry, AuthorizeResource}` | **Partial** | `#[authorize]` metadata is enforced at runtime (`crates/rustasea-router/src/authorize.rs:216`, `dispatch.rs:209`); no `Authorizable` trait on models yet. |
| `Illuminate\Contracts\Auth\CanResetPassword` | — | **Planned** | No password-reset broker. |
| `Illuminate\Contracts\Auth\MustVerifyEmail` | `rustasea-auth::EmailVerification` | **Partial** | Verification trait + memory impl real; the runnable app delivers the verification mail through `rustasea-mail` (`crates/rustasea-app/src/routes/auth/verification.rs:165`). |
| `Illuminate\Contracts\Auth\PasswordBroker` | `rustasea-auth::PasswordResetStore` | **Partial** | Token-store seam with hashed-at-rest tokens, an in-memory store, and a fail-closed default (`crates/rustasea-auth/src/password_reset.rs:73`, `:135`, `:174`); no database-backed token table yet. |
| `Illuminate\Auth\GuardHelpers` (trait) | `rustasea-auth::Guard` default methods | **Partial** | Shared guard defaults; surface smaller. |
| `Illuminate\Contracts\Validation\Validator` | `rustasea-validation::Rules` | **Partial** | Rule engine real; contract breadth smaller. |
| `Illuminate\Contracts\Validation\ValidatesWhenResolved` | `rustasea-validation::FormRequest` | **Adopted** | Form-request validation-on-resolve implemented. |
| `Illuminate\Contracts\Validation\Rule` | `rustasea-validation::Rules` | **Partial** | Rule registration real; not a per-rule trait object. |
| `Illuminate\Contracts\Validation\DataAwareRule` / `ValidatorAwareRule` | `rustasea-validation::Rules` | **Partial** | Data-aware validation exists; aware-rule contracts folded into `Rules`. |
| `Illuminate\Validation\Concerns\ValidatesAttributes` (trait) | `rustasea-validation::rules` | **Partial** | Core rules implemented; full Laravel rule catalogue not ported. |
| `Illuminate\Contracts\Session\Session` | `rustasea-auth::SessionPolicy` | **Partial** | Policy/hardening real; store-backed sessions through `DatabaseSessionStore` (`crates/rustasea-auth/src/session/database.rs:74`). |
| `google/auth` (`Google\Auth\Credentials\ServiceAccountCredentials`) | `rustasea-google` (`ServiceAccount`, `GoogleAuthClient`, `AssertionClaims`, `AccessToken`) | **Adopted** | Service-account JSON parse/validate, RS256 JWT assertion, token exchange + cached `AccessToken` (single-flight, 80% lifetime refresh); `GoogleCredentials` block in `config/services.toml`, feature `google` (ADOPT-026). |

### Support / Console / Filesystem / Advanced

| Laravel interface/trait | RustaSea equivalent | Status | Rationale |
|---|---|---|---|
| `Illuminate\Support\Enumerable` | `std::iter` / `Vec` | **N-A** | Iteration is idiomatic Rust; no `Enumerable` interface. |
| `Illuminate\Support\Traits\Macroable` | — | **N-A** | Rust has no runtime method injection; proc-macros cover the use case. |
| `Illuminate\Support\Traits\Conditionable` | — | **N-A** | Conditional chaining is expressed with combinators/`if`. |
| `Illuminate\Support\Traits\Tappable` | — | **N-A** | `inspect`/`tap` helpers are trivial in Rust and not a shared trait. |
| `Illuminate\Console\Command` | `rustasea-cli::Command` | **Adopted** | Command trait + registry real. |
| `Illuminate\Contracts\Console\Kernel` | `rustasea-cli::Artisan` | **Partial** | Console entrypoint real; full kernel contract thinner. |
| `Illuminate\Contracts\Console\PromptsForMissingInput` | `rustasea-cli::prompt` | **Partial** | Prompting helpers real; contract not formalized. |
| `Illuminate\Console\Attributes\Usage` / `Help` / `Hidden` | `rustasea-macros::#[usage]` / `#[help]` / `#[hidden]` | **Adopted** | Attribute surface present and consumed by `artisan list`. |
| `Illuminate\Console\Scheduling\ManagesFrequencies` (trait) | `rustasea-schedule::ScheduleBuilder` | **Partial** | Frequency builder real; full cron grammar thinner. |
| `Illuminate\Contracts\Filesystem\Filesystem` | `rustasea-storage::Storage` | **Adopted** | `get`/`put`/`delete` storage trait implemented. |
| `Illuminate\Contracts\Filesystem\Factory` | `rustasea-storage::StorageManager` | **Partial** | Manager + disks real; driver matrix smaller. |
| `Illuminate\Contracts\Filesystem\Cloud` | `rustasea-storage::ObjectDisk` | **Partial** | `object_store`-backed disk real; visibility/temporary-URL surface thinner. |
| `Illuminate\Filesystem\FilesystemAdapter` | `rustasea-storage::{LocalDisk, ObjectDisk}` | **Partial** | Disk adapters real; adapter method breadth smaller. |
| `league/flysystem-sftp-v3` | `rustasea-storage::SftpDisk` (feature `sftp`) | **Partial** | Pure-Rust `russh` + `russh-sftp` disk: `put`/`get`/`exists`/`delete` + `list`, path confinement, SHA-256 host-key pin, bounded reconnect + retry (ADOPT-025); driver behind the `sftp` feature. |
| `nwidart/laravel-modules` (+ `wikimedia/composer-merge-plugin`) | `rustasea-modules` (`Module`, `ModuleRegistry`, `ModuleManifest`) + `make:module` / `module:list` / `module:enable` / `module:disable` | **Adopted** | `Module` trait plus a deterministic registry that mounts the enabled modules' routes, providers, and migrations; `[modules]` enabled/disabled manifest table; `make:module` generates a workspace crate and `cargo rustasea new --modular` wires `modules/*` as members (ADOPT-027). Composer auto-discovery is replaced by the Cargo workspace member glob. |
| `Illuminate\Contracts\Broadcasting\ShouldBroadcast` | `rustasea-broadcast::ShouldBroadcast` | **Adopted** | Broadcast marker implemented. |
| `Illuminate\Contracts\Broadcasting\Broadcaster` | `rustasea-broadcast::Broadcaster` (`BroadcastHub`, `PusherBroadcaster`, `RedisBroadcaster`) | **Partial** | In-process hub + Pusher HTTP + Redis Pub/Sub broadcasters real (ADOPT-022); Ably absent. |
| `Illuminate\Contracts\Broadcasting\Factory` | `rustasea-broadcast::BroadcastManager` | **Partial** | Named connections + `[broadcasting]` config + env bridge real (ADOPT-022); thinner than Laravel's factory. |
| `Illuminate\Foundation\Testing\TestCase` (class) | `rustasea-testing::TestCase` | **Partial** | Base test case real; refresh/DB traits absent. |
| `Illuminate\Foundation\Testing\RefreshDatabase` (trait) | — | **Planned** | No DB refresh/transaction test trait. |
| `Illuminate\Foundation\Testing\WithFaker` (trait) | `rustasea-testing::faker::Faker` | **Partial** | Locale-aware `Faker` facade (`fake` 5.1) behind the `faker` feature: seeded deterministic generation, 14 locales, `unique_*` helpers; the trait-style mixin is not reproduced (ADOPT-012; `crates/rustasea-testing/src/faker/`). |

## 7. Verification

**Spot-check (≥10 names) against the fetched references and the raw `doc-index.html`:**

| # | Name | Source page | Result |
|---|---|---|---|
| 1 | `Illuminate\Container\Container` | namespaces + classes + doc-index | present |
| 2 | `Illuminate\Contracts\Container\Container` | interfaces | present |
| 3 | `Illuminate\Contracts\Support\MessageBag` | interfaces | present |
| 4 | `Illuminate\Contracts\Database\Eloquent\Builder` | interfaces | present |
| 5 | `Illuminate\Contracts\Queue\Queue` | interfaces | present |
| 6 | `Illuminate\Contracts\Auth\Guard` | interfaces | present |
| 7 | `Illuminate\Contracts\Validation\ValidatesWhenResolved` | interfaces | present |
| 8 | `Illuminate\Contracts\Routing\Registrar` | interfaces | present |
| 9 | `Illuminate\Contracts\Filesystem\Filesystem` | interfaces | present |
| 10 | `Illuminate\Database\Eloquent\SoftDeletes` | traits | present |
| 11 | `Illuminate\Support\Traits\Macroable` | traits | present |
| 12 | `Illuminate\Console\Scheduling\ManagesFrequencies` | traits | present |
| 13 | `Illuminate\Routing\Contracts\ControllerDispatcher` | interfaces | present |
| 14 | `Illuminate\Console\Attributes\Usage` | classes | present |
| 15 | `Illuminate\Contracts\Broadcasting\ShouldBroadcast` | interfaces | present |

**RustaSea spot-check (source tree):** `rustasea-foundation::Container` (`crates/rustasea-foundation/src/lib.rs:35`), `rustasea-orm::Model` (`crates/rustasea-orm/src/model.rs:107`), `rustasea-cache::Store` (`crates/rustasea-cache/src/store.rs:17`), `rustasea-queue::QueueDriver` (`crates/rustasea-queue/src/driver.rs:29`), `rustasea-events::Dispatcher` (`crates/rustasea-events/src/dispatcher.rs:90`), `rustasea-auth::Guard` (`crates/rustasea-auth/src/guard.rs:120`), `rustasea-validation::FormRequest` (`crates/rustasea-validation/src/form_request.rs`), `rustasea-router::Router` (`crates/rustasea-router/src/router.rs:15`), `rustasea-http::HttpClient` (`crates/rustasea-http/src/lib.rs:237`), `rustasea-storage::Storage` (`crates/rustasea-storage/src/storage.rs:13`), `rustasea-jsonapi::JsonApiResource` (`crates/rustasea-jsonapi/src/resource.rs:83`), `rustasea-broadcast::ShouldBroadcast` (`crates/rustasea-broadcast/src/lib.rs:36`).

`doc-index.html` note: the live page is 8.7 MB and exceeded the fetch tool's 5 MB response cap, so it was retrieved as a raw document and grepped for each mapped name (all returned non-zero matches, except `Illuminate\Hashing\Hasher`, which is not a class in this release — the contract is `Illuminate\Contracts\Hashing\Hasher`, used above).

## 8. Gaps & Recommended Adoption Order (M0–M6)

The dominant pattern: **RustaSea already has the shape of most Laravel surfaces (a trait or type), but the execution path behind them is frequently a stub.** Parity work is therefore mostly *finishing* existing surfaces rather than inventing new ones.

| Order | Milestone | Gap to close | Target Laravel surface |
|---|---|---|---|
| 1 | **M0** Bootstrap & Core | Contextual bindings + auto-construction (the provider DAG, the registries, and database/cache/queue bindings at boot landed) | `Contracts\Container\ContextualBindingBuilder`, `SelfBuilding`, `Contracts\Foundation\Application` |
| 2 | **M1** Routing & HTTP | Add URL generation + implicit binding (`route:list` introspection is live; idle timeout now enforced) | `Contracts\Routing\Registrar`, `UrlGenerator`, `UrlRoutable`, `Contracts\Http\Kernel` |
| 3 | **M2** ORM & Database | Attribute casts; reflect live DB column types in `show:model` (source-level `show:model` introspection landed) | `CastsAttributes`, `Castable` |
| 4 | **M3** Auth, Middleware & Validation | Database-backed password-reset token store; database user provider wiring in the scaffold templates (general-purpose `Gate`, the database session store, the database user provider, and runtime `#[authorize]` enforcement landed) | `Contracts\Auth\Access\Gate`, `Authorizable`, `StatefulGuard`, `PasswordBroker` |
| 5 | **M4** Queue, Cache, Scheduling & Events | Scheduler dispatch through the queue and shared pause state; after-response event flush; cache/queue factory depth; lock owner/force-release surface (`ShouldBeUnique` landed) | `Contracts\Queue\Factory`, `ShouldBeUnique`, `Contracts\Cache\Lock` |
| 6 | **M5** DX, CLI & Testing | `make:middleware`/`make:request`, `artisan new`, real cycle detection; DB refresh test traits | `Contracts\Console\Kernel`, `Foundation\Testing\RefreshDatabase`, `WithFaker` |
| 7 | **M6** Advanced | An agent loop that calls an `AiProvider`; provider-native Gemini/Bedrock adapters; encryption surface (real HTTP AI adapters, the view/template engine, translation, and mail landed) | `rustasea-ai` adapters; `Illuminate\Encryption`, `Translation`, `View`, `Mail` analogues |

> The ordering above mirrors the P0–P5 gap program in [`docs/milestones.md`](milestones.md): foundation unblockers first (M0–M2), then security/async correctness (M3–M4), then DX and advanced surfaces (M5–M6).

## 9. Related Documents

- [`README.md`](../README.md) — project goal, milestones, and crate layout.
- [`docs/laravel-13-research.md`](laravel-13-research.md) — Laravel 13 feature research.
- [`docs/milestones.md`](milestones.md) — evidence-backed M0–M6 status.
