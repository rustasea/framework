# Gap Analysis and Traceability Matrix

> **Scope:** GAP-022 through GAP-031 (10 gaps).
> **Audit date:** 2026-09-17.
> **Sources compared:** `README.md`, `docs/` (milestones, laravel-parity, ADRs), `.agents/documents/` (requirements, design, application/modules), and the implementation under `crates/` plus `xtask/`.
> **Purpose:** Record where documentation diverges from the shipped implementation, assign priority, and trace every gap to an actionable task specification.

## Executive Summary

This audit covers ten findings produced by comparing four documentation surfaces against the mechanical state of the repository.

Three findings (GAP-022, GAP-023, GAP-024) are rated P2 because they actively mislead readers about the shipped product. The documentation understates the implementation and promotes it at the same time. `README.md` and `docs/milestones.md` claim the workspace holds 29 crates when it holds 43 plus `xtask`; they describe `route:list` as printing an empty table and the declarative attributes as having no runtime consumer when both are fully wired; and the CLI reference omits the ADOPT command surface, including `mcp:serve`, `lang:check`, `log:show`, `module:*`, `make:module`, and `make:action`.

Five findings (GAP-025 through GAP-029) are rated P3. They are internal contradictions and gaps: the `make:*` generator count is stated as 15, 16, and 17 in different places while the enum has exactly 17 variants; the Laravel parity matrix marks `Illuminate\Translation` as planned while `rustasea-i18n` and `lang:check` ship; the README Tech Stack table omits seven adopted crates; the repository root lacks `CHANGELOG.md`, `CONTRIBUTING.md`, and `SECURITY.md` despite the roadmap committing to a changelog at M0; and `show:model` is a registered placeholder that performs no introspection.

Two findings (GAP-030, GAP-031) are rated P4. They are real but lower-impact: the HTTP client's `idle_timeout` is stored and never enforced, and the 500-line file limit is documented but not enforced by `xtask` or CI.

Direction of drift is mostly "documentation lags implementation": six gaps (GAP-023, GAP-024, GAP-026, GAP-027, and the count corrections in GAP-022 and GAP-025) require documentation to catch up to code that already exists. GAP-028 requires new documents. GAP-029, GAP-030, and GAP-031 require either implementation or an explicit, tracked deferral.

## Summary Matrix

| Gap ID | Priority | Title | Layers Involved | Implementation Status |
| :--- | :--- | :--- | :--- | :--- |
| GAP-022 | P2 | Workspace crate count and manifest drift | `.agents/documents/application/modules/manifest.md`, `docs/adr/ADR-0006.md`, `README.md`, `Cargo.toml` | Implementation true (43 crates plus xtask, 44 members); all documentation stale |
| GAP-023 | P2 | Stale documentation of `route:list` and attribute consumption | `README.md`, `docs/milestones.md`, `docs/laravel-parity.md` vs `crates/rustasea-cli`, `crates/rustasea-router`, `crates/rustasea-macros` | Implemented; documentation stale |
| GAP-024 | P2 | README command surface omits ADOPT commands | `README.md` vs `crates/rustasea-cli/src/commands/` | Implemented (37 default commands plus feature-gated `mcp:serve`); README incomplete |
| GAP-025 | P3 | Internal generator count contradiction | `README.md`, `docs/milestones.md`, `.agents/documents/requirements/prd.md` vs `crates/rustasea-cli/src/generators/mod.rs` | Implemented (17 `Kind` variants); documents state 15, 16, and 17 |
| GAP-026 | P3 | Laravel parity table missing recent adoption crate status | `docs/laravel-parity.md`, `docs/milestones.md` vs adopted crates | Implemented (i18n and others, 31 ADOPT milestones); matrices stale |
| GAP-027 | P3 | README Tech Stack and feature table missing ADOPT crates | `README.md` vs `crates/rustasea/Cargo.toml` and adopted crates | Implemented; Tech Stack table and tree incomplete |
| GAP-028 | P3 | Missing repository root governance and hygiene documents | Repository root vs `.agents/documents/tasks/roadmap.md` and open-source standards | Not created (`CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md` absent) |
| GAP-029 | P3 | `show:model` remains a non-introspecting placeholder | `crates/rustasea-cli/src/commands/inspect.rs` vs `docs/milestones.md` | Partial (registered, prints file path and table name only) |
| GAP-030 | P4 | HTTP client idle timeout unenforced in runtime transport | `crates/rustasea-http/src/lib.rs` vs `README.md`, `docs/milestones.md` | Partial (`idle_timeout` stored, never applied) |
| GAP-031 | P4 | Missing 500-line file limit linter enforcement in CI / xtask | `docs/adr/ADR-0009.md` vs `xtask/` and `.github/workflows/ci.yml` | Not enforced (no `lines:check` task, no CI step) |

> **Resolution note (2026-11-18):** the Implementation Status column above is the state on the audit date. Every finding has since had a closing change land: `docs: [GAP-022] synchronize canonical crate inventory to 44 workspace packages`, `docs: [GAP-023] update stale route:list and attribute consumption documentation`, `docs: [GAP-024] document complete artisan CLI command surface in README`, `docs: [GAP-025] reconcile make:* generator count to 17 across documentation`, `docs: [GAP-026] align laravel-parity and milestones with all 31 ADOPT implementations`, `docs: [GAP-027] document ADOPT crates and umbrella features in README`, `docs: [GAP-028] add CHANGELOG, CONTRIBUTING, and SECURITY governance documents`, `feat(cli): [GAP-029] implement source-level show:model introspection with JSON output`, `feat(http): [GAP-030] enforce per-chunk idle timeout on streamed response bodies`, and `feat(xtask): [GAP-031] add lines:check 500-line linter and enforce in xtask ci`. For this note only two oracles were re-checked on the current tree: GAP-028 (the three root documents exist and are linked from `README.md`) and GAP-031 (`cargo xtask lines:check` runs and passes). The other falsification checks below were not re-run, so treat their closure as recorded, not re-verified. Documentation has drifted again since the audit; the current as-built status is [`docs/milestones.md`](../milestones.md).

Legend for Implementation Status: **Implemented** means the code exists and is reachable. **Partial** means the code exists but does not deliver the documented behaviour. **Not created** / **Not enforced** means the artifact does not exist. In every case the gap describes the documentation or enforcement shortfall, not a missing feature.

## Detailed Breakdown

Each subsection states the gap, its priority, the evidence layers, and a link to the task file that carries the full analysis and acceptance criteria.

### GAP-022: Workspace Crate Count and Manifest Drift (P2)

- **Layers:** `.agents/documents/application/modules/manifest.md:14`, `docs/adr/ADR-0006.md:19`, `README.md:454-459`, `docs/milestones.md:35`, `Cargo.toml:1-3`.
- **Core finding:** `manifest.md` claims 21 crates plus `xtask`; `README.md` and `milestones.md` claim 29 plus `xtask`; the `crates/*` glob expands to 43 crate directories plus `xtask`, for 44 members.
- **Action:** Reconcile all documents to the true 44-member inventory or automate derivation from `cargo metadata`.
- **Task:** [GAP-022](../../.agents/documents/_tasks/GAP-022.md)

### GAP-023: Stale Documentation of route:list and Attribute Consumption (P2)

- **Layers:** `README.md:129,151`, `docs/milestones.md:44,46,95,167,328-330`, `docs/laravel-parity.md:63,153` vs `crates/rustasea-cli/src/routes.rs`, `crates/rustasea-app/src/bootstrap/app.rs`, `crates/rustasea-cli/src/commands/inspect.rs`, `crates/rustasea-router/src/metadata.rs`, `dispatch.rs`, `authorize.rs`.
- **Core finding:** Documentation still reports an empty `route:list` table and unconsumed attribute metadata; both are implemented through `RouteSource`, the live six-column table, `MiddlewareRegistry`, and `AuthorizeRegistry`.
- **Action:** Update `README.md`, `docs/milestones.md`, and `docs/laravel-parity.md` to implemented status with `path:line` evidence.
- **Task:** [GAP-023](../../.agents/documents/_tasks/GAP-023.md)

### GAP-024: README Command Surface Omits ADOPT Commands (P2)

- **Layers:** `README.md:125,170,505` vs `crates/rustasea-cli/src/commands/` (registry and command modules).
- **Core finding:** The CLI registers 37 default commands plus the feature-gated `mcp:serve`; the README documents only the older subset and omits `mcp:serve`, `tinker` details, `lang:check`, `log:show`, `module:list`/`enable`/`disable`, `make:module`, `make:action`, and `make:test --browser`.
- **Action:** Expand the README command reference to cover all 37 default commands plus the gated command, with exact flags.
- **Task:** [GAP-024](../../.agents/documents/_tasks/GAP-024.md)

### GAP-025: Internal Generator Count Contradiction (P3)

- **Layers:** `README.md:170`, `docs/milestones.md:48,247`, `.agents/documents/requirements/prd.md:143` vs `crates/rustasea-cli/src/generators/mod.rs:19-54`.
- **Core finding:** The generator count is stated as 15, 16, and 17 in different places; the `Kind` enum has exactly 17 variants.
- **Action:** Reconcile to 17 across `README.md`, `docs/milestones.md`, and `prd.md`.
- **Task:** [GAP-025](../../.agents/documents/_tasks/GAP-025.md)

### GAP-026: Laravel Parity Table Missing Recent Adoption Crate Status (P3)

- **Layers:** `docs/laravel-parity.md:63,84,85,86,98,153` and `docs/milestones.md` (ADOPT references) vs `crates/rustasea-i18n`, `rustasea-mail`, `rustasea-view`, and the adopted crates.
- **Core finding:** `Illuminate\Translation` is marked planned though `rustasea-i18n` and `lang:check` ship; `docs/milestones.md` references only 9 of the 31 ADOPT IDs, leaving 22 implementations uncross-referenced.
- **Action:** Update both documents with complete, current statuses for all Illuminate packages and all 31 ADOPT milestones.
- **Task:** [GAP-026](../../.agents/documents/_tasks/GAP-026.md)

### GAP-027: README Tech Stack and Feature Table Missing ADOPT Crates (P3)

- **Layers:** `README.md:193-215` (Tech Stack) and `README.md:402-433` (tree) vs `crates/rustasea/Cargo.toml` and the adopted crate manifests.
- **Core finding:** The Tech Stack table and tree omit `rustasea-i18n`, `rustasea-excel`, `rustasea-image`, `rustasea-modules`, `rustasea-action`, `rustasea-google`, and the `rustasea-storage` SFTP disk, along with their dependencies.
- **Action:** Extend the table and tree with the adopted crates and their underlying dependencies, noting optional umbrella features.
- **Task:** [GAP-027](../../.agents/documents/_tasks/GAP-027.md)

### GAP-028: Missing Repository Root Governance and Hygiene Documents (P3)

- **Layers:** Repository root vs `.agents/documents/tasks/roadmap.md:22,148` and open-source standards.
- **Core finding:** `CHANGELOG.md`, `CONTRIBUTING.md`, and `SECURITY.md` are absent; the roadmap states the changelog is initiated at M0.
- **Action:** Create all three documents covering M0-M6 and ADOPT-001..031 history, the development workflow and `xtask` surface, and a vulnerability disclosure policy.
- **Task:** [GAP-028](../../.agents/documents/_tasks/GAP-028.md)

### GAP-029: show:model Remains a Non-Introspecting Placeholder (P3)

- **Layers:** `crates/rustasea-cli/src/commands/inspect.rs:103-145` vs `docs/milestones.md:96`.
- **Core finding:** `show:model` prints only the model name, the computed file path, and a derived table name; it inspects no columns, types, relations, casts, or hidden attributes.
- **Action:** Implement schema inspection with `--json` support, or document `show:model` as a tracked roadmap enhancement and correct the milestone status.
- **Task:** [GAP-029](../../.agents/documents/_tasks/GAP-029.md)

### GAP-030: HTTP Client Idle Timeout Unenforced in Runtime Transport (P4)

- **Layers:** `crates/rustasea-http/src/lib.rs:325,405-412` vs `README.md:129`, `docs/milestones.md:95`.
- **Core finding:** `HttpClient::idle_timeout` is stored but `send_with` applies only the total request timeout to reqwest; the idle timeout is never read at runtime.
- **Action:** Implement a body-streaming idle timeout watcher, or document the known limitation and the route-level mitigation.
- **Task:** [GAP-030](../../.agents/documents/_tasks/GAP-030.md)

### GAP-031: Missing 500-Line File Limit Linter Enforcement in CI / xtask (P4)

- **Layers:** `docs/adr/ADR-0009.md` and the coding standards vs `xtask/src/main.rs` and `.github/workflows/ci.yml`.
- **Core finding:** ADR-0009 mandates a 500-line limit with a documented `fsd.md` exemption, but no `xtask` task or CI step counts lines, so files can exceed the limit without a gate failing.
- **Action:** Add a `lines:check` subcommand to `xtask` (with ADR exemptions) and include it in `cargo xtask ci`.
- **Task:** [GAP-031](../../.agents/documents/_tasks/GAP-031.md)

## Methodology

The audit followed a fixed comparison protocol so that findings are reproducible and each one can be independently falsified.

1. **Select comparable surfaces.** For each topic, pair a documentation artifact with the mechanical source it describes: `Cargo.toml` for the workspace, the command registry for the CLI, the generator enum for `make:*`, crate manifests for dependencies, and the relevant command or library source for behaviour.
2. **Extract claims as discrete assertions.** Each sentence that states a count, a status, or a behaviour was reduced to an assertion with a source location (`path:line`).
3. **Verify against the mechanical source.** Counts were checked by enumerating directories or reading enum variants; behaviour was checked by reading the exact function and its call sites, following the chain from registration to runtime use.
4. **Classify the drift.** Findings were labelled as documentation-lagging (code ahead), missing-artifact (document or gate absent), or partial (code present but not delivering the documented behaviour).
5. **Assign priority.** P2 for findings that actively mislead about shipped capability, P3 for internal contradictions and governance gaps, P4 for real but low-impact enforcement and behavioural gaps.
6. **Trace to tasks.** Each finding was written up as a task file under `.agents/documents/_tasks/` with the mandated three-section structure: Context and Analysis, Step and Implementation, Acceptance and Verification.

## Falsifiable Audit Criteria

Every finding is stated so that it can be disproved by a mechanical check. The criteria below are the oracles a reviewer should run. A criterion that cannot fail does not verify anything; each of these can fail.

| Gap | Falsification check | Expected result when the gap is closed |
| :--- | :--- | :--- |
| GAP-022 | Count `crates/*` directories plus `xtask` and compare with the number stated in each document | All documents state the same count, equal to the enumerated total |
| GAP-023 | Grep for "empty table", "nothing consumes", "no runtime consumer", "metadata only" in the affected docs | Zero hits for `route:list`, `#[middleware]`, and `#[authorize]` |
| GAP-024 | Extract registered command names from the CLI registry and diff against the README table | Sets are equal; every command documented, no phantom entries |
| GAP-025 | Count `Kind` variants and compare with each stated generator count | Every document states 17 and lists the same 17 names |
| GAP-026 | For each Illuminate row marked planned, assert no corresponding crate exists; diff ADOPT ID sets between the two documents | No planned row has an implemented crate; ADOPT ID sets are equal |
| GAP-027 | For each dependency named in the README table, confirm it exists in the crate's `Cargo.toml` | Every named dependency resolves; all seven adopted crates appear |
| GAP-028 | Assert `CHANGELOG.md`, `CONTRIBUTING.md`, and `SECURITY.md` exist at the repository root | All three paths exist and are linked from `README.md` |
| GAP-029 | Run `show:model` against a known model and inspect the output | Real schema fields appear, or the command is explicitly documented as deferred |
| GAP-030 | Stalled-body request against a slow server, with an idle timeout configured | Request aborts within the idle window, or the limitation is documented as accepted |
| GAP-031 | Run `xtask lines:check` with a file grown past 500 lines; confirm `fsd.md` is exempt | Non-zero exit and a report for the violation; `fsd.md` does not trigger a failure |

A finding counts as closed only when its falsification check passes on the current tree and the documented status matches the check result. Passing checks are evidence; assertions in prose are not.
