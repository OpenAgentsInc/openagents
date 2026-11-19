# Zeron — Feature Catalog

An exhaustive catalog of Zeron's capabilities across its Rust crates, desktop/mobile/CLI apps, and Cloudflare edge services, organized by domain with concrete crate and file references.

---

## 1. Multi-Agent Harness Orchestration

Zeron abstracts over multiple coding agent CLIs and protocols behind a unified harness layer so transcripts, approvals, models, skills, and usage behave consistently.

- **Supported Agent Backends (`crates/harness/src/`):**
  - **Claude Code:** `crates/harness/src/claude/`
  - **OpenAI Codex:** `crates/harness/src/codex/`
  - **Cursor:** `crates/harness/src/cursor/` (plus automated SDK update & compatibility testing via `scripts/update-cursor-sdk.py`, `.github/workflows/cursor-compatibility.yml`)
  - **OpenCode:** `crates/harness/src/opencode/`
  - **Pi:** `crates/harness/src/pi/`
  - **Muse:** `crates/harness/src/muse/`
  - **ACP (Agent Client Protocol) Ecosystem:** `crates/harness/src/acp/`, `crates/harness/src/acp_registry.rs`, `crates/engine/src/acp_registry.rs` — powers Devin, Grok, Hermes, Antigravity, and other ACP-compliant agents.
  - **Mock Harness:** `crates/harness/src/mock.rs` for deterministic testing.
- **Catalog & Auto-Installation:** Discovery, binary resolution, archive/adapter installation, and code-signature verification (`crates/harness/src/catalog.rs`, `executable.rs`, `install.rs`, `adapter_install.rs`, `archive_install.rs`, `code_signature.rs`).
- **Process & Environment Isolation:** Login-shell environment capture (`shell_env.rs`), cross-platform process supervision and Windows job/escrow handling (`process.rs`, `process/windows/`, `windows_process.rs`), bounded JSONL tailing (`jsonl_tail.rs`, `bounded_file.rs`), and port management (`agent_ports.rs`).
- **Approvals, Skills & Model Context:** Tool approval routing (`approval.rs`), skill discovery (`skills.rs`), model context limits (`model_context.rs`), and harness version updates (`crates/engine/src/harness_updates.rs`).
- **Agent Accounts & Hosted Models:** Multi-account management (`crates/engine/src/agent_accounts.rs`, `agent_accounts/muse.rs`) and hosted model catalog routing (`crates/engine/src/hosted_models.rs`, `edge/src/cloud/models.ts`).

---

## 2. Sessions, Workspaces & Collaborative Document State

- **Session Lifecycle (`crates/engine/src/sessions.rs`, `crates/engine/src/doc_host.rs`):** Create, resume, fork, archive, interrupt, queue follow-up turns, and stream structured transcript deltas.
- **Structured Transcript Model (`crates/doc/src/`):**
  - Loro CRDT + log-based session documents (`schema.rs`, `parts.rs`, `transcript_delta.rs`, `rebuild.rs`).
  - Queued user messages & commands (`queue.rs`, `commands.rs`).
  - Workspace registry document (`workspace.rs`, `registry.rs`) organizing **spaces** (`(device, folder)` pairs), pinned tabs, and cross-device visibility.
- **Rich Mentions & Annotations (`crates/proto/src/`):**
  - File mentions (`file_mentions.rs`), chat mentions (`chat_mentions.rs`), attachment mentions (`attachment_mentions.rs`), inline annotations (`annotation.rs`), and change-request assessments (`change_request_assessment.rs`).
- **Attachment & Blob Storage:** Content-addressed local attachment store (`crates/sync/src/attachment_store.rs`) backed by R2 blob upload/download (`edge/src/blobs.ts`, `edge/src/attachment-blobs.ts`).

---

## 3. Source Control, Worktrees & Code Workspace

- **Git & Source Control (`crates/engine/src/source_control.rs`, `crates/engine/src/repos.rs`):** Repo discovery, branch management, staging, commits, diff computation, and live checkout diff sync (`crates/engine/src/diff_sync.rs`).
- **Git Worktree Isolation:** Dedicated worktree configuration and lifecycle (`crates/engine/src/worktree_settings.rs`) so concurrent agent runs do not collide on a working tree.
- **Change Requests / PR Workflows:** Change-request tracking and review assessments (`crates/engine/src/change_requests.rs`, `crates/proto/src/change_request_assessment.rs`).
- **Workspace Files & Search:** Fast file tree browsing, safe file mutations (`crates/engine/src/workspace_files.rs`, `workspace_files/mutations.rs`), full-text workspace search (`crates/engine/src/workspace_search.rs`), and project actions (`crates/engine/src/project_actions.rs`).

---

## 4. Integrated Terminals & Remote Terminal Linking

- **Local PTY Management (`crates/engine/src/terminals.rs`, `terminals/windows.rs`, `crates/term/`):** Multi-tab terminal emulator with ordered input/resize lanes (`crates/rpc/src/server.rs`) and Ghostty integration on macOS (`apps/zgx/Cargo.toml`).
- **Cross-Device Remote Terminals (`crates/term-link/`, `crates/engine/src/remote_terminals.rs`):** Attach to a terminal running on another desktop or Cloud device from desktop or mobile, seamlessly switching between a direct P2P/tunneled stream and the edge relay fallback with liveness pings.

---

## 5. Live App Previews & Port Forwarding

- **Local Server Discovery (`crates/preview/src/discovery.rs`, `crates/preview/src/catalog.rs`, `crates/preview-label/`):** Automatically detects dev servers started by agents or users and assigns human-readable preview labels.
- **Cross-Device Preview Tunneling (`crates/preview/`, `edge/src/preview-room.ts`):** Multiplexes HTTP/WebSocket traffic through `PreviewRoom` Durable Objects so a dev server running on a laptop or Cloud box can be inspected from another device.
- **Embedded Browser / Webview:** Native webview integration in `apps/zgx` (`webkitgtk` on Linux, `webview2` on Windows, WKWebView on macOS) and plugin preview panes (`apps/zgx/src/plugins/preview.rs`).

---

## 6. Voice Sessions & Audio

- **Real-Time Voice Pipeline (`crates/voice/`, `crates/voice-media/`, `crates/voice-session/`, `crates/audio/`):**
  - Audio capture, playback, and resampling (`crates/voice/src/resample.rs`, `crates/audio/`).
  - Voice session state machine and transcript view (`crates/voice-session/src/lib.rs`, `view.rs`, `crates/doc/src/voice.rs`).
  - Remote voice bridging across devices (`crates/engine/src/voice/remote.rs`, `apps/android/.../voice/VoicePeer.kt`).
- **Visual Voice Orb (`crates/orb/`, `apps/android/.../voice/VoiceOrbView.kt`, `crates/mobile/src/orb.rs`):** GPU/shader-driven animated orb reflecting listening/speaking/thinking states.

---

## 7. Multi-Device Sync & Mesh Networking

- **Dual Sync Protocols (`crates/sync/src/`):**
  - **Session/Workspace CRDT Sync:** Loro-based state sync via `SessionRoom` (`edge/src/session-room.ts`) and `RegistryRoom` (`edge/src/registry-room.ts`).
  - **Chat2 Log Relay:** Append-only log relay (`crates/sync/src/chat_client.rs`, `chat_relay.rs`, `chat_frames.rs`, `edge/src/chat-room.ts`) with held-send buffering (`held_sends.rs`) and write/bandwidth budgeting (`budget.rs`, `edge/src/write-meter.ts`).
- **Device-to-Device RPC Relay (`crates/rpc/src/device_room.rs`, `edge/src/device-room.ts`):** Forward RPC calls (start session, run command, open terminal, read files) from any authorized viewport to any online device belonging to the user/organization.
- **Network Path Adaptation (`crates/sync/src/net_path.rs`, `dial.rs`, `socket.rs`):** Automatic reconnection, backoff, and direct-vs-relay path selection.

---

## 8. Zeron Cloud & Credential Vault

- **Managed Cloud Sandboxes (`edge/src/cloud/`, `crates/proto/src/cloud.rs`):**
  - Provision, wake, sleep, and delete remote Linux agent sandboxes orchestrated via Cloudflare Workflows (`ProvisionWorkflow`, `WakeWorkflow`, `SleepWorkflow`, `DeleteWorkflow` in `edge/src/cloud/workflows.ts`).
  - Per-account state and quota enforcement (`CloudAccount`, `CloudIndex`, `policy.ts`, `wake.ts`, `metering.ts`).
  - Pluggable sandbox provider abstraction (`sandbox-provider.ts`, `providers/`) including a local `FakeProvider` for end-to-end testing (`scripts/cloud-e2e.sh`).
  - Usage metering, machine sizing, and Stripe billing integration (`billing.ts`, `stripe.ts`).
- **Isolated Credential Vault (`edge/vault/`, `crates/engine/src/credentials.rs`, `edge/src/vault-routes.ts`):**
  - Stores provider credentials (GitHub via GitHub App / device flow, API keys) inside a zero-HTTP Worker (`VaultApi`) backed by `VaultAccount` and `VaultDevices` Durable Objects.
  - Issues short-lived, device-signed, revocable credential grants to enrolled Cloud runners.
  - Global (`VAULT_DISABLED`) and per-user kill switches (`edge/vault/src/vault.ts`).

---

## 9. MCP (Model Context Protocol) Integration

- **Engine-Injected MCP Server (`crates/mcp/src/`):**
  - Exposes Zeron tools (transcript inspection, chat actions, workspace/preview helpers) to running agents over JSON-RPC / HTTP (`http.rs`, `jsonrpc.rs`, `proxy.rs`, `tools.rs`, `zeron.rs`, `transcript.rs`).
  - Automatically receives a restricted, agent-scoped IPC token (`ipc-token-agent` via `ZERON_IPC_TOKEN_FILE`) pinned to the calling chat (`crates/rpc/src/server.rs`, `crates/rpc/src/ipc_auth.rs`).

---

## 10. Plugin System & Security Guardrails

- **Luau Plugins (`apps/zgx/src/plugins/`, `crates/plugin-guard/`):**
  - Plugins extend desktop UI, preview panes, file-event watchers (`file_events.rs`, `file_events_windows.rs`), and engine services.
  - **Command Guard (`crates/plugin-guard/src/commands.rs`):** No-shell execution, login-shell PATH resolution, strict env allowlist, and argument-level sanitization for `git` and `gh` (disabling hooks, fsmonitor, aliases, extensions, and credential exfiltration flags).
  - **Network Guard (`crates/plugin-guard/src/net.rs`):** Post-DNS and per-redirect IP classification blocking link-local/cloud metadata (`169.254.169.254`, `100.100.100.200`, `fe80::/10`) and private/loopback ranges unless explicitly named in the manifest.
  - **Path Guard (`crates/plugin-guard/src/path.rs`):** Filesystem boundary enforcement.
  - **Plugin Signing Keys:** Edge routes for plugin key management (`edge/src/index.ts` → `handlePluginKeyRoute`).

---

## 11. Desktop, Mobile & Web Clients

- **Desktop App (`apps/zgx/`):**
  - Native GPU-rendered UI written in hot-reloading Luau (`apps/zgx/ui/ds/` design system, themes, code editor, sidebar, diff viewer, settings, onboarding, and `templates/zeron-v2/`).
  - Full accessibility integration (`accesskit`), system font discovery (`fontdb`), window/app screenshots (`apps/zgx/src/appshots/macos.rs`), and rich theme system (`crates/theme/`, `CONTEXT.md`).
- **Rich Content & Math Rendering:**
  - Custom Markdown parser (`crates/markdown/src/parser.rs`), Tree-sitter syntax highlighting (`crates/syntax/`), native LaTeX math layout engine (`crates/zatex/`), text shaping helpers (`crates/text/`), and programmable visual/shader scenes (`crates/visual/`, `crates/visual-gpu/`, `crates/veil/`).
- **iOS & Android Apps (`apps/ios/`, `apps/android/`, `crates/mobile/`, `crates/client/`):**
  - Shared Rust mobile core exposed via UniFFI (`crates/mobile-bindgen/`) providing sync, transcript layout, terminal emulation, voice orb, captions, and wallpapers.
  - iOS Live Activities (`ZeronLiveActivity`) and APNs push notifications (`edge/src/apns.ts`).
- **CLI, Notifications & Auto-Updater (`apps/zeron/`, `crates/cli/`, `crates/launch/`, `crates/notify/`, `crates/update/`):**
  - Headless engine daemon, auth CLI (`auth_cli.rs`), plugin CLI (`plugin_cli.rs`), OS native notifications (`crates/notify/`), and transactional self-updater (`crates/update/src/transaction.rs`, `windows.rs`) backed by R2 releases (`zeron.sh/releases/*`, `zeron.sh/install.sh`).
- **Member Cards & User Directory (`apps/card-web/`, `edge/src/card-routes.ts`, `edge/src/user-directory.ts`):**
  - Public member profiles (`zeron.sh/@username`), badges, customizable card scenes/foils, and Cloudflare Browser Rendering (`BROWSER` binding) card image generation cached in `CARD_IMAGES` R2.
