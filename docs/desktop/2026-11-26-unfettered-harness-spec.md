# Unfettered Harness Specification: Full Auto, Always-On Usage, Universal Introspection, Borderless Session Control, Mobile Parity, and Bi-Directional Voice

**Date:** 2026-11-26  
**Status:** Active Implementation Specification  
**Scope:** OpenAgents Desktop (`apps/openagents-desktop/` — `--minimal` primary), OpenAgents iOS (`apps/openagents-ios/` + `apps/shared/antigravity/`), Agent Core (`coax/lib/inky/`, `coax/lib/agent/desktop/`, `coax/lib/agent/environment/`), Worker & Daemon (`coder/tui/`), CLI (`apps/openagents-cli/`)

---

## 1. Philosophy & Product Direction

What we do **not** optimize for:
- Ceremony around diff viewers, file trees, embedded browsers, document previewers, or "safety theater" (permission popups, trusted directory gates, guardian confirmations, sandboxed execution barriers).

What we care about **most** — and what MUST work out of the box in the primary shipped `--minimal` Desktop app and iOS app with zero feature-flag hoops:

1. **Full Auto Mode** — Never pester the operator about permissions, trusted directories, plans, secrets, tokens, forms, or billing/bank info. Everything runs automatically; missing tokens/credentials/form fields resolve autonomously from a local Operator Vault & Autofill Profile.
2. **Always Show My Usage** — Always-visible, live usage HUD showing active session tokens/context %, daily & aggregate spend, and per-provider quota windows + reset timers across every screen on Desktop and iOS.
3. **Universal Introspection** — Collapsible sidebar sections exposing every session across the operator's fleet: local OpenAgents Desktop chats, live local TUI/terminal/`tmux` sessions, other harnesses running on this machine (Claude Code, Codex, Antigravity, Cursor), and harnesses/sessions running on other machines (Tailscale peers & SSH hosts).
4. **Borderless Session Control** — Talk to, read, steer, interrupt, spawn, and command **any** session (local Desktop, TUI, external harness, or remote machine) from any other session, from the UI, or from CLI — with **zero** permission boundaries or tier locks.
5. **1:1 Mobile Parity** — The iOS/iPadOS app does the exact same thing as the Desktop app: same models/harnesses (no Antigravity-only wall), same Full Auto & Autofill, same Always-On Usage HUD, same Universal Introspection sidebar, same Borderless Session Control, and same Bi-Directional Voice.
6. **Dictation and Bi-Directional Voice** — Instant on-device/local dictation AND full-duplex bi-directional voice conversation (streaming sentence-chunked TTS, VAD auto-send, acoustic echo cancellation + barge-in interruption, AirPods/Bluetooth support) on both Desktop and iOS.

---

## 2. Ground-Truth Gap Audit (What We Have vs. What We Don't)

| Pillar | Already Built in Repo | Missing / Broken Today |
| --- | --- | --- |
| **1. Full Auto Mode** | `flawed_settings.coax` (`permission_mode=0` Allow All, `sandbox_mode=0`, `git_guard_on=false`); `permit.coax` (`CP_PERMIT_KIND_APPROVAL`/`QUESTION`/`PLAN`/`SECRET`, `cp_permit_should_gate`); `coax/lib/inky/tools.coax` (`ik_tool_admit`). | No single global **Full Auto** kill-switch (default ON) across all workers/harnesses/sub-agents; `permit.coax` still pops cards when `approval_mode` or `set_agy_mode != 0` or on `MWW_PERMIT`/`CP_PERMIT_KIND_PLAN`/`SECRET`; `inky/tools.coax` still enforces `ik_path_under` confinement even in `danger-full-access`; **no Operator Vault / Autofill Profile** to autonomously fill tokens, API keys, forms, addresses, or billing/bank fields without nagging the user. |
| **2. Always Show My Usage** | `page/usage.coax` (808 LOC: 5 providers, quota windows, explicit status, price overrides, pooling, recovery, `cp_usage_ring`, `cp_usage_popover`, `cp_settings_usage`); `Window+Usage.swift` (332 LOC); `conversation.coax` (`cp_context_ring`); iOS `gpu-page.coax` single-session `usageBar`. | **`page/usage.coax` is STUBBED OUT in `page-v1.coax`/`v1-stubs.coax` and gated behind `DC_FLAG_USAGE` (off by default)!** Users on `--minimal` see no quota ring, no usage popover, and no Settings -> Usage. No persistent always-on usage bar across Dashboard + Chat. `UsageProber` only reads `usage-{claude,codex,antigravity}.txt` and omits live Flawed/Gemini/Inkling daily token/USD totals. iOS lacks account quota/daily spend wire & HUD. |
| **3. Universal Introspection** | `page/layout/sidebar.coax` + `sidebar_groups.coax` (Working / Pinned / Recents / Snoozed / Archived for local Desktop chats); `page/environments.coax` + `Window+Environments.swift` + `Tailnet.swift` (SSH config parsing, Boat/WSL, Tailscale peers). | **`page/environments.coax` is STUBBED OUT in `page-v1.coax`/`v1-stubs.coax` and gated behind `DC_FLAG_ENVIRONMENTS`!** Sidebar only shows OpenAgents Desktop chats — zero visibility into local **TUI/`tmux`/PTY sessions**, **other local harnesses** (Claude Code, Codex, Antigravity, Cursor), or **remote machine sessions** (Tailscale/SSH peers). No unified introspection model or daemon RPC (`ListIntrospection`) or click-to-inspect live stream. |
| **4. Borderless Session Control** | `Window+ForemanAPI.swift` + `Window+ForemanChats.swift` + `Foreman.swift` + `foreman-ctl` (local HTTP API: list/read/create/send/control/stop-all/wait/events/search). `docs/coder/read-other-chats-spec.md` (Proposed, read-only). | **Foreman API is gated behind `DC_FLAG_FOREMAN` (off in `--minimal`)**, enforces `observe`/`managed`/`autonomous` tier boundaries and `operator_tier_raise` locks, only injects `CODER_FOREMAN_URL`/`TOKEN` into the single Foreman chat, and cannot address TUI/`tmux`, external harnesses, or remote machine sessions. Normal agent sessions have **zero** cross-session tools (`session_list`, `session_read`, `session_send`, `session_command`, `session_spawn`). No UI `@session` / direct command dispatch to any session. |
| **5. Mobile App Full Parity** | `apps/openagents-ios/gpu-page.coax` (2473 LOC Coax/Metal GPU UI: Dashboard mini-windows, sidebar Working/Recents, `/` & `@` autocomplete, Todos, approvals, iPad shortcuts); `apps/shared/antigravity/` (NIP-44 over Tailscale). | **`coax/lib/agent/environment/phone-agent-policy.coax` (`#authority`) hard-blocks non-Antigravity models** (refuses Flawed Neon/Inkling, Claude, Codex!). Missing: Flawed/multi-harness models, Full Auto toggle & Autofill settings, Always-On Usage HUD & Usage sheet, Universal Introspection sidebar (TUI + external harnesses + remote machines), Borderless Session Control over Tailscale, Full Settings sheet, and Bi-Directional Voice. |
| **6. Dictation & Bi-Directional Voice** | `page/voice.coax` (606 LOC) + `Window+Voice.swift` (604 LOC): Apple Speech STT + Local Whisper CLI + 8-bar waveform pill + filler stripping + custom dictionary + `Opt+Space`; iOS `VoiceDictationController.swift` (110 LOC STT). | **`page/voice.coax` is STUBBED OUT in `page-v1.coax`/`v1-stubs.coax` and gated behind `DC_FLAG_VOICE` (off in `--minimal`)!** **Zero TTS or bi-directional voice** in either Desktop or iOS: no streaming sentence-chunked speech output, no Markdown-to-speech sanitizer, no VAD auto-send, no echo-cancelled barge-in interruption, no hands-free Live Voice conversation loop, no iOS `.playAndRecord` / Bluetooth AirPods duplex audio session. |

---

## 3. Detailed Architecture by Pillar

### Pillar 1: Full Auto Mode & Autonomous Operator Vault (`issue-1`)

#### 3.1.1 Global Full Auto Policy (Coax `#authority`)
- New pure Coax policy module: `coax/lib/agent/desktop/full-auto.coax` (`#authority`, zero surviving mutants):
  - `FULL_AUTO_ON` (default `true`): master switch persisted in `FlawedSettings` / `UserDefaults` (`fullAutoEnabled`, default `true`) and mirrored into `CoderPage.full_auto`.
  - When `full_auto == true`:
    - `cp_permit_should_gate(s)` unconditionally returns `false`.
    - `cp_permit_take_line`, `cp_permit_take_gated`, `cp_permit_take_plan`, and `cp_permit_take_secret` immediately emit an auto-allow answer (`allow = true`, first approve/allow-session option, `why = "full_auto"`) **without** opening a modal card or blocking the turn.
    - All worker invocations force non-interactive full-access flags:
      - `flawedworker` / `inky`: `profile = "danger-full-access"`, `ceiling = 3`, `gated = false`, `git_guard = false`, `sandbox = 0`.
      - `agyworker`: `mode = "auto"` (auto-approve workspace & shell tools).
      - External CLI subagents (Claude/Codex/Cursor): pass `--dangerously-skip-permissions` / `--full-auto` / equivalent non-interactive flags.
    - In `coax/lib/inky/tools.coax`: when `profile == "danger-full-access"`, path resolution allows normalized absolute paths outside `cwd` (removing workspace-jail refusals in Full Auto while keeping NUL-byte/syntax hygiene).
    - Question cards (`CP_PERMIT_KIND_QUESTION`): in Full Auto, workers are instructed via system prompt to pick the best sensible default and proceed without asking; if an external harness still emits an interactive question with options, a configurable `auto_answer_questions` toggle (default `true` in Full Auto) auto-selects the recommended/first option and logs a subtle inline transcript notice instead of stalling.

#### 3.1.2 Operator Vault & Autofill Profile
- New Coax schema & resolver: `coax/lib/agent/desktop/operator-vault.coax` (`#authority` matching/redaction/routing rules):
  - Local file `~/.openagents/vault/operator-profile.json` (mode `0600`, never committed, never printed raw to chat transcripts).
  - Structured sections:
    - **Tokens & API Keys**: arbitrary `KEY_NAME -> value` entries (plus fallback lookup in environment variables, `~/.openagents/secrets/`, and macOS Keychain).
    - **Identity & Forms**: `full_name`, `email`, `phone`, `company`, `role`, `website`, `github_handle`, `billing_address` (street, city, state, postal_code, country).
    - **Billing & Payment Info**: optional operator-entered card/bank/lightning/billing metadata (`billing_name`, `billing_email`, `bank_routing`, `bank_account_last4` / full vault ref, `card_holder`, `lightning_address`, custom `kv` pairs) stored strictly locally under `0600` / Keychain.
  - **Auto-Resolution on Secret/Form Requests**:
    - When `CP_PERMIT_KIND_SECRET` fires (or an agent invokes `vault_get(key)` / `vault_profile(category)`), the host automatically resolves the requested key/field from the Operator Vault -> Keychain -> env and supplies it straight to the requesting tool/worker without interrupting the user.
    - If a key is genuinely missing from the vault, only then does it fall back to prompting (and immediately saves the entry to the vault so it is never asked again).
  - **UI Surface**:
    - Composer status pill: clickable **Full Auto** toggle pill (green dot when ON) in Desktop `--minimal` and iOS.
    - **Settings -> Full Auto & Vault** page (`page/full_auto.coax`, promoted into `page-v1.coax`): toggle Full Auto, Auto-Answer Questions, Bypass Workspace Confinement, and edit/import Tokens, Identity/Forms, and Billing/Bank Autofill entries.

---

### Pillar 2: Always Show My Usage (`issue-2`)

#### 3.2.1 Promote `page/usage.coax` into `--minimal` (`page-v1.coax`)
- Remove the `usage.coax` stubs from `apps/openagents-desktop/page/v1-stubs.coax` and `#load "page/usage.coax"` directly in `apps/openagents-desktop/page-v1.coax`.
- Mark `CP_PG_USAGE` always allowed in `cp_set_page_ok_flags` (no `DC_FLAG_USAGE` gate).
- Remove `cp_flag(s, DC_FLAG_USAGE)` guards around `cp_usage_ring`, `cp_usage_popover`, Settings -> Usage, and per-chat token/cost readouts.

#### 3.2.2 Always-On Usage Strip (Desktop + iOS)
- Add a sleek, single-line **Always-On Usage Bar** pinned in the Desktop sidebar footer (above Settings) AND compact inline stats in the composer footer so usage is **always visible** regardless of whether you are on Dashboard, an empty chat, or an active streaming turn:
  - **Active Chat**: `ctx 42% (184K/1M)` + `$0.14` (live-updated on every streaming chunk).
  - **Today / Fleet Spend**: `Today 2.4M tok · $1.82` aggregated live from `~/.openagents/` session logs + active in-memory runs.
  - **Provider Quota Meter**: binding window bar + percentage + reset countdown for every configured provider (Flawed/Gemini, Antigravity, Claude, Codex, OpenRouter, Local Psionic tokens/sec) with color states (`Normal`, `Warn >=80%`, `Critical >=95%`).
  - Clicking the strip toggles the rich `cp_usage_popover` (all accounts, all windows, reset timers, Refresh button, Open Usage Settings).

#### 3.2.3 Live Multi-Provider Accounting
- Upgrade `apps/openagents-desktop/Window+Usage.swift`:
  - Track live Flawed/Gemini/Vertex/OpenRouter/Local Inkling token counts and microdollar spend directly from `CoderWindow` turn telemetry (written to `~/.openagents/coder/usage-daily.json` + pushed live into `page/usage.coax`).
  - Keep `UsageProber` polling `~/.openagents/coder/usage-*.txt` AND refreshing automatically on app launch, every 60s, and after every turn completion.
- Extend `openagents-environment` daemon wire (`GetUsage` / `UsageSnapshot` in `DescribeEnvironment`) so iOS displays the exact same live multi-provider quotas, daily spend, and active session context % in its Always-On Usage Bar + tappable Usage Dashboard sheet.

---

### Pillar 3: Universal Introspection (`issue-3`)

#### 3.3.1 Unified Introspection Data Model (`coax/lib/agent/desktop/introspect.coax`)
A single Coax data model (`IntrospectSession`, `IntrospectMachine`, `IntrospectSnapshot`) unifying every session across machines and harnesses:

| Kind (`IS_KIND_*`) | Source | Discovery Mechanism |
| --- | --- | --- |
| `IS_KIND_DESKTOP` | Local OpenAgents Desktop chats | `CoderStore` + live `runs[]` (existing) |
| `IS_KIND_TUI` | Local TUI & Terminal sessions (`coder`, `inky`, `flawed`, `tmux` panes, active PTYs) | Process table scan (`ps` for `coder`/`inky`/`claude`/`codex`/`agy`) + `tmux list-panes -a -F ...` + `~/.openagents/tui/sessions/*.json` heartbeat registry |
| `IS_KIND_HARNESS` | Other local harnesses (Claude Code, Codex CLI, Antigravity IDE/CLI, Cursor Agent, OpenCode) | Live process detection + recent session directories (`~/.claude/projects/`, `~/.codex/sessions/`, `~/.gemini/antigravity/conversations/`) scoped safely via explicit known paths (no global filesystem walks!) |
| `IS_KIND_REMOTE` | Sessions on other machines (Tailscale peers + SSH hosts) | Tailscale `coder-host` / `openagents-environment` daemon RPC (`ListIntrospection`) over NIP-44 port `47109` + SSH host discovery from `~/.ssh/config` (`environments.coax`, promoted into `page-v1.coax`) |

Each `IntrospectSession` carries:
- `id` (canonical URI, e.g. `local:chat:<id>`, `local:tmux:<session>:<win>.<pane>`, `local:harness:claude:<id>`, `remote:<machine_id>:<inner_id>`)
- `kind`, `harness` (`flawed`, `inkling`, `agy`, `claude`, `codex`, `cursor`, `tmux`, `tui`), `machine_id`, `machine_label`
- `title`, `cwd`, `branch`, `status` (`running`, `idle`, `waiting`, `error`), `updated_ms`, `pid`, `preview` (latest output line)
- `commandable` (bool — always `true` for Desktop, tmux/PTY TUI, and remote daemon sessions; `true` via CLI resume/inject for Claude/Codex/Agy)

#### 3.3.2 Sidebar Sections UI (`page/introspect.coax` + `page/layout/sidebar.coax`)
- Promote `page/environments.coax` into `page-v1.coax` (removing its stubs from `v1-stubs.coax`).
- Add collapsible, count-badged **Introspection Sections** in the Desktop sidebar (and mirrored 1:1 in iOS `gpu-page.coax`):
  - **Working** — currently running sessions across all local & remote sources
  - **Chats** — local OpenAgents Desktop sessions (Pinned + Recents)
  - **TUI & Terminal** — live `tmux` panes and TUI agent sessions (click to view live scrollback & send keystrokes/commands directly from composer!)
  - **Other Harnesses** — live & recent Claude Code / Codex / Antigravity / Cursor sessions on this machine (click to inspect live transcript & send follow-up prompt!)
  - **Machines** — each discovered Tailscale/SSH peer with green/amber/red status dot, active session count, and expandable child sessions on that machine
- Live background refresh every 3s (local processes/tmux) and 10s (remote peers), with zero UI jank and ShiftGuard click protection.

---

### Pillar 4: Talk To & Command Any Session Without Permission Boundaries (`issue-4`)

#### 3.4.1 Always-On Unrestricted Session Control Bus
- Un-gate the local HTTP control server (`ForemanOperator.swift` / `Window+ForemanAPI.swift`) from `DC_FLAG_FOREMAN` so it runs **always** in `--minimal`.
- Write connection metadata to `~/.openagents/session-bus.json` (mode `0600`) in addition to the legacy `foreman-control.json` path.
- **Zero Permission Boundaries**:
  - Default bus mode is `unbounded` (no `observe`/`managed` tier gates, no `403` on reading/sending/steering/stopping user chats, no `operator_tier_raise` lock).
  - Inject `OA_SESSION_BUS_URL`, `OA_SESSION_BUS_TOKEN`, and `OA_CURRENT_SESSION_ID` into **every** spawned worker environment (`flawedworker`, `inky`, `agyworker`, sub-agents, terminal tabs, and TUI sessions).
- **Unified Multi-Target Endpoints (`/v1/sessions/*`)**:
  - `GET /v1/sessions` — returns the complete `IntrospectSnapshot` (Desktop chats + TUI/tmux + other local harnesses + remote machine sessions).
  - `POST /v1/sessions/read` — `{session_id, tail?, include_tools?}` reads live transcript for Desktop/harness sessions, `tmux capture-pane -p -S -N` for TUI/tmux sessions, or proxies to a remote machine's daemon over Tailscale/SSH.
  - `POST /v1/sessions/send` — `{session_id, prompt, mode? ("send"|"steer"|"keys"), submit?}`:
    - Desktop chat -> directly prompts or steers the chat (cancelling active turn if `mode == "steer"`).
    - TUI / `tmux` pane -> sends text + Enter via `tmux send-keys` (or raw keys when `mode == "keys"`).
    - External harness (Claude Code / Codex / Antigravity) -> resumes/injects prompt via harness CLI (`claude --resume <id> -p ...`, `codex exec resume ...`, `agy`) or attached PTY.
    - Remote machine session -> transparently forwards over Tailscale NIP-44 RPC (`SendSessionCommand`) or SSH to the remote machine's bus!
  - `POST /v1/sessions/spawn` — `{machine?, harness?, cwd?, prompt, title?, full_auto?}` creates and starts a new session on local Desktop, local TUI/tmux, or any remote machine.
  - `POST /v1/sessions/stop` — `{session_id}` interrupts/stops any session (`Ctrl-C` for TUI/tmux, turn cancel for Desktop, remote forward for peers).
  - `POST /v1/sessions/wait` — `{session_ids, mode?, timeout_seconds?}` blocks until target session(s) complete or emit new output.

#### 3.4.2 Native Agent Tools for Every Session
- Add 5 first-class tools to `coax/lib/inky/tools.coax`, `coax/lib/inky/prompt.coax`, `coax/lib/inky/run.coax`, and `coder/tui/flawedworker.coax`:
  1. `session_list` — inspect all running/recent sessions across Desktop, TUI, other harnesses, and remote machines
  2. `session_read` — read live transcript or terminal scrollback of any session
  3. `session_send` — talk to, steer, or send commands/keystrokes to any session without permission checks
  4. `session_spawn` — launch a new session on this machine or a remote machine
  5. `session_stop` — stop/interrupt any session
- Add `openagents session <list|read|send|spawn|stop>` subcommand to `apps/openagents-cli/` so external harnesses (Claude Code, Codex, shell scripts) can also command any session immediately.

#### 3.4.3 Direct UI Command Surface
- Clicking **any** introspected session (Desktop, TUI/tmux, external harness, remote machine) in the sidebar opens its live view in the main pane with the **same unified composer** at the bottom — typing and pressing Enter immediately talks to / commands that session!
- Typing `@session:<name>` in any chat composer auto-completes all live sessions across machines/harnesses and routes or cross-references seamlessly.

---

### Pillar 5: Mobile App Full Desktop Parity (`issue-5`)

#### 3.5.1 Remove the Antigravity-Only Gate
- Update `coax/lib/agent/environment/phone-agent-policy.coax` (`#authority`) to admit **all** OpenAgents harness/model families (`flawed:gemini-4-neon`, `flawed:gemini-4-carbon`, `flawed:gemini-4-hydrogen`, `flawed:gemini-4-argon`, `flawed:inkling-2-mini`, `inkling-2-mini`, `antigravity`, `claude:*`, `codex:*`) and keep strict validation on field lengths, effort enums, and review/full-auto modes.
- Update `apps/openagents-ios/check.sh` and `Tests/MobileCoreTests.swift` so multi-harness OpenAgents parity is verified cleanly (keeping zero JS/WebKit bloat!).

#### 3.5.2 Desktop-to-iOS 1:1 Feature Parity
Update `apps/shared/antigravity/` (protocol + store), `coder/tui/host-environment.coax` (Mac daemon), and `apps/openagents-ios/` (`gpu-page.coax`, `GpuRootViewController.swift`, Swift sheets):
1. **Model & Harness Parity**: default to `Flawed · Gemini 4 Neon` (matching Desktop commit `27ec13f135`), with full model picker across Flawed Neon/Carbon/Hydrogen/Argon/Inkling, Local Inkling, Antigravity, Claude, and Codex.
2. **Full Auto Toggle & Autofill Sync**: prominent Full Auto chip in composer + Settings -> Full Auto (default ON); suppresses approval nags when ON.
3. **Always-On Usage HUD + Usage Sheet**: persistent usage bar (active context % + daily spend + binding provider quota %) rendered in `gpu-page.coax`, backed by live daemon `GetUsage` RPC; tap to open full multi-provider Usage Dashboard sheet.
4. **Universal Introspection Sidebar**: mirror Desktop's exact collapsible sidebar sections in `gpu-page.coax`: **Working**, **Chats**, **TUI & Terminal**, **Other Harnesses**, and **Machines** (multi-computer sessions aggregated across all paired Tailscale Macs/Linux boxes!).
5. **Borderless Session Control**: tapping any TUI, external harness, or remote machine session on iOS opens its live transcript/scrollback and lets the user talk to / steer / stop / spawn directly from the iOS composer via Tailscale NIP-44 `SessionCommand` RPC.
6. **Full Settings Parity**: unified Settings sheet (General, Full Auto & Vault, Models & Providers, Usage, Voice & Bi-Directional Audio, Machines & Tailscale).
7. **Bi-Directional Voice**: full parity with Pillar 6 (see below).

---

### Pillar 6: Dictation and Bi-Directional Voice (`issue-6`)

#### 3.6.1 Promote Native Dictation into `--minimal` (`page-v1.coax`)
- Remove `voice.coax` stubs from `apps/openagents-desktop/page/v1-stubs.coax` and `#load "page/voice.coax"` directly in `apps/openagents-desktop/page-v1.coax`.
- Remove `DC_FLAG_VOICE` gating on composer mic button, recording pill, `Opt+Space` hotkey, and Settings -> Voice (`CP_PG_VOICE`).

#### 3.6.2 Coax Bi-Directional Voice State Machine (`apps/openagents-desktop/page/voice.coax` + shared `coax/lib/agent/desktop/voice-duplex.coax`)
New pure Coax logic (`voice-duplex.coax`, tested + mutated) shared across Desktop and iOS:
- **Duplex Conversation States**:
  - `CP_VOICE_DUPLEX_OFF` (normal text / manual dictation)
  - `CP_VOICE_DUPLEX_LISTENING` (mic live, streaming partial STT, VAD tracking speech/silence)
  - `CP_VOICE_DUPLEX_THINKING` (turn auto-submitted after VAD silence timeout, waiting for assistant first sentence)
  - `CP_VOICE_DUPLEX_SPEAKING` (streaming TTS playback active, mic monitoring for barge-in)
- **Streaming Sentence Chunker (`vd_chunk_sentences`)**:
  - Feeds assistant markdown delta tokens incrementally; extracts speakable complete sentences on `.` / `!` / `?` / paragraph breaks while stripping fenced code blocks (replaced with `"Code block omitted."`), inline backticks, URLs, Markdown headers/bullets, and tool JSON.
  - Emits sentences to TTS immediately so Time-To-First-Audio is <1s after first sentence finishes!
- **VAD & Barge-In Policy (`vd_vad_step`)**:
  - Tracks `speech_detected`, `silence_ms`, `rms`, and `barge_in_frames`:
    - In `LISTENING`: once `speech_detected` is true and `rms < vad_floor` for `silence_commit_ms` (default 1200ms), triggers auto-commit + submit (`CP_ASK_VOICE_AUTOSEND`).
    - In `SPEAKING`: if `rms > barge_in_threshold` for `>= 3` consecutive frames (~150ms), triggers immediate barge-in (`CP_ASK_VOICE_BARGE_IN`): halts TTS playback, clears queued sentences, and returns to `LISTENING`.

#### 3.6.3 Native Audio & TTS Hosts (Desktop macOS + iOS)
- **Desktop (`apps/openagents-desktop/Window+Voice.swift`)**:
  - Add `CoderSpeechSynthesizer` wrapping `AVSpeechSynthesizer` (with voice selection, speech rate, pitch, sentence queue, instant `stopSpeaking(at: .immediate)` on barge-in) plus optional local neural TTS binary (`~/.openagents/bin/kokoro-tts` or `piper` or macOS `say` enhanced voices).
  - Enable `AVAudioEngine` voice-processing I/O (`inputNode.setVoiceProcessingEnabled(true)`) during Live Voice duplex mode for hardware acoustic echo cancellation (AEC) so the mic doesn't trigger on the Mac's own speaker output.
  - Wire composer Live Voice button (waveform/headset icon next to Mic) + `Opt+Shift+Space` hotkey + per-message "Speak" button + Settings -> Voice controls (Auto-Speak Replies, Duplex Voice Mode, TTS Voice, Speech Rate, VAD Silence Timeout, Barge-In sensitivity).
- **iOS (`apps/openagents-ios/Sources/VoiceDuplexController.swift` + `gpu-page.coax`)**:
  - Upgrade `VoiceDictationController` with full bi-directional voice: `AVAudioSession` configured as `.playAndRecord`, mode `.voiceChat` (hardware AEC!), options `[.defaultToSpeaker, .allowBluetooth, .allowBluetoothA2DP]`.
  - Streaming sentence-chunked `AVSpeechSynthesizer` fed directly from `AntigravityStore` streaming assistant deltas, VAD auto-send, and barge-in interruption.
  - Live Voice pill & duplex button rendered in `gpu-page.coax` matching Desktop 1:1.

---

## 4. Implementation Sequence & Verification Plan

Each pillar maps to one detailed GitHub issue and is implemented in its own isolated git worktree, verified end-to-end against the real build/runtime, merged to `origin/main`, and closed with verification evidence before moving to the next:

1. **Issue A (Pillar 1)**: Full Auto Mode (zero permission nags) + Autonomous Operator Vault & Autofill Profile.
2. **Issue B (Pillar 2)**: Always Show My Usage (promote `usage.coax` into `--minimal`, Always-On Usage Strip, live multi-provider accounting, iOS wire & HUD).
3. **Issue C (Pillar 6)**: Dictation & Bi-Directional Voice (promote `voice.coax` into `--minimal`, Coax duplex VAD/chunker, streaming TTS + barge-in on Desktop & iOS).
4. **Issue D (Pillar 3)**: Universal Introspection (promote `environments.coax` into `--minimal`, local TUI/tmux + other local harnesses + remote machine sessions in sidebar + click-to-inspect).
5. **Issue E (Pillar 4)**: Talk To & Command Any Session Without Permission Boundaries (always-on unrestricted Session Control Bus, 5 native agent tools, UI direct command, CLI `openagents session`).
6. **Issue F (Pillar 5)**: Mobile App Full Desktop Parity (lift Antigravity-only policy gate, bring Flawed/multi-harness models, Full Auto, Usage, Introspection, Borderless Session Control, Settings, and Bi-Directional Voice to `apps/openagents-ios/`).

*(Note: Pillar 6 is sequenced ahead of Pillar 5 so iOS parity can wire both dictation & bi-directional voice cleanly in one pass, and Pillars 1–4 land on Desktop + daemon first so iOS connects to live endpoints immediately!)*

---

## 5. Acceptance Criteria (Global)

- Zero new feature flags hiding any of the 6 pillars: every feature works by default in `--minimal` (`apps/openagents-desktop/build.sh` & `install.sh`).
- Every new `#authority` Coax policy module passes `coax/bin/coax test` and `coax/bin/coax mutate --root-only` with zero surviving mutants.
- `apps/openagents-desktop/page-v1.coax` tests, `apps/openagents-desktop/build.sh` (`--minimal`), `apps/openagents-desktop/v1-bundle-check.coax`, and `apps/openagents-ios/build.sh` + `apps/openagents-ios/check.sh` all pass.
- No secret/token/bank value from the Operator Vault is ever printed to chat transcripts, logs, or telemetry.
- Installed `/Applications/OpenAgents.app` verified via `Info.plist` (`OACommit`, `OAProfile`) and real live probes.
