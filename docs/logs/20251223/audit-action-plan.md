# Consolidated Audit Action Plan
**Date:** 2025-12-23
**Source:** Five audit reports (1705-audit.md, 1719-audit-deep.md, 1734-audit-comprehensive.md, 1735-audit-extended.md, 1747-audit-more-comprehensive.md)

---

## Executive Summary

The audits reveal significant progress on ACP integration, parallel autopilot tooling, local inference infrastructure, and metrics/APM tracking. However, **critical blockers** prevent core functionality from working:

1. **Parallel autopilot containers are fundamentally broken** (cannot run issue MCP, shared DB not wired)
2. **d-012 stub violations** block wallet/marketplace/compute/ACP flows in default CLI/GUI
3. **Bifrost cryptographic core is non-functional** (ECDH/signing unimplemented)
4. **Container infrastructure has multiple failure modes** (healthcheck, resource limits, fragile CLI install)

---

## Phase 1: Unblock Parallel Autopilot (CRITICAL)

These issues prevent parallel autopilot containers from functioning at all.

### 1.1 Fix Issue MCP in Containers
**Problem:** `--with-issues` writes `.mcp.json` that runs `cargo run -p issues-mcp`, but Docker image has no Cargo.
**Files:** `crates/autopilot/src/main.rs:1156`, `docker/autopilot/Dockerfile:18`, `docker/autopilot/Dockerfile:62`
**Fix:** Bundle pre-built `issues-mcp` binary in image OR change MCP config to use bundled binary path.

### 1.2 Wire Shared Issues DB
**Problem:** Compose sets `ISSUES_DB=/shared/autopilot.db` but autopilot ignores it; entrypoint omits `--issues-db`.
**Files:** `docker/autopilot/docker-compose.yml:23`, `crates/autopilot/src/cli.rs:67`, `crates/autopilot/src/lib.rs:1060`
**Fix:** Make autopilot read `ISSUES_DB` env var and update entrypoint to pass `--issues-db $ISSUES_DB`.

### 1.3 Fix Container Healthcheck
**Problem:** Healthcheck uses `pgrep` but `procps` not installed.
**Files:** `docker/autopilot/Dockerfile:56`
**Fix:** Install `procps` OR change healthcheck to `test -f /tmp/healthy` and touch that file in entrypoint.

### 1.4 Configure SQLite for Concurrent Access
**Problem:** No WAL/busy_timeout set; parallel agents will hit "database is locked".
**Files:** `crates/issues/src/db.rs:20`, `crates/marketplace/src/db.rs:14`, `crates/autopilot/src/metrics/mod.rs:379`
**Fix:** Add `PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;` after connection opens.

---

## Phase 2: Container Infrastructure Hardening (HIGH)

### 2.1 Fix Resource Limits for Non-Swarm
**Problem:** `deploy.resources` ignored by `docker-compose` (only works in Swarm mode).
**Files:** `docker/autopilot/docker-compose.yml:29`
**Fix:** Use Compose v2 `mem_limit` and `cpus` instead of `deploy.resources`.

### 2.2 Make CLI Install Robust
**Problem:** `curl | bash` ignores failures; containers run without agent CLI.
**Files:** `docker/autopilot/Dockerfile:41`
**Fix:** Add `set -e` before install OR pre-bake CLI in image OR fail build on install error.

### 2.3 Support Both docker-compose and docker compose
**Problem:** Rust orchestration hardcodes `docker-compose` binary.
**Files:** `crates/autopilot/src/parallel/docker.rs:72`
**Fix:** Check for `docker compose` first, fall back to `docker-compose`.

---

## Phase 3: d-012 Stub Elimination (CRITICAL for User-Facing Features)

Eliminate all "not implemented" paths that block real functionality. Prioritize by user impact.

### 3.1 ACP Integration (Highest Impact)
| Location | Issue |
|----------|-------|
| `src/gui/routes/acp.rs:224` | Prompt endpoint is TODO |
| `src/gui/routes/acp.rs:268` | Cancel endpoint is TODO |
| `crates/acp-adapter/src/client.rs:227` | Terminal creation stub (no PTY spawn) |

**Fix:** Wire prompt/cancel to actual ACP connection layer; implement terminal spawning with PTY.

### 3.2 Wallet Integration
| Location | Issue |
|----------|-------|
| `src/gui/routes/wallet.rs:14` | GUI routes return NotImplemented |
| `crates/wallet/src/cli/bitcoin.rs:5` | All payment flows hard-fail |
| `crates/wallet/src/gui/server.rs:111` | Send/receive/history placeholders |
| `crates/wallet/src/cli/identity.rs:381` | Contacts need relay integration |

**Fix:** Complete Spark SDK integration per d-001; wire Nostr relay for contacts.

### 3.3 Marketplace/Compute Integration
| Location | Issue |
|----------|-------|
| `src/gui/routes/marketplace.rs:15` | GUI routes return NotImplemented |
| `crates/marketplace/src/compute/consumer.rs:260` | Job submission stubbed |
| `crates/marketplace/src/skills/browse.rs:248` | Skills browse stubbed |
| `crates/marketplace/src/data/discover.rs:289` | Data discover stubbed |
| `crates/compute/src/services/relay_service.rs:82` | Relay publish/subscribe stubbed |
| `crates/marketplace/src/cli/compute.rs:516` | Cancel is placeholder |

**Fix:** Implement Nostr relay client and integrate across compute/skills/data.

### 3.4 GitAfter
| Location | Issue |
|----------|-------|
| `src/gui/routes/gitafter.rs:13` | GUI routes return NotImplemented |
| `crates/gitafter/src/server.rs:3724` | LNURL payment not implemented |
| `crates/gitafter/src/stacks/restack.rs:215` | Base layer rebase not implemented |

---

## Phase 4: Cryptographic Core (CRITICAL for Sovereignty)

### 4.1 Implement Bifrost ECDH/Signing
**Problem:** Threshold ECDH aggregation unimplemented; E2E tests expect failure.
**Files:** `crates/frostr/src/bifrost/aggregator.rs:190`, `crates/frostr/src/bifrost/node.rs:556`, `crates/frostr/tests/bifrost_e2e.rs:121`
**Fix:** Implement threshold ECDH aggregation; convert tests to expect success.

---

## Phase 5: Security Fixes (HIGH)

### 5.1 Fix GPT-OSS Path Traversal
**Problem:** apply_patch falls back to non-canonicalized path when target doesn't exist, allowing `../` traversal.
**Files:** `crates/gpt-oss-agent/src/tools/apply_patch.rs:83`
**Fix:** Canonicalize parent directory and validate prefix check after resolution.

### 5.2 Review Permission Bypass
**Problem:** Autopilot runs with `dangerously_skip_permissions(true)` in run/resume paths.
**Files:** `crates/autopilot/src/main.rs:1130`, `crates/autopilot/src/main.rs:1942`
**Fix:** Evaluate if this is intentional; if so, document risks; if not, enable permission checks.

---

## Phase 6: Portability & Consistency (MEDIUM)

### 6.1 Make Daemon Portable
**Problem:** Uses Unix domain sockets and `tokio::signal::unix`; builds fail on Windows.
**Files:** `crates/autopilot/src/daemon/control.rs:9`, `crates/autopilot/src/bin/autopilotd.rs:167`
**Fix:** Add `#[cfg(unix)]` gates OR implement Windows alternatives OR document Unix-only.

### 6.2 Fix Agent CLI/Runtime Mismatch
**Problem:** CLI advertises `gpt-oss` but runtime only supports `claude`/`codex`.
**Files:** `crates/autopilot/src/cli.rs:31`, `crates/autopilot/src/main.rs:1199`
**Fix:** Either implement gpt-oss support OR remove from CLI options.

### 6.3 Fix GUI Blocking DB Access
**Problem:** Blocking rusqlite calls in async handlers; fixed DB paths ignore config.
**Files:** `crates/autopilot-gui/src/server/routes.rs:46`, `crates/autopilot-gui/src/sessions.rs:38`
**Fix:** Use `spawn_blocking` or async SQLite driver; make DB path configurable.

### 6.4 Add Worktree Cleanup Confirmation
**Problem:** `--force` and `-D` used without confirmation in cleanup.
**Files:** `crates/autopilot/src/parallel/worktree.rs:116`, `scripts/parallel-autopilot.sh:177`
**Fix:** Add `--dry-run` mode and require `--force` flag for destructive operations.

---

## Phase 7: Testing & Verification (MEDIUM)

### 7.1 Implement Real E2E Tests
**Problem:** Marketplace tests are conceptual; NIP-SA tests missing; Bifrost tests expect failure.
**Files:**
- `crates/marketplace/tests/compute_e2e.rs:58`
- `crates/marketplace/tests/skill_e2e.rs:1`
- Missing: `crates/nostr/tests/nip_sa.rs`

**Fix:** Implement relay-backed E2E tests for d-014/d-015; add NIP-SA integration tests.

### 7.2 Fix Non-Assertive Tests
**Problem:** Tests skip on missing DB without asserting expected behavior.
**Files:** `crates/autopilot-gui/src/sessions.rs:146`
**Fix:** Use in-memory DB or fixtures; add actual assertions.

---

## Phase 8: Cleanup & Housekeeping (LOW)

### 8.1 Fix GPT-OSS Default Model
**Files:** `crates/gpt-oss/src/client.rs:8`
**Fix:** Change default from `gpt-4o-mini` to a valid GPT-OSS model name.

### 8.2 Fix Unbuildable Crate
**Files:** `crates/claude-mcp/Cargo.toml:10`
**Fix:** Add `src/main.rs` or remove bin target; add to workspace members.

### 8.3 Implement Remaining Stubs
- `crates/autopilot/src/context_analysis.rs:94` - Context loss analyzer
- `crates/autopilot/src/model_selection.rs:484` - TaskComplexity deserialization
- `crates/gpt-oss-agent/src/tools/browser.rs:134` - Browser search
- `crates/wallet/src/cli/settings.rs:30` - Wallet settings set
- `crates/autopilot/src/planmode.rs:270` - Swarm launch

### 8.4 Align Documentation
- Docker compose `AGENT_COUNT` referenced but not used
- `autopilot` binary vs `openagents` binary guidance conflicts
- Known-good binary instructions need update

---

## Process Issues to Address

### Git Policy Violations Observed in rlogs
- Force pushes to `main` with `--force-with-lease`
- Commit amendments without explicit request
- Test runs killed mid-execution

**Action:** Reinforce in CLAUDE.md; add pre-push hooks if needed.

---

## Recommended Execution Order

```
Week 1: Phase 1 (Unblock Parallel Autopilot)
        Phase 2 (Container Hardening)
        Phase 5 (Security Fixes)

Week 2: Phase 3.1 (ACP Integration)
        Phase 4 (Bifrost Cryptography)
        Phase 6.1-6.2 (Critical Portability)

Week 3: Phase 3.2-3.4 (Remaining d-012 Stubs)
        Phase 6.3-6.4 (Remaining Portability)
        Phase 7 (Testing)

Week 4: Phase 8 (Cleanup)
```

**Note:** Phases 1-2 and Phase 5 should be completed first as they are either completely blocking (parallel autopilot) or security-sensitive. Phase 3 depends on Nostr relay integration being available, so it may require parallel work to build that foundation.
