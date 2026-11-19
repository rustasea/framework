# RustaSea Milestone Status

> **Last updated:** 2026-11-19
> **Scope:** Authoritative done / partial / missing status for milestones **M0–M6**, plus the **gap-closure program** progress (P0–P5).

## Purpose

`README.md` defines the goal, scope, deliverables, and success criteria for each
milestone, but it does not track what is actually implemented. This document is
the single source of truth for **milestone status**, grounded in the current
source tree. Every status claim cites a real `path:line` and/or a task ID so
it can be re-verified.

## Status Legend

| Status | Meaning |
|---|---|
| **Done** | All deliverables for the milestone are implemented and reachable from real code paths. |
| **Partial** | A meaningful subset is implemented and usable; some deliverables are stubs, placeholders, or unwired. |
| **Missing** | No implementation beyond contracts/placeholders, or the deliverable does not exist. |
| **N/A** | Not applicable at this stage (e.g. a milestone explicitly deferred). |

A milestone is **Partial** when its core surface exists but at least one named
deliverable or success criterion is not yet met. The "Partial" list on each
section records the reason.

## Sources

- Current source tree at the repository root (workspace `Cargo.toml:2` — `members = ["crates/*", "xtask"]`), read directly to verify every claim below.
- P0 gap-closure work: `GAP-001` (sqlx pool, async execution, transactions), `GAP-002` (router → controller dispatch), and `GAP-003` (runtime consumer for the declarative attributes).
- Gap-closure program completions: `GAP-004` (Redis cache store), `GAP-005`–`GAP-006` (queue drivers, worker, failed jobs), `GAP-007` (session guard), `GAP-008`–`GAP-009` (async listeners, metrics), `GAP-010`–`GAP-013` (migrations, CRUD, pgvector, eager loading), `GAP-014`–`GAP-015` (AI HTTP providers, AI queueing + MCP), `GAP-016`–`GAP-018` (CLI generators, testcontainers, xtask), `GAP-019`–`GAP-020` (config/DAG/bootstrap, templating + mail + storage facade), `GAP-021` (documentation sync), `GAP-022` (crate inventory), `GAP-023` (this route:list + attribute-consumption reconciliation).
- Commit-reference policy: [`docs/documentation-conventions.md`](documentation-conventions.md) — milestones, task IDs, and `path:line` only; no raw commit SHAs.
- Task registry: `DOC-001`, `DOC-002`, `DOC-ROOT`, `GAP-ROOT`, `GAP-P0`…`GAP-P5`.
- Prior research: [`docs/laravel-13-research.md`](laravel-13-research.md).
- Workspace inventory: **42 crates under `crates/` + `xtask` (43 workspace packages total)**, verified 2026-09-17 with `cargo metadata --format-version 1 --no-deps` and per `Cargo.toml:2` (`members = ["crates/*", "xtask"]`). The [canonical crate inventory](../.agents/documents/application/modules/manifest.md#canonical-crate-inventory-source-of-truth) enumerates all 43 members by milestone and adoption layer; the workspace `members` glob is the mechanical source of truth. Reconciled under `GAP-022`.

---

## Summary Matrix

| Milestone | Goal | Status | Headline evidence |
|---|---|---|---|
| **M0** | Bootstrap & Core | **Partial** | Container, provider DAG, and config auto-discovery real (`crates/rustasea-foundation/src/lib.rs:275`, `:296`; `crates/rustasea-config/src/lib.rs:89`); bootstrap registries populated (`bootstrap/providers.rs:35`, `bootstrap/commands.rs:12`) ; boot mounts the config loader (`crates/rustasea-foundation/src/lib.rs:309`) and the app's `AppServiceProvider` binds environment-derived defaults (`crates/rustasea-app/src/bootstrap/providers.rs:124`); remaining: no database/cache/queue container bindings at boot |
| **M1** | Routing & HTTP | **Partial** | Router DSL + controller dispatch real (`crates/rustasea-router/src/dispatch.rs:23`, `:62`); `route:list` renders the live 6-column table published at boot via `RouteSource` (`crates/rustasea-cli/src/routes.rs:19`, `:33`; `crates/rustasea-app/src/bootstrap/app.rs:57`; `crates/rustasea-cli/src/commands/inspect.rs:44`); `show:model` now parses `app/models/{snake}.rs` with `syn` for source-level metadata (`crates/rustasea-cli/src/model_inspect.rs:123`, `:217`; `crates/rustasea-cli/src/commands/inspect.rs:129`) and the HTTP client idle (inter-chunk) timeout is now enforced for streamed bodies (`crates/rustasea-http/src/idle.rs:23`, wired in `crates/rustasea-http/src/lib.rs:448`) |
| **M2** | ORM & Database | **Done** | Real sqlx pool + async execution (`crates/rustasea-orm/src/db.rs:24`, `db/exec.rs:72`); model CRUD (`model_ops.rs:28`), transactions (`tx.rs:67`), custom `Migrator` (`migration.rs:131`, `:189`), eager loading (`eager.rs:59`), pgvector (`vector.rs:100`); `raw`/`raw_sql` remain display-only fragments (`execution.rs:239`, `:247`) |
| **M3** | Auth, Middleware & Validation | **Partial** | JWT/CSRF/throttle/validation real; session guard now real via `tower-sessions` (`crates/rustasea-auth/src/session.rs:99`, `impl Guard` `:291`, login `:296`, parse `:352`, refresh `:380`, logout `:408`); declarative attributes are consumed at runtime through `MiddlewareRegistry` (`crates/rustasea-router/src/metadata.rs:237`, `dispatch.rs:109`) and `AuthorizeRegistry` (`crates/rustasea-router/src/authorize.rs:216`, `dispatch.rs:135`); store-backed options are real: `DatabaseSessionStore` (`crates/rustasea-auth/src/session/database.rs:74`) and `DatabaseUserProvider` (`crates/rustasea-auth/src/users/provider/database.rs:49`); remaining: scaffolded apps do not wire the database user provider yet (see section) |
| **M4** | Queue, Cache, Scheduling & Events | **Partial** | Database/Redis queue drivers + worker + persistent failed jobs real (`crates/rustasea-queue/src/driver/database.rs:36`, `driver/worker.rs:75`); `queue:work`/`queue:failed`/`queue:retry` CLI real (`crates/rustasea-cli/src/commands/queue.rs:18`, `ops.rs:21`, `:72`); Redis cache store real behind the `redis` feature (`crates/rustasea-cache/src/redis.rs:155`); async listeners enqueue (`crates/rustasea-events/src/dispatcher.rs:82`); declarative job attributes bind through `JobPolicy` at registration (`crates/rustasea-queue/src/policy.rs:21`, `driver/worker.rs:66`); failed `sync` dispatches are dead-lettered through the configured default queue connection (`crates/rustasea-queue/src/driver.rs:192`); remaining: the scheduler cannot dispatch a due command (`crates/rustasea-schedule/src/scheduler.rs:146`), schedule pause state is per-process, and after-response events are never flushed (`crates/rustasea-events/src/dispatcher.rs:254`) |
| **M5** | DX, CLI & Testing | **Partial** | CLI + 17 `make:*` generators real (canonical `enum Kind`: `crates/rustasea-cli/src/generators/mod.rs:19-54`; registrations `crates/rustasea-cli/src/commands/mod.rs:17-31`); `cargo rustasea new --variant` real (`crates/rustasea/src/bin/cargo-rustasea.rs:46`); real `xtask` cycle detection (`xtask/src/cycles.rs:32`) and `xtask migrate` (`xtask/src/main.rs:37`); testcontainers-backed `PostgresTestDb` (`crates/rustasea-testing/src/fixtures.rs:49`) with a Docker-gated suite (`crates/rustasea/tests/feature/`); remaining: integration tests are opt-in and generated controllers still need manual route registration (`crates/rustasea-cli/src/generators/kinds/controller.rs:35`) |
| **M6** | Advanced (Broadcast, Search, FS, AI) | **Partial** | Broadcast WS/SSE, `object_store` storage + `config/storage.toml` facade, JSON:API, `rustasea-mail`, `rustasea-view`/`rustasea-inertia`/`rustasea-livewire`/`rustasea-scaffold`, real AI HTTP providers (`crates/rustasea-ai/src/providers/mod.rs:27`), AI queueing (`crates/rustasea-ai/src/queue.rs:41`), MCP client (`crates/rustasea-ai/src/mcp/client.rs:40`), and feature-gated pgvector (`crates/rustasea-search/src/pgvector.rs:24`) all real; Gemini/Bedrock served through their OpenAI-compatible endpoints (`crates/rustasea-ai/src/providers/client.rs:225`, `:382`); remaining: the agent runtime does not call a provider (`crates/rustasea-ai/src/agent.rs:247`), provider-native Gemini/Bedrock APIs, and `InProcessProvider` kept for deterministic tests |

---

## M0 — Bootstrap & Core

**Goal recap:** Bootable application skeleton with typed config, service
container, provider lifecycle, and graceful shutdown.

**Status: Partial** — the application boots, resolves container bindings, orders
providers by a real dependency DAG, loads all `config/*.toml`, and shuts down
gracefully. Boot mounts the config loader in the container and the runnable
app's `AppServiceProvider` binds environment-derived defaults. Remaining: no
database, cache, or queue bindings are installed in the container at boot.

**Done**
- Service container with `bind` / `singleton` / `instance` / `get` — `crates/rustasea-foundation/src/lib.rs:103`, `:114`, `:125`, `:136`.
- `Application::boot` runs register → boot with a real topological sort — `crates/rustasea-foundation/src/lib.rs:275`, `:296`.
- Provider dependency declaration + typed cycle detection — `crates/rustasea-foundation/src/lib.rs:27` (`ServiceProvider::dependencies`), `:187` (`BootError::DependencyCycle`), `:311`/`:338` (cycle extraction); tests `crates/rustasea-foundation/tests/boot_dag.rs`.
- Graceful shutdown on `SIGTERM`/`SIGINT` — `crates/rustasea-foundation/src/lib.rs:426`, `:437-452`.
- Config loader auto-discovers all `config/*.toml` with TOML + `.env` overlay (`__` separator, TOML only) — `crates/rustasea-config/src/lib.rs:32`, `:40`, `:57`, `:89`.
- Populated bootstrap registries — `bootstrap/providers.rs:35` (provider vec), `bootstrap/commands.rs:12` (`register_default()` loads the CLI command registry); mirror in `crates/rustasea-app/src/bootstrap/{providers.rs:33,commands.rs:13}`.
- Project scaffolder `cargo rustasea new <app> --variant {blade|react|vue|svelte|livewire}` — `crates/rustasea/src/bin/cargo-rustasea.rs:46`, `:64-74`; starter kits in `crates/rustasea-scaffold/src/variant.rs:13`.
- Runnable app boots, registers routes, serves real handlers, and shuts down — `crates/rustasea-app/src/main.rs` (`configure()`, `app.shutdown()`).

**Partial (reason)**
- The runnable app's `AppServiceProvider` binds only environment-derived defaults (environment name, password policy, destructive-command guard) — `crates/rustasea-app/src/bootstrap/providers.rs:124`; `AuthServiceProvider` installs the session guard (`:173`).
- The config loader is mounted at boot (`crates/rustasea-foundation/src/lib.rs:309`, read back through `Application::config` `:299`), but no database pool, cache manager, or queue driver is bound in the container; features that need a pool connect on their own (the queue dashboard, and the database user provider — `crates/rustasea-app/src/bootstrap/auth.rs:333`).
- `graceful` waits for `SIGTERM`/`SIGINT` but ignores its timeout argument (`crates/rustasea-foundation/src/shutdown.rs:6`); the only enforced drain is axum's graceful shutdown, which has no timeout.
- The root `bootstrap/` directory is an uncompiled skeleton mirror; its `AppServiceProvider` is a no-op (`bootstrap/providers.rs:17`). The compiled bootstrap is `crates/rustasea-app/src/bootstrap/`.

**Missing**
- Database, cache, and queue bindings installed in the container at boot.

**Evidence:** `crates/rustasea-foundation/src/lib.rs:27-452`; `crates/rustasea-config/src/lib.rs:32-89`; `bootstrap/{app.rs,providers.rs,commands.rs}`; `crates/rustasea-app/src/main.rs`; tasks `GAP-001`, `GAP-019`.

**Next actions**
- Bind the database pool, cache manager, and queue drivers in the container at boot.
- Remove or regenerate the uncompiled root `bootstrap/` mirror.

---

## M1 — Routing & HTTP

**Goal recap:** Expressive HTTP layer with routing, middleware, request/response
ergonomics, and an HTTP client.

**Status: Partial**: the router DSL, real controller dispatch (GAP-002), and the
HTTP client `throw` semantics are implemented, and `route:list` now renders the
live application route table. `show:model` now performs real source-level model
introspection. The HTTP client idle (inter-chunk) timeout is enforced for
streamed bodies (GAP-030). Remaining: live DB column types are not yet
reflected.

**Done**
- Router DSL: `get`/`post`/`put`/`delete`/`patch`/`options`/`any`, `group`, prefix/name/domain/resource — `crates/rustasea-router/src/router.rs:46` (`prefix`), `:61` (`domain`), `:73` (`get`), `:288` (`group`), `:317` (`resource`).
- Controller dispatch to real handlers (`GAP-002`) — `crates/rustasea-router/src/dispatch.rs:23` (`into_axum_router`), `:62` (`resolve`).
- `#[route]` metadata consumed at registration — `crates/rustasea-router/src/router.rs:218` (`route_meta`); route table introspection surface — `:358` (`get_routes`).
- `route:list` renders the live 6-column table (Method, URI, Name, Action, Middleware, Binding). The application publishes a `RouteSource` closure at boot (`crates/rustasea-app/src/bootstrap/app.rs:57`), stored process-wide (`crates/rustasea-cli/src/routes.rs:19` `RouteSource`, `:33` `set_route_source`, `:52` `routes`) and read back by the command (`crates/rustasea-cli/src/commands/inspect.rs:44`, rendered at `:49-67`). Covered by `crates/rustasea-cli/src/commands/inspect.rs:182-274` and `crates/rustasea-app/src/bootstrap/app.rs:347-356`.
- `show:model` introspects `app/models/{snake}.rs` with `syn`: the struct name, resolved table (`#[model(table)]` override, hand-written `table_name()`, else `snake_plural`), attributes (name/type/nullable), declared casts, soft-delete/timestamps flags, and best-effort relations from the `relations()` body; `--json` emits the `model-inspector` contract shape. Both the `#[derive(Model)]` style and the hand-written `impl Model` emitted by `make:model` / the app scaffold are recognised. Parser in `crates/rustasea-cli/src/model_inspect.rs:123` (`inspect_source`), `:217` (`inspect_model`), hand-written `impl` metadata in `crates/rustasea-cli/src/model_inspect/impl_model.rs:37` (`model_impl_target`), `:55` (`parse_impl_model`), relation-token scanning in `crates/rustasea-cli/src/model_inspect/relations.rs:14` (`scan_tokens`); command in `crates/rustasea-cli/src/commands/inspect.rs:129` (`run`), `:152` (`render_model`), `:164` (`render_human`).
- HTTP client `throw` / `try_throw` callbacks and typed `HttpError` — `crates/rustasea-http/src/lib.rs:286`, `:298`, `:207`.
- HTTP client idle (inter-chunk) timeout enforced for streamed bodies: when `idle_timeout` is set, the response body is drained chunk by chunk under a per-chunk `tokio::time::timeout`, aborting with `HttpError::Timeout { kind: TimeoutKind::Idle }` on a stall longer than the budget; the plain path returns the response untouched (`crates/rustasea-http/src/idle.rs:23`, `buffer_with_idle_timeout`), wired in `crates/rustasea-http/src/lib.rs:448`; tests in `crates/rustasea-http/tests/idle_timeout.rs`.
- Runnable app serves real handlers from `routes/web.rs` (welcome page rendered through `rustasea::view::MinijinjaEngine`).

**Partial (reason)**
- `show:model` reports declared source metadata only; live DB column types/constraints are not queried (the console has no pool handle).
- Domain routes are ordered ahead of non-domain routes (`crates/rustasea-router/src/router.rs:397`) but the request `Host` is never matched: the first route on a method+path wins (`crates/rustasea-router/src/dispatch.rs:57`), so a domain route answers for every host and a catch-all on the same path is unreachable.

**Missing**
- Live database column introspection for `show:model` (source metadata is implemented).

**Evidence:** `crates/rustasea-router/src/router.rs:46-358`; `crates/rustasea-router/src/dispatch.rs:23-83`; `crates/rustasea-http/src/lib.rs:241-379`; `crates/rustasea-http/src/idle.rs:23-60`; `crates/rustasea-http/tests/idle_timeout.rs`; `crates/rustasea-cli/src/routes.rs:19-57`; `crates/rustasea-cli/src/commands/inspect.rs:44-67`, `:129-176`; `crates/rustasea-cli/src/model_inspect.rs:123-231`; `crates/rustasea-cli/src/model_inspect/impl_model.rs:37-88`; `crates/rustasea-cli/src/model_inspect/relations.rs:14-87`; `crates/rustasea-app/src/bootstrap/app.rs:57`; task `GAP-002` (completed), `GAP-029` (completed), `GAP-030` (completed).

**Next actions**
- Optionally add live DB column introspection to `show:model` when a pool is available.

---

## M2 — ORM & Database

**Goal recap:** Fluent, type-safe database layer with query builder,
migrations, seeders, factories, and vector support.

**Status: Done** — the fluent SQL builder, real sqlx pool, async query
execution, model CRUD, transactions, migrations/seeders/factories, eager
loading, and pgvector support all execute through the runtime `sqlx` API. The
ORM is sqlx-based end to end (no `sea-orm`, no `sqlx::migrate!` — migrations use
the crate's own `Migrator`); `raw`/`raw_sql` remain display-only fragments.
Tracked by `GAP-001`, `GAP-010`–`GAP-013`.

**Done**
- Fluent SQL builder — `crates/rustasea-orm/src/builder.rs`, `crates/rustasea-orm/src/clause.rs`; async execution `crates/rustasea-orm/src/builder/exec.rs:105` (`get`), `:115` (`first`), `:148` (`paginate`).
- Real sqlx pool + connection foundation — `crates/rustasea-orm/src/db.rs:24` (`DbPool`), `:43` (`connect`), `:70` (`ping`), `:91` (`close`); driver dispatch — `crates/rustasea-orm/src/db/exec.rs:72` (`fetch_json`), `:97` (`execute_bind`), `:118` (`execute_script`).
- Async model operations — `crates/rustasea-orm/src/model_ops.rs:28` (`create`), `:47` (`save` upsert), `:64` (`update`), `:78` (`delete`), `:87` (`force_delete`), `:100` (`soft_delete`), `:118` (`refresh`), `:133` (`first_for_update`).
- Real transactions + executor dispatch — `crates/rustasea-orm/src/tx.rs:67` (`begin`), `crates/rustasea-orm/src/execution.rs:325` (`transaction`).
- Real migrations, seeders, and factory state — custom `Migrator` at `crates/rustasea-orm/src/migration.rs:131`, `:189` (`run`), `:240` (`rollback`), `:279` (`fresh`), `:288` (`seed`); `crates/rustasea-orm/src/factory.rs`.
- Eager loading + relation serde round-trip — `crates/rustasea-orm/src/eager.rs:18` (`EagerPlan`), `:59` (`eager_load`), `crates/rustasea-orm/src/relations.rs:22`.
- JSON-embedded relations (ADOPT-020, `staudenmeir/eloquent-json-relations` parity) — `Relation::belongs_to_json`/`has_many_json`/`belongs_to_many_json` with a `JsonSpec` (`crates/rustasea-orm/src/relation.rs`), dialect predicates `where_json_in`/`where_json_contains_any` (`crates/rustasea-orm/src/builder/json.rs`, `crates/rustasea-orm/src/types.rs`), batched eager loaders (`crates/rustasea-orm/src/eager/json.rs`), and a `Blueprint::json_index` expression-index helper (`crates/rustasea-orm/src/schema/emit.rs`).
- pgvector support (native bind/encode behind the `vector` feature) — `crates/rustasea-orm/src/vector.rs:100` (`to_vector_literal`), `:141` (`has_extension_sql`), `:149` (`vector_param`); feature in `crates/rustasea-orm/Cargo.toml`.
- Populated `config/database.toml` (driver/url/pool settings).

**Partial (reason)**
- `raw` / `raw_sql` emit statement fragments for display only — `crates/rustasea-orm/src/execution.rs:239`, `:247`.
- `insert_or_ignore_returning` returns SQL text that nothing executes (`crates/rustasea-orm/src/model.rs:425`); there is no `find` shorthand (use `query().where_key(id).first(pool)`); `OrmError::MissingTable` is never constructed (`crates/rustasea-orm/src/error.rs:41`); `make:migration` has no `--create` flag.

**Missing**
- Compile-time `query!` macros (intentionally excluded: CI has no `DATABASE_URL`).

**Evidence:** `crates/rustasea-orm/src/db.rs:24-130`; `crates/rustasea-orm/src/db/exec.rs:72-138`; `crates/rustasea-orm/src/model_ops.rs:28-143`; `crates/rustasea-orm/src/tx.rs:67-192`; `crates/rustasea-orm/src/migration.rs:131-296`; `crates/rustasea-orm/src/eager.rs:18-59`; `crates/rustasea-orm/src/vector.rs:100-152`; tasks `GAP-001`, `GAP-010`–`GAP-013`.

**Next actions**
- Extend pgvector index management surfaces (M6).
- Optionally reflect live DB column types in `show:model` (source-level metadata already lands via `GAP-029`).

---

## M3 — Auth, Middleware & Validation

**Goal recap:** Complete auth, authorization, and validation with hardened
security defaults.

**Status: Partial**: JWT, CSRF, throttling, and validation are real, the
session guard is a working `tower-sessions`-backed guard (GAP-007), and the
declarative attributes are consumed at runtime through the router registries.
A database session store and a database-backed user provider are implemented and
selectable. Remaining: both defaults stay in-memory / fail-closed until
configured, and scaffolded apps do not wire the database user provider yet.

**Done**
- JWT guard — `crates/rustasea-auth/src/jwt.rs`.
- Origin-aware CSRF protection (`Sec-Fetch-Site`) — `crates/rustasea-auth/src/csrf.rs`.
- Rate limiter / throttle — `crates/rustasea-auth/src/throttle/`.
- Validation crate (rules, ErrorBag, form requests) — `crates/rustasea-validation/`.
- Guard manager with typed `GuardMismatch` errors — `crates/rustasea-auth/src/guard.rs:197`, `:302`.
- Real session guard over `tower-sessions` (`GAP-007`): `crates/rustasea-auth/src/session.rs:99` (`SessionGuard<S: SessionStore = MemoryStore>`), `:291` (`impl Guard`), `:296` (`login`, fresh session id on login), `:331` (`login_using_id`, opt-in via `:194`), `:352` (`parse`), `:380` (`refresh`, id cycling), `:408` (`logout`, flushes the store), `:427` (`user`), `:442` (`id`); dependency `crates/rustasea-auth/Cargo.toml:20`.
- Declarative attributes consumed at runtime: `#[middleware]` emits a doc-hidden `__RUSTASEA_MIDDLEWARE_<Fn>` spec list consumed by `Router::middleware_meta` (`crates/rustasea-router/src/metadata.rs:237`) and enforced by `apply_middleware` (`crates/rustasea-router/src/dispatch.rs:109`); `#[authorize]` emits `__RUSTASEA_AUTHORIZE_<Fn>` tuples consumed by `Router::authorize_meta` (`crates/rustasea-router/src/authorize.rs:216`) and enforced by `apply_authorize` (`crates/rustasea-router/src/dispatch.rs:135`), a missing/ambiguous resource failing closed with `RouteError::UnknownAuthorization`.
- Role-based access control (ADOPT-001, `spatie/laravel-permission` parity): `crates/rustasea-auth/src/rbac/` (`Role`, `RbacRegistry`, `HasRoles`, `PermissionResolver`): roles hold permissions, permission checks are the union over a user's roles, lookups are cache-backed, and the ability `Gate` consults an optional `PermissionResolver` after its defined abilities (`crates/rustasea-auth/src/rbac/{mod,role,registry,has_roles}.rs`).
- Model-change audit trail (ADOPT-002, `spatie/laravel-activitylog` parity): `crates/rustasea-activitylog` (`ActivityLogger`, `ActivityEvent`): `#[logs_activity]` opts a model in, the ORM write path builds a diff event, and the logger persists it to the `audit_log` table with an optional `batch_uuid` (`crates/rustasea-activitylog/src/{lib,recorder,model}.rs`).
- Authentication log (ADOPT-003, `rappasoft/laravel-authentication-log` parity): `crates/rustasea-authlog` (`AuthenticationLogLogger`, `NewDeviceNotifier`): one row per login/failed/lockout/logout with client IP and `User-Agent`, plus a queued new-device notification on an unknown IP (`crates/rustasea-authlog/src/{lib,recorder,notifier}.rs`).
- Sentry error tracking (ADOPT-004): opt-in `sentry` feature forwarding a request-context middleware (`crates/rustasea-http/src/sentry.rs`) and a logging layer with a `before_send` secret-scrubbing hook (`crates/rustasea-logging/src/sentry/mod.rs`); inert until a DSN is bound (`crates/rustasea/Cargo.toml:128`).

**Partial (reason)**
- The default session store is in-memory (`MemoryStore`). `driver = "database"` in `config/session.toml` selects the persistent `DatabaseSessionStore` (`crates/rustasea-auth/src/session/database.rs:74`), built by `SessionGuard::from_config_database` (`crates/rustasea-auth/src/session.rs:155`); the `redis` driver is rejected at load time.
- The guard's default synchronous lookup is the fail-closed `StaticLookup` (`crates/rustasea-auth/src/session.rs:194`). The database-backed `DatabaseUserProvider` (`crates/rustasea-auth/src/users/provider/database.rs:49`) covers registration, login, password reset, verification, and profile updates over the `users` table; the runnable app installs it at boot when the default guard's provider is ORM-backed and the table exists (`crates/rustasea-app/src/bootstrap/auth.rs:333`, called from `crates/rustasea-app/src/main.rs:32`).
- `DatabaseUserProvider` is exercised on SQLite only (`crates/rustasea-auth/tests/database_user_provider.rs`); it has not been run against Postgres or MySQL.
- `JwtGuard::logout` is a no-op with no revocation or deny-list (`crates/rustasea-auth/src/jwt.rs:267`).
- The `in:` / `in_array:` rule parses each list item as JSON and drops what does not parse (`crates/rustasea-validation/src/rules/support.rs:134`), so a bare-word list such as `in:admin,editor` yields an empty allow-list and rejects every value; numbers and quoted strings work.
- The deserialization allow-list (`SessionPolicy::allow`) is defined but not called; `cache.serializable_classes` is parsed and unused (`crates/rustasea-cache/src/config.rs:191`).

**Missing**
- Database user provider wiring in the scaffold templates (`crates/rustasea-scaffold/src/templates/`), so generated apps get it without hand-wiring.
- A Redis session store.

**Evidence:** `crates/rustasea-auth/src/{jwt,csrf,guard,session}.rs`; `crates/rustasea-auth/src/rbac/`; `crates/rustasea-activitylog/`; `crates/rustasea-authlog/`; `crates/rustasea-http/src/sentry.rs`; `crates/rustasea-logging/src/sentry/`; `crates/rustasea-auth/Cargo.toml:20`; `crates/rustasea-router/src/{metadata.rs:237,authorize.rs:216,dispatch.rs:109,135}`; tasks `GAP-003`, `GAP-007`, `ADOPT-001`–`ADOPT-004`.

**Next actions**
- Wire `DatabaseUserProvider` into the scaffold templates.
- Run the `DatabaseUserProvider` suite against Postgres in the Docker-backed integration tests.

---

## M4 — Queue, Cache, Scheduling & Events

**Goal recap:** Async workloads, caching, scheduling, and event dispatch with
observable queue metrics.

**Status: Partial** — in-memory cache, the sync queue driver, inline events, and
the scheduler exist; the gap-closure program added real `database`/`redis` queue
drivers with a worker loop, persistent failed jobs, `queue:work`/
`queue:failed`/`queue:retry` CLI, a real feature-gated Redis cache store,
queue-backed async listeners, and a runtime consumer for the declarative job
attributes. A failed inline `sync` dispatch is dead-lettered through the
configured default queue connection when its driver is registered. Remaining: the scheduler cannot run a due
command, its pause state is not shared across processes, and events buffered by
`dispatchAfterResponse` are never delivered.

**Done**
- In-memory cache store (lock-guarded map; no `moka` dependency) — `crates/rustasea-cache/src/memory.rs:35`.
- Real feature-gated Redis cache store (`GAP-004`) — `crates/rustasea-cache/src/redis.rs:62` (`RedisStore`), `:155` (full `Store` impl over `deadpool-redis`); feature `crates/rustasea-cache/Cargo.toml:12`; without the feature every op returns a typed `StoreUnavailable` (`:278`).
- Synchronous queue driver — `crates/rustasea-queue/src/driver.rs:115`.
- Real `database` queue driver (`GAP-005`) — `crates/rustasea-queue/src/driver/database.rs:36` (`push`/`pop`/`ack`/`release`/`dead_letter` over a `jobs` table), with DB-backed failed jobs at `:23` (`FAILED_JOBS_TABLE`), `:71` (`failed_jobs`), `:95` (`retry_failed`).
- Real `redis` queue driver (feature-gated) — `crates/rustasea-queue/src/driver/redis.rs:33` (`RedisDriver`), `:43` (`from_url`; disabled when no URL), `:329` (`dead_letter`).
- Queue worker loop + handler registry (`GAP-006`) — `crates/rustasea-queue/src/driver/worker.rs:75` (`run_worker`), `:36` (`register_job`).
- Declarative job policy consumed at runtime: the macros emit `__RUSTASEA_TRIES_<Type>` / `__RUSTASEA_BACKOFF_SECS_<Type>` / `__RUSTASEA_TIMEOUT_SECS_<Type>` consts bound through `JobPolicy` (`crates/rustasea-queue/src/policy.rs:21`, `from_seconds` at `:44`) and registered with `register_job_with_policy` (`crates/rustasea-queue/src/driver/worker.rs:66`); the worker reads the per-job policy and drives retry/backoff/timeout (`crates/rustasea-queue/src/driver/worker.rs:198`).
- Queue migrations for `jobs`/`failed_jobs` — `crates/rustasea-queue/src/migrations.rs:16`, `:48`, `:82`, `:90`; `queue:work` CLI — `crates/rustasea-cli/src/commands/queue.rs:18`; `queue:failed`/`queue:retry` CLI — `crates/rustasea-cli/src/commands/ops.rs:21`, `:72`.
- Inline event dispatch + `dispatchAfterResponse` — `crates/rustasea-events/src/dispatcher.rs:182`; async listeners enqueue a `ListenerJob` through the queue facade (`GAP-008`) — `crates/rustasea-events/src/dispatcher.rs:82-100`, `crates/rustasea-events/src/job.rs:15`.
- Scheduler with pause/resume — `crates/rustasea-schedule/src/lib.rs:20` (`SchedulePaused`/`ScheduleResumed` re-exports).
- Queue dashboard (ADOPT-021, `laravel/horizon` parity): `crates/rustasea-queue-dashboard` (`DashboardConfig`, `sampler`, `QueueMetricsHistory`): `/queue` live depth/age, `queue_metrics` history with retention, failed-job retry/forget, and worker heartbeats; feature `queue-dashboard` (`crates/rustasea-queue-dashboard/src/{lib,sampler}.rs`, `crates/rustasea-app/src/routes/queue_dashboard.rs`).
- Broadcast fan-out (ADOPT-022): Pusher HTTP driver (HMAC-SHA256 request signing + MD5 body digest, `pusher`/`private-`/`presence-` channel auth) and a Redis Pub/Sub driver for cross-process fan-out behind the `pusher`/`redis` features (`crates/rustasea-broadcast/src/`, `crates/rustasea-app/src/routes/broadcasting.rs`).

**Partial (reason)**
- The scheduler cannot dispatch: `dispatch_command` returns `ScheduleError::Dispatch` for every due command because no queue dispatch path is wired (`crates/rustasea-schedule/src/scheduler.rs:146`), so a tick with a due command fails. `schedule:run` runs one tick and exits (`crates/rustasea-cli/src/commands/ops.rs:253`); the 60-second `tick_interval` (`crates/rustasea-schedule/src/scheduler.rs:168`) has no caller.
- Schedule pause state is a process-local `RwLock` (`crates/rustasea-schedule/src/pause.rs:28`), not stored in the cache or database, so `schedule:pause` in one process does not reach another. `on_one_server` records a flag and a `lock_key` (`crates/rustasea-schedule/src/command.rs:46`) but no lock is taken.
- `dispatchAfterResponse` only buffers into `EventSink` (`crates/rustasea-events/src/dispatcher.rs:254`); no non-test code drains it, so listeners for after-response events never run. `Dispatcher::dispatch` returns `EventError::Unhandled` when an event has no listener (`:244`), which also makes `schedule:pause` / `schedule:resume` report an error when nothing listens for `SchedulePaused` / `ScheduleResumed`.
- `Queue::chain` runs its jobs inline on the caller's task and never goes through a routed driver (`crates/rustasea-queue/src/registry.rs:175`).
- A failed `sync` dispatch is dead-lettered through the configured default queue connection, or `database` when the queue config names none, and reaches the `failed_jobs` table only when that connection's registered driver is the database driver (`dead_letter_sync`, `crates/rustasea-queue/src/driver.rs:192`, connection resolved at `:201`; callers `crates/rustasea-queue/src/registry.rs:150`, `:186`). Without one, or when the insert fails, it falls back to the process-local list (`crates/rustasea-queue/src/driver.rs:174`), which does not survive the process.
- `JobAttempted` and `QueueBusy` are defined (`crates/rustasea-events/src/event.rs:32`, `:51`) but nothing emits them.

**Missing**
- A queue dispatch path for due scheduled commands, and a long-running `schedule:run` loop.
- Shared (cache or database) schedule pause state and an `onOneServer` lock.
- A flush of the after-response event buffer from the HTTP layer.

**Evidence:** `crates/rustasea-cache/src/{memory,redis}.rs`; `crates/rustasea-queue/src/driver.rs:115-197`; `crates/rustasea-queue/src/driver/{database,redis,worker}.rs`; `crates/rustasea-queue/src/policy.rs:21-55`; `crates/rustasea-queue/src/migrations.rs`; `crates/rustasea-events/src/dispatcher.rs:82-182`; `crates/rustasea-schedule/src/lib.rs:20`; `crates/rustasea-queue-dashboard/`; `crates/rustasea-broadcast/`; tasks `GAP-004`–`GAP-009`, `ADOPT-021`, `ADOPT-022`.

**Next actions**
- Wire scheduled commands to the queue and run the scheduler tick on an interval.
- Persist schedule pause state and take the `onOneServer` lock through the cache `Lock`.
- Drain `EventSink` after the response is sent.

---

## M5 — DX, CLI & Testing

**Goal recap:** First-class CLI, code generation, and a Laravel-like testing
story.

**Status: Partial** — the CLI and 17 `make:*` generators are real (including
`make:middleware`/`make:request`/`make:action`/`make:module`), `cargo rustasea new
--variant` scaffolds real starter kits (and `--modular` a module-ready workspace),
`xtask` has real cycle detection plus `migrate`, and the testing
stack uses real `testcontainers` with a Docker-gated feature suite. Remaining:
the integration suite is opt-in, and generated controllers still require manual
route registration.

**Done**
- `cargo artisan` CLI with command registry — `crates/rustasea-cli/src/registry.rs`, `crates/rustasea-cli/src/lib.rs`.
- 17 `make:*` generators: controller, middleware, request, model, provider, command, job, event, listener, observer, test, seeder, migration, agent, tool, action, module — registrations `crates/rustasea-cli/src/commands/mod.rs:17-31`; kinds `crates/rustasea-cli/src/generators/kinds/{middleware,request}.rs:17` (`GAP-016`).
- Base-controller convention (TASK-091, TASK-092, TASK-093, Hypervel parity): the framework ships a base `Controller` trait with shared response/redirect helpers (`crates/rustasea-http/src/controller.rs`), the app scaffold emits `app/http/controllers/controller.rs` plus struct controllers implementing it (`crates/rustasea-scaffold/src/templates/app_http.rs`), and `make:controller` now emits base-controller-conforming structs (`crates/rustasea-cli/src/generators/kinds/controller.rs`).
- Scaffold project-structure parity (TASK-094, TASK-095, TASK-096, Hypervel parity): generated apps emit Hypervel-style root hygiene and directory placeholders (`.gitattributes`, `LICENSE`, `public/{robots.txt,favicon.ico}`, `bootstrap/cache/`, the `storage/` subdirectory `.gitignore` set; `crates/rustasea-scaffold/src/templates/core.rs`), broadcasting/CORS config templates (`config/broadcasting.toml`, `config/cors.toml`; `crates/rustasea-scaffold/src/templates/config.rs`), boot-time migration registration in `bootstrap/commands.rs` (the framework command surface + queue migrations via `rustasea::cli::load_default_commands()` plus the app's own nine migrations via `rustasea::orm::register_migration`; `crates/rustasea-scaffold/src/templates/bootstrap.rs`), and a `rustfmt.toml` + minimal `.github/workflows/ci.yml` (`crates/rustasea-scaffold/src/templates/tooling.rs`).
- Modular application support (ADOPT-027) — `crates/rustasea-modules` (`Module` trait + deterministic `ModuleRegistry` mounting the enabled modules' routes/providers/migrations, `ModuleManifest` `[modules]` enabled/disabled table); `make:module` generates `modules/<name>/` (`crates/rustasea-cli/src/generators/kinds/module.rs`); `module:list`/`module:enable`/`module:disable` (`crates/rustasea-cli/src/commands/modules.rs`); `cargo rustasea new --modular` emits the `modules/*` workspace (`crates/rustasea-scaffold/src/templates/mod.rs`); umbrella feature `modules` (`crates/rustasea/Cargo.toml:79`).
- Typed command args/flags and prompt/table helpers.
- Project scaffolder `cargo rustasea new <app> --variant {blade|react|vue|svelte|livewire}` — `crates/rustasea/src/bin/cargo-rustasea.rs:46`, `:64-74`; starter kits `crates/rustasea-scaffold/src/variant.rs:13`; `cargo artisan` remains a legacy alias via `normalize_args` (`:103`).
- Starter-kit variant set and distribution: the five variants are declared in `crates/rustasea-scaffold/src/variant.rs:13` (`StarterKitVariant::ALL`, `:28`; `SUPPORTED` `:37`), and each variant ships as its own repository mirroring the `laravel/<x>-starter-kit` split: `rustasea/rustasea` (blade skeleton), `rustasea/react-starter-kit`, `rustasea/vue-starter-kit`, `rustasea/svelte-starter-kit`, and `rustasea/livewire-starter-kit` (ADR-0002 note "Svelte variant (Sycamore) and starter-kit distribution").
- Real `xtask` cycle detection (`GAP-018`) — `xtask/src/cycles.rs:32` (`run()`, DFS over `cargo metadata`); task dispatch `xtask/src/main.rs:23` (`ci`), `:24` (`fmt`), `:25` (`clippy`), `:36` (`check-cycles`), `:37` (`migrate`).
- `cargo xtask migrate` — `xtask/src/migrate.rs` via `xtask/src/main.rs:37`.
- Real testcontainers-backed fixtures (`GAP-017`) — `crates/rustasea-testing/src/fixtures.rs:49` (`PostgresTestDb`), `:63` (`start`); feature `crates/rustasea-testing/Cargo.toml:10`; integration suite `crates/rustasea/tests/feature/` gated behind the `integration` feature (`crates/rustasea/Cargo.toml:86`) and `#[ignore = "requires docker"]` (`crates/rustasea/tests/feature/route_to_db.rs:81`).
- Action pattern (ADOPT-028) — `crates/rustasea-action` (`Action` trait + HTTP/queue/CLI/event adapters, `make:action` generator); scaffold auth actions demonstrate the pattern (`crates/rustasea-scaffold/src/templates/app_auth.rs`).
- Log viewer (ADOPT-014) — `rustasea-logging::reader` (parse/filter/resolve/tail), `cargo artisan log:show` (`crates/rustasea-cli/src/commands/log_show.rs`: `--level`/`--channel`/`--since`/`--grep`/`--limit`/`--follow`/`--json`), and a dev-only `/_logs` surface behind the `log-viewer` feature (`crates/rustasea-app/src/routes/log_viewer.rs`).
- Browser (WebDriver) e2e harness (ADOPT-029) — `crates/rustasea-testing/src/browser/` (`Browser`, `ServerHandle`, `BrowserError`; feature `browser`, `fantoccini` + `rustls`): Dusk-style `visit`/`fill`/`click`/`select`/`check`/`press`/`wait_for`/`assert_see` helpers, an ephemeral axum server, and failure screenshots. Auto-skips when `WEBDRIVER_URL` is unreachable; live tests are `#[ignore]`d (`crates/rustasea-testing/tests/browser_e2e.rs`); `make:test --browser` emits `tests/browser/<slug>.rs` (`crates/rustasea-cli/src/generators/kinds/test.rs`); umbrella feature `browser` (`crates/rustasea/Cargo.toml`).
- Docker dev stack (ADOPT-007, `laravel/sail` parity): `cargo xtask docker:up`/`docker:down`/`docker:logs` wrap the compose CLI with plugin/standalone detection (`xtask/src/docker.rs`), and `cargo rustasea new` emits a multi-stage `Dockerfile` + `docker-compose.yml` (`crates/rustasea-scaffold/src/templates/docker.rs`).
- Interactive REPL (ADOPT-008, `laravel/tinker` parity): `cargo artisan tinker`: a `rustyline` shell over a booted application (`config`/container/route/command inspection, `help`); piped stdin scripts the session (`crates/rustasea-cli/src/commands/tinker.rs:43`).
- Dev request profiler (ADOPT-009, `barryvdh/laravel-debugbar` parity): `crates/rustasea-debugbar` (`Profiler`, recorders, `from_fn` middleware): per-request SQL/cache/event capture into an in-memory ring buffer with a JSON/HTML surface behind the `debugbar` feature (`crates/rustasea-debugbar/src/{lib,middleware,recorders}.rs`, `crates/rustasea-app/src/routes/debugbar.rs`).
- Dev/prod error renderers (ADOPT-010, `spatie/laravel-ignition` parity): `rustasea-http` application error type with a dev page + prod JSON envelope and panic catching with dev panic-location capture (`crates/rustasea-http/src/{error,panic}.rs`, `crates/rustasea-app/src/routes/errors.rs`).
- OpenAPI generation (ADOPT-011, `dedoc/scramble` parity): `crates/rustasea-openapi` + `cargo artisan openapi:generate`: an OpenAPI 3.1 document built from the live route table (the same registry `route:list` renders) (`crates/rustasea-openapi/src/lib.rs`, `crates/rustasea-cli/src/commands/openapi.rs`).
- Faker (ADOPT-012, `fakerphp/faker` parity): `rustasea-testing::faker` (`Faker`, `unique_*`): seeded deterministic, locale-aware fake data (`fake` 5.1, 14 locales) behind the `faker` feature (`crates/rustasea-testing/src/faker/`).
- Testing fakes (ADOPT-013, `Illuminate\Support\Testing\Fakes` parity): `rustasea-testing::fakes` (`FakeQueue`, `FakeMailer`, `FakeDispatcher`, `FakeCache`): recording doubles that intercept side effects with `assert_*` helpers (`crates/rustasea-testing/src/fakes.rs`, `crates/rustasea-testing/src/fakes/`).
- MCP knowledge server (ADOPT-015, `laravel/boost` parity): `cargo artisan mcp:serve` serves routes, docs, commands, and redacted config over MCP stdio behind the `mcp` feature (`crates/rustasea-cli/src/commands/mcp_serve.rs`).
- Slug generation (ADOPT-016, `cviebrock/eloquent-sluggable` parity): `crates/rustasea-orm/src/sluggable.rs`: Unicode-aware `slugify`, deterministic `-2`/`-3` collision suffixing, `#[sluggable(...)]` derive opt-in, write-path hooks (`crates/rustasea-orm/src/model_ops/slug_hooks.rs`), and `find_by_slug`/`find_by_slug_or_fail` with `{post:slug}` route binding (`crates/rustasea-router/src/binding.rs`).
- Composite-key relations (ADOPT-017, `awobaz/compoships` parity): composite `has_many`/`belongs_to`/`many_to_many` builders and `#[model(primary_key = ["a", "b"])]` models with a full create/update/delete round-trip (`crates/rustasea-orm/src/relation.rs`, `crates/rustasea-orm/src/model_ops/composite.rs`, `crates/rustasea-macros/src/model_primary_key.rs`).
- Cascade soft deletes (ADOPT-018, `spatie/laravel-cascade-soft-deletes` parity): `#[cascade_soft_deletes("posts", ...)]` fans the delete/restore/force-delete out to the named relations in chunks (single level) (`crates/rustasea-orm/src/model_ops/cascade.rs`).
- Model/query result caching (ADOPT-019, `spatie/laravel-model-caching` parity): `QueryBuilder::cache(ttl)`/`cache_forever()` store decoded rows through a process-wide `QueryCacheStore`, with O(1) per-table generation invalidation on write (`crates/rustasea-orm/src/cache.rs`, `crates/rustasea-orm/src/builder/cached.rs`, `crates/rustasea-orm/src/model_ops/cache_hooks.rs`).
- Quality gate (ADOPT-030): toolchain and lint config as the CI gate: `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`, `.cargo/audit.toml`, and the `.cargo/config.toml` alias to the `xtask` binary; `cargo xtask ci` runs fmt then clippy then `deps:check` then the cycle check (`xtask/src/main.rs:58`).
- Workspace dependency management (ADOPT-031): the root `[workspace.dependencies]` table is the single source of truth for shared crate versions, and `cargo xtask deps:check` fails CI when a member manifest pins a managed version inline (`Cargo.toml:16`, `xtask/src/deps.rs:4`).

**Partial (reason)**
- Generated controllers leave route registration manual — `crates/rustasea-cli/src/generators/kinds/controller.rs:35` ("Register routes against these handlers in `routes/web.rs`").
- Generator smoke tests lock wiring only (output path, non-empty source, `--force`); compile/rustfmt cleanliness is exercised separately against an app fixture — `crates/rustasea-cli/tests/make_generators.rs:1-9`.
- Docker-backed integration tests are opt-in: `cargo test -p rustasea --features integration -- --ignored`.

**Missing**
- Generator-to-route-registration automation.

**Evidence:** `crates/rustasea-cli/src/commands/mod.rs:17-31`; `crates/rustasea-cli/src/commands/builtins.rs`; `crates/rustasea/src/bin/cargo-rustasea.rs:46-103`; `xtask/src/{main.rs:23-37,cycles.rs:32,migrate.rs,deps.rs,docker.rs}`; `crates/rustasea-testing/src/fixtures.rs:49`; `crates/rustasea-testing/src/{faker,fakes}/`; `crates/rustasea/tests/feature/`; module registry + CLI gates `crates/rustasea-modules/tests/{registry,manifest}.rs`, `crates/rustasea-cli/tests/{make_module,module_compiles}.rs`, `crates/rustasea-scaffold/tests/modular.rs`; tasks `GAP-016`–`GAP-018`, `ADOPT-007`, `ADOPT-008`–`ADOPT-013`, `ADOPT-015`–`ADOPT-019`, `ADOPT-027`, `ADOPT-029`, `ADOPT-030`, `ADOPT-031`.

**Next actions**
- Automate route registration in generated controllers (or a route-table generator).
- Keep the integration suite green in CI with Docker enabled.

---

## M6 — Advanced (Broadcasting, Search, Filesystem, AI SDK, Real-time)

**Goal recap:** AI-native capabilities and full Laravel parity on advanced
features.

**Status: Partial** — broadcasting (WS/SSE), `object_store`-backed storage with a
TOML config facade, JSON:API, the mail crate, the presentation crates
(view/Inertia/Livewire/scaffold), real HTTP-backed AI providers, AI queueing, and
MCP are all real. Gemini and Bedrock are served through their OpenAI-compatible
endpoints and provider-backed embeddings send a dedicated embedding model.
Remaining: the agent runtime echoes the prompt instead of calling a provider,
the provider-native Gemini/Bedrock APIs are not implemented, and the in-process
AI provider is kept for tests.

**Done**
- Broadcasting: WebSocket (`axum/ws`, feature-gated default) + SSE + `ShouldBroadcast` — `crates/rustasea-broadcast/Cargo.toml:20`, `:22`; `crates/rustasea-broadcast/src/lib.rs:35`.
- Filesystem on real `object_store` (0.14.1) with cloud features — `crates/rustasea-storage/Cargo.toml:10`, `:20-22`; `crates/rustasea-storage/src/manager.rs:56` (`StorageManager`), `:127` (`get` read-through), `:144` (`put`), `:171` (`ObjectDisk`).
- Storage config facade (`GAP-020`) — `config/storage.toml` (`[storage] default = "local"`), `crates/rustasea-storage/src/facade.rs:36` (`StorageFacadeConfig::from_toml`), `:168` (`StorageManager::from_toml`).
- SFTP storage disk (ADOPT-025) — `crates/rustasea-storage/src/sftp/` (pure-Rust `russh`/`russh-sftp`, feature `sftp`): `put`/`get`/`exists`/`delete` + `list`, `SFTP_*` env overlay, path confinement, SHA-256 host-key pin, single bounded reconnect; `DiskDefinition::Sftp` (`crates/rustasea-storage/src/facade.rs:76`) and the facade re-export (`crates/rustasea/Cargo.toml:146`).
- Excel/CSV import-export (ADOPT-023) — `crates/rustasea-excel` (streaming import + validation report, chunked export, queued job, signed links).
- Image manipulation (ADOPT-024) — `crates/rustasea-image` (fluent pipeline, EXIF orient, storage round-trip, queued transforms).
- Google service-account auth (ADOPT-026) — `crates/rustasea-google`: service-account JSON parse/validate, RS256 JWT assertion, token exchange + cached access token (single-flight refresh at 80% lifetime); `GoogleCredentials` block in `config/services.toml` (`crates/rustasea-config/src/services.rs`); umbrella feature `google`.
- JSON:API resources with correct content type — `crates/rustasea-jsonapi/src/wire.rs:7`.
- Mail & notifications (`GAP-020`) — `crates/rustasea-mail/`: `Mailable` (`mailable.rs:11`), `Mailer`/`ArrayMailer`/`LogMailer` (`mailer.rs:12`, `:22`, `:73`), `SmtpMailer` (feature `smtp`, `smtp.rs:14`), `QueuedNotification` dispatched through the queue (`notification.rs:41`); re-exported from the umbrella as `rustasea::mail` (`crates/rustasea/src/lib.rs:156`) with the `mail-smtp` / `mail-templates` features (`crates/rustasea/Cargo.toml:143`, `:146`).
- View layer — `rustasea-view` with askama (default) + minijinja (`runtime-templates`) — `crates/rustasea-view/Cargo.toml:14`, `:17`, `:22`; `crates/rustasea-view/src/askama_engine.rs:35`.
- Inertia protocol + WASM client + framework adapters — `crates/rustasea-inertia/src/page.rs:25` (`Page<T>`), `crates/rustasea-inertia-client/`, `crates/rustasea-inertia-adapters/` (Dioxus for react, Leptos for vue, Sycamore for svelte: `crates/rustasea-inertia-adapters/src/{dioxus,leptos,sycamore}_adapter.rs`; umbrella features `wasm-dioxus`/`wasm-leptos`/`wasm-sycamore` at `crates/rustasea/Cargo.toml:112`, `:118`, `:124`).
- Livewire analogue — `crates/rustasea-livewire/src/authorizer.rs:27` (`ActionAuthorizer`).
- Starter-kit scaffolder — `crates/rustasea-scaffold/src/variant.rs:13` (five variants: blade, react, vue, svelte, livewire).
- Svelte variant page parity: the `svelte` kit shares the full auth/settings page set with the react/vue kits (`crates/rustasea-scaffold/src/templates/inertia_svelte/{auth,settings}.rs`).
- Real AI HTTP providers (`GAP-014`) — `crates/rustasea-ai/src/providers/mod.rs:27` (`provider_from_env`), `openai.rs:21` (`OpenAiProvider`), `anthropic.rs:18` (`AnthropicProvider`); config resolution for openai/anthropic/groq/xai/deepseek/mistral/openrouter/ollama/azure (`providers/client.rs:82`).
- AI queueing (`GAP-015`) — `crates/rustasea-ai/src/queue.rs:41` (`AgentRunJob`), routed on connection `ai` / queue `agents`.
- MCP client (`GAP-015`) — `crates/rustasea-ai/src/mcp/client.rs:40` (`McpClient`: connect/list/call), `crates/rustasea-ai/src/mcp/mod.rs:99` (`McpRegistry`).
- pgvector-backed vector store (`GAP-012`) — `crates/rustasea-search/src/pgvector.rs:24` (`PgVectorStore`, feature `pgvector`); in-memory store remains the default.
- Internationalization (ADOPT-005, `Illuminate\Translation` parity): `crates/rustasea-i18n`: `resources/lang/{locale}/*.toml`/`*.json` loading with dot-namespaced keys, active-locale-then-fallback resolution, interpolation, and pluralization, plus global `__`/`trans_choice` helpers (`crates/rustasea-i18n/src/{loader,translator,message,global}.rs`); `cargo artisan lang:check` reports missing/unused/duplicate entries (`crates/rustasea-cli/src/commands/langcheck.rs:106`).
- Timezone mapping (ADOPT-006, `glhd/laravel-timezone-mapper` parity): `crates/rustasea-timezone`: IANA validation plus a deterministic user-timezone resolution chain (stored preference, session, request header, app default) and UTC/local formatting helpers (`crates/rustasea-timezone/src/{lib,mapper}.rs`).

**Partial (reason)**
- Gemini and Bedrock resolve to their OpenAI-compatible endpoints only (`crates/rustasea-ai/src/providers/client.rs:225`, `bedrock_from_env` `:382`): chat completions, streaming, and embeddings in the OpenAI wire format, with `GEMINI_API_KEY` / a Bedrock API key (`AWS_BEARER_TOKEN_BEDROCK`). The provider-native APIs (Gemini `generateContent`, Bedrock Converse with SigV4) are not implemented. Neither provider has been called against the live service from this repository.
- Provider-backed embeddings are real: `StrToEmbeddings` (`crates/rustasea-ai/src/embed.rs:18`) calls `AiProvider::embeddings`, and the HTTP adapter sends the configured embedding model (`crates/rustasea-ai/src/providers/client.rs:246`, `:317`; `crates/rustasea-ai/src/providers/openai.rs:163`). The model comes from `<PROVIDER>_EMBEDDING_MODEL`, then a built-in default for `gemini`, `mistral`, and `openai` (the OpenAI default applies only to OpenAI's own endpoint, not a custom `OPENAI_BASE_URL`), then the chat model. Bedrock has no built-in default: an embeddings call without `BEDROCK_EMBEDDING_MODEL` returns a configuration error (`crates/rustasea-ai/src/providers/openai.rs:157`). The engine-agnostic `Str::to_embeddings` in `rustasea-search` still falls back to a deterministic hashing stub when its closure yields no vector (`crates/rustasea-search/src/embeddings.rs:97`).
- `InProcessProvider` remains for deterministic tests — `crates/rustasea-ai/src/adapters.rs:50`.
- The agent runtime does not call a model: `Agent::run_inner` echoes the prompt and runs a tool only when the prompt literally contains `tool:name(args)` (`crates/rustasea-ai/src/agent.rs:247`), so `Agent::run`, `prompt_stream`, and the queued `AgentRunJob` never reach an `AiProvider` even though the HTTP providers are real. `provider_from_env` returns a boxed provider that `Ai::register` cannot take directly.
- Image, audio, rerank, and files exist only on the in-process stand-in; the HTTP adapters cover text, streaming, and (OpenAI-style only) embeddings. `AiProvider` has no `vector_stores` method.

**Missing**
- Provider-native Gemini and Bedrock adapters.
- An agent loop that sends the prompt and tool results to an `AiProvider`.

**Evidence:** `crates/rustasea-broadcast/{Cargo.toml,src/lib.rs}`; `crates/rustasea-storage/{Cargo.toml,src/manager.rs:56-171,src/facade.rs}`; `config/storage.toml`; `crates/rustasea-jsonapi/src/wire.rs:7`; `crates/rustasea-mail/`; `crates/rustasea-view/`; `crates/rustasea-ai/src/{providers,queue.rs,mcp,adapters.rs}`; `crates/rustasea-search/{src/pgvector.rs,src/embeddings.rs}`; `crates/rustasea-google/`; `crates/rustasea-config/src/services.rs`; `crates/rustasea-i18n/`; `crates/rustasea-timezone/`; tasks `GAP-012`, `GAP-014`, `GAP-015`, `GAP-020`, `ADOPT-005`, `ADOPT-006`, `ADOPT-026`.

**Next actions**
- Drive `Agent` through an `AiProvider` (prompt, tool calls, streaming).
- Add provider-native Gemini/Bedrock adapters where the OpenAI-compatible surface is not enough.

---

## P0 Progress (Foundation Unblockers)

**Parent:** `GAP-P0` — *P0 — Foundation unblockers (DB backend + router dispatch)* — status **completed** (children below).

| Task | Title | Status | Evidence |
|---|---|---|---|
| **GAP-001** | Wire sqlx DB backend + connection pool into `rustasea-orm` | **Done** | `crates/rustasea-orm/src/db.rs:24`, `crates/rustasea-orm/src/db/exec.rs:72` |
| **GAP-002** | Router → controller dispatch (real handler binding) | **Done** | `crates/rustasea-router/src/dispatch.rs:23-83` |
| **GAP-003** | Runtime consumer for declarative attributes (`#[middleware]`, `#[authorize]`, `#[tries]`, `#[backoff]`, `#[timeout]`) | **Done** | `#[middleware]` resolves through `MiddlewareRegistry` (`crates/rustasea-router/src/metadata.rs:237`, `dispatch.rs:109`); `#[authorize]` resolves through `AuthorizeRegistry` (`crates/rustasea-router/src/authorize.rs:216`, `dispatch.rs:135`); job policy binds through `JobPolicy` (`crates/rustasea-queue/src/policy.rs:21`, `driver/worker.rs:66`) |

**Other gap phases (all completed):** `GAP-004`–`GAP-009` (P1), `GAP-010`–`GAP-013`
(P2), `GAP-014`–`GAP-015` (P3), `GAP-016`–`GAP-018` (P4), `GAP-019`–`GAP-020`
(P5); `GAP-021` (P5, this documentation sync) completed. See `GAP-ROOT` for
the full program.

> **Note (2026-09-17):** `GAP-P0` closed with `GAP-001`/`GAP-002` verified in the
> tree. The `GAP-003` registry consumer has since landed: `#[middleware]` and
> `#[authorize]` metadata are resolved and enforced at dispatch, and the
> `#[tries]`/`#[backoff]`/`#[timeout]` job policy is bound at registration. This
> document now matches the registry; the reconciliation discrepancy recorded
> earlier is resolved under `GAP-023`.

---

## Related Documents

- [`README.md`](../README.md) — milestone goals, scope, deliverables, success criteria.
- [`docs/laravel-13-research.md`](laravel-13-research.md) — Laravel 13 feature research.
- [`docs/laravel-parity.md`](laravel-parity.md) — Laravel 13.x API adoption mapping (maintained separately).
- [`docs/documentation-conventions.md`](documentation-conventions.md) — commit-reference policy (milestone + date, no raw SHAs).
