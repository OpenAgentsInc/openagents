# Audit: t3code v0.1.11 Antigravity Support & OpenAgents Parity Plan

Date: 2026-11-20  
Reference repo: `~/work/projects/repos/t3code` (`v0.1.10` `7d32075e37` .. `v0.1.11` `59651c1761`, checked at `HEAD` `d15123668e`)  
Prior OpenAgents audit: [`docs/audits/2026-11-11-antigravity-in-coder-and-golemancy.md`](2026-11-11-antigravity-in-coder-and-golemancy.md)

---

## 1. Executive Summary

`t3code` `v0.1.11` (released 2026-11-20, 643 commits since `v0.1.10`) shipped **45 Antigravity-specific commits** (plus **5 adjacent commits** touching Antigravity files). The release completes `t3code`'s migration onto Google's native **Antigravity CLI 1.5+** (`agy`, minimum `>=1.5.0 <2`, recommended `>=1.5.12 <2`) over **ACP (Agent Client Protocol)** and closes the main UX and reliability gaps that previously made Antigravity feel second-class compared to Claude Code and Codex:

1. **Auto review mode** — uses `agy`'s own built-in `auto` mode (`session/set_mode` with `modeId: "auto"`) so `agy` approves routine steps itself and only escalates `session/request_permission` when it genuinely needs the user.
2. **Edit from here (conversation rollback)** — enables editing/rewinding earlier messages by opening a fresh `session/new` with replayed history while preserving pre-rollback subagent attribution.
3. **Background commands & detached continuations** — keeps turns alive while non-daemon `execute` commands outlive `session/prompt`, waits for `usage_update` (up to 120s) on detached replies, supports `Stop` (`session/cancel`) without tearing down the runtime, and wires `background_wait`.
4. **Subagents** — reads Antigravity 1.5's detached subagent descriptors and inbox messages from `antigravity-acp/brain/`, steers mid-turn via `session/prompt` instead of `session/cancel` so running subagents aren't killed, and detects `SUBAGENT_STATE_KILLED` so killed children don't wedge parent turns.
5. **Runtime 1.5+ & Auth/Profile isolation** — enforces `>=1.5.0`, disables `agy` background self-updates (`AGY_CLI_DISABLE_AUTO_UPDATE=1`), isolates per-profile Google OAuth storage (`SSH_CONNECTION` workaround for 1.5 keyring sharing), and captures OAuth URLs via `agy acp ... --login`.
6. **Compaction, resilience, MCP, skills & usage** — gates `/compact` on `available_commands_update`, survives host sleep and local harness crashes, recovers from lost Gemini thinking signatures, normalizes `call_mcp_tool`, surfaces native slash commands, honors global `AGENTS.md`/`GEMINI.md`, and reads 1.5.x usage/limits.

OpenAgents already has a working Coax ACP client and Antigravity engine (`coax/lib/agent/cursor/*`, `coax/lib/agent/engine/antigravity.coax`, `coder/tui/coder.coax`, `apps/everglade-desktop/next.coax`), but currently runs `agy` strictly in `default` mode with no background-command continuation, no subagent support, no Auto mode, no profile/auth isolation, and no `/compact` or thinking-signature recovery. This audit documents every `t3code` mechanism and lays out a phased Coax-first plan to bring OpenAgents to parity.

---

## 2. Complete Inventory of `t3code v0.1.11` Antigravity Commits

Range: `v0.1.10..v0.1.11` (45 Antigravity commits + 5 adjacent commits).

### Features (4)
- `5dd565d6b1` — `feat(antigravity): Auto uses agy's own Auto review (#39378)`
- `a99faf5a62` — `feat(antigravity): edit earlier messages with Edit from here (#38227)`
- `2109868980` — `feat(cli): allow Antigravity in terminal triage (#31761)`
- `cdf5a6c0fc` — `fix(antigravity): allow compaction when the runtime advertises it (#37020)`

### Runtime, Installation & Compatibility (8)
- `90a86d5cb1` — `fix(server): refuse Antigravity runtimes below 1.5 (#38970)`
- `d1822be5a9` — `fix(server): stop managed Antigravity CLI from self-updating in place and recover on reinstall (#39363)`
- `68ea2220c1` — `fix(server): managed Antigravity and Codex runtimes prune old versions (#38740)`
- `0058a4abb3` — `fix(server): Antigravity finds the agy CLI on PATH when no binary path is set (#37916)`
- `86b5734cfc` — `fix(server): Windows .cmd launchers work as the Antigravity binary path (#24683)`
- `1b555ccb01` — `fix(server): Antigravity install on Windows survives a briefly locked runtime folder (#29279)`
- `67692a7549` — `fix(server): ACP Registry Antigravity no longer leaves unpacked temp folders (#39028)`
- `b1ca56209c` — `fix(server): an Antigravity update refreshes every instance's model list (#36035)`

### Authentication & Instance Isolation (5)
- `084ad5e1a0` — `fix(server): each Antigravity Google sign-in stays in its own instance (#39485)`
- `944d1820e2` — `fix(server): Antigravity Google sign-in shows its link with the native CLI (#38953)`
- `2ba4fde9a5` — `fix(server): Antigravity 1.5 asks for sign-in when it cannot open a browser (#36037)`
- `320ab2a2ac` — `fix(server): Antigravity drops a stale sign-in failure once a session signs in (#39443)`
- `71e1bb2dcc` — `fix(server): Antigravity sign-in names a runtime crash instead of blaming Google (#28355)`

### Background Commands & Detached Continuations (4)
- `12be92d034` — `fix(server): Antigravity threads keep going after a background command finishes (#38864)`
- `f9b9bc742d` — `fix(server): Antigravity threads stay working while a background command runs, and Stop ends it (#38916)`
- `505f6d6cf4` — `fix(server): Antigravity accepts t3_background_wait for its own background commands (#39421)`
- `e8aa0debb3` — `test(server): agy permission continuation test waits for the reply's usage_update (#38899)`

### Subagents (4)
- `194726492b` — `fix(server): a killed Antigravity subagent no longer keeps its turn running (#39519)`
- `cfd24da4a7` — `fix(server): steering an Antigravity turn no longer stops its subagents (#39165)`
- `066be30005` — `fix(server): read detached Antigravity 1.5 subagent transcripts (#39004)`
- `43b37b6d3c` — `fix(server): Antigravity invoke_subagent calls show as running subagents again (#38966)`

### Sessions, Turn Lifecycle & Recovery (7)
- `990f31cac2` — `fix(server): host sleep no longer times out an Antigravity turn (#39395)`
- `9f6f5540cf` — `fix(server): worktree handoff no longer leaves Antigravity threads Waiting (#39422)`
- `94baff19c6` — `fix(server): Antigravity resumes after its local harness fails (#23654)`
- `f9a3c38c50` — `fix(antigravity): recover threads after a lost thinking signature (#28889)`
- `c8b214158e` — `fix(server): Antigravity 1.5 temporary setup and text-helper sessions clean up their files (#39275)`
- `9d0d00656f` — `fix(server): keep Antigravity thread cwd in workspace by omitting attachmentsDir from additionalDirectories (#37997)`
- `c67deb12dd` — `fix(antigravity): thread sessions spawn with their T3 thread and MCP environment (#36495)`

### MCP, Protocol Normalization, Attachments & Models (6)
- `7f928416b2` — `fix(antigravity): MCP calls through call_mcp_tool show the tool, not "Ran command" (#39474)`
- `158394194c` — `fix(shared): html_render cards show for Antigravity's acp-mcp-call terminal fallback (#38898)`
- `6c0698c0af` — `fix(server): Antigravity turns no longer fail on a roots notification to the T3 MCP server (#36439)`
- `5ee1c2f652` — `fix(server): accept Antigravity model IDs with thinking levels when delegating (#39413)`
- `ff8a831067` — `fix(antigravity): accept UTF-16 text attachments with a BOM (#39456)`
- `c4dc9ed55b` — `fix(antigravity): keep truncated command output valid Unicode (#38837)`

### Rules, Skills, Slash Commands & Usage Limits (7)
- `e923b5d8d8` — `fix(server): Antigravity threads follow the user's global AGENTS.md and GEMINI.md (#39531)`
- `bb234221ce` — `fix(server): Antigravity refreshes no longer erase scanned folders' project skills (#39037)`
- `292a8e448b` — `fix(server): list skills from plugins in the Antigravity instance profile (#38960)`
- `00200bd412` — `fix(server): Antigravity native slash commands reach the composer (#35621)`
- `91f166adf3` — `fix(server): Antigravity native slash commands reach the composer after a restart (#38955)`
- `5283f33ca9` — `fix(server): Antigravity limits read the 1.5.x CLI's stored sign-in (#38957)`
- `490a0d687b` — `fix(server): Antigravity usage skips agy index databases instead of reporting partial history (#39435)`

### Adjacent Commits Touching Antigravity (5)
- `90aa7669a8` — `fix(server): one compatibility lookup and one refusal for broken provider versions (#39427)`
- `b5bef4d939` — `fix(server): /logout in chat needs providers:manage (#39027)`
- `1a42ba1b5f` — `fix(server,web): models stay in the picker after one failed provider check (#39114)`
- `38f7761148` — `fix(shared): keep literal quotes in POSIX PATH directories (#38238)`
- `2b2d65d131` — `fix(server): report managed runtime verification timeouts (#37896)`

---

## 3. Technical Deep Dive: How `t3code` Integrates Antigravity

In `t3code`, Antigravity is a first-class ACP provider built on `packages/effect-acp` + `packages/provider-acp` with Antigravity-specific adapter/driver/protocol files in `apps/server/src/provider/` and `apps/server/src/orchestration-v2/Adapters/AntigravityAdapterV2.ts`.

### 3.1 Spawning, CLI Flags & Environment (`90a86d5cb1`, `084ad5e1a0`, `d1822be5a9`, `2ba4fde9a5`)
- **Legacy pre-1.5 harness removed**: `--uid=` and `--alsologtostderr` are gone. `t3code` checks `initializeResult.agentInfo.version` (stripping any `agy_acp_server_` prefix) and refuses `<1.5.0` as `broken` (recommended `>=1.5.12 <2`).
- **Exact spawn command**:
  ```text
  <agy> acp --gemini-dir <profile.geminiHome> --app-data-dir antigravity-acp
  ```
  Spawned with `extendEnv: false`, `shell: false`. Note that `agy` always joins `--app-data-dir` beneath `--gemini-dir` even if given an absolute path, so `antigravity-acp` is passed as a relative folder name.
- **Environment variables**:
  - `GEMINI_HOME=<profile.geminiHome>` — isolates per-profile state.
  - `AGY_ACP_FORCE_FILE_STORAGE=1` — legacy file-storage hint.
  - `SSH_CONNECTION=127.0.0.1 0 127.0.0.1 0` (when `baseEnv.SSH_CONNECTION` is unset, for `oauth-personal` profiles) — **critical 1.5 workaround**: Antigravity 1.5 ignores `AGY_ACP_FORCE_FILE_STORAGE` and shares a single OS keyring entry across instances unless `SSH_CONNECTION` is set, which forces file-backed token storage inside `profile.geminiHome`.
  - `AGY_CLI_DISABLE_AUTO_UPDATE=1` — stops `agy` from self-updating in place while managed.
- **OAuth sign-in (`--login`)**:
  - Because `agy` ignores custom browser helpers during ACP `authenticate`, `t3code` spawns `<agy> acp --gemini-dir ... --app-data-dir antigravity-acp --login` in a temp dir with `DISPLAY` and `WAYLAND_DISPLAY` stripped.
  - `agy --login` prints the `https://accounts.google.com/...` OAuth URL to stdout; `t3code` surfaces that link to the user, writes the pasted callback/code + `\n` to stdin, and awaits exit `0`.

### 3.2 Auto Review Mode (`5dd565d6b1`)
- **Capability detection**: Inspects `session/new` / `session/load` result's `modes.availableModes` for an entry with `id === "auto"`.
- **ACP mode mapping (`session/set_mode`)**:
  - `auto` → `modeId: "auto"` **only if** `"auto"` is advertised in `availableModes`; otherwise falls back to `"default"`.
  - `full-access` (`yolo`) and `auto-accept-edits` (`auto_edit`) still send `modeId: "default"` to `agy` (since `agy` only exposes `default` / `plan` / `auto`) and enforce auto-approval in the host's `session/request_permission` handler.
- **Escalated permission handling (`session/request_permission`)**:
  - When `runtimeMode === "auto"`, `agy`'s own reviewer has already approved routine actions and only sends `session/request_permission` for actions it intentionally escalated to the human.
  - Therefore, when `runtimeMode === "auto"`, any non-`deny` host policy disposition (`allow` or `ask`) is coerced to `"ask"` so the host always prompts the user instead of silently auto-approving an escalated step. Explicit `deny` rules still deny.

### 3.3 Edit From Here / Conversation Rollback (`a99faf5a62`)
- **No native ACP rewind RPC**: `agy` has no `session/rewind` or `session/rollback` method.
- **Host-side rollback**:
  - `AntigravityDriver` advertises `supportsConversationRollback: true`.
  - On edit/rollback to an earlier message, `t3code` opens a fresh `session/new` and replays the kept conversation history as context.
  - Subagent records created before the rollback stay attributed to the original Antigravity session ID by resolving `rootSessionId` from `attempt.nativeThreadId` before falling back to the thread's current `nativeThreadRef.nativeId`.

### 3.4 Background Commands & Detached Continuations (`12be92d034`, `f9b9bc742d`, `505f6d6cf4`)
Antigravity's `session/prompt` RPC can resolve *while* a non-daemon shell command is still running in the background, and `agy` later streams a detached continuation reply once that command finishes. `AntigravityAdapterV2` enables five ACP flavor flags to handle this cleanly:

| Flag | Behavior |
| --- | --- |
| `deferFinalizeForBackgroundWork: true` | When `session/prompt` settles while a matching tool or subagent is still active, hold the turn open instead of finalizing it. |
| `toolOutlivesPrompt` | Matches `tool.kind === "execute" && tool.rawInput?.IsDaemon !== true` (background commands hold the turn; daemons do not). |
| `enablePostSettleContinuation: true` & `wakeOnDetachedRequest: true` | Tracks detached `session/update` streams and detached `session/request_permission` calls that arrive after `session/prompt` settled. |
| `replyEndsWithUsageUpdate: true` | Because `agy` terminates each detached reply with a `usage_update` notification, the adapter waits for `usage_update` with a 120s safety ceiling (`DETACHED_CONTINUATION_USAGE_LIMIT_MS = 120_000`) instead of a short 3s quiet debounce. |
| `preserveRuntimeOnSettledInterrupt: true` | A normal follow-up message while a background command runs does *not* cancel or tear down the runtime. An explicit **Stop** (`interruptTurn`) sends `session/cancel` (`{ sessionId }`) to stop the background command while keeping the `agy` process alive. |

For `background_wait`, because Antigravity's background `execute` tool calls carry no native task ID, if any `toolOutlivesPrompt` tool is still running on the held turn, `awaitBackgroundTasks(taskIds)` returns all requested `taskIds` as matched so the orchestrator waits on the held turn.

### 3.5 Subagents (`194726492b`, `cfd24da4a7`, `066be30005`, `43b37b6d3c`)
- **On-disk 1.5 detached subagent state** (under `<geminiHome>/antigravity-acp/brain/`):
  - Child descriptor: `brain/<rootSessionId>/.system_generated/subagents/<childId>.json` (`conversationId`, `subagentDescriptor.role`, `state`).
  - Child inbox: `brain/<childId>/.system_generated/messages/*.json` (`sourceMetadata.tool.toolCall` with `name === "invoke_subagent"`, `id`, `argumentsJson`).
- **Mid-turn steering without killing subagents (`cfd24da4a7`)**:
  - Sending `session/cancel` to steer a turn kills Antigravity's running subagents.
  - `AntigravityAdapterV2` sets `steersWithPrompt: true`: `steerTurn` sends a new `session/prompt` directly mid-turn without calling `session/cancel`. `agy acp` treats a mid-turn `session/prompt` as an in-flight steer and keeps subagents alive.
- **Killed subagents (`194726492b`)**:
  - When `manage_subagents` kills a child, `agy` writes `"state": "SUBAGENT_STATE_KILLED"` to `.system_generated/subagents/<childId>.json` and never posts a completion message to the parent inbox.
  - Checking `state === "SUBAGENT_STATE_KILLED"` and excluding killed children from active work prevents the parent turn from hanging forever.

### 3.6 Compaction, Resilience & Cleanup (`cdf5a6c0fc`, `990f31cac2`, `f9a3c38c50`, `94baff19c6`, `c8b214158e`)
- **Compaction**: `available_commands_update` is recorded per session; `/compact` is only allowed when `"compact"` is in the advertised command set, and is dispatched as a `/compact` prompt.
- **Host sleep**: Guards elapsed-time checks and runtime teardown transitions so waking from sleep doesn't immediately time out an active turn.
- **Lost thinking signature**: Detects Gemini/Antigravity missing/corrupted thinking-signature errors and recovers by rebuilding context on a fresh session.
- **Harness crash recovery**: Marks `runtimeRestartRequired` on unexpected `agy` exit and transparently respawns + `session/load`s on the next turn.
- **Temp session cleanup**: Before unlinking temporary `<sessionId>.db` / `<sessionId>.pbtxt` files, verifies ownership by reading `trajectory_metadata_blob` in `antigravity-acp/conversations/<sessionId>.db` and decoding the `TrajectoryMetadata` protobuf (`workspace_uri`, `session_id`) so real user sessions are never touched.

### 3.7 MCP, Rules, Skills, Slash Commands & Usage
- **MCP normalization**: Recognizes `call_mcp_tool` calls and displays the underlying MCP tool name/args instead of generic `"Ran command"`; handles `roots` notifications cleanly without failing turns.
- **Global rules**: Ensures Antigravity threads respect global `AGENTS.md` and `GEMINI.md`.
- **Native slash commands**: Preserves `available_commands_update` across restarts and surfaces them in the composer.
- **Usage & limits**: Reads 1.5.x stored sign-in for quota/limits and skips `agy` index databases when aggregating usage history.

---

## 4. Current State of Antigravity in OpenAgents

OpenAgents already has a solid Coax ACP foundation:

- **Shared ACP transport & client**: `coax/lib/agent/cursor/transport.coax` and `coax/lib/agent/cursor/client.coax` implement JSON-RPC 2.0 over stdio (`initialize`, `authenticate`, `session/new`, `session/load`, `session/resume`, `session/prompt`, `session/cancel`, `session/set_mode`, `session/set_model`, `session/set_config_option`, `session/request_permission`).
- **Shared ACP engine**: `coax/lib/agent/engine/cursor.coax` (`CuEngine`, `cue_*`) manages process lifecycle, resume, steering/cancel, and a 120s/2 stall watchdog (`cue_watch`).
- **Antigravity profile**: `coax/lib/agent/engine/antigravity.coax` (`agy_agent`, `agy_program`, `agy_env_load` for `~/.openagents/coder/antigravity.env`, `agy_launch`, `agy_configure` defaulting to `low` effort and `default` mode, `agy_signed_out`, `agy_provider`, `agy_turn_failed`, `agy_outside`).
- **Transcript observer**: `coax/lib/agent/observe/antigravity.coax` parses `~/.gemini/antigravity-cli/brain/<id>/.system_generated/logs/transcript.jsonl` for Golems / Verse OS.
- **Surfaces**:
  - `coder/tui/coder.coax` supports `--engine antigravity` (`agy`), `/engine antigravity`, `Shift-Tab`, `run`, `fork`, `replay`, `--ask`, models/effort, resume, per-turn token usage, and `coder acp`.
  - `apps/everglade-desktop/next.coax` runs Antigravity as an automated Workshop coder (`EVERGLADE_CODER_ENGINE=antigravity`), pricing turns via `coax/lib/gateway/card.coax` and explicitly barring subagents/web/browser tools.
  - `apps/openagents-desktop/` does not yet expose Antigravity.

---

## 5. Gap Analysis Matrix (`t3code v0.1.11` vs. OpenAgents)

| Capability | `t3code v0.1.11` | OpenAgents Today | Priority | Target OpenAgents Files |
| --- | --- | --- | --- | --- |
| **Runtime 1.5+ check & flags** | Requires `>=1.5.0`, `--gemini-dir`, `--app-data-dir antigravity-acp`, `AGY_CLI_DISABLE_AUTO_UPDATE=1` | Spawns `agy acp` with no version gate or `--gemini-dir` / `--app-data-dir` | **P0** | `coax/lib/agent/engine/antigravity.coax`, `coax/lib/agent/cursor/client.coax` |
| **Auto review mode** | Detects `"auto"` in `availableModes`, sends `session/set_mode` `"auto"`, coerces escalated permissions to `"ask"` | Always forces `modeId: "default"` | **P0** | `coax/lib/agent/cursor/client.coax`, `coax/lib/agent/engine/antigravity.coax`, `coder/tui/coder.coax` |
| **Background commands & detached continuation** | Holds turn open for non-daemon `execute`, waits for `usage_update` (120s cap), `Stop` sends `session/cancel` without killing runtime | Turn ends when `session/prompt` settles; Everglade tells agent to wait in foreground | **P0** | `coax/lib/agent/engine/cursor.coax`, `coax/lib/agent/engine/antigravity.coax` |
| **Subagents & mid-turn steering** | Reads `brain/<id>/.system_generated/subagents/*.json`, handles `SUBAGENT_STATE_KILLED`, steers via `session/prompt` without `session/cancel` | Subagents barred in Everglade; steering cancels turn; no detached subagent reader | **P1** | `coax/lib/agent/engine/cursor.coax`, `coax/lib/agent/observe/antigravity.coax` |
| **Edit from here / rollback** | Fresh `session/new` + replayed kept history; preserves pre-rollback subagent `rootSessionId` | Has `fork`/`replay` in CLI, no in-place edit-from-here rollback | **P1** | `coder/tui/coder.coax`, `coax/lib/agent/session/fork.coax` |
| **Profile & OAuth isolation** | `GEMINI_HOME`, `SSH_CONNECTION` workaround, `agy acp ... --login` OAuth URL capture | Uses ambient `agy` login or `antigravity.env` ADC; no isolated profiles or `--login` flow | **P1** | `coax/lib/agent/engine/antigravity.coax` |
| **Compaction (`/compact`)** | Tracks `available_commands_update`, enables `/compact` only when advertised | No compaction support for Antigravity | **P1** | `coax/lib/agent/cursor/client.coax`, `coax/lib/agent/engine/cursor.coax`, `coder/tui/coder.coax` |
| **Resilience (sleep, harness crash, lost thinking signature)** | Sleep-safe timeouts, auto-respawn + `session/load`, thinking-signature recovery | Stall watchdog (120s/2) and `⚠` failure detection; no crash-respawn or thinking-signature recovery | **P2** | `coax/lib/agent/engine/cursor.coax`, `coax/lib/agent/engine/antigravity.coax` |
| **MCP tool normalization & native slash commands** | Unwraps `call_mcp_tool`, surfaces `available_commands_update` in composer | Raw tool titles; slash commands not surfaced | **P2** | `coax/lib/agent/cursor/client.coax`, `coder/tui/coder.coax` |
| **Desktop UI exposure** | Full picker, auth banner, mode selector, usage | Not yet wired into `apps/openagents-desktop/` | **P2** | `apps/openagents-desktop/` |

---

## 6. Recommended Implementation Plan (Coax-First)

### Phase 1 — Runtime 1.5+ Contract & Auto Mode (P0)
1. **Version gate & spawn flags (`coax/lib/agent/engine/antigravity.coax`, `coax/lib/agent/cursor/client.coax`)**:
   - Parse `agentInfo.version` from `initialize` response (strip `agy_acp_server_` prefix); refuse `< 1.5.0` with an actionable upgrade message.
   - Support optional isolated `--gemini-dir <dir> --app-data-dir antigravity-acp` and set `AGY_CLI_DISABLE_AUTO_UPDATE=1` when spawning managed/profiled `agy`.
2. **Auto mode (`coax/lib/agent/cursor/client.coax`, `coax/lib/agent/engine/antigravity.coax`, `coder/tui/coder.coax`)**:
   - Parse `modes.availableModes` from `session/new` / `session/resume` / `session/load` responses and record whether `"auto"` is advertised.
   - Add an `auto` mode option in `agy_configure` / Coder (`--mode auto` or `Shift-Tab` / `/mode auto`) that sends `session/set_mode` with `modeId: "auto"` when advertised (falling back to `"default"`).
   - In `session/request_permission`, when active `runtime_mode == "auto"`, coerce any non-`deny` disposition to `ask` so escalated approvals are always shown to the user rather than silently auto-approved.
   - Add `#test` coverage in `coax/lib/agent/cursor/tests.coax` and `coder/tui/agy_standin.coax`.

### Phase 2 — Background Commands & Detached Continuations (P0)
1. **Held-turn state machine (`coax/lib/agent/engine/cursor.coax`)**:
   - Track active non-daemon `execute` tool calls (`kind == "execute"` and `rawInput.IsDaemon != true`).
   - When `session/prompt` resolves while such a tool is still running, transition to a `HELD_BACKGROUND` state instead of finalizing the turn.
   - While held, accept detached `session/update` notifications and detached `session/request_permission` requests.
   - End the detached reply when `usage_update` arrives (with a 120s safety cap).
   - Distinguish follow-up steering from explicit Stop: explicit Stop sends `session/cancel` without killing the `agy` process.

### Phase 3 — Subagents & Safe Mid-Turn Steering (P1)
1. **Mid-turn steering (`coax/lib/agent/engine/cursor.coax`)**:
   - Add an agent capability flag `steers_with_prompt` (true for `agy`). When steering an active turn, send `session/prompt` directly without `session/cancel` so running subagents are preserved.
2. **Detached subagent observer (`coax/lib/agent/observe/antigravity.coax`)**:
   - Read `brain/<rootSessionId>/.system_generated/subagents/<childId>.json` and `brain/<childId>/.system_generated/messages/*.json`.
   - Treat `state == "SUBAGENT_STATE_KILLED"` as terminal so killed subagents never hold a parent turn open.
   - Expose live subagent status in Coder TUI and Golems.

### Phase 4 — Edit From Here, Compaction & Auth Isolation (P1)
1. **Edit from here (`coder/tui/coder.coax`, `coax/lib/agent/session/fork.coax`)**:
   - Implement message-index rollback for Antigravity by starting a fresh `session/new` and replaying the kept prefix, preserving original session IDs on prior subagent records.
2. **Advertised `/compact` & slash commands (`coax/lib/agent/cursor/client.coax`, `coder/tui/coder.coax`)**:
   - Store `available_commands_update` per session; allow `/compact` when `"compact"` is advertised.
3. **Profile isolation & `--login` (`coax/lib/agent/engine/antigravity.coax`)**:
   - When using isolated profiles with Google OAuth, pass `GEMINI_HOME=<profile>` and `SSH_CONNECTION=127.0.0.1 0 127.0.0.1 0` to keep credentials file-backed per profile, and support `agy acp ... --login` with `DISPLAY`/`WAYLAND_DISPLAY` unset to capture the OAuth URL.

### Phase 5 — Resilience, MCP Normalization & Desktop Integration (P2)
1. **Recovery (`coax/lib/agent/engine/cursor.coax`, `coax/lib/agent/engine/antigravity.coax`)**:
   - Auto-respawn `agy` + `session/load` (or `session/resume`) on unexpected local process exit.
   - Detect Gemini lost thinking-signature errors and recover on a fresh session.
   - Make stall watchdog resilient to host sleep/wake jumps.
2. **MCP normalization**:
   - Unwrap `call_mcp_tool` in tool-call display so Coder/Desktop show the actual MCP tool name instead of generic command labels.
3. **Desktop UI (`apps/openagents-desktop/`)**:
   - Surface Antigravity in the engine/model picker with Auto mode and status indicators once Phases 1–2 land in the shared Coax engine.
