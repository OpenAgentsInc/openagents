# Comprehensive Gap Analysis: OpenAgents vs. T3 Code (`v0.1.12-nightly`)

> [!NOTE]
> **Status (2026-11-22): Historical 2026-11-20 point-in-time snapshot — superseded by `docs/t3/`.**
> Since `855654fc81`, OpenAgents shipped **Inky** (`coax/lib/inky/`, replacing Microcoder for local Inkling 2 Mini on native Psionic Metal 4 / CUDA at 150+ tok/s), the 29-flag `--minimal` Desktop release architecture (`openagents-v1-spec.md`), complete Antigravity 1.5+ core parity (including `SUBAGENT_STATE_KILLED`, `--login` OAuth, `/compact`, lost thinking-signature recovery, `call_mcp_tool` unwrap, recall rollback, reactive wakeup holds `a159b463e7`, and native `Settings -> Providers`), and Desktop/phone Tailscale chat sync (`8da287e936`). Meanwhile `t3code` moved +321 commits ahead to `18850a5eeb` (durable workflows `#38882`/`#38884`, MCP Events webhooks `#41141`, multi-repo projects `#35925`, 17 MCP toolkits, 8 Git forges, 110+11 SQLite migrations, 6 eval suites).
>
> **Read instead:** [`docs/t3/README.md`](../t3/README.md) (supersedes §2 corpus index), [`docs/t3/reference.md`](../t3/reference.md) (supersedes §3 architecture summary), [`docs/t3/antigravity-quirks.md`](../t3/antigravity-quirks.md), and [`docs/t3/2026-11-22-t3code-borrowing-theory.md`](../t3/2026-11-22-t3code-borrowing-theory.md) (supersedes §6 roadmap).

**Date:** 2026-11-20  
**T3 Code baseline:** `~/work/projects/repos/t3code` (`pingdotgg/t3code`), commit [`8cfd2b5211`](https://github.com/pingdotgg/t3code/commit/8cfd2b5211) (`v0.1.12-nightly.20261121.5024`, package version `0.1.11`, **1,759 commits** ahead of the prior `0.1.9` `f313d0a6cb` audit).  
**OpenAgents baseline:** `origin/main` commit `855654fc81` (2026-11-20).  
**Method:** Direct source inspection of both repositories. No T3 Code scripts, installers, binaries, or tests were executed; no credential files were opened.

---

## 1. Executive Summary

Both codebases have moved rapidly since our last full comparison on 2026-11-14 (`docs/research/t3code/09-current-gap-analysis.md`, which compared OpenAgents `097e11c86d` against T3 Code `0.1.9` `f313d0a6cb`).

### What OpenAgents Closed Since 2026-11-14
OpenAgents is no longer a thin WebView/JS prototype with a narrow Claude wrapper. On `855654fc81`, OpenAgents ships:
- **Compiled GPU Desktop (`apps/openagents-desktop/`)** on macOS (Swift/AppKit + Metal), Linux (Wayland + Vulkan), and Windows (Win32 + Vulkan, cross-compiled via Zig), backed by a single shared Coax core (`coax/lib/agent/desktop/`).
- **Compiled GPU iOS (`apps/openagents-ios/` + `apps/shared/app/`)** rendered in one Metal view with zero JS/Wasm/WebView, push-right GPU sidebar, synced Pin/Rename/Archive metadata (`coax/lib/agent/chat-library.coax`), subagent transcript view with `StopSubagent`, and a 4-tab **Workspace** screen (**Diff / Branch / Trees / PRs**).
- **Unified Resident Host Daemon (`coder/tui/host*.coax`)** serving both local Unix socket (`openagents.environment.daemon.v1alpha1`) and Nostr relay (`NIP-HOST` / `NIP-44`), plus **zero-config Tailnet phone pairing** (`Tailnet.swift`), **per-chat Git worktrees** with CoW clones (`Worktree.swift`, `.coder/worktree.json`), **Foreman** local orchestration, **iOS Simulator pane** (`oasim serve`), and **Ed25519-signed updates**.
- **Durable Execution Primitives**: `#authority` hash-chained execution journal (`coax/lib/agent/journal*.coax`), persisted worker follow-up queue (`mcworker-queue.coax`), `workmem` anti-weakening verification, and `DurableChat`/`NativeTaskApp` idempotent command fencing.
- **Antigravity 1.5+ Core Parity**: Almost all P0/P1 engine work from `docs/audits/2026-11-20-t3code-antigravity-v0-1-11-audit.md` has already landed in `coax/lib/agent/engine/antigravity.coax`, `cursor.coax`, `coax/lib/agent/cursor/client.coax`, and `coder/tui/agyworker.coax` (`>=1.5.0` version gate, `AGY_CLI_DISABLE_AUTO_UPDATE=1`, `SSH_CONNECTION` profile isolation, Auto review mode + escalated `CU_DISP_ASK`, `defer_finalize_background`, `steers_with_prompt`, `available_commands_update`, crash auto-respawn).
- **Desktop Effect Mismatch Resolved**: The op 14/15/16 host-contract mismatch flagged in `09-current-gap-analysis.md` is completely gone (`NativeHost.swift` and `EnvironmentWindow.swift` were replaced by typed `FX_` enums and separate desktop core).

### What T3 Code Added Since `0.1.9` (+1,759 Commits)
In the same week, T3 Code shipped a massive expansion of its multi-machine control plane and coding workbench:
1. **Multi-environment peer linking & remote delegation** (`apps/server/src/peer/`, `PeerThreadRelay`, `linkOrigin` fencing, `EnvironmentsMap`, DPoP-renewing pairing links).
2. **Loopback preview tunneling & port forwarding** (`/api/preview/tunnel`, desktop SOCKS5 fixed proxy for remote `localhost`, mobile Safari-style localhost tabs).
3. **Full Issue Tracking & Forge MCP Toolkits** (GitHub, GitLab, Azure DevOps, Forgejo, Linear issue trackers; 7 Git forges; agent MCP tools for reading/opening/merging PRs, landing GitHub stacks, inspecting failing CI logs, and re-running checks).
4. **11 Built-in Provider Drivers + ACP Registry** (Claude, Codex, Cursor SDK 1.4.2 with rewind, Antigravity 1.5+, OpenCode 1.x/2.x, Pi, Muse, Grok, Kiro, Fx, generic ACP Registry) behind a normalized V2 capability contract, with typed turn results and native stopped-thread resume.
5. **Native macOS Apple Speech (`native/apple-speech/AppleSpeech.swift`)**, streaming transcription, ZDR OpenRouter audio, and background dictation.
6. **Scheduled tasks + offline webhook Durable Object inbox**, **1Password/secret cards**, **HMAC guest share links**, **MCP OAuth sign-in from warnings**, **VS Code/OpenVSX theme import + 100k agent CSS**, **full app i18n**, **iOS/Android home-screen widgets**, and **5 automated eval suites** (`evals/`).

### Bottom-Line Verdict
- **Where OpenAgents leads or deliberately diverges:** Native compiled GPU speed/footprint (no Electron/RN), Coax `#authority` mutation-tested policy invariants, owned Microcoder + Psionic ML stack, Nostr decentralized relay + zero-config Tailnet pairing, live iOS Simulator control (`oasim`), Wasm plugins, Golemancy, Metaverse/Worlds, and Bitcoin-only payments.
- **Where the largest true gaps remain:**
  1. **Common durable orchestration + offline projection/outbox** across desktop, mobile, and remote hosts (OpenAgents still splits account chats, managed host slots, daemon runs, and routed execution handoff).
  2. **Interactive coding-workbench UX** (desktop permission/approval UI & mode switcher, structured multi-question/elicitation forms, proposed plan cards, interactive PTY terminals, per-turn file checkpoints/rewind, working/settled/snoozed attention shelves, full-text transcript search).
  3. **Provider & MCP breadth** (Codex mobile/desktop parity + `turn/steer` + `/compact`, general MCP client in Microcoder/Desktop, built-in PR/CI/issue/worktree MCP toolkits).
  4. **Multi-environment peer linking, port forwarding / preview tunneling, and Windows desktop parity**.

---

## 2. Audit of All Existing `t3code` Mentions in OpenAgents

Every file in `openagents` (`855654fc81`) referencing `t3code` / `T3 Code` was audited:

| File(s) | Pinned T3 Rev | Pinned OA Rev | Current Status & Findings |
| --- | --- | --- | --- |
| `docs/research/t3code/README.md`, `01`–`06`, `08-source-map.md` (8 files) | `f313d0a6cb` (`0.1.9`) | `6fe29cc703` | **Stale by 1,759 T3 commits.** Core architecture (SQLite V2 orchestrator, Effect RPC, Claude SDK, Codex app-server) remains accurate, but misses peer links, preview tunnels, issue trackers, PR MCP toolkits, Antigravity 1.5+, Cursor 1.4.2, Fx/Grok/Kiro/ACP Registry, Apple Speech, guest links, and DPoP renewal. |
| `docs/research/t3code/07-openagents-path.md` | `f313d0a6cb` | `6fe29cc703` | **Largely superseded** by `09-current-gap-analysis.md` and this document. |
| `docs/research/t3code/09-current-gap-analysis.md` | `f313d0a6cb` (`0.1.9`) | `097e11c86d` (iOS 1085) | **Stale on OpenAgents.** Claims about UIKit/JS iOS, missing Workspace/Diff/PRs, missing Pin/Rename/Archive, missing subagent view, and the desktop op 14/15/16 mismatch are now **resolved/obsoleted**. |
| `docs/audits/2026-11-20-t3code-antigravity-v0-1-11-audit.md` | `d15123668e` (`v0.1.11`) | Unpinned | **Partially superseded same-day.** Phases 1–2 (`>=1.5.0` gate, `AGY_CLI_DISABLE_AUTO_UPDATE`, `SSH_CONNECTION`, Auto review mode, `defer_finalize_background`, `steers_with_prompt`, `available_commands_update`, crash auto-respawn, `agyworker`) have already landed in Coax. Still open from that audit: detached subagent transcript reader (`SUBAGENT_STATE_KILLED`), in-place edit-from-here rollback, `--login` OAuth URL capture, `/compact` dispatch, lost thinking-signature recovery, `call_mcp_tool` unwrapping, and usage/limits probe. |
| `docs/coder/compaction-theory.md` | `a6d2bed55b` (2026-11-18) | Current | **Accurate.** Microcoder's 3-tier rolling compaction (`MC_GONE_LIST :: 20`, prefix-cache boundary, history-not-instructions wording) is implemented in `coax/lib/microcoder/state.coax`. P3 idle-cache expiry warning remains open. |
| `coax/lib/agent/engine/mcworker-queue.coax`, `mcworker-wire.coax` | Unpinned | Current | **Accurate attribution.** Implemented persisted FIFO queue (`MWQ_ITEMS = 16`, 64-entry dedupe ring, pause-on-error/restore) inspired by T3's queued follow-ups. |
| `coax/lib/microcoder/state.coax` | Unpinned | Current | **Accurate attribution.** `MC_GONE_LIST :: 20` and prefix-cache retention aligned with T3 handover design. |
| `coax/native.md`, `coax/native/README.md`, `coax/native/composer.md` | `f313d0a6cb` / unpinned | Older Coax Native | **Historical design notes.** Note that `apps/openagents-ios/` has moved from WebKit/Wasm Coax Native to compiled Metal GPU (`apps/openagents-ios/README.md`). |
| `components/fonts/README.md`, `components/layout/{native-composer,workspace}.coax`, `components/skin/native-chat.coax` | `f313d0a6cb` / unpinned | Current | **Accurate provenance/attribution** (DM Sans typography and layout metrics adapted from MIT-licensed T3 Code). |
| `docs/protocol/environments-claude-and-world-editing.md` | Links to `docs/research/t3code/` | Current plan | **Links point to `0.1.9` audit**; architectural boundary (HOST/ENV/SESS vs T3 RPC) remains valid. |
| `docs/research/zeron/2026-11-14-current-gap-analysis.md` | Cross-links `09-current-gap-analysis.md` | `097e11c86d` | **Historical point-in-time snapshot.** |
| `docs/verification/2026-11-13-native-*.md`, `2026-11-14-native-*.json` (4 files) | `f313d0a6cb` | Historical | **Immutable verification records** (keep as-is). |
| `docs/research/claude-ios/screen-index.md`, `docs/transcripts/{253,255,274}.md` | Historical | Historical | **Historical media/transcript mentions** (keep as-is). |

---

## 3. What Changed in T3 Code Since `0.1.9` (`f313d0a6cb` → `8cfd2b5211`)

Between `f313d0a6cb` (2026-11-13) and `8cfd2b5211` (2026-11-20), T3 Code merged **1,759 commits**. The major new capabilities not covered in `docs/research/t3code/01`–`09` are:

1. **Multi-Environment Peer Linking & Remote Delegation (`apps/server/src/peer/`)**:
   - One T3 environment signs into another's `/mcp` as an OAuth client (`t3code-peer-link`).
   - Forwarded calls carry `T3-Mode-Limit` (narrowing ceiling only); non-full-access links stamp immutable `linkOrigin` fencing on created threads/subagents/forks and skip project setup scripts.
   - `PeerThreadRelay` streams a linked environment's thread read-only through the linking environment (`LinkedThreadPage`, `LinkedThreadPane`, unified with subagents and side chats in `SideThreadPane`).
   - `RemoteDelegation` lets a parent thread delegate tasks to a linked machine and resume following on restart.
   - `EnvironmentsMap` visualizes connected machines; multiple environments can be linked at once; links renew on use (`T3-Link-Expires-At`).
2. **Loopback Preview Tunneling & Port Forwarding (`apps/server/src/preview/PreviewTunnel.ts`)**:
   - `/api/preview/tunnel` tunnels TCP over WebSocket to a remote environment's `localhost` ports.
   - Desktop uses a fixed SOCKS5 proxy (`*,<-loopback>,169.254.0.0/16,fe80::/10`) so webviews render remote `localhost` tabs natively; desktop also forwards another environment's ports locally; mobile opens `localhost` tabs with a Safari-style browser bar; gated by `allowsPreviewTunnel` managed policy.
3. **DPoP-Renewing Pairing Sessions & Connect Device Approval (`apps/server/src/auth/`, `cloud/`)**:
   - Plain pairing links exchanged with DPoP keys renew in place for 30 days without breaking live sockets; T3 Connect adds opt-in `requireConnectDeviceApproval` per DPoP thumbprint so a compromised relay cannot mint active sessions silently.
   - Relay `HookInboxObject` (Cloudflare Durable Object + SQLite) optionally holds automation webhooks up to 24h while an environment is offline, with `x-t3-relay-delivery-id` deduplication.
4. **Issue Trackers & Agent PR/CI/Stack MCP Tools (`apps/server/src/issue/`, `pullRequest/`, `mcp/toolkits/`)**:
   - Full Issues UI on web & mobile (`IssueService`, GitHub, GitLab, Azure DevOps, Forgejo, Linear), linking issues to threads & PRs, hover previews, saved named filter views, fork upstream-issue listing.
   - Built-in agent MCP tools: `pull_request_read` (compact summary by default), open/update/respond/merge PRs, read failing CI logs & re-run checks, land a GitHub PR stack in one merge, and issue tools.
5. **11 Built-in Provider Drivers + V2 Orchestration Enhancements**:
   - Added/matured **Antigravity 1.5+** (Auto review, background commands, detached subagents, edit-from-here, compaction, OAuth isolation), **Cursor SDK 1.4.2** (isolated worker runner + edit-from-here rewind), **Fx** (per-thread backend, full access, safe CLI updates), **Grok**, **Kiro**, **OpenCode 2.x** (single server per instance with per-thread MCP permission fencing), **Pi** (RPC + MCP bridge + tool_search), **Muse**, and **ACP Registry** (checksum-verified managed binaries + npm/uvx).
   - Typed turn results (`2770bf3c61`), native resume of stopped provider threads (`9e19b7f7e1`), and manual continuation.
6. **Voice Input & Native Apple Speech (`native/apple-speech/`, `apps/server/src/speech/`)**:
   - macOS native Apple Speech Swift helper (`AppleSpeech.swift`), streaming transcription toggle, local `transcribe-cpp` + Silero VAD with model recommendations by language/speed/accuracy, ZDR-verified OpenRouter audio, background dictation across thread switches.
7. **Secrets, Guest Sharing, Themes, i18n & Mobile Polish**:
   - `secret-provider-onepassword` + multi-value secret cards with show/hide toggle; HMAC guest share links from web & mobile; MCP OAuth sign-in directly from warning banners (`McpSignInAction.tsx`); agent-restylable themes (up to 100k CSS + background images) + VS Code/OpenVSX theme import; full app localization; iOS native Messages-style image viewer; assistant text selection/citation/commenting; Android usage limits widget; macOS Finder & Windows Explorer file-pill drag-and-drop; `localEnvironmentEnabled: false` desktop client-only mode.

---

## 4. Comprehensive Gap Analysis Matrix

Status legend:
- **Parity**: Equivalent end-to-end capability exists.
- **Ahead / Different**: OpenAgents has a stronger or distinct capability.
- **Partial**: Core building blocks exist, but key workflows or surfaces are missing.
- **Missing**: Not implemented in OpenAgents.
- **Deliberately Different**: Architectural choice to retain.

### 4.1 Platform, Architecture & Orchestration

| # | Domain | T3 Code (`8cfd2b5211`) | OpenAgents (`855654fc81`) | Status | Priority |
| --- | --- | --- | --- | --- | --- |
| 1 | **App & UI Runtime** | Electron desktop, React web, Expo/React Native iOS & Android; shared TS `packages/client-runtime`; Chromium/WebView rendering. | Compiled Coax GPU apps on macOS (Metal), Linux (Wayland/Vulkan), Windows (Win32/Vulkan), iOS (Metal, zero JS/WebView), Web (Coax/Wasm + WebGPU); shared Coax core (`coax/lib/agent/desktop/`, `apps/shared/app/`). | **Ahead / Different** | Retain |
| 2 | **Durable Orchestration** | Unified SQLite V2 event store, projections, command receipts, `EffectOutbox`, `EffectWorker`, `RunFinalizationService`, `ThreadSettlementService`, typed turn results, pruned log with snapshot fallback. | Split across 4 families: `#authority` hash-chained binary journal (`journal*.coax`), `mcworker-queue.coax` persisted FIFO, `coder-host` daemon runs (`host-runs.coax`, 12 slots) + 4 `EsHost` slots, and routed execution (`environment/execution/v1alpha1`, router/mobile UI unimplemented). No single unified SQLite/event-sourced projection across all chats. | **Partial** | **P0** |
| 3 | **Connection & Offline Sync** | Per-env supervisor, ordered multi-route failover + preflight + learned LAN/tailnet routes, DPoP renewing sessions, HTTP shell snapshot + WS stream, `steadyShells`, 5-min detail cache, persistent mobile offline outbox. | Local Unix socket (`service.sock`) + Nostr relay (`NIP-HOST`/`NIP-44`) + Tailnet zero-config Bonjour pairing (`Tailnet.swift`). Outbox with monotonic op IDs & fencing (`DurableChat`, `NativeTaskApp`), `ACT_REJOIN`, 3-way field merge for chat library (`chat-library-sync.coax`). **Gap:** No persistent offline transcript/projection cache or multi-message offline send queue surviving mobile app kill; no multi-route auto-preflight/failover between LAN/Tailnet/Relay. | **Partial** | **P0** |
| 4 | **Multi-Environment, Peer Linking & Tunnels** | Multi-env client catalog, `EnvironmentsMap`, peer links (`PeerLinks`, `PeerThreadRelay`, `linkOrigin` fencing, `RemoteDelegation`), `/api/preview/tunnel` + desktop SOCKS5 proxy + port forwarding + mobile `localhost` tabs. | Single bound workspace per host grant; Foreman orchestrates local desktop chats only (`Foreman.swift`, requires desktop open); `coder --on ssh:\|cloud\|boat:` in TUI only. **Gap:** No host-to-host peer linking, no remote thread relay, no loopback preview tunnel or port forwarding. | **Missing** | **P1** |
| 5 | **Desktop Platform Parity** | macOS, Linux, Windows (Electron) with SSH remote launcher, WSL backend, Snap-Shot accessibility capture, Finder/Explorer file drag, `localEnvironmentEnabled: false` thin-client mode, auto-updater. | macOS full-featured (`apps/openagents-desktop/` + Ed25519 updater); Linux Wayland/Vulkan working & headlessly tested (`flow-test.sh`); Windows cross-compiled via Zig & Wine-smoked, **missing**: background daemon, Foreman, Gym, Antigravity, image attachment previews, pinned reorder, OLE drag indicator, updater, Authenticode/installer. | **Partial** | **P1** |

### 4.2 Providers, Agents & MCP

| # | Domain | T3 Code (`8cfd2b5211`) | OpenAgents (`855654fc81`) | Status | Priority |
| --- | --- | --- | --- | --- | --- |
| 6 | **Provider Breadth** | **11 built-in drivers**: Claude Code, Codex, Cursor (`@cursor/sdk@1.4.2`), Antigravity 1.5+, OpenCode (1.x/2.x), Pi, Muse, Grok, Kiro, Fx, and generic **ACP Registry** (managed binaries/npm/uvx); unified `ProviderCapabilities` & `ProviderInstanceRegistry`; auto-updater per owning package manager (brew/npm/pnpm/bun/volta/mise). | **6 engines**: Claude Code (`0.3.296`/`2.1.312`), Codex (`0.163.0`), Antigravity (`agy >=1.5.0`), Cursor (`agent acp`), owned **Microcoder** (Vertex/Gemini/OpenRouter/gateway), local **Psionic** (`gpt-oss-20b`), plus Echo. **Gap:** `cursor`/`antigravity` not yet in `engine.coax` `Agent.mode`; Codex not wired into mobile/desktop environment host; no OpenCode/Pi/Grok/Kiro/ACP Registry; no automated provider installer/updater. | **Partial** | **P1** |
| 7 | **Claude Code Integration** | SDK `0.3.321`+, streaming, tools, permissions, structured questions, plan mode, MCP injection, multi-account homes, usage limits, reset credits, native fork/rewind. | SDK `0.3.296` stream-json (`coax/lib/agent/claude/`), isolated profiles (`CLAUDE_CONFIG_DIR`), Keychain read without dialog, `/api/oauth/usage` probe, `PreToolUse` `git-guard`, in-process SDK MCP, `--resume`/`--fork-session`. **Gap:** `rewind_files`, `elicitation`/`request_user_dialog`, `set_max_thinking_tokens`, `mcp_server_status`, Windows; Desktop lacks interactive permission UI. | **Partial** | **P1** |
| 8 | **Codex Integration** | `codex app-server` stdio, thread/turn routing, `turn/steer`, async questions, `/goal`, `/compact`, `/review`, subagents, managed ChatGPT OAuth handoff, usage limits, thread revert. | `codex app-server` (`0.163.0`) in `coax/lib/agent/codex/` + TUI usage probe (`wham/usage`) + exact `turn-receipt.coax` interrupt + dynamic tools. **Gap:** Not exposed in Desktop/iOS environment host; missing `turn/steer`, `/compact`, `/review`, subagents, `requestUserInput`, MCP elicitation, managed remote OAuth handoff. | **Partial** | **P1** |
| 9 | **Antigravity Integration** | `agy >=1.5.0` ACP: Auto mode, background command hold + `usage_update` + `t3_background_wait`, `steers_with_prompt`, detached subagents + `SUBAGENT_STATE_KILLED`, edit-from-here rollback, `--login` OAuth capture, `/compact`, lost thinking-signature recovery, `call_mcp_tool` unwrap, usage limits. | `agy >=1.5.0` gate, `AGY_CLI_DISABLE_AUTO_UPDATE`, `SSH_CONNECTION` profile isolation, Auto mode + escalated `CU_DISP_ASK`, `defer_finalize_background`, `steers_with_prompt`, `available_commands_update`, crash auto-respawn, `agyworker` in Desktop. **Gap:** Detached subagent reader (`SUBAGENT_STATE_KILLED`), edit-from-here rollback, `--login` OAuth URL capture, `/compact` dispatch, lost thinking-signature recovery, `call_mcp_tool` unwrap, usage/limits probe. | **Partial** | **P1** |
| 10 | **MCP Server & Client** | Built-in `/mcp` HTTP server with **14 toolkits** (`attachment`, `device`, `environment`, `html`, `issues`, `message`, `orchestrator`, `preview`, `previewControls`, `project`, `pullRequests`, `theme`, `thread`, `toolSearch`, `worktree`), per-session scoped tokens, outside-agent OAuth (`/mcp` PKCE), **MCP Apps** (`mcpApps/`), and one-click OAuth sign-in from warnings (`McpSignInAction`). | Standalone `coder mcp` (stdio + HTTP, 12 tools: `coder_run/ls/status/log/send/stop/fork/replay/decide/remember/recall/session_read`) + Claude SDK in-process MCP (`claude/mcp.coax`). **Gap:** No general MCP client in Microcoder or Desktop; no built-in PR/CI/issue/worktree/preview/theme MCP toolkits; no MCP OAuth sign-in UI or MCP Apps rendering. | **Partial** | **P1** |

### 4.3 Coding Workbench, Git, Diffs, Terminals & Issues

| # | Domain | T3 Code (`8cfd2b5211`) | OpenAgents (`855654fc81`) | Status | Priority |
| --- | --- | --- | --- | --- | --- |
| 11 | **Workspaces, Worktrees & Checkpoints** | Multi-repo projects, worktrees + sparse-checkout (`t3.json`), setup scripts, **hidden Git-ref per-turn checkpoints** (`CheckpointStore`, `CheckpointRollbackService`, `CheckpointRestoreSafety` refusing shared-checkout clobber), edit-from-here conversation+workspace rewind. | Per-chat worktrees (`Worktree.swift`, `.coder/worktree.json` CoW clones, shared `COAX_TOOL_CACHE`/`CARGO_TARGET_DIR`), `openagents.environment.workspace.v1` (CAS digests, idempotency receipts, argv-only `git`), `git-guard.coax` shared-checkout protection, `session/fork.coax` (TUI/ACP only). **Gap:** No per-turn filesystem checkpoints or safe combined conversation+workspace rewind/edit-from-here; no sparse-checkout config; `fork` not wired into Desktop UI. | **Partial** | **P1** |
| 12 | **Source Control, PRs & Issue Trackers** | **7 Git forges** (GitHub, GitLab, Bitbucket, Azure DevOps, Forgejo, GitCafe, Cursor Origin), **5 issue trackers** (GitHub, GitLab, Azure DevOps, Forgejo, Linear), PR review guide & file ordering, named saved filter views, agent MCP PR/CI/stack/issue tools, auto-settle on external merge. | iOS/shared 4-tab Workspace (**Diff / Branch / Trees / PRs**, `gh api` GitHub-only), Desktop right-pane Changes/Review diff viewer (`page/layout/review.coax`). **Gap:** No Issues UI or issue tracker integrations (GitHub Issues/Linear/etc.); no non-GitHub forges; no PR review guide, CI log viewer, saved filter views, or auto-settle on PR merge. | **Partial** | **P2** |
| 13 | **Terminals / PTYs & Preview** | Persistent per-thread interactive PTY drawer (`ThreadTerminalDrawer`, mobile terminal route, configurable shell), `ServerBrowser` / `DesktopBrowser` / preview PiP / remote `localhost` tunneling, terminal triage (`triage/`). | TUI-only 100×30 PTY tool (`coder/tui/term.coax`) + `$ ` shell escape. **Gap:** No interactive PTY terminal in Desktop, iOS, or `coder-host`; no embedded web preview browser. **Ahead:** Built-in live **iOS Simulator viewer/controller** (`oasim serve`, `Window+Simulator.swift`) + 3D PLE Workbench (`Window+Gym.swift`). | **Partial / Ahead** | **P1** |

### 4.4 UI, Composer, Voice, Security, Automations & Evals

| # | Domain | T3 Code (`8cfd2b5211`) | OpenAgents (`855654fc81`) | Status | Priority |
| --- | --- | --- | --- | --- | --- |
| 14 | **Navigation, Attention Shelves & Search** | Working / Settled / Snoozed attention shelves with custom snooze presets, pin/archive/rename, project grouping, full-text thread search, command palette, title regeneration glimmer. | GPU sidebar (`New chat`, `Search`, `Pinned`, `Recents` with All/Pinned/Archived filters), synced pin/rename/archive (`chat-library-sync.coax`), `⌘K` command palette (`components/app/palette.coax`). **Gap:** No Working/Settled/Snoozed attention shelves or snooze presets; search is title/computer only (no full-text message-content search). | **Partial** | **P2** |
| 15 | **Composer, Rich Transcript & Interactions** | Tiptap rich editor, file/image attachments with retained leases & upload queue, `@` mentions, `/` native slash commands, assistant text selection/citations/comments, structured multi-question forms (`PendingUserInputCard`), proposed plan cards (`ProposedPlanCard`), `html_render` cards, copyable file previews. | Shared GPU Markdown (`components/core/markdown.coax`), folded activity tree (`components/app/activity.coax`), subagent transcript page, Desktop `@`/`/` mentions + attachments + follow-up steer/queue tray + file viewer/editor (`Window+Files.swift`). **Gap:** Desktop lacks interactive permission UI & mode switcher; no structured multi-question/elicitation card, no proposed plan review card, no assistant citations/comments, no `html_render` cards. *(iOS hides model/effort, attachments, and mic by explicit owner choice.)* | **Partial** | **P0 / P1** |
| 16 | **Voice & Dictation** | Native macOS Apple Speech (`AppleSpeech.swift`), server `transcribe-cpp` + Silero VAD, cloud/ZDR OpenRouter audio, streaming transcription toggle, background dictation across thread switches, optional LLM transcript cleanup. | `apps/shared/dictation*.coax` (`#authority` policy, local capture, Metaverse resampler/VAD, multi-segment cutting) wired into CLI, Everglade Desktop, and Desktop conversation layout; hidden on iOS by owner choice; not wired into `/chat` web. **Gap:** No Apple Speech native dictation helper, no streaming toggle, no background dictation across chat switches. | **Partial** | **P2** |
| 17 | **Secrets, Security & Policies** | `secret-provider-onepassword` + multi-value secret cards with show/hide, enterprise `managedPolicy`, DPoP-bound sessions + Connect device approval, `linkOrigin` fencing, `McpToolAccess` compile-time scope declarations. | `#authority` zero-survivor mutation-tested policy units (`accounts`, `journal`, `workmem`, `workspace`, `chat-library`, `sim`, `dictation`), 0600 key files, dialog-free Keychain reads, `fence.coax` prompt-injection fencing, `git-guard.coax`, Foreman dual-credential anti-escalation, Ed25519 signed updates. **Gap:** macOS shell runs unconfined (`bwrap` is Linux-only); no secret-request cards or 1Password integration; no enterprise managed policy file. | **Ahead / Partial** | **P1 / P2** |
| 18 | **Scheduled Tasks, Guest Links, Widgets, Themes, i18n & Evals** | `ScheduledTaskService` (cron/interval + relay offline webhook inbox), HMAC guest share links, iOS/Android usage & activity widgets, push notifications, VS Code/OpenVSX theme import + 100k agent CSS, full app i18n, 5 eval suites (`evals/`) + `scripts/scenarios/*.e2e.ts`. | `evals/coder-desktop/` multi-tier harness + `coder/tests/qualify.coax` independent qualification + `docs/coder/claude-history-evals-spec.md` (proposed); 4 skins on `/chat-gpu` (mono on iOS); local Share Excerpt. **Gap:** No scheduled-task/webhook service in `coder-host`, no guest share URLs, no mobile widgets/push notifications, no custom theme import, no i18n (English only). | **Partial** | **P2** |

---

## 5. Where OpenAgents Is Ahead or Deliberately Different

Do **not** blindly port T3 Code patterns where OpenAgents has an intentional architectural advantage:

1. **Coax-First Compiled Native GPU Stack**: Zero Electron/Chromium/React Native overhead; one Coax codebase drives Metal (macOS/iOS), Vulkan (Linux/Windows), and WebGPU with deterministic memory budgets.
2. **Formal `#authority` + Mutation Testing + Hash-Chained Journal**: T3 relies on TypeScript/Effect runtime tests; OpenAgents gates money, admission, workmem, journal, and workspace policies with zero-survivor mutation testing (`coax mutate`) and SHA-256 hash-chained intent-before-effect logs (`AJR1`).
3. **Owned Agent & ML Stack**: Microcoder (`coax/lib/microcoder/`) + `workmem` anti-weakening + Psionic (`psionic/`) give OpenAgents full control over context compaction, prompt-injection fencing (`fence.coax`), and local inference — T3 Code is purely a wrapper around external agent CLIs.
4. **Decentralized Transport & Zero-Config Tailnet**: `NIP-HOST`/`NIP-44` over Nostr relays + Bonjour/Tailscale `whois` pairing avoid a centralized Clerk/Cloudflare relay dependency.
5. **Live iOS Simulator Control & 3D/Metaverse Surfaces**: `oasim serve` + `Window+Simulator.swift`, PLE Workbench (`Window+Gym.swift`), Metaverse Engine, Content Projects, and Bitcoin-only payments (`pay/`) are OpenAgents-native differentiators.
6. **Wasm + Nostr-Signed Plugins & Golemancy**: `coder/tui/plugins.coax` (NIP-78 signed, capability dependency graph, Wasm sandbox) and `coder/golemancy/golems.coax` (multi-host agent observability) have no direct T3 equivalent.

---

## 6. Prioritized Implementation Roadmap

### P0 — Core Reliability & Workbench Unblockers
1. **Desktop Interactive Permission UI & Mode Switcher** (`apps/openagents-desktop/page/`, `Window.swift`): Surface `can_use_tool` / `session/request_permission` / `MWW_ASK` approval cards and Auto/Supervised mode switching in OpenAgents Desktop (already implemented in `coder/tui/` and `coax/lib/agent/cursor/client.coax`).
2. **Finish Routed Execution Handoff & Unified Durable Recovery** (`docs/protocol/environment-execution-v1alpha1.md`, `apps/shared/app/`): Complete API router dispatch and mobile task controls for `environment/execution/v1alpha1`; add persistent mobile transcript/projection caching and multi-message offline send queue surviving app termination; add automatic Tailnet → LAN → Nostr relay route preference/failover.
3. **Structured Interaction Cards** (`components/app/`, `proto/openagents/environment/`): Extend the runtime wire and shared GPU components beyond boolean Allow/Deny to support typed multi-question forms (`requestUserInput` / `cursor/ask_question` / Claude elicitation) and inert fallback for unknown item kinds.

### P1 — Coding Workbench Parity
4. **Per-Turn Git Checkpoints & Safe Rewind / Edit-From-Here** (`coax/lib/agent/workspace/`, `coax/lib/agent/session/fork.coax`, `apps/openagents-desktop/`): Add hidden Git-ref workspace checkpoints with shared-checkout restore safety (refusing rollback if another active chat shares the checkout), wire `session/fork.coax` into Desktop UI, and complete Antigravity edit-from-here rollback.
5. **Complete Codex & Antigravity Provider Parity**:
   - **Codex** (`coax/lib/agent/codex/`, `coder/tui/host-environment.coax`): Wire Codex into Desktop & mobile environment host, add `turn/steer`, `/compact`, subagent tracking, and `requestUserInput`.
   - **Antigravity** (`coax/lib/agent/engine/antigravity.coax`, `observe/antigravity.coax`): Finish remaining v0.1.11 items — detached subagent reader + `SUBAGENT_STATE_KILLED`, `--login` OAuth URL capture, `/compact` dispatch, lost thinking-signature recovery, `call_mcp_tool` unwrapping, and 1.5.x usage/limits probe.
   - Promote `cursor` and `antigravity` into `coax/lib/agent/engine/engine.coax` unified `Agent.mode`.
6. **General MCP Client & Built-in Orchestration/Forge Toolkits** (`coax/lib/microcoder/`, `coder/tui/mcp*.coax`): Add MCP client support to Microcoder/Desktop and expand `coder mcp` with scoped PR, CI-log, worktree, and issue toolkits.
7. **Interactive PTY Terminal & Port Forwarding** (`coder/tui/host*.coax`, `apps/openagents-desktop/`, `apps/shared/app/`): Expose a bounded PTY session over `coder-host` for Desktop and iOS (replacing TUI-only `term.coax`), plus opt-in loopback port forwarding for remote dev servers.
8. **Windows Desktop Parity** (`apps/openagents-desktop/windows/`): Bring `coder-host` background daemon, Antigravity, image attachment previews, Foreman, and signed updater to Windows and hardware-verify.
9. **Multi-Environment Peer Linking**: Allow one `coder-host` / Foreman instance to delegate to another enrolled machine with mode-ceiling narrowing and `linkOrigin`-style fencing.

### P2 — Breadth, Polish & Ecosystem
10. **Attention Shelves & Full-Text Search** (`apps/shared/app/history/`, `apps/openagents-desktop/`): Add Working / Settled / Snoozed shelves and message-content search.
11. **Issue Trackers & Additional Git Forges**: Add GitHub Issues (and optional Linear/GitLab) to `openagents.environment.workspace.v1` and the iOS/Desktop Workspace pane.
12. **Scheduled Tasks / Cron Automations in `coder-host`**: Stand up recurring agent runs and optional webhook triggers without requiring the Desktop window to stay open.
13. **Claude History Evals Implementation**: Execute `docs/coder/claude-history-evals-spec.md` (`evals/claude-history/`) alongside `evals/coder-desktop/`.
14. **Guest Share Links, Mobile Push/Widgets, Themes & i18n**: Add read-only shareable conversation links, background push notifications for pending approvals/completions, theme selection on iOS/Desktop, and localization if/when non-English markets are prioritized.

### Explicitly Out of Scope (Do Not Port)
- Electron, Chromium, React Native, Expo, or Effect-TS runtime dependencies.
- Centralized Clerk/Cloudflare-only relay lock-in (keep Nostr + Tailnet).
- Non-Bitcoin billing/payment rails.
- Exposing hidden iOS composer controls (model/effort/attachments/mic) unless the owner revisits `apps/openagents-ios/README.md`.
