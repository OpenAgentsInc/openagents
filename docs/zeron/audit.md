# Zeron — Senior-Advisor Codebase Audit

> Strictly read-only architectural, security, reliability, performance, testing, and maintainability audit of `zeronsh/zeron` (v`0.3.9`). No secret values are reproduced anywhere in this report.

---

## Executive Summary

Zeron is an unusually ambitious, well-engineered native codebase (~524K lines of Rust across 33 crates + `apps/zgx`, ~296K lines of Luau UI, and a substantial TypeScript Cloudflare Workers/DO control plane). Its **security architecture is notably mature** for a fast-moving product: the two-gate localhost IPC auth with separate user/agent tokens, the HTTP-less `zeron-vault` credential worker, the `zeron-plugin-guard` SSRF/command sandbox, and runner JWT scoping are all thoughtfully designed.

The primary risks are **structural scale and drift** rather than foundational security flaws: several monolithic "god files" (6K–9K lines), a large `apps/zgx` separate workspace pinned to a single upstream git commit, stale top-level `ARCHITECTURE.md` documentation still describing the retired GPUI frontend, a few subtle edge/IPC hardening opportunities, and duplicated UI template trees.

---

## 1. Architectural Strengths & Notable Design Decisions

1. **Defense-in-Depth on Localhost IPC (`crates/rpc/src/server.rs:290-330`, `crates/rpc/src/ipc_auth.rs:1-30,196-227`):**
   - Rejects any WebSocket handshake containing an `Origin` header (blocking browser-based CSWSH attacks) *and* requires a 32-byte owner-only bearer token compared in constant time.
   - Separates full user authority (`ipc-token`) from agent MCP authority (`ipc-token-agent`), enforced by `AgentGuard` (`crates/rpc/src/server.rs:256-286`) which both allowlists methods and pins calling-chat methods to the agent's own `chatId`.
2. **Zero-HTTP-Surface Credential Vault (`edge/vault/wrangler.jsonc:1-15`, `edge/vault/src/index.ts:7`, `edge/vault/src/vault.ts:1-20`):**
   - Isolates third-party credentials (GitHub App / provider tokens) into a separate Cloudflare Worker reachable exclusively via Service Binding RPC (`VaultApi`), with global (`VAULT_DISABLED=1`) and per-user kill switches, device enrollment/revocation, and a separate required-reviewer `vault-production` deployment environment (`.github/workflows/deploy.yml:59-84`).
3. **Shared Plugin Sandbox (`crates/plugin-guard/src/commands.rs`, `crates/plugin-guard/src/net.rs`):**
   - Both engine services and desktop UI plugins share one guard crate that enforces no-shell execution, strict `env` allowlists (blocking `LD_*`, `DYLD_*`, `NODE_OPTIONS`, `PYTHON*`, `GIT_*`, `BASH_ENV`), `git`/`gh` subcommand and flag sanitization (`core.hooksPath=/dev/null`, `core.fsmonitor=false`), and post-DNS / per-redirect IP classification blocking RFC1918, CGNAT/Tailscale, and cloud metadata (`169.254.169.254`, `100.100.100.200`, `fe80::/10`).
4. **Zero `TODO`/`FIXME`/`HACK` Debt Markers in Production Rust:**
   - Automated scan across all 788 `.rs` files found `0` `TODO`/`FIXME`/`HACK`/`XXX` comments in non-test code, reflecting strong hygiene and explicit design docs in `docs/`.
5. **Ordered Terminal Lanes (`crates/rpc/src/server.rs:25-44`):**
   - Routing `WRITE_TERMINAL` and `RESIZE_TERMINAL` through per-terminal ordered lanes prevents tokio task scheduling from reordering keystrokes or terminal escape replies.

---

## 2. Concrete Findings & Prioritized Recommendations

### P0 / High Priority

#### F-1. [Security / Edge] Query-String Bearer Token Accepted on All HTTP Routes, Not Just WebSocket Upgrades
- **Evidence:** `edge/src/auth.ts:44-49` (`bearerFromRequest`) falls back to `url.searchParams.get("token")` unconditionally for *every* request (including plain HTTPS API calls and `auth-routes.ts:146,203,295`), even though the doc comment (`edge/src/auth.ts:3-5`) states `?token=` exists because WebSocket clients cannot always set headers.
- **Risk:** Accepting JWTs in URL query parameters on standard HTTP endpoints increases the risk of bearer leakage into proxies, referrers, browser history, or access/error logs if any client accidentally sends `?token=` on non-WS requests.
- **Recommendation:** Restrict `?token=` extraction to WebSocket upgrade requests (`request.headers.get("upgrade")?.toLowerCase() === "websocket"`) or require short-lived, single-use WS tickets.

#### F-2. [Security / Supply Chain] Unpinned GitHub Actions Tags in `deploy.yml`
- **Evidence:** `.github/workflows/deploy.yml:41,70,71` uses mutable tags (`actions/checkout@v4`, `actions/setup-node@v4`) in the production `vault` and `edge` deployment workflow, whereas `.github/workflows/release.yml:36,75` pins exact 40-character commit SHAs.
- **Risk:** `deploy.yml` holds `CLOUDFLARE_API_TOKEN` for both `vault-production` and `cloudflare` production environments; mutable action tags widen supply-chain exposure.
- **Recommendation:** Pin every `uses:` in `deploy.yml` (and audit all 19 workflows) to full 40-char commit SHAs.

#### F-3. [Reliability / DoS] Unbounded Per-Terminal Channel in RPC Server
- **Evidence:** `crates/rpc/src/server.rs:38` uses `mpsc::unbounded_channel` for each terminal's ordered lane (`lane()`), and `crates/rpc/src/server.rs:41` spawns an unbounded number of `respond()` tasks per lane message.
- **Risk:** A misbehaving local client or runaway script pasting/streaming into a terminal can grow memory without backpressure if the PTY or downstream consumer stalls.
- **Recommendation:** Replace `mpsc::unbounded_channel` with a bounded channel (e.g., 256–1024 frames) with explicit backpressure or slow-client disconnect.

---

### P1 / Medium Priority

#### F-4. [Reliability] Non-Test `.unwrap()` on Mutex Locks in Desktop RPC Client
- **Evidence:** `apps/zgx/src/engine/rpc.rs:103,118,391,398,429,436` calls `.lock().unwrap()` on shared `Mutex`es in production code (6+ sites). Across the repo, 572 `.unwrap()` calls appear before the first `#[cfg(test)]` marker (many are in `#[cfg(all(test, unix))]` or `examples/`, such as `crates/engine/src/remote_terminals.rs:86-128`, which is test-only, but `apps/zgx/src/engine/rpc.rs` is live desktop client code).
- **Risk:** Any panic while holding an inbox/state lock poisons the mutex and cascades into panics across all RPC readers/writers in the desktop UI.
- **Recommendation:** Use a poison-recovering helper (`unwrap_or_else(PoisonError::into_inner)`) or `parking_lot::Mutex` consistently (as `remote_terminals.rs:28` already does in its test harness), and enable `clippy::unwrap_used` on production modules.

#### F-5. [Maintainability / Architecture] Extreme File Concentration ("God Files")
- **Evidence:** Top production source files by line count:
  - `apps/zgx/src/chat.rs`: **9,091 lines**
  - `crates/harness/src/acp/mod.rs`: **8,490 lines**
  - `crates/engine/src/doc_host.rs`: **8,476 lines**
  - `crates/engine/src/sessions.rs`: **8,236 lines**
  - `crates/engine/src/rpc.rs`: **8,086 lines**
  - `crates/engine/src/harness_updates.rs`: **7,051 lines**
  - `crates/engine/src/agent_accounts.rs`: **6,896 lines**
  - `crates/harness/src/opencode/mod.rs`: **6,368 lines**
  - `apps/zgx/ui/ds/sidebar.luau`: **5,287 lines**
  - `edge/src/cloud/cloud-account.ts`: **~120 KB**
- **Risk:** Slow incremental Rust compilation, high merge-conflict frequency, difficult code review, and blurred module boundaries.
- **Recommendation:** Split `doc_host.rs`, `sessions.rs`, `rpc.rs`, `acp/mod.rs`, and `apps/zgx/src/chat.rs` into focused submodules (e.g., by RPC domain, lifecycle state, and transport/codec) and decompose `cloud-account.ts` into state machine + persistence + HTTP handler modules.

#### F-6. [Documentation Drift] `ARCHITECTURE.md` Still Describes the Retired GPUI Frontend
- **Evidence:** `ARCHITECTURE.md:4,11,19,27,186,195-245,299-338` describes Zeron's desktop UI as a `gpui` / `zeron-ui` crate pinned to Zed, whereas `apps/zeron/src/main.rs` has no GUI window (`this zeron has no window`), `crates/ui` no longer exists in `Cargo.toml`, and the actual shipping desktop app is `apps/zgx` (Rust + Luau, built by `.github/workflows/release.yml` and `zgx-build.yml`).
- **Risk:** Misleads new contributors and automated tools about where UI code lives and how desktop releases are built.
- **Recommendation:** Update `ARCHITECTURE.md` to reflect the `apps/zgx` + Luau desktop architecture, the 33-crate workspace, and the `edge/vault` split.

#### F-7. [Security / Unsafe Rust Surface] 466 Non-Test `unsafe` Lines Concentrated in Platform FFI
- **Evidence:** Static scan found **466** non-test `unsafe` lines across Rust files, led by:
  - `apps/zgx/src/appshots/macos.rs` (95 lines — macOS window/screen capture FFI)
  - `crates/notify/src/lib.rs` (24 lines)
  - `crates/preview/src/discovery.rs` (22 lines)
  - `crates/harness/src/process.rs` (18 lines)
  - `apps/zgx/src/plugins/file_events.rs` & `file_events_windows.rs` (16 lines each)
  - `crates/update/src/windows.rs` (15 lines) & `apps/zeron/src/allocator.rs` (15 lines)
- **Risk:** While platform FFI legitimately requires `unsafe`, high concentration in screenshot capture, process inspection, and updater code carries memory-safety risk on malformed OS responses.
- **Recommendation:** Enforce `// SAFETY:` invariants via `clippy::undocumented_unsafe_blocks` across the workspace and isolate raw macOS/Windows FFI behind small, unit-tested safe wrappers.

---

### P2 / Low Priority & Tech Debt

#### F-8. [Maintainability] Duplicated Luau UI Tree (`apps/zgx/ui/ds/` vs. `apps/zgx/ui/templates/zeron-v2/ds/`)
- **Evidence:** `apps/zgx/ui/ds/sidebar.luau` (5,287 lines) and `apps/zgx/ui/templates/zeron-v2/ds/sidebar.luau` (5,129 lines) coexist alongside a 420 KB `templates/zeron-v2` tree (confirmed by recent git history: `#1517 Zeron v2: in step with the default's page after #1515`).
- **Risk:** Bug fixes and accessibility/rendering improvements must be manually ported between `ds/` and `templates/zeron-v2/`, inviting silent UI regressions.
- **Recommendation:** Factor shared logic into common modules or complete the `zeron-v2` transition and retire the duplicate tree.

#### F-9. [Build / Dependency Hygiene] Two Separate Cargo Workspaces + Git-Pinned Fork Dependencies
- **Evidence:** `apps/zgx/Cargo.toml:35-65,165-194` is excluded from the root Cargo workspace and pins 14 crates (`zgx*`, `wgpu-hal`, `winit`, `accesskit_*`) to a single git revision (`b8f298b6b9d1a363adf9e21655334a6e260dc4c7`) of `zeronsh/zgx`.
- **Risk:** Duplicates `Cargo.lock` dependency resolution, increases CI build times/cache pressure, and complicates security patching of transitive crates (`winit`, `wgpu-hal`).
- **Recommendation:** Keep `cargo audit` / `cargo deny` running on *both* `Cargo.lock` and `apps/zgx/Cargo.lock` in CI, and document the `zgx` fork bump procedure.

#### F-10. [Edge / Maintenance] Legacy `loro-crdt-legacy` Dual-Version Dependency & `wrangler` Version Skew
- **Evidence:** `edge/package.json` carries both `loro-crdt` (`^1.17.1`) and `loro-crdt-legacy` (`npm:loro-crdt@1.13.7`), and `wrangler` is `^4.151.1` in `edge/package.json` vs. `^4.111.0` in `edge/vault/package.json`. `edge/src/index.ts:227-235` also carries legacy `s2/` session rooms alongside `chat2` rooms.
- **Risk:** Bundle bloat and behavioral divergence between `edge/` and `edge/vault/` tooling.
- **Recommendation:** Align `wrangler` versions between `edge/` and `edge/vault/` and set a deprecation milestone for legacy Loro / `SessionRoom` compatibility shims once all active clients are on `chat2`.

#### F-11. [Repo Hygiene] 46,082-Line Demo Snapshot File Committed in `apps/zgx/ui/demos/`
- **Evidence:** `apps/zgx/ui/demos/files_snapshot.luau` is 46,082 lines (~15% of all `.luau` lines in the repo), and 444 `.png` files are committed in-tree.
- **Risk:** Inflates clone size, grep noise, and language-metric distortion.
- **Recommendation:** Move large generated fixtures/screenshots to compressed test fixtures or Git LFS / CI artifacts where feasible.

---

## 3. Summary Priority Roadmap

| Priority | Action | Primary Files |
| --- | --- | --- |
| **P0** | Restrict `?token=` auth fallback to WebSocket upgrades only | `edge/src/auth.ts:44-49` |
| **P0** | Pin all GitHub Actions in `deploy.yml` to 40-char commit SHAs | `.github/workflows/deploy.yml:41,70-71` |
| **P0** | Bound per-terminal RPC lanes with backpressure | `crates/rpc/src/server.rs:38` |
| **P1** | Eliminate production `.lock().unwrap()` panics in `apps/zgx` RPC client | `apps/zgx/src/engine/rpc.rs:103,118,391-436` |
| **P1** | Update `ARCHITECTURE.md` to reflect `apps/zgx` (Luau) & `edge/vault` | `ARCHITECTURE.md` |
| **P1** | Enable `clippy::undocumented_unsafe_blocks` & audit `macos.rs` FFI | `apps/zgx/src/appshots/macos.rs` |
| **P1** | Decompose 7K–9K line monoliths (`chat.rs`, `doc_host.rs`, `sessions.rs`, `rpc.rs`, `acp/mod.rs`) | `crates/engine/src/`, `crates/harness/src/acp/`, `apps/zgx/src/` |
| **P2** | Deduplicate `apps/zgx/ui/templates/zeron-v2/` and align `edge/` vs. `edge/vault/` `wrangler` | `apps/zgx/ui/`, `edge/vault/package.json` |
