# Maud Usage History Audit

Date: 2026-02-22  
Scope: full git history for `maud` usage inside this repository  
Audit target: all code/docs/log artifacts that reference `maud` (dependency, imports, template usage, and migration notes)

## Executive Summary

1. `maud` appears in `156` commits (word-boundary match in patch lines).
2. Those commits touch `351` unique files with `1,563` line-level `maud` hits.
3. Most hits are historical artifacts, not active runtime code:
   - `912` hit lines in `docs/logs/*`
   - `482` hit lines in `crates/*`
   - `98` hit lines in `.openagents/directives/*`
4. Active dependency status today:
   - No `maud` entries in current `Cargo.toml`/`Cargo.lock`.
   - Current working tree only contains two documentation mentions:
     - `crates/wgpui/README.md:73`
     - `crates/wgpui/README.md:481`

## Methodology

Primary extraction:

```bash
git log --all -p -G'maud' --no-color > /tmp/maud_patch.log
```

Filtering rule (word-boundary match in added/removed lines only):

- match regex: `(^|[^[:alnum:]_])maud([^[:alnum:]_]|$)` (case-insensitive)
- collect `(commit, file, line)` tuples from patch hunks

Generated evidence manifests are committed under:

1. `docs/audits/2026-02-22-maud-history-commit-manifest.tsv`
2. `docs/audits/2026-02-22-maud-history-file-manifest.tsv`
3. `docs/audits/2026-02-22-maud-history-code-file-manifest.tsv`
4. `docs/audits/2026-02-22-maud-history-cargo-events.tsv`

## Timeline

Monthly commit distribution (commits with `maud` patch hits):

- `2025-12`: `148`
- `2026-01`: `7`
- `2026-02`: `1`

Historical window:

1. First word-boundary hit commit: `36d8bb6e63074d1269045bed34e03ae2bdb3348b` (2025-12-19)
2. First dependency-introduction commit: `7e848adcd05752d6b3e7005e6438ba2130c3ac81` (2025-12-19)
3. Peak activity: 2025-12-19 through 2025-12-25 (initial adoption, broad integration, then mass removal)
4. Last observed commit hit: `08d6a8cd8825eab207ef3a28fd179db99f648258` (2026-02-01, directives/history text)

## Cargo Dependency Lifecycle (Direct Evidence)

Additions:

1. `7e848ad...` adds `maud` to `crates/storybook/Cargo.toml`.
2. `daa3727...` adds `maud` to `crates/desktop/Cargo.toml` and `crates/ui/Cargo.toml`.
3. `e9703ca...` adds `maud` to `crates/autopilot/Cargo.toml`.
4. `0f148f9...` adds `maud` to `crates/agentgit/Cargo.toml`.
5. `9f4f9cd...` adds `maud` to `crates/wallet/Cargo.toml`.
6. `1a5e9e1...` adds `maud` to `crates/marketplace/Cargo.toml`.
7. `0a432dc...` adds `maud` to `crates/autopilot-gui/Cargo.toml`.
8. `c175788...` adds workspace-level `maud = "0.27.0"` in root `Cargo.toml`.
9. `35aa13d...` re-adds `maud` to `crates/gitafter/Cargo.toml` after mass-removal phase.

Removals:

1. `29b37f0...` removes `maud` from `crates/desktop/Cargo.toml`.
2. `37a78a5...` removes `maud` from root/workspace and these crates:
   - `crates/autopilot-gui`
   - `crates/autopilot`
   - `crates/gitafter`
   - `crates/marketplace`
   - `crates/storybook`
   - `crates/ui`
   - `crates/wallet`
3. `2c8e5a9...` removes remaining `maud` from `crates/gitafter/Cargo.toml`.

Result:

- Current repo has no active `maud` dependency in workspace manifests/lockfile.

## Where Maud Was Used

Historical code usage clusters (by crate, unique file count with `maud` hits):

1. `crates/ui`: `126`
2. `crates/storybook`: `84`
3. `crates/autopilot-gui`: `11`
4. `crates/gitafter`: `8`
5. `crates/desktop`: `8`
6. `crates/agentgit`: `6`
7. `crates/autopilot`: `5`
8. `crates/wallet`: `2`
9. `crates/marketplace`: `1`

Typical usage forms seen in history:

1. `use maud::{html, Markup, ...}`
2. server-rendered views/modules returning `Markup`
3. dependency declarations in per-crate and workspace Cargo manifests
4. migration/deprecation language describing removal of Maud/HTMX stacks

## Non-Code Historical Surfaces

Unique file counts containing `maud` hits:

1. `docs/logs/*`: `77` files
2. `.openagents/directives/*`: `8` files
3. other docs/root metadata (README, tests, top-level manifests): `15` files

These surfaces dominate total hit-lines and represent historical transcripts/plans more than active runtime behavior.

## Current-State Findings

1. No active `maud` dependency in current manifests.
2. No active Rust source files in current tree import `maud`.
3. Two remaining documentation references in `crates/wgpui/README.md` still mention prior Maud/HTMX architecture as historical comparison.

## Risks and Recommendations

Risks:

1. Historical references in docs/logs can create false positives during audits and migration checks.
2. README references may confuse contributors about canonical UI stack.

Recommendations:

1. Update `crates/wgpui/README.md` to mark Maud/HTMX references as explicit historical/archive-only context.
2. Add a lightweight invariant check in local CI:
   - fail if `maud` appears in active Cargo manifests or active Rust source trees (`crates/*`, `apps/*`), excluding archived/log paths.
3. Keep using append-only audit manifests (commits/files/events) so future removals/reintroductions are diffable.

## Evidence

Complete evidence for this audit is stored in:

1. `docs/audits/2026-02-22-maud-history-commit-manifest.tsv`
2. `docs/audits/2026-02-22-maud-history-file-manifest.tsv`
3. `docs/audits/2026-02-22-maud-history-code-file-manifest.tsv`
4. `docs/audits/2026-02-22-maud-history-cargo-events.tsv`
