# Security Policy

RustaSea is a Rust web framework that ships authentication, session, storage,
and other security-sensitive components. We take vulnerability reports
seriously and appreciate responsible disclosure.

## Supported versions

RustaSea is pre-1.0 and has no published releases or version tags yet. The only
supported line is the latest state of the `master` branch:

| Version | Supported |
|---|---|
| `master` (latest commit) | Yes |
| Any tagged release | None published yet |

Once tagged releases exist (see the versioning plan in `CHANGELOG.md` and
`.agents/documents/tasks/roadmap.md`, section 5), this table will list the
supported minor lines. Until then, please verify a report against the current
`master` branch.

## Reporting a vulnerability

**Please do not open a public issue for a security vulnerability.**

Report privately through GitHub's private security advisory flow:

1. Go to the repository's **Security** tab:
   <https://github.com/rustasea/framework/security>
2. Select **Advisories** and then **Report a vulnerability**
   (GitHub private security advisory).
3. Provide a description, the affected crate(s) and version/commit, reproduction
   steps, and the impact you believe it has.

This route keeps the report private until a fix and advisory are ready. If you
cannot use GitHub advisories, open a minimal public issue that only asks for a
private contact channel; do not include vulnerability details in it.

Please include, where applicable:

- The affected crate(s) (for example `rustasea-auth`, `rustasea-storage`,
  `rustasea-orm`).
- The `master` commit or date you tested against.
- A minimal reproduction or proof of concept.
- The security impact and any suggested remediation.

## What to expect

- **Acknowledgement:** we aim to acknowledge a report within 3 business days.
- **Assessment:** we aim to provide an initial assessment (confirmed,
  needs-more-info, or out-of-scope) within 10 business days.
- **Fix and disclosure:** for confirmed issues, we aim to land a fix and publish
  a GitHub security advisory within 90 days of acknowledgement. If a fix takes
  longer, we will keep you updated and agree on a disclosure date with you.
- **Credit:** we are happy to credit reporters in the advisory unless you prefer
  to remain anonymous.

Please give us a reasonable window to remediate before any public disclosure.

## Scope

In scope (the framework crates under `crates/`, including the CLI):

- Authentication and authorization: `rustasea-auth` (JWT, session guard, CSRF,
  throttling, RBAC), `rustasea-authlog`, and `rustasea-activitylog`.
- Session handling (`tower-sessions`-backed guard) and password hashing
  (`argon2`).
- Data layer: `rustasea-orm` (query builder, migrations, pgvector, MongoDB) and
  the SQL/parameter handling behind it.
- HTTP and routing: `rustasea-http` (including the client and error renderers)
  and `rustasea-router`.
- Storage and integrations: `rustasea-storage` (including the SFTP disk and its
  path confinement), `rustasea-google` (service-account handling), and
  `rustasea-broadcast`.
- Validation (`rustasea-validation`) and configuration loading
  (`rustasea-config`).
- The `cargo artisan` CLI (`rustasea-cli`) and the `cargo rustasea` scaffolder
  (`cargo-rustasea`).

Out of scope:

- **Test fixtures, stubs, and example scaffolding.** Generated starter-kit code
  and anything under test fixtures is illustrative; report only if it exposes a
  real framework defect.
- **Development tooling.** The `xtask` helper crate, local Docker compose
  configuration, and CI workflow definitions are development infrastructure, not
  shipped framework code. Report issues there as normal (non-security) bugs.
- **Third-party dependencies.** Vulnerabilities in upstream crates should be
  reported to those projects. If a dependency advisory affects RustaSea, we
  track it through our supply-chain gate (see below).
- **Non-security bugs and feature requests.** Use the normal issue tracker.

## Dependency and supply-chain maintenance

RustaSea gates its dependency graph on every CI run:

- `cargo deny check` enforces the license allow-list, the advisory database,
  banned crates and duplicate versions, and source provenance. The policy lives
  in `deny.toml`.
- `cargo audit` runs the RustSec advisory scan as a required CI job.
  Unpatchable advisories are allow-listed with a documented reason in
  `deny.toml` and mirrored in `.cargo/audit.toml`. The current exceptions are
  `RUSTSEC-2023-0071` (`rsa` 0.9, which remains only through `sqlx-mysql` and a
  test dependency), `RUSTSEC-2026-0459` (the SQLite bundled by `libsqlite3-sys`,
  which `sqlx` 0.8 pins below the patched release), and the unmaintained notices
  for `chrono` and `chrono-tz` (`RUSTSEC-2026-0543`, `RUSTSEC-2026-0555`).

Both checks run in `.github/workflows/ci.yml` and can be reproduced locally:

```bash
cargo deny check
cargo audit
```

Known, accepted dependency exceptions are documented inline in `deny.toml` with
their rationale. If you believe an exception is no longer valid, please report
it.
