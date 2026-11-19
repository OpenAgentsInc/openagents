# Zeron — Technical Architecture Deep Dive

This document describes the **actual current architecture** of `zeronsh/zeron` (workspace version `0.3.9`, Rust edition `2024`), noting where the codebase has evolved past earlier design notes in `ARCHITECTURE.md` (notably the migration of the desktop frontend from GPUI to `zgx` + Luau).

---

## 1. High-Level System Topology

Zeron separates **UI viewports** from a **background Rust engine** (`zeron-engine`) and an optional **Cloudflare Workers / Durable Objects edge** (`edge/`) plus an isolated **Credential Vault** (`edge/vault/`).

```text
┌──────────────────────────┐        ┌──────────────────────────┐
│ Desktop Viewport (zgx)   │        │ CLI / Headless Daemon    │
│ apps/zgx (Rust + Luau)   │        │ apps/zeron (`zeron`)     │
└────────────┬─────────────┘        └────────────┬─────────────┘
             │ ws://127.0.0.1:<port>             │ in-proc or WS IPC
             │ (NDJSON RPC + ipc-token)          │
             └──────────────┬────────────────────┘
                            ▼
                 ┌─────────────────────┐
                 │    zeron-engine     │  ◄── Local Agent CLIs (Claude, Codex,
                 │  (crates/engine)    │      Cursor, OpenCode, Pi, Muse, ACP)
                 │  • DocHost / Loro   │  ◄── MCP (`zeron mcp` w/ agent token)
                 │  • PTY Terminals    │  ◄── Git / Worktrees / Previews
                 └──────────┬──────────┘
                            │ Optional Cloud Sync (TLS WebSocket + HTTPS)
                            ▼
        ┌───────────────────────────────────────────┐
        │      Cloudflare Edge (edge/src/)          │
        │  • WorkOS AuthKit + Runner JWT verify     │
        │  • Durable Objects: SessionRoom, ChatRoom,│
        │    DeviceRoom, RegistryRoom, PreviewRoom, │
        │    CloudAccount, CloudIndex, UserDirectory│
        │  • R2: BLOBS, RELEASES, CARD_IMAGES       │
        │  • Workflows: Provision/Wake/Sleep/Delete │
        └─────────┬───────────────────────┬─────────┘
                  │ Service Binding       │ Sync / Relay
                  ▼                       ▼
     ┌────────────────────────┐  ┌─────────────────────────┐
     │ zeron-vault (edge/vault│  │ Remote Viewports &      │
     │ • NO HTTP surface      │  │ Cloud Runners           │
     │ • VaultApi RPC only    │  │ • iOS (apps/ios)        │
     │ • VaultAccount/Devices │  │ • Android (apps/android)│
     └────────────────────────┘  │ • Cloud Sandbox Runners │
                                 └─────────────────────────┘
```

---

## 2. Rust Workspace & Crate Dependency Map

The root `Cargo.toml` defines **33 workspace members** (`edition = "2024"`, `version = "0.3.9"`). `apps/zgx` is intentionally a **separate Cargo workspace** because it pins patched forks of `zgx`, `wgpu-hal`, `winit`, and `accesskit_*` from `https://github.com/zeronsh/zgx`.

### Workspace Crate Matrix

| Path | Crate Name | Internal Workspace Dependencies | Responsibility |
| --- | --- | --- | --- |
| `crates/proto` | `zeron-proto` | *(leaf)* | Shared domain models, wire DTOs, entity/mention/annotation/cloud/preview/voice types. |
| `crates/doc` | `zeron-doc` | `zeron-proto` | Loro CRDT + log-based session/workspace document schemas, deltas, queues, rebuilds. |
| `crates/sync` | `zeron-sync` | `zeron-doc` | Edge sync transport: ChatRoom/SessionRoom clients, relays, held sends, budgets, attachment store. |
| `crates/rpc` | `zeron-rpc` | `zeron-proto`, `zeron-sync` | Typed NDJSON RPC protocol, WS server/client, localhost IPC auth (`ipc_auth.rs`), `AgentGuard`, `DeviceRoom` relay. |
| `crates/harness` | `zeron-harness` | `zeron-proto`, `zeron-voice-media` | Agent adapters (Claude, Codex, Cursor, OpenCode, Pi, Muse, ACP), process supervision, install/signature checks. |
| `crates/engine` | `zeron-engine` | `zeron-doc`, `zeron-harness`, `zeron-launch`, `zeron-mcp`, `zeron-notify`, `zeron-plugin-guard`, `zeron-preview`, `zeron-proto`, `zeron-rpc`, `zeron-sync`, `zeron-term-link`, `zeron-theme`, `zeron-update`, `zeron-visual` | Central orchestration daemon: sessions, `DocHost`, `EngineRpc`, git/worktrees, PTYs, previews, plugins, Cloud credential broker. |
| `crates/client` | `zeron-client` | `zeron-doc`, `zeron-preview`, `zeron-proto`, `zeron-rpc`, `zeron-sync`, `zeron-term-link`, `zeron-voice-session` | Shared multi-device client state & edge connection layer used by mobile apps. |
| `crates/mobile` | `zeron-mobile` | `zatex`, `zeron-client`, `zeron-doc`, `zeron-markdown`, `zeron-orb`, `zeron-proto`, `zeron-syntax`, `zeron-term`, `zeron-text`, `zeron-veil`, `zeron-visual`, `zeron-visual-gpu`, `zeron-voice-session` | UniFFI-exported mobile core for iOS & Android (transcript layout, terminal, visuals, voice, wallpaper). |
| `crates/mobile-bindgen` | `zeron-mobile-bindgen` | *(leaf)* | UniFFI binding generator binary for Swift and Kotlin. |
| `crates/mcp` | `zeron-mcp` | `zeron-doc`, `zeron-proto`, `zeron-rpc` | MCP JSON-RPC & HTTP server/proxy exposing scoped Zeron tools to running agents. |
| `crates/plugin-guard` | `zeron-plugin-guard` | *(leaf)* | Shared plugin sandbox policy for commands (`git`/`gh` argument guards, env allowlist), network (SSRF/private-IP blocking), and paths. |
| `crates/preview` | `zeron-preview` | `zeron-preview-label`, `zeron-proto` | Local dev-server discovery, port proxying, and multiplexed preview tunnels (`mux::BoxIo`). |
| `crates/preview-label` | `zeron-preview-label` | *(leaf)* | Preview label/URL parsing and formatting utilities. |
| `crates/term` | `zeron-term` | *(leaf)* | Terminal emulation / grid state. |
| `crates/term-link` | `zeron-term-link` | `zeron-preview`, `zeron-proto`, `zeron-rpc` | Remote terminal hub with automatic direct-path vs. relay failover. |
| `crates/voice` | `zeron-voice` | *(leaf)* | Audio resampling and voice session pipeline. |
| `crates/voice-media` | `zeron-voice-media` | `zeron-proto`, `zeron-voice-session` | Voice media framing and harness bridge. |
| `crates/voice-session` | `zeron-voice-session` | `zeron-orb`, `zeron-proto`, `zeron-rpc` | Voice session state machine and view projections. |
| `crates/audio` | `zeron-audio` | `zeron-proto` | Audio device / notification sound helpers. |
| `crates/orb` | `zeron-orb` | *(leaf)* | Animated voice orb state/geometry math. |
| `crates/veil` | `zeron-veil` | *(leaf)* | Visual scrim / veil effects. |
| `crates/visual` | `zeron-visual` | *(leaf)* | Luau-driven declarative visual scene graph and raster model. |
| `crates/visual-gpu` | `zeron-visual-gpu` | `zeron-visual` | `wgpu` shader and GPU renderer for `zeron-visual`. |
| `crates/markdown` | `zeron-markdown` | *(leaf)* | Streaming Markdown parser (`crates/markdown/src/parser.rs`, 4,304 lines). |
| `crates/syntax` | `zeron-syntax` | *(leaf)* | Syntax highlighting engine. |
| `crates/text` | `zeron-text` | *(leaf)* | Text measurement and layout utilities. |
| `crates/zatex` | `zatex` | *(leaf)* | Fast, low-memory LaTeX math layout over OpenType math fonts. |
| `crates/theme` | `zeron-theme` | *(leaf)* | Theme family/variant definitions and surface treatment resolution. |
| `crates/notify` | `zeron-notify` | *(leaf)* | Native OS desktop notifications. |
| `crates/launch` | `zeron-launch` | *(leaf)* | App/daemon launching and process discovery helpers. |
| `crates/update` | `zeron-update` | *(leaf)* | Signed self-updater, atomic transactions, Windows/macOS/Linux update application. |
| `crates/cli` | `zeron-cli` | `zeron-launch`, `zeron-update` | CLI argument parsing and subcommand dispatch. |
| `apps/zeron` | `zeron` | `zeron-cli`, `zeron-engine`, `zeron-mcp`, `zeron-notify`, `zeron-rpc`, `zeron-sync`, `zeron-update` | Headless engine daemon & CLI binary (`zeron headless`, `zeron mcp`, auth/plugin CLI). |

### Layered Dependency Structure

1. **Leaf / Pure Utility Layer:** `zeron-proto`, `zeron-plugin-guard`, `zeron-theme`, `zeron-markdown`, `zeron-syntax`, `zeron-text`, `zatex`, `zeron-term`, `zeron-orb`, `zeron-veil`, `zeron-visual`, `zeron-preview-label`, `zeron-launch`, `zeron-notify`, `zeron-update`, `zeron-voice`.
2. **Protocol & Sync Layer:** `zeron-doc` → `zeron-sync` → `zeron-rpc`.
3. **Subsystem Layer:** `zeron-harness`, `zeron-mcp`, `zeron-preview`, `zeron-term-link`, `zeron-voice-session`, `zeron-voice-media`, `zeron-visual-gpu`.
4. **Orchestration Layer:** `zeron-engine` (aggregates 14 internal crates) and `zeron-client` (aggregates 7 internal crates for remote/mobile clients).
5. **Application Entrypoints:** `apps/zeron` (CLI/daemon), `crates/mobile` (UniFFI library for `apps/ios` and `apps/android`), and `apps/zgx` (separate Cargo workspace desktop app).

---

## 3. Applications & Viewports

- **`apps/zgx` (Primary Desktop App):** Rust host (`apps/zgx/src/`, ~86 files) embedding the `zgx` UI engine (`zgx-desktop`, `zgx-script`, `zgx-gpu`, `zgx-run`) with a hot-reloadable **Luau UI** (`apps/zgx/ui/`, 758 `.luau` files, ~296K lines). Connects to `zeron-engine` over localhost WebSocket RPC (`apps/zgx/src/engine/rpc.rs`) and supports embedded Ghostty terminals (macOS), WebView2 (Windows), WebKitGTK (Linux), and a sandboxed plugin host.
- **`apps/zeron` (CLI & Headless Engine):** Ships the `zeron` binary (`apps/zeron/src/main.rs`). Explicitly has **no GUI window** (`Window` command bails with a message directing users to open the Zeron app or run `zeron headless`).
- **`apps/ios` (Swift / UIKit):** Native iOS viewport + `ZeronLiveActivity` extension calling into `crates/mobile` via UniFFI (`apps/ios/Zeron/Core/Generated/zeron_core.swift`).
- **`apps/android` (Kotlin):** Native Android viewport calling into `crates/mobile` via UniFFI.
- **`apps/landing` & `apps/card-web`:** Static web properties for `zeron.sh` and interactive member cards (`zeron.sh/@<user>`), sharing design tokens with `apps/zgx/ui/ds/themes/zeron-dark.json`.

---

## 4. Key Subsystems & Data Flow

### 4.1 Localhost IPC & RPC Protocol (`crates/rpc`)  
- **Framing:** Newline-delimited JSON (NDJSON) frames over WebSocket (`ws://127.0.0.1:<port>`, default `27654`).
- **Concurrency & Ordering (`crates/rpc/src/server.rs`):** Most RPC calls spawn independent tokio tasks; terminal input/resize (`WRITE_TERMINAL`, `RESIZE_TERMINAL`) are routed through per-terminal ordered FIFO lanes so keystrokes and escape-sequence responses never reorder.
- **Two-Gate Localhost Security (`crates/rpc/src/server.rs`, `ipc_auth.rs`):**
  1. **Origin Rejection:** Any WebSocket handshake carrying an `Origin` header is rejected with `403 Forbidden`, preventing cross-origin browser pages from reaching localhost IPC.
  2. **Scoped Bearer Tokens:** On startup, the engine mints two 32-byte random tokens atomically written with owner-only permissions: `{data_dir}/ipc-token` (full user authority) and `{data_dir}/ipc-token-agent` (restricted to `agent_allowed` methods and pinned to the calling chat via `AgentGuard`). Tokens are compared in constant time.

### 4.2 Session Execution & Document Sync (`crates/engine`, `crates/doc`, `crates/sync`)
1. User submits a prompt from a viewport (`apps/zgx`, iOS, Android, or CLI) via RPC.
2. `crates/engine/src/sessions.rs` dispatches the turn to the selected harness in `crates/harness` (Claude, Codex, Cursor, OpenCode, Pi, Muse, or ACP).
3. Harness streams structured events (text deltas, thinking, tool calls/results, approvals, usage) back to `crates/engine/src/doc_host.rs`.
4. `DocHost` mutates the session's `zeron-doc` state and persists snapshots/logs locally, while notifying local RPC subscribers.
5. If signed in, `crates/sync` replicates updates to the Cloudflare Edge:
   - **Session / Chat Sync:** `SessionRoom` (`s2/<chatId>`, Loro CRDT) and `ChatRoom` (`chat2` append-only log relay) Durable Objects.
   - **Workspace / Device Registry:** `RegistryRoom` DO tracks devices, spaces, and sessions with runner ownership enforcement (`registry-runner.ts`).
   - **Cross-Device RPC Relay:** `DeviceRoom` DO relays RPC calls between online devices (e.g., mobile phone starting a session on a laptop).
   - **Large Payloads:** Tool output blobs (>1 MiB limit per blob) and attachments are offloaded to R2 (`BLOBS` bucket).

### 4.3 Remote Terminals & Previews (`crates/term-link`, `crates/preview`)
- Local PTYs (`crates/engine/src/terminals.rs`) and local web previews (`crates/preview`) can be accessed from remote devices.
- `zeron-term-link` implements a hybrid transport: it prefers a direct multiplexed stream (`zeron_preview::mux::BoxIo`) when available and automatically falls back to `DeviceRoom` RPC relay when direct connectivity drops.

### 4.4 Edge Control Plane, Cloud Sandboxes & Credential Vault (`edge/`, `edge/vault/`)
- **Edge Worker (`edge/src/index.ts`, `edge/wrangler.jsonc`):**
  - Serves custom domains `edge.zeron.sh`, `zeron.sh/install.sh`, `zeron.sh/releases/*`, `zeron.sh/@*`, `zeron.sh/c/*`, `zeron.sh/card/*`, and `staging.zeron.sh`.
  - **8 Durable Object classes:** `SessionRoom`, `DeviceRoom`, `PreviewRoom`, `RegistryRoom`, `ChatRoom`, `CloudAccount`, `CloudIndex`, `UserDirectory`.
  - **4 Cloudflare Workflows:** `ProvisionWorkflow`, `WakeWorkflow`, `SleepWorkflow`, `DeleteWorkflow` for Cloud sandbox lifecycle.
  - **3 R2 Buckets:** `BLOBS`, `RELEASES`, `CARD_IMAGES`.
  - **Browser Rendering binding (`BROWSER`):** Renders OG/shareable member card images via `@cloudflare/puppeteer`.
  - **Cron (`17 * * * *`):** Hourly metering, reconciliation, and cleanup.
- **Authentication (`edge/src/auth.ts`, `runner-token.ts`, `runner-policy.ts`):**
  - Verifies **WorkOS AuthKit JWTs** via remote JWKS (`jose`) for human users (`kind: "user"`).
  - Verifies **Runner JWTs** signed by `RUNNER_JWT_PRIVATE_KEY` / `RUNNER_JWT_PUBLIC_KEY` for Cloud sandboxes (`kind: "runner"`), strictly constrained by `runner-policy.ts` and `runner-access.ts` so a compromised sandbox cannot manage the account or access unauthorized sessions.
  - Operator endpoints (`/admin/*`) are gated by a constant-time `ADMIN_TOKEN` check.
- **Isolated Credential Vault (`edge/vault/`):**
  - Deployed as a **separate Worker (`zeron-vault`)** with **zero HTTP surface** (`fetch` always returns 404; no routes, no `workers.dev`).
  - Reachable only via Cloudflare Service Binding (`VAULT` → `VaultApi` `WorkerEntrypoint`).
  - Enforces global (`VAULT_DISABLED=1`) and per-user kill switches, device enrollment + fresh cryptographic request signatures (`VaultDevices`), and per-account authorization (`VaultAccount`).

---

## 5. CI, Testing & Release Infrastructure

Zeron uses **19 GitHub Actions workflows** under `.github/workflows/`:
- **`ci.yml`:** Smart planner that determines which Rust, Edge, Vault, UI, and platform suites are needed per PR/merge queue.
- **Platform & Feature Suites:** `macos.yml`, `windows.yml`, `android.yml`, `zgx-tests.yml`, `ui-tests.yml`, `preview-tests.yml`, `voice-tests.yml`, `cursor-compatibility.yml`, `linux-installer.yml`, `release-profile-tests.yml`, `weekly-tests.yml`.
- **`release.yml` + `zgx-build.yml`:** Triggered on `v*` tags (verified against workspace `Cargo.toml` version). Builds macOS arm64 DMG + updater tarball, Linux x86_64/aarch64 tarballs, and Windows x86_64 installer/zip/exe; publishes GitHub Releases and uploads artifacts + `latest.txt` to the R2 `RELEASES` bucket.
- **`deploy.yml`:** Deploys `edge/vault` behind a required-reviewer `vault-production` GitHub environment before deploying `edge/` (behind the `cloudflare` environment), plus `apps/landing`.
- **`testflight.yml` & `zgx-beta.yml`:** Continuous iOS TestFlight and desktop beta channel releases.
