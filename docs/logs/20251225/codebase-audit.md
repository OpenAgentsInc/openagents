# Codebase Audit - 2025-12-25

## Scope
- Scanned repository structure, workspace membership, docs, and CLI routing.
- Searched for stub/NotImplemented/TODO markers, legacy/deprecated references, and local artifacts.
- Reviewed log footprint and non-workspace crates.

This is a broad audit pass, not a line-by-line review of every module. Prior deep dives exist in `docs/logs/20251222/` and `docs/logs/20251223/`.

## Executive Summary
- Strengths: WGPUI component coverage is deep; autopilot and wgpui tests are extensive; unified binary is mostly wired.
- Critical gaps: d-012 violations (stubs/NotImplemented/TODOs) still present across wallet, marketplace, compute, ACP adapter, GPT-OSS search, and some GUI routes.
- Documentation drift: many docs still show legacy `cargo autopilot` or standalone binaries, despite the unified `openagents` binary directive.
- Repo hygiene: local SQLite databases and the `target/` directory exist at repo root; `docs/logs` is much larger than the rest of `docs`.

## High Priority Findings (d-012 / functional gaps)
1) Stubbed or NotImplemented routes in the unified GUI:
   - `src/gui/routes/wallet.rs`
   - `src/gui/routes/marketplace.rs`
   - `src/gui/routes/gitafter.rs`
   These routes return `HttpResponse::NotImplemented()` and violate d-012 for exposed UI paths.

2) Wallet/marketplace/compute stubs and TODOs:
   - `crates/wallet/src/cli/bitcoin.rs` (balance/send/history stubs)
   - `crates/wallet/src/cli/frostr.rs` (threshold signing TODOs)
   - `crates/marketplace/src/compute/consumer.rs` (job submission returns errors)
   - `crates/marketplace/src/skills/browse.rs` and `crates/marketplace/src/data/discover.rs` (browse/search not implemented)
   - `crates/compute/src/services/ollama_service.rs` (explicit not implemented errors)
   - `crates/compute/src/services/mod.rs` (Spark SDK TODO)

3) ACP and GPT-OSS stubs:
   - `crates/acp-adapter/src/client.rs` (terminal spawn TODO)
   - `crates/gpt-oss-agent/src/tools/browser.rs` (search tool returns placeholder)

4) Nostr NotImplemented errors:
   - `crates/nostr/core/src/nip07.rs`
   - `crates/nostr/core/src/nip47.rs`

5) TODO markers still in core paths (policy violation):
   - `crates/autopilot/src/daemon/nostr_trigger.rs`
   - `crates/autopilot/src/memory.rs`
   - `crates/marketplace/src/core/payments.rs`
   - `crates/claude-agent-sdk/src/query.rs`

## Medium Priority Findings (alignment and maintenance)
1) Unified CLI still defers to legacy binaries in places:
   - `src/cli/autopilot.rs` (comments and fallback behavior)
   - `src/cli/daemon.rs` (bails with a legacy command string)
   These should align with the unified `openagents` CLI expectations.

2) Documentation drift (legacy commands still referenced):
   - `docs/apm.md`
   - `docs/metrics/README.md`
   - `docs/desktop-live-streaming.md`
   - `docs/architecture/unified-binary.md`
   Many examples still use `cargo autopilot ...` or standalone binaries; update to `openagents ...`.

3) Workspace edition compliance:
   - Many `crates/*/Cargo.toml` rely on workspace inheritance instead of explicit `edition = "2024"`.
   - Repo guidance says each crate should declare `edition = "2024"` explicitly.

4) Non-workspace crates present:
   - `crates/agentgit`, `crates/claude-mcp`, `crates/codex` are present but not in the workspace list.
   - Decide whether to add them to the workspace or archive.

## Low Priority Findings (polish / housekeeping)
- `docs/logs` is 54M while `docs` is 536K; consider pruning or archiving older logs.
- `docs/logs/20251224` has restricted permissions (`drwx------`) which may complicate multi-agent workflows.
- Example/demo placeholders in storybook and UI panes are expected but should be clearly labeled as demo-only.

## Cleanup Candidates (move to ~/code/backroom)
- Local artifacts at repo root:
  - `autopilot.db`, `autopilot-metrics.db`, `autopilot-metrics.db-shm`, `autopilot-metrics.db-wal`
  - `target/`
- Old log directories under `docs/logs/` (older than the active sprint):
  - `docs/logs/20251219/`
  - `docs/logs/20251220/`
  - `docs/logs/20251221/`
  - `docs/logs/20251222/`
  - `docs/logs/20251223/`
  - `docs/logs/20251224/`
- Non-workspace crates (if unused):
  - `crates/agentgit`
  - `crates/claude-mcp`
  - `crates/codex`

## Relevance Map (what to focus on)
- High relevance (active product):
  - `crates/autopilot`, `crates/autopilot-gui`, `crates/wgpui`, `crates/compute`, `crates/marketplace`, `crates/wallet`, `crates/nostr/core`, `src/cli/*`, `src/gui/*`
- Medium relevance (supporting infra):
  - `crates/testing`, `crates/issue-tool`, `crates/issues-mcp`, `crates/gitafter`, `crates/spark`
- Lower relevance / likely archival:
  - Non-workspace crates (`crates/agentgit`, `crates/claude-mcp`, `crates/codex`)
  - Older audit/planning logs in `docs/logs/` beyond the current sprint

## Recommendations
1) Remove or implement all stub/NotImplemented paths on user-facing routes (GUI + CLI).
2) Align docs and CLI messaging to the unified `openagents` binary.
3) Explicitly add `edition = "2024"` to all crate `Cargo.toml` files to match policy.
4) Move local DB artifacts and old log bundles to `~/code/backroom` and ensure `.gitignore` coverage.
5) Standardize permissions on `docs/logs/*` directories to avoid multi-agent access issues.

## Evidence / Scans
- Stub/TODO search: `rg -n "TODO|FIXME|HACK|XXX" -g"*.rs"`
- NotImplemented search: `rg -n "unimplemented!\(|todo!\(|NotImplemented|not implemented" -g"*.rs"`
- Legacy references: `rg -n "deprecated|legacy" -g"*.md" -g"*.rs"`
- Log size: `du -sh docs/logs docs` (54M vs 536K)
