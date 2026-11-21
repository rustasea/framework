# Changelog

All notable changes to RustaSea are recorded in this file.

This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and intends to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
RustaSea is pre-1.0: while the major version is `0`, the public API may still
change between minor releases, and any breaking change is called out in the
entry that introduces it. The versioning and tagging intent is defined in
`.agents/documents/tasks/roadmap.md` (section 5, "Release & Tagging Strategy").

## How to read this file

- **Format:** entries are grouped by delivery program and stamped with the date
  the work landed, newest first. Each entry lists the affected milestone
  (`M0`-`M6`), the task identifiers (`GAP-*`, `ADOPT-*`, `LARAVEL-*`,
  `AUTH-*`, `CFG-*`, `DB-*`, `RTE-*`, `SK-*`, `STG-*`, `TST-*`), and the
  commit subject lines where useful.
- **Traceability:** per `docs/documentation-conventions.md` (standard
  `STD-001`), this file cites milestone IDs, task IDs, and commit subject
  lines. It does not embed raw commit SHAs.
- **Status source:** the authoritative, evidence-backed per-milestone status is
  `docs/milestones.md`. Where a milestone is delivered only in part, that is
  stated here explicitly rather than implied.
- **Releases:** no git tags exist in this repository yet. Nothing below is a
  published release; the entries describe work merged to `master`. The planned
  release mapping is recorded in the "Versioning plan" section.

## Versioning plan

Per `.agents/documents/tasks/roadmap.md` (section 5), each milestone is intended
to ship as a tagged release on `master` with migration notes tracked in this
file. No tag has been created yet, so the mapping below is a plan, not a record
of released versions.

| Planned tag | Milestone | Focus |
|---|---|---|
| `v0.1.0` | M0 | Bootstrap & Core |
| `v0.2.0` | M1 | Routing & HTTP |
| `v0.3.0` | M2 | ORM & Database |
| `v0.4.0` | M3 | Auth, Middleware & Validation |
| `v0.5.0` | M4 | Queue, Cache, Scheduling & Events |
| `v0.6.0` | M5 | DX, CLI & Testing |
| `v0.7.0` | M6 | Advanced (Broadcast, Search, Filesystem, AI SDK) |

`1.0.0` is a candidate after the framework is dog-fooded (roadmap section 5).
Per the same section, each tag is expected to pass `cargo xtask ci` and
`cargo xtask check-cycles` before it is cut.

---

## [Unreleased]

### Added

- 2026-11-20 - Database, cache, and queue bindings at boot: `rustasea::services`
  adds `DatabaseServiceProvider` (a lazily connecting `ConnectionResolver`),
  `CacheServiceProvider` (a `CacheManager`), and `QueueServiceProvider` (the
  `sync`/`redis` connections and the parsed `QueueConfig`), plus
  `connect_database_queues` for `database` connections after boot. The runnable
  app registers them (M0; `feat(rustasea): bind database, cache, and queue
  services at boot`; `feat(app): register the framework service providers at
  boot`).
- 2026-11-18 - Database-backed user provider: `DatabaseUserProvider` implements
  the async `UserProvider` seam over the scaffold `users` table (registration,
  login, password reset, email verification, profile and timezone updates;
  soft-deleted users do not resolve), and the runnable app installs it at boot
  when the default guard's provider is ORM-backed and the table exists,
  otherwise staying fail-closed (M3; `feat(auth): add DatabaseUserProvider over
  the users table`; `feat(app): read users from the database when the users
  table exists`).
- 2026-11-18 - Gemini and Bedrock providers through their OpenAI-compatible
  endpoints: `provider_from_env("gemini")` uses `GEMINI_API_KEY`, and
  `provider_from_env("bedrock")` uses a Bedrock API key
  (`AWS_BEARER_TOKEN_BEDROCK`) with `AWS_REGION`; the provider-native APIs are
  not implemented (M6; `feat(ai): serve gemini and bedrock through their
  OpenAI-compatible endpoints`).
- 2026-11-18 - Failed inline `sync` dispatches are persisted to `failed_jobs`
  through the registered `database` connection as a retryable job envelope, so
  they survive the process and appear in `queue:failed`; without a database
  connection they still fall back to the in-memory list (M4; `feat(queue):
  persist failed sync jobs through the database connection`).
- 2026-11-18 - `rustasea::mail` re-export of `rustasea-mail` from the umbrella
  crate, with `mail-smtp` and `mail-templates` features (M6; `feat(rustasea):
  re-export rustasea-mail as rustasea::mail`).
- 2026-09-24 - `storage-s3` umbrella feature forwarding to
  `rustasea-storage/aws` (the `aws` feature needs rustc 1.89+ through
  `object_store`'s `crc-fast` dependency), and a Docker-backed RustFS suite
  (`crates/rustasea-storage/tests/rustfs_container.rs`, run with
  `--features aws -- --ignored`) that round-trips an `s3` disk over a
  plain-HTTP endpoint. `cargo xtask ci` and the CI test job now also cover
  `rustasea-storage` with its opt-in `aws` and `sftp` drivers (M6;
  `feat(rustasea): add storage-s3 umbrella feature`; `ci: lint and test
  rustasea-storage with the aws and sftp features`).
- 2026-09-17 - Svelte starter-kit variant with full auth/settings page parity: the
  `svelte` variant scaffolds the same 11 auth/settings pages as the react/vue
  kits on the shared Inertia contract
  (`feat(scaffold): [TASK-107] complete auth and settings page parity for Inertia
  variants`; `feat(scaffold): [TASK-106] add svelte starter-kit variant backed by
  Sycamore`).
- 2026-09-17 - Sycamore WASM adapter for the svelte variant: `rustasea-inertia-adapters`
  gains a `svelte` feature (Sycamore router context, provider, and Inertia link)
  and the umbrella exposes `wasm-sycamore`, so the `svelte` kit maps to
  Rust-native fine-grained reactive WASM with no Node.js toolchain
  (`feat(inertia): [TASK-105] add Sycamore WASM adapter for the svelte variant`).
- 2026-09-17 - Four first-party starter-kit repositories published, mirroring the
  `laravel/<x>-starter-kit` split (ADR-0002): `rustasea/react-starter-kit`
  (Dioxus WASM + Inertia), `rustasea/vue-starter-kit` (Leptos WASM + Inertia),
  `rustasea/svelte-starter-kit` (Sycamore WASM + Inertia), and
  `rustasea/livewire-starter-kit` (askama + HTMX); `rustasea/rustasea` remains the
  Blade skeleton (TASK-108, TASK-109, TASK-110, TASK-111).
- 2026-09-17 - Example application domain in the runnable `rustasea-app` crate:
  a scaffold-style `app/` tree with a `Post` model plus in-memory
  `PostRepository`, a JSON CRUD `PostController` implementing the base
  `Controller` trait, `StorePostRequest`/`UpdatePostRequest` validatable form
  requests, and a `CreatePostAction` demonstrating the `Action` pattern; served
  as a demo JSON API at `/examples/posts` and wired into the route table
  (`feat(app): [TASK-097] add example app domain module (models, controllers,
  requests, actions) to rustasea-app`).
- 2026-09-17 - Generated-app boot parity: the scaffold's `bootstrap/commands.rs`
  registers the framework command surface (and queue migrations) via
  `rustasea::cli::load_default_commands()` plus the application's nine own
  migrations through `rustasea::orm::register_migration`, so `cargo artisan
  migrate` runs the real schema instead of reporting an empty registry
  (`feat(scaffold): [TASK-096] wire generated-app boot parity and emit
  rustfmt.toml + CI workflow`).
- 2026-09-17 - Generated apps ship a `rustfmt.toml` mirroring the framework's
  formatting contract and a minimal `.github/workflows/ci.yml` running
  `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test` on stable (`feat(scaffold): [TASK-096] wire generated-app boot
  parity and emit rustfmt.toml + CI workflow`).
- 2026-09-17 - App scaffold emits Hypervel-parity broadcasting and CORS config
  templates (`config/broadcasting.toml`, `config/cors.toml`) with their
  `.env.example` mirror (`feat(scaffold): [TASK-095] emit broadcasting and CORS
  config templates (Hypervel parity)`).
- 2026-09-17 - App scaffold emits Hypervel-parity root hygiene and directory
  placeholders: `.gitattributes`, `LICENSE`, a `public/` web root
  (`robots.txt`, `favicon.ico`), `bootstrap/cache/`, and the `storage/`
  subdirectory `.gitignore` set (`feat(scaffold): [TASK-094] emit
  Hypervel-parity root hygiene and directory placeholders`).
- 2026-09-17 - Base `Controller` trait with shared JSON/validation/redirect
  helpers and the `redirect`/`see_other` free functions (Hypervel parity)
  (`feat(http): [TASK-091] add base Controller trait and redirect helpers
  (Hypervel parity)`).
- 2026-09-17 - App scaffold emits the `app/http/controllers/controller.rs` base
  controller and struct controllers implementing it (Hypervel parity)
  (`feat(scaffold): [TASK-092] emit app base controller and struct controllers
  (Hypervel parity)`).
- 2026-09-17 - `make:controller` now generates base-controller-conforming
  structs (`impl Controller for <Name> {}`) for both plain and `--resource`
  output (Hypervel parity) (`feat(cli): [TASK-093] generate
  base-controller-conforming make:controller output (Hypervel parity)`).
- 2026-09-17 - Interactive auth and settings forms for the Inertia React/Vue and
  Blade starter kits (`feat(ui): [UI-AUTH-001] interactive auth and settings
  forms for Inertia React/Vue and Blade`).
- 2026-09-17 - Gap analysis and traceability matrix with task specifications for
  `GAP-022` through `GAP-031` (`docs(audit): add gap analysis matrix and GAP-022
  through GAP-031 task specifications`).

### Changed

- 2026-11-20 - Domain routes match the request host. A route under
  `Router::domain` previously answered every host and made the catch-all on the
  same method+path unreachable; a request is now routed by its `Host`, falling
  back to the route without a domain, and gets `404` when neither applies (M1;
  `fix(router): match domain routes against the request host`).
- 2026-11-20 - `in:` / `in_array:` accept bare words: `in:admin,editor`
  previously rejected every value (M3; `fix(validation): accept bare words in
  in: and in_array: lists`).
- 2026-11-20 - The shutdown drain is bounded: after SIGTERM/SIGINT the process
  exits with status 1 if draining outlives the timeout (30 s through
  `Application::shutdown`) (M0; `fix(foundation): bound the shutdown drain by
  its timeout`).
- 2026-11-20 - `JwtGuard::logout` revokes the token pair and refresh tokens
  are single-use. Tokens carry a new optional `sid` claim; revocations live in a
  cache `Store`, per-process memory unless `with_revocation_store` supplies a
  shared one (M3; `fix(auth): revoke JWT pairs on logout and make refresh
  tokens single-use`).
- 2026-11-18 - **Breaking:** `HttpProviderConfig` gains the public field
  `embedding_model`, so downstream struct literals that construct it directly
  must set the field. It is populated from `<PROVIDER>_EMBEDDING_MODEL` (else
  the provider's known embedding model) and, when neither is present, embeddings
  requests fall back to the configured chat model (M6; `fix(ai): send an
  embedding model, not the chat model, on embeddings requests`).
- 2026-11-18 - Documentation brought in line with the code: `docs/laravel-parity.md`
  rows for events, batches, unique jobs, sessions, the user provider, mail,
  notifications, the gate, the password-reset store, the scheduler, and the AI
  SDK; as-built status on the roadmap and sprint documents; and a checked Status
  column with open items on the 110 implementation-task rows under
  `.agents/documents/application/modules/` (55 done, 53 partial, 2 not
  started). `docs/milestones.md` returns M4 to Partial (the scheduler cannot
  dispatch a due command, schedule pause state is per-process, and
  after-response events are never flushed) and records that the AI agent
  runtime does not call a provider (M0-M6; `docs: sync planning and status
  documents with the code`).
- 2026-11-18 - Embeddings requests send an embedding model instead of the
  provider's chat model: `<PROVIDER>_EMBEDDING_MODEL`, else the provider's known
  embedding model (openai, gemini, mistral), else the default model as before
  (M6; `fix(ai): send an embedding model, not the chat model, on embeddings
  requests`).
- 2026-11-18 - Sync dead letters keep the job payload: the body is captured
  before the job runs, where it was previously read after execution had consumed
  it and stored as `null` (M4; `feat(queue): persist failed sync jobs through
  the database connection`).
- 2026-11-18 - A UUID stored in a column declared `UUID` now reads back on
  SQLite, where rows fetched through `fetch_json` previously returned `null` for
  the id (M2; `fix(orm): decode a UUID stored in a column declared UUID on
  SQLite`).
- 2026-09-24 - `s3` disks now work with S3-compatible services such as RustFS
  and MinIO: an explicit `http://` endpoint enables `object_store`'s
  `allow_http` (previously every request failed with a reqwest builder error,
  including the documented `endpoint = "http://localhost:9000"` example).
  The `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_DEFAULT_REGION`,
  `AWS_BUCKET`, and `AWS_ENDPOINT` variables documented in `.env.example`,
  previously never read, now override the matching keys of every `s3` disk
  in `config/storage.toml`; note that the stock `.env.example` sets
  `AWS_DEFAULT_REGION=us-east-1`. `bucket` may be omitted when `AWS_BUCKET`
  is set, and a blank bucket is rejected with `StorageError::Config`.
  `S3DiskConfig` moved to `rustasea_storage::s3` (still re-exported from the
  crate root and `facade`) and redacts `secret_access_key` from `Debug`
  output (M6; `refactor(storage): move S3DiskConfig into its own module and
  share env helpers`; `fix(storage): support S3-compatible http endpoints
  and AWS_* env for s3 disks`).
- 2026-09-17 - `cargo install rustasea` now installs the application scaffolder:
  the `cargo-rustasea` binary moved into the `rustasea` facade package (ships
  behind the `scaffold` feature, enabled by default) so `cargo rustasea new`
  works from a single install, and the standalone `cargo-rustasea` package was
  removed. The workspace is now 42 crates under `crates/` + `xtask`
  (43 workspace packages) (`refactor(scaffold): [TASK-103] fold cargo-rustasea
  bin into the rustasea facade package`).
- 2026-09-17 - App scaffold now emits Laravel-parity port 8000: the generated
  `.env.example`, README, `Dockerfile` (`EXPOSE`/healthcheck), and
  `docker-compose.yml` all use 8000 (matching `php artisan serve` and
  `config/app.toml`), and the generated `main.rs` derives its bind address from
  `APP_URL` (host:port) with an `0.0.0.0:8000` fallback instead of hard-coding
  the port, fixing the README claim that `APP_URL` overrides the bind
  (`fix(scaffold): [TASK-101] emit Laravel-parity port 8000 and derive the bind
  from APP_URL`).
- 2026-09-17 - Repository renamed to `rustasea/framework` (analog
  `laravel/framework`); the application skeleton lives at `rustasea/rustasea`
  (analog `laravel/laravel`). In-repo URLs, `Cargo.toml` `repository` metadata
  (workspace-inherited across all 43 packages), and the README repository layout
  section were updated (`chore(repo): [TASK-098] rename to rustasea/framework
  and add repository metadata`).
- 2026-09-17 - Documentation reconciliation: canonical crate inventory
  synchronized to the 43 workspace packages (`GAP-022`); stale `route:list` and
  attribute-consumption documentation corrected (`GAP-023`); complete artisan
  CLI command surface documented (`GAP-024`); `make:*` generator count
  reconciled to 17 (`GAP-025`); Laravel parity and milestone status aligned with
  all 31 ADOPT implementations (`GAP-026`); ADOPT crates and umbrella features
  documented in the README (`GAP-027`).
- 2026-09-17 - Source-level `show:model {name} [--json]` introspection of a
  model's attributes, casts, soft-delete/timestamp flags, and relations
  (`GAP-029`, `feat(cli): [GAP-029] implement source-level show:model
  introspection with JSON output`).
- 2026-09-17 - Per-chunk HTTP idle-timeout enforcement on streamed response
  bodies (`GAP-030`, `feat(http): [GAP-030] enforce per-chunk idle timeout on
  streamed response bodies`).
- 2026-09-17 - `cargo xtask lines:check` 500-line linter wired into `cargo
  xtask ci` (`GAP-031`, `feat(xtask): [GAP-031] add lines:check 500-line linter
  and enforce in xtask ci`).

### Fixed

- 2026-11-21 - A query against a missing table returns
  `OrmError::MissingTable` with the table name; it was an untyped
  `OrmError::Storage` string and the variant was never constructed (M2;
  `fix(orm): report a missing table as OrmError::MissingTable`).
- 2026-11-21 - Insert-or-ignore runs and ignores conflicts on SQLite:
  `InsertBuilder::insert_or_ignore` executes on a pool and reports whether the
  row was inserted, and the SQLite shape is now `INSERT OR IGNORE`, where it was
  a plain `INSERT` that failed on a conflict (M2; `fix(orm): execute
  insert-or-ignore, and ignore conflicts on SQLite`).
- 2026-11-21 - `ModelOps::find` loads a row by id, hiding soft-deleted rows
  (M2; `feat(orm): add ModelOps::find`).

### Added (governance)

- 2026-09-17 - Repository root governance documents: `CHANGELOG.md`,
  `CONTRIBUTING.md`, and `SECURITY.md` (`GAP-028`).

---

## Program history

### 2026-09-15 to 2026-09-16 - Starter-Kit Adoption program (ADOPT-001..ADOPT-031)

A 31-item adoption wave mapping Laravel ecosystem packages onto RustaSea crates.
Each item landed as its own commit; the milestone each belongs to is noted. See
`docs/milestones.md` (M3, M4, M5, M6 sections) for the authoritative per-item
status and evidence.

**ADOPT-001..ADOPT-019 (2026-09-15):**

- `ADOPT-001` - RBAC roles and permissions with cached lookups and ability-level
  authorize (`spatie/laravel-permission` parity, M3).
- `ADOPT-002` - Activity log audit trail with opt-in model hooks and query API
  (`spatie/laravel-activitylog` parity, M3).
- `ADOPT-003` - Authentication log with typed events, recorder, and new-device
  notification (`rappasoft/laravel-authentication-log` parity, M3).
- `ADOPT-004` - Feature-gated Sentry integration with 5xx capture and secret
  scrubbing (M3).
- `ADOPT-005` - `lang:check` translation checker with missing, unused, and
  duplicate detection (`Illuminate\Translation` parity, M6).
- `ADOPT-006` - Timezone mapper crate with scheduler wall-clock support and user
  timezone plumbing (`glhd/laravel-timezone-mapper` parity, M6).
- `ADOPT-007` - Docker dev environment with compose stack and scaffold emission
  (`laravel/sail` parity, M5).
- `ADOPT-008` - `tinker` database query and model verbs with read-only guard
  (`laravel/tinker` parity, M5).
- `ADOPT-009` - Request profiler with SQL, cache, and event hooks
  (`barryvdh/laravel-debugbar` parity, M5).
- `ADOPT-010` - Dev error page with panic snippets and production JSON error
  envelope (`spatie/laravel-ignition` parity, M5).
- `ADOPT-011` - OpenAPI 3.1 generation with `openapi:generate` CLI and dev docs
  UI (`dedoc/scramble` parity, M5).
- `ADOPT-012` - Faker data generation with locale support and deterministic
  seeding (`fakerphp/faker` parity, M5).
- `ADOPT-013` - `FakeCache` with operation recording and cache assertions
  (`Illuminate\Support\Testing\Fakes` parity, M5).
- `ADOPT-014` - `log:show` filter/tail command and dev-only log viewer with a
  tolerant tracing parser (M5).
- `ADOPT-015` - MCP server with stdio protocol engine and `mcp:serve` command
  (`laravel/boost` parity, M5).
- `ADOPT-016` - Slug generation with derive attributes and opt-in route model
  binding (`cviebrock/eloquent-sluggable` parity, M5).
- `ADOPT-017` - Composite-key relations with row-value eager loading and
  composite primary keys (`awobaz/compoships` parity, M5).
- `ADOPT-018` - Cascade soft deletes with derive opt-in and restore path
  (`spatie/laravel-cascade-soft-deletes` parity, M5).
- `ADOPT-019` - Query caching with generation-based write invalidation
  (`spatie/laravel-model-caching` parity, M5).

**ADOPT-020..ADOPT-031 (2026-09-16):**

- `ADOPT-020` - JSON-column relations with dialect overlap predicates and
  batched eager loading (`staudenmeir/eloquent-json-relations` parity, M2).
- `ADOPT-021` - Queue dashboard with metrics history, worker heartbeats, and
  failed-job management (`laravel/horizon` parity, M4).
- `ADOPT-022` - External broadcast drivers with Pusher protocol signing and
  Redis pub/sub fan-out (M4).
- `ADOPT-023` - Spreadsheet import-export with streaming CSV/xlsx readers and
  queued exports (`maatwebsite/excel` parity, M6).
- `ADOPT-024` - Image manipulation pipeline with EXIF auto-orient and queued
  transforms (`intervention/image` parity, M6).
- `ADOPT-025` - SFTP storage disk with path confinement and reconnection retry
  (M6).
- `ADOPT-026` - Google service-account auth with RS256 assertions and
  single-flight token cache (`google/auth` parity, M6).
- `ADOPT-027` - Modular application support with `make:module` generator and
  module registry (M5).
- `ADOPT-028` - Action pattern with HTTP, queue, CLI, and event adapters plus a
  `make:action` generator (M5).
- `ADOPT-029` - Browser testing harness with WebDriver selector helpers and
  skip semantics (M5).
- `ADOPT-030` - Quality gate with fmt/clippy/deny configs, the `cargo xtask`
  alias, and the CI workflow (M5).
- `ADOPT-031` - Consolidated workspace dependencies and the `deps:check` drift
  lint (M5).

### 2026-09-14 - Authentication program, Laravel parity, and configuration wave

- 2026-09-14 - Full auth core: session guard, login/logout, profile/password,
  and CSRF (`AUTH-001..008`, `AUTH-010`, `FIX-AUTH-01/02`).
- 2026-09-14 - Signed URLs, registration, password confirmation, email
  verification, and password reset (`AUTH-009`, `AUTH-011..014`,
  `FIX-AUTH-03`).
- 2026-09-14 - Two-factor TOTP, passkeys (WebAuthn), database session store,
  mail templates, and a migration guard (`AUTH-015..019`).
- 2026-09-14 - Laravel parity pass: live `route:list` (`LARAVEL-001`),
  `ConfigLoader` mounted into boot (`LARAVEL-002`), controller `#[middleware]`
  consumption (`LARAVEL-003`), attribute casting (`LARAVEL-004`), global query
  scopes (`LARAVEL-005`), write-path timestamps (`LARAVEL-006`), `raw`/`raw_sql`
  execution (`LARAVEL-007`), Gate and Policies (`LARAVEL-008`/`009`),
  record-level `#[authorize]` (`LARAVEL-010`), job attribute consumer
  (`LARAVEL-011`), unique jobs (`LARAVEL-012`), job batches (`LARAVEL-013`),
  `RefreshDatabase` (`LARAVEL-014`), Mail/Queue/Event fakes (`LARAVEL-015`),
  `tinker` REPL (`LARAVEL-016`), and internationalization (`LARAVEL-017`).

### 2026-09-13 - Connections, config parity, and starter-kit conventions

- 2026-09-13 - Named connections, read/write split, schema builder, and MongoDB
  (`DB-001..005`).
- 2026-09-13 - Full Laravel 13.x config parity across app/auth/cache/database/
  queue/session/storage (`CFG-001..010`), all 11 configs scaffolded and
  `.env.example` completed (`CFG-010`), Fortify config parity and auth/session
  env bridges (`CFG-011/012`).
- 2026-09-13 - Starter-kit conventions adopted from the livewire kit: resources
  layouts/partials/assets (`SK-001..003`), storage `.gitignore` (`STG-001/002`),
  test conventions (`TST-001..003`), and named routes/URL resolver/redirects
  (`RTE-001..003`).

### 2026-09-11 to 2026-09-12 - Gap-closure program (GAP-001..GAP-021)

- 2026-09-11 - Foundation unblockers: sqlx database backend and connection pool
  (`GAP-001`), router-to-controller dispatch (`GAP-002`), and the runtime
  consumer for declarative attributes (`GAP-003`).
- 2026-09-12 - Metrics, CLI generators, xtask, testcontainers, and pgvector
  (`GAP-009..018`); real Redis cache store, provider DAG boot, and async event
  listeners (`GAP-004`, `GAP-008`); storage facade, SMTP mailer, and queued
  notifications (`GAP-020`); AI HTTP providers, real queueing, MCP client, and
  ranked loaders (`GAP-014`/`015`); migrations, CRUD, pgvector, and eager
  loading (`GAP-010..013`); CLI generators, testcontainers, and xtask
  (`GAP-016..018`).
- 2026-09-12 - `cargo xtask migrate` task and the `check-cycles` submodule.
- 2026-09-12 - Documentation sync of README and milestones with the shipped
  code state (`GAP-021`).

### 2026-09-07 to 2026-09-09 - Milestone delivery (M0-M6)

The seven milestones were implemented in sequence; `docs/milestones.md` holds
the authoritative, evidence-backed status. M2 (ORM & Database) is **Done**; M0,
M1, M3, M4, M5, and M6 are **Partial**, each with a documented remaining-work
list.

- 2026-09-07 - M0 Bootstrap & Core: workspace, container, config, and service
  providers (`TASK-015`).
- 2026-09-07 - M1 Routing & HTTP: axum router, HTTP facade, and macros
  (`TASK-016`).
- 2026-09-07 - M2 ORM & Database: query builder, model, migrations, and vector
  (`TASK-017`).
- 2026-09-08 - M3 Auth, Middleware & Validation: JWT, CSRF, guards, and
  throttle (`TASK-018`).
- 2026-09-08 - M4 Queue, Cache, Scheduling & Events: routing, touch, and
  pause/resume (`TASK-019`).
- 2026-09-08 - M5 DX, CLI & Testing: clap commands, generators, and the
  `TestCase` harness (`TASK-020`).
- 2026-09-08 - M6 Advanced: broadcast, storage, search, and AI SDK with
  WS/SSE, read-through storage, vector search, and the provider trait
  (`TASK-021`).
- 2026-09-09 - M6 advanced surface extended with broadcast/search/storage/AI/
  JSON:API, M4 `withContext`, and queue notifications
  (`feat(advanced): M6 broadcast/search/storage/ai/jsonapi + M4 withContext +
  queue notification`).
- 2026-09-09 - Project rebranded from Rustavel to RustaSea
  (`refactor: rebrand Rustavel -> RustaSea`). Historical documents keep the
  original name for traceability; current branding is RustaSea.

### 2026-01-31 - Project inception

- 2026-01-31 - Initial README for the project (`Add initial README.md for
  rustavel project`), followed by the Laravel 13 research and the M0-M6
  milestone README (`TASK-002`/`TASK-003`).

---

## Notes

- The per-milestone statuses above reflect `docs/milestones.md` as of its last
  update; consult that document for the current evidence (`path:line`) and
  remaining work.
- The ADOPT wave, the authentication program, and the gap-closure program all
  landed after the initial milestone implementation; their entries are grouped
  separately above so the milestone history stays readable.
