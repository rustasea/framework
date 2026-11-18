# Feature: CLI (M5)

> **Module:** `developer-platform` — [overview.md](overview.md) · **FSD:** FS-M5-01 · **FR:** FR-500, FR-502..505 · **BC:** BC-5
> **Stories:** US-M5-01 (list + prompts + Artisan::call) · **BDD:** `@cli`, `@attributes`

## 1. Feature Overview
- **Brief Description:** `cargo rustasea <command> [args] [--json]` via `cargo-xtask` (`cargo-xtask` bin) using `clap` derive with typed `Args`/`Flags` per command, `list [--json] [--all]` enumerating commands with `#[usage]`/`#[help]`/`#[hidden]` (hidden omitted without `--all`), prompts `ask`/`secret`/`confirm`/`choice`/`multiSelect` via `dialoguer` and `table`/`progressBar`/`spinner` via `indicatif`/`comfy-table`, `Shutdownable` on long workers (`queue:work`, `schedule:run`), `Artisan::call(command, args)` in-process invocation (no subprocess), unknown command suggests `did you mean?` via `strsim`.
- **Role in Module:** Surface of every framework capability; `list` is the discoverability contract.
- **Business Value:** Artisan-parity CLI; `list --json` machine-readable for agents/tooling.

## 2. User Stories

### US-M5-01 — cargo rustasea CLI with typed commands and prompts
**Sebagai** Rust developer **Saya ingin** `cargo rustasea list` + typed args/flags + prompts + `Artisan::call` **Sehingga** CLI like Artisan

**AC:** `cargo rustasea list --json` contains `make:controller` + `migrate` with `usage`; `#[usage("app:send {user}")]` on `AppSend` shown in help; `confirm("Proceed?")` → `n` aborts with non-zero; `Artisan::call("migrate", vec![])` migrates in-process; `#[hidden]` command omitted without `--all`; `make:controll` → suggests `make:controller` (outline).

## 3. Business Flow & Rules

### 3.1 Business Flow
```mermaid
%%{init: {"theme": "base", "themeVariables": {"background": "#ffffff", "mainBkg": "#ffffff", "primaryColor": "#bbdefb", "secondaryColor": "#fff9c4", "tertiaryColor": "#c8e6c9"}}}%%
sequenceDiagram
    actor Dev as Developer
    participant CLI as cargo rustasea (clap+xtask)
    participant Registry as Command registry
    participant Prompts as dialoguer/indicatif

    Dev->>CLI: cargo rustasea list --json
    CLI->>Registry: enumerate #[usage]/#[help]/#[hidden]
    Registry-->>CLI: {name, usage, hidden}[]
    CLI-->>Dev: JSON entries usage strings
    Dev->>CLI: cargo rustasea app:send --user=42
    CLI->>Prompts: confirm("Proceed?") -> n
    Prompts-->>CLI: aborted non-zero
    Dev->>CLI: Artisan::call("migrate", vec![])
    CLI->>CLI: in-process invocation (no subprocess)
    CLI-->>Dev: CommandOutput
```

### 3.2 Business Rules
- Typed `Args`/`Flags` via proc-macro; `ExitCode 0` on success.
- Unknown command suggests `did you mean?` via `strsim` edit distance.
- Long-running workers implement `Shutdownable` so `SIGTERM` drains via foundation shutdown.

## 4. Data Model

```mermaid
%%{init: {"theme": "base", "themeVariables": {"background": "#ffffff", "mainBkg": "#ffffff", "primaryColor": "#bbdefb", "secondaryColor": "#fff9c4", "tertiaryColor": "#c8e6c9"}}}%%
erDiagram
    Command {
        string signature PK
        string usage
        string help
        bool hidden
    }
    Args {
        string name PK
        string type
    }
    Command ||--o{ Args : has
```

## 5. Public Interface

```rust
#[derive(clap::Parser)]
struct Cli { command: String, #[arg(long)] json: bool, #[arg(long)] all: bool }

trait Command: Send + Sync {
    fn signature() -> &'static str;
    async fn handle(&self, args: Args, io: &mut Io) -> ExitCode;
}
struct Artisan;
impl Artisan { fn call(cmd: &str, args: Vec<String>) -> CommandOutput; } // in-process
trait Shutdownable { async fn shutdown(&self, handle: ShutdownHandle) -> Result<()>; }
// Attributes: #[usage("app:send {user}")] #[help("...")] #[hidden]
```

## 6. Dependencies
- `clap` derive, `dialoguer`, `indicatif`, `comfy-table`, `strsim`, `xtask`, `foundation` shutdown handle.

## 7. Limitations
- `secret` echo leakage not allowed (security-audit checks).

## 8. Compliance
- `list --json` stable shape: `{ name, usage, help, hidden }` per entry; `cargo tree`-visible `clap` not `async-openai`.

## 9. Implementation Tasks

| ID | Component | Status | Description |
|----|-----------|--------|-------------|
| F-M5-CLI-01 | CLI registry | Partial | `clap` derive + `list` + `#[usage]`/`#[help]`/`#[hidden]` |
| F-M5-CLI-02 | Prompts | Done | `ask`/`secret`/`confirm`/`choice`/`multiSelect` + table/spinner |
| F-M5-CLI-03 | Artisan::call | Partial | in-process dispatch + `Shutdownable` |
| F-M5-CLI-04 | Tests | Partial | `list --json` entries, usage help, confirm abort, `did you mean?` |

> **As-built note (2026-11-18):** the Status column was checked against the source tree on this date; the Component and Description columns are the original plan and are unchanged. `Done` means every element named in the row exists in reachable non-test code (for a Tests row: a test exists for each named case). Tests were read, not run. Live status: `docs/milestones.md`.
>
> Open items:
> - `F-M5-CLI-01` — the clap-derive CLI, the registry, and `list` are real; `list` reads usage/help/hidden from the `Command` trait methods (`crates/rustasea-cli/src/artisan.rs:71`), and the `#[usage]` / `#[help]` / `#[hidden]` attribute macros emit constants the registry never reads.
> - `F-M5-CLI-03` — `Artisan::call` dispatches in-process (`crates/rustasea-cli/src/artisan.rs:107`); `Shutdownable` has no implementation outside tests and `queue:work` does not use it or `drain_with_timeout`.
> - `F-M5-CLI-04` — "did you mean?" and hidden filtering are tested; no test runs `list --json` and checks the entries, none covers usage/help output, and none covers aborting on `confirm`.

## 10. Cross-References
- API: [api-cli](../../api/developer-platform/api-cli.md)
- Tests: [test-cli](../../testing/developer-platform/test-cli.md) · BDD `@cli`
- Design: `component-inventory.md` CLI group

## 11. Skill Reference
| Layer | Skill |
|-------|-------|
| QA | `test-planning` `@cli` |
| BDD | `test-generation` — `list --json` contract |
| Security | `security-audit` — hidden enum leak |
| Chaos | `non-functional-testing` — `Shutdownable` drain under `SIGTERM` |

## 12. Command Surface

Beyond `list` and the `make:*` generators, the CLI registers the inspection and
operation commands below. Each usage string is the command's exact `usage()`
line, verified against its parser in `crates/rustasea-cli/src/commands/`.

| Command | Usage | Source | Description |
|---------|-------|--------|-------------|
| `new` | `new <app> --variant {blade\|react\|vue\|svelte\|livewire}` | `cargo-rustasea.rs` | Scaffold a new application from the selected starter-kit variant (ADR-0002); `rustasea-scaffold` is the library behind it (`crates/rustasea-scaffold/src/variant.rs:13`). |
| `show:model` | `show:model {name} [--json]` | `inspect.rs` | Source-level model introspection: attributes, casts, soft-delete and timestamp flags, and relations parsed from `app/models/{snake}.rs` (`GAP-029`). |
| `module:list` | `module:list [--json]` | `modules.rs` | List discovered `modules/<name>/` crates with status, version, and path (`ADOPT-027`). |
| `module:enable` | `module:enable {name}` | `modules.rs` | Mark an existing module enabled in the `[modules]` manifest table (`ADOPT-027`). |
| `module:disable` | `module:disable {name}` | `modules.rs` | Mark an existing module disabled in the `[modules]` manifest table (`ADOPT-027`). |
| `lang:check` | `lang:check [--locale=xx] [--path=resources/lang] [--json]` | `langcheck.rs` | Report missing, unused, and duplicate translation entries; missing and duplicate findings fail the command (`ADOPT-005`). |
| `log:show` | `log:show [--level=error] [--channel=daily] [--since=2026-09-15T00:00:00Z] [--grep=text] [--limit=200] [--follow] [--json]` | `log_show.rs` | Filter, print, and optionally tail the configured log file (`ADOPT-014`). |
| `tinker` | `tinker` | `tinker.rs` | Interactive REPL over a booted application; piped stdin scripts the session (`ADOPT-008`). |
| `mcp:serve` | `mcp:serve` | `mcp_serve.rs` | Serve project knowledge (routes, docs, commands, redacted config) over MCP stdio; feature-gated behind the `mcp` cargo feature (`ADOPT-015`). |

### 12.1 Flag Notes

- `show:model` resolves the project root, parses `app/models/{snake}.rs`, and
  renders a human summary or, with `--json`, the contract shape
  `{ model, table, soft_delete, timestamps, attributes[], casts{}, relations[] }`.
- `module:list --json` emits a name-sorted array of
  `{ name, status, version, path }`; enable/disable reject a name that is not
  present on disk before writing the manifest.
- `lang:check` accepts both `--flag=value` and `--flag value`; `--locale`
  defaults to the configured `app_locale`, `--path` to `resources/lang`. Unused
  keys are informational and never fail the command.
- `log:show` accepts both `--flag=value` and `--flag value`; `--level` is an
  inclusive minimum severity, `--since` is an RFC 3339 lower bound, `--grep` is
  a case-insensitive substring, `--limit` defaults to 200, and `--follow` polls
  every 250 ms until `Ctrl-C`.
- `tinker` and `mcp:serve` stream straight to the process stdio and leave the
  `Io` buffer empty; `mcp:serve` is registered only when the `mcp` feature is
  enabled.

### 12.2 xtask Tasks

The `cargo xtask` entrypoint (`xtask/src/main.rs`) dispatches the CI-facing
tasks:

| Task | Source | Description |
|------|--------|-------------|
| `cargo xtask deps:check` | `xtask/src/deps.rs` | Fail when a member manifest pins an inline version for a crate already managed by `[workspace.dependencies]` (`ADOPT-031`). |
| `cargo xtask lines:check` | `xtask/src/lines.rs` | Fail when any `crates/**` or `xtask/src/**` Rust source exceeds the 500-line limit (ADR-0009); wired into `cargo xtask ci` (`GAP-031`). |

---

> **Archive note (rebrand 2026-09-09):** project renamed from Rustavel to **RustaSea**.
> This document is archived as-is under the historical `Rustavel` name for traceability;
> current branding is RustaSea (`rustasea` crates, `RustaSea` prose).
