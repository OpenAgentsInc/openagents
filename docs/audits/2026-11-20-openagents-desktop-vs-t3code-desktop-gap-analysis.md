# OpenAgents Desktop vs. t3code Desktop — Comprehensive Gap Analysis

> [!NOTE]
> **Status (2026-11-22): Historical 2026-11-20 snapshot — roadmap superseded.**
> Within 24 hours of this audit, agents built nearly all 22 issues of the `#13890` parity epic, which triggered the 2026-11-21 bundle reckoning ([`2026-11-21-openagents-desktop-inkling-first-release-audit.md`](2026-11-21-openagents-desktop-inkling-first-release-audit.md), [`docs/desktop/openagents-v1-spec.md`](../desktop/openagents-v1-spec.md)). Those features now live behind the 29-flag Coax registry (`coax/lib/agent/desktop/flags.coax`, default `false`) and are stripped from `--minimal` release builds (`page-v1.coax`, `v1-bundle-policy.coax`). Since this snapshot, OpenAgents Desktop also cut local Inkling 2 Mini over from Microcoder/`llama-server` to **Inky** (`coax/lib/inky/`, `#13982`) + native **Psionic Metal 4 / CUDA** (`150+ tok/s` Metal 4, `75+ tok/s` RTX 4080) and promoted Antigravity 1.5+ + native `Settings -> Providers` into `--minimal` core. Meanwhile `t3code` advanced +321 commits to `18850a5eeb` (durable workflows, MCP Events webhooks, multi-repo projects, 8th forge `tanstack-source`, 17 MCP toolkits, 110+11 SQLite migrations, 28 settings routes, environment-instead-of-branch sidebar rows).
>
> **Read instead:** [`docs/t3/README.md`](../t3/README.md) (canonical index), [`docs/t3/2026-11-22-t3code-borrowing-theory.md`](../t3/2026-11-22-t3code-borrowing-theory.md) (supersedes the P0–P3 roadmap in §13), [`docs/t3/2026-11-22-desktop-sidebar-gap-analysis.md`](../t3/2026-11-22-desktop-sidebar-gap-analysis.md) (updated sidebar status), and [`docs/t3/reference.md`](../t3/reference.md) (`18850a5eeb` reference).

- **Date:** 2026-11-20
- **OpenAgents snapshot:** `90e3f9361a` (`origin/main`, including Antigravity parity `#13831`–`#13839`)
- **t3code snapshot:** `b568fcbaa6` (`origin/main`, `v0.1.12-nightly` — pulled immediately prior to this audit)
- **Scope:** Direct desktop-to-desktop comparison of **OpenAgents Desktop** (`apps/openagents-desktop/`, `coax/lib/agent/desktop/`, `apps/shared/host/`) against **t3code Desktop** (`apps/desktop/`, `apps/web/`, and desktop-facing `apps/server/` + `native/` helpers).
- **Companion audits:**
  - Whole-codebase audit: `docs/audits/2026-11-20-t3code-comprehensive-gap-analysis.md`
  - Antigravity v0.1.11 audit: `docs/audits/2026-11-20-t3code-antigravity-v0-1-11-audit.md`
  - Desktop consolidation audit: `docs/audits/2026-11-19-openagents-desktop-consolidation-audit.md`

---

## 1. Executive Summary

OpenAgents Desktop and t3code Desktop are built on opposite architectural philosophies that produce sharply different strengths and blind spots:

- **OpenAgents Desktop** is a **native GPU-rendered desktop application** built in **Coax + Metal (macOS) / Vulkan (Linux & Windows)** with a thin platform shell (AppKit/Swift on macOS, Wayland/C on Linux, Win32/C on Windows) and a **resident local daemon (`coder-host`)** over Unix domain sockets. It has no Chromium, no Node.js runtime, 120 Hz ProMotion rendering, CoW worktree hydration, autonomous `/goal` loops with verification audits, per-chat USD budgets, an embedded iOS Simulator pane (`oasim`), a 3D Metal PLE Workbench (`Gym`), a programmable multi-chat orchestrator (`Foreman`), local GGUF inference (`llama-server`), and zero-config Tailscale/Bonjour iPhone pairing.
- **t3code Desktop** is an **Electron + React 19 + TanStack Router + Effect-TS** application backed by a **Node.js + SQLite (Orchestration V2) + WebSocket RPC server** (`apps/server`) plus targeted native Rust/Swift/C helpers (`apple-speech`, `browser-secret`, `libghostty-vt`, `resource-monitor`, Wayland/KDE/Hyprland capture). It trades native footprint for breadth of developer-workflow polish: **11 provider drivers**, **integrated Ghostty VT WASM terminal**, **full Git actions + multi-forge PR/issue workflows**, **per-turn Git checkpoints & rewind**, **embedded Chromium Preview Browser with DOM element picker**, **global SnapShot screen capture**, **native + cloud voice dictation**, **full-text transcript search**, **multi-project/multi-environment sidebar**, **customizable VS Code-style keybindings & themes**, **provider usage/quota dashboards with multi-account pooling**, **SSH + WSL2 + Cloud Relay remote environments**, and **Windows/Linux/macOS feature parity**.

### Scorecard Across 18 Desktop Domains

| # | Domain | OpenAgents Desktop | t3code Desktop | Gap Direction |
| --- | --- | --- | --- | --- |
| 1 | Rendering, Footprint & Native Polish | Native Metal/Vulkan 120 Hz, zero Chromium, frosted glass, VoiceOver | Electron/Chromium, zoom-aware titlebar, multi-display bounds | **OpenAgents ahead** (perf/footprint) |
| 2 | Cross-Platform Parity (macOS / Linux / Windows) | macOS full; Linux & Windows missing daemon, image previews, Antigravity, updater, signing/installers | Near-full parity across macOS `.dmg`, Linux `AppImage`/AUR, Windows `NSIS` + WSL2 + Smart App Control signing | **Large gap (t3code ahead)** |
| 3 | Process & Daemon Resilience | Resident `coder-host` daemon outlives app quit; live reattach; 12 concurrent workers | `DesktopBackendPool` + SQLite Orchestration V2 + single-owner lock + keep-awake | **Roughly even** (different strengths) |
| 4 | Workspace, Projects & Sidebar | Flat Pinned + Recents (max 50), no folders/grouping, no archive UI, title-only search | Multi-environment project groups, archiving, snoozing, bulk select, full-text message search, undo notices | **Large gap (t3code ahead)** |
| 5 | Composer & Input Ergonomics | Custom GPU editor, `@` mentions, 4 slash commands, follow-up queue/steer/interrupt, `/goal`, budgets | Tiptap rich editor, code blocks, prompt stash/history, multi-model fan-out, skill/slash search, voice dictation | **Medium–large gap (t3code ahead on richness; OpenAgents ahead on `/goal` & budgets)** |
| 6 | Transcript, Tools & Interactive Flows | Virtualized Markdown, activity groups, `permit.coax` interactive permissions/questions, subagent pane, `OA-SHOW-LIVE` 3D | Rich Markdown (Mermaid, LaTeX, tables, HTML/MCP Apps), approvals, user input, secret requests, plan mode, minimap, in-thread find, side/linked threads | **Medium gap (t3code ahead on breadth)** |
| 7 | Per-Turn Checkpoints & Rewind | None (only `/fork` and edit history) | Full Git-backed per-turn checkpoints (including untracked files) + one-click revert | **Large gap (t3code ahead)** |
| 8 | Diff Viewer & Code Review | Edit-tool hunks only (`review.coax`); no working-tree `git diff`, no inline review comments | Full `git diff` + checkpoint diff, file tree, hide-test-files, inline review comments fed back to composer | **Large gap (t3code ahead)** |
| 9 | File Explorer & Inline Editor | Single clicked-file pane (16 KB edit cap, no syntax highlight, no tree, 3D `.ple-scene.json` viewer) | Full file explorer tree + read-only rich viewers (PDF, Office/OpenXML, Jupyter notebooks, Mermaid, CSV/TSV, fonts, audio, binary) | **Large gap (t3code ahead on browsing/previews; OpenAgents has in-place edit)** |
| 10 | Integrated Terminal | **None** | Multi-tab Ghostty VT WASM terminal (`node-pty`, 5,000-line/8 MB history, send-selection-to-composer, port scanner, resource telemetry) | **Critical gap (t3code ahead)** |
| 11 | Preview, Simulator & Visual Capture | Native iOS Simulator pane (`oasim`, live 15 fps, touch/pinch/keys, agent permission card) + 3D PLE Gym | Embedded Chromium Preview Browser (DOM picker, annotations, cookie import, passkeys, screen recording/PiP) + global SnapShot + iOS/Android/Expo device hosts | **Mixed** (OpenAgents ahead on native iOS sim & 3D Gym; t3code ahead on web preview & SnapShot) |
| 12 | Git, PRs & Issue Trackers | Auto worktrees + CoW hydration + branch chip + `gitguard` hook; zero Git/PR/issue UI actions | Full Git Actions (commit/push/PR/pull, AI commit/PR text), 7 source-control forges, PR list + guided AI review tours, Linear multi-team issues | **Critical gap (t3code ahead)** |
| 13 | Providers, Accounts & Auth | 5 providers (Gemini Vertex/API, OpenRouter, Claude sub-agents, Antigravity, local Inkling 2 Mini); OpenAgents Passkey/Nostr/OTP | 11 built-in drivers + ACP Registry, multi-instance accounts, OAuth flows, 1Password secret resolver, Clerk auth | **Large gap (t3code ahead on breadth)** |
| 14 | Usage, Quota Limits & Spend | Per-chat USD spend vs budget + context fullness ring; no Desktop Settings → Usage panel | Full Usage page, live quota probes (Claude/Codex/Antigravity), multi-account pooling, automatic limit-recovery/reset-credit worker, LiteLLM pricing + overrides | **Large gap (t3code ahead)** |
| 15 | MCP & Skills Management | Read-only `~/.claude/skills` scan injected into system note; no MCP UI | Built-in MCP HTTP server (14 toolkits), MCP OAuth sign-in UI, MCP Apps iframe rendering, skill search & inline chips | **Large gap (t3code ahead)** |
| 16 | Settings, Themes, Keybindings & i18n | 5 settings pages, fixed monochrome Zeron skin, fixed shortcuts, English only | 26 settings routes, built-in + VS Code/OpenVSX themes + custom CSS, VS Code `when`-clause keybinding editor, 10-locale i18n | **Large gap (t3code ahead)** |
| 17 | Notifications & Audio Feedback | Account-security APNs only; no turn-completion notifications or sounds | Configurable OS notifications + Web Audio completion/input-needed sounds + Dock/taskbar unread badge counts | **Medium gap (t3code ahead)** |
| 18 | Remote Environments & Mobile Sync | Zero-config Tailscale/Bonjour iPhone pairing; no SSH/remote/cloud workspace UI | SSH environments (`~/.ssh/config` + password prompts + remote bootstrap), WSL2, LAN/Tailscale Serve, Cloud Relay, Peer Federation, Guest thread sharing | **Large gap (t3code ahead on remote dev; OpenAgents ahead on zero-config mobile pairing)** |

---

## 2. Platform Shell, Rendering, Packaging & Cross-Platform Parity

### 2.1 Rendering & Window Shell
| Capability | OpenAgents Desktop | t3code Desktop | Notes |
| --- | --- | --- | --- |
| UI stack & renderer | Coax UI (`page/*.coax`, `components/zeron/`) → Metal (`CAMetalLayer`, `components.metal`, `composition.metal`) on macOS; Vulkan (`vulkan-ui.c`, `components.spv`) on Linux/Windows | Electron + React 19 + TanStack Router + Tailwind + WebGL/Canvas (Ghostty WASM) | OpenAgents avoids Chromium's RAM/CPU overhead and renders natively at 120 Hz ProMotion (`CADisplayLink`). |
| Multi-window support | **Absent** — single `CoderWindow` (`AppDelegate.swift`) | Single primary window + auxiliary permission/PiP windows (`MacSettingsWindow.ts`, `preview-pip-preload.ts`) | Neither supports true multi-window workspace windows yet, though t3code supports pop-out PiP and side/linked panes. |
| Window bounds persistence | Basic macOS frame save | Multi-display validated bounds persistence (debounced 500 ms, 1 px edge tolerance, `normalBoundsOnWindowDisplay`) | OpenAgents should validate display bounds on monitor disconnect. |
| Custom titlebar & traffic lights | Transparent inset titlebar (`Window+TitleBar.swift`), custom traffic lights, double-click zoom/minimize | 40 px / 52 px custom titlebar overlay, dynamic macOS traffic-light repositioning across zoom levels (`syncMacosWindowButtons`) | Both strong on macOS. |
| Quit protection | Closing window hides to `NSStatusBar` tray; `Cmd+Q` quits immediately | `QuitHold.ts` (`makeQuitShortcutHandler`) hold-to-quit or double-press `Cmd+Q` protection with overlay hint | Useful safety feature in t3code when agents are running (though OpenAgents' macOS daemon keeps runs alive after quit). |
| System tray / Dock badge | `NSStatusBar` menu bar item ("Show OpenAgents" / "Quit"); no Dock badge count | No tray icon; Dock/taskbar badge counts (`app.setBadgeCount`, Windows `setOverlayIcon` canvas PNG), auto-cleared on focus | OpenAgents lacks unread/attention Dock & taskbar badge counts. |
| Single-instance & CLI activation | macOS app delegate handles file/URL open; no `openagents-desktop` CLI control socket | Single-instance lock + Unix socket / named pipe control server (`DesktopAppActivation.ts`) + in-app `t3` CLI shim installer (`DesktopCliCommand.ts`) | t3code lets users install the `t3` CLI to `PATH` from inside the app and open folders/threads directly in the running window. |
| Deep links & file associations | `.coax` owner, `public.source-code` alternate, universal links (`openagents.com/login#...`, `/projects/...#invite=...`) | Custom protocol `t3code://app/` + Linux URL handler (`DesktopLinuxUrlHandler.ts`) + OAuth callback routing (`CodexAuthCallback.ts`, `providerAuth.ts`) + web link queue | OpenAgents lacks a custom `openagents://` protocol scheme for local OAuth callbacks and external thread deep-linking. |
| IME, clipboard, drag-and-drop | macOS: full `NSTextInputClient` IME, paste text/files/images, `"Drop to attach"` overlay. Linux/Win: IME + clipboard + drag-drop (no Win OLE drag-over overlay) | Chromium native IME + clipboard + `DesktopPrimaryPaste.ts` (Linux middle-click suppression) + native file drag out (`ipc/methods/fileDrag.ts`) + context-menu image copy | OpenAgents macOS is strong; Windows lacks OLE drag-over feedback and Linux/Win lack image preview decoding (`CD_FLAT_PICTURES 0`). |
| Accessibility | macOS VoiceOver `NSAccessibilityElement` tree (`apple/ui-host.m`); Linux AT-SPI SDK foundation (`linux/access.c`, unfinished); Windows none | Chromium/ARIA accessibility across all 3 platforms + `@crowecawcaw/xa11y` for SnapShot | OpenAgents needs full Linux AT-SPI consumer wiring and Windows UI Automation. |
| Localization (i18n) | English only | `gt-i18n` + `gt-react` across 10 locales (both native menus and React UI) | Gap if targeting international users. |

### 2.2 Auto-Updater, Signing & Distribution
| Capability | OpenAgents Desktop | t3code Desktop |
| --- | --- | --- |
| macOS packaging & signing | `.app` + `sign.sh` (Developer ID + hardened runtime + `notarytool` + `stapler`) + `install.sh` local build | `.dmg` with custom `latest`/`nightly` SVG backgrounds, notarized, restricted entitlements verification (`sign-macos.ts`) |
| macOS auto-updater | Ed25519-signed `openagents.desktop.update.v1` manifest (`Updater.swift`, `update.coax`), SHA-256 + `codesign --verify --deep --strict`, atomic swap + rollback, 15-min poll, local staged build detector | `electron-updater` Effect state machine (`DesktopUpdates.ts`), `stable`/`nightly` channel switcher with guarded downgrade, failed-install marker detection, enterprise managed-policy lockouts, SSH tunnel teardown before `quitAndInstall` |
| Linux packaging & updater | Bare binary + `.desktop` file; no AppImage/deb/rpm/AUR; no in-app auto-updater | `AppImage` with glibc compatibility checks (`linux-glibc.ts`), AppStream metainfo XML, AUR packaging, auto-updater, early scale-factor persistence across AppImage updates |
| Windows packaging & signing | Cross-compiled `OpenAgents.exe` + `mcworker.exe` zip (`windows/build.sh`); **unsigned** (SmartScreen warning), **no installer**, **no auto-updater**, **no Direct3D 12 fallback** if `vulkan-1.dll` missing | `NSIS` installer (`installer.nsh`), Smart App Control `.node`/`.dll` signing (`windows-jsign.ts`), bundled `server.asar`, auto-updater |

### 2.3 Linux & Windows Feature Parity (OpenAgents' Largest Platform Gap)
As documented in `apps/openagents-desktop/windows/README.md` and `host-main.c`, OpenAgents Desktop on Linux and Windows currently stubs out (`not_here()`) several core macOS capabilities:
- **No resident daemon** (replies die when the app quits).
- **No Antigravity, Claude sub-agents, Foreman, or Gym** (Microcoder `mcworker` only).
- **No image attachment thumbnails** (`cd_decode_image` is `#ifdef __APPLE__` only).
- **No pinned-chat drag reordering**, **no in-app updater download**, and **no Windows DPI change glyph reload without restart**.

t3code Desktop maintains near-complete parity across macOS, Linux, and Windows (plus first-class **WSL2 backend** support on Windows).

---

## 3. Process, Daemon & Session Execution Architecture

| Dimension | OpenAgents Desktop | t3code Desktop |
| --- | --- | --- |
| Backend process model | Resident `coder-host` daemon (`~/.openagents/coder/host/service.sock`, `openagents.environment.daemon.v1alpha1`) + per-turn worker processes (`mcworker`, `claudeworker`, `agyworker`, `llama-server`, `oasim`) | `DesktopBackendPool` spawns Node.js `apps/server` (scans port `3773+`), single-owner lock (`DesktopHomeOwner`), bootstrap secrets over `fd3`/`stdin`, up to 3-min startup poll for DB migrations |
| Persistence across app quit | **Yes (macOS):** active runs continue in `coder-host` and reattach seamlessly on relaunch from saved `ID.out` byte offsets; up to 12 concurrent workers | Backend shuts down with Electron app on desktop (unless connected to an external/remote/WSL2 server); graceful shutdown with 10s stall detection (`DesktopLifecycle.ts`) |
| Storage engine | Local JSON files (`~/Library/Application Support/Coder/chats/ID.json`, `index.json`, `turns/RUN.json`) + NDJSON run logs (`~/.openagents/coder/host/runs/`) | SQLite event-sourced / projection store (`persistence/Migrations/`, 103+ migrations) with full-text search index (`messageSearchIndex.ts`) and crash-safe transactions |
| Background runs while switching chats | Headless Coax page (`cd_headless`) ticks, saves, pumps sub-agents, marks `unread` | Server-side `Orchestration V2` streams all threads independently over WebSocket RPC (`ThreadStream.ts`) |
| System sleep / keep-awake | None | `KeepAwake.ts` + `HostPowerMonitor.ts` + `BackgroundPolicy.ts` prevent system sleep while agent turns are active |
| Command sandboxing | Linux `bwrap` only; macOS runs `danger-full-access` | Provider-native sandbox policies + workspace leases + scoped RPC authorization capabilities (`orchestration:operate`, `source-control:write`, `terminal:operate`, `preview:operate`) |
| Provider CLI lifecycle | Expects `claude` / `agy` installed on `PATH` (managed `agy` layout primitives just landed in `#13839`, live downloader pending) | Managed runtime installation, version leases, pruning, "Update Everything" dialog (desktop + server + provider CLIs), `ProviderInstallationGate` |
| Scheduled / background tasks | `/goal` autonomous loop inside an open chat | Full `ScheduledTaskService.ts` (cron schedules + incoming webhooks with signature verification & deduplication) |

---

## 4. Workspace, Projects, Sidebar & Organization

| Feature | OpenAgents Desktop | t3code Desktop |
| --- | --- | --- |
| Project / repo grouping | **Absent** — flat list (`Pinned` + `Recents`, capped at 50 visible) | Multi-environment project folders (`SidebarProjectFolderHeader.tsx`), custom project favicons (`pickProjectFavicon`), drag-and-drop folders onto sidebar (`SidebarFolderDrop.tsx`) |
| Per-chat working folder | Single folder chip (`Cmd+O`), stored per chat | Project-level root + per-thread environment/worktree selector (`BranchToolbar.tsx`) |
| Git worktree mode | Automatic per-chat worktree (`~/.openagents/worktrees/REPO/CHAT`) with APFS/reflink/robocopy CoW hydration of `.coder/worktree.json` clone paths & shared `COAX_TOOL_CACHE`/`CARGO_TARGET_DIR`; opt-out only via hidden `defaults write` | Explicit in-UI toggle between **Local checkout** and **Worktree** (`BranchToolbarEnvModeSelector.tsx`), branch selector/creator (`BranchPicker.tsx`), setup script card (`WorktreeSetupCard.tsx`), worktree checkout cache & cleanup UI (`worktreeCleanup.ts`) |
| Archiving & snoozing | **Absent** — only permanent Delete (which sets daemon `archived=true` with no way to view/restore) | Archive threads + dedicated Settings → Archived view + **Snooze** (`CustomSnoozeDialog.tsx`) + undo toast on delete/archive (`SidebarThreadUndoNotice.tsx`) |
| Bulk thread actions | **Absent** | Multi-select threads (`threadSelectionStore.ts`) for bulk archive/delete/move |
| Sidebar search & full-text search | `Cmd+K` searches chat **titles**, commands, and settings only (full-text search exists only via CLI `foreman-ctl search-chats`) | Full-text message search across all threads (`ThreadSearch.ts` + `messageSearchIndex.ts`) + 3-mode Command Palette (`command`, `files`, `content`) + in-thread find bar (`ThreadFindBar.tsx`) |
| Pinned chat reordering | Drag-to-reorder on macOS (missing on Linux/Windows) | Full drag-and-drop organization across projects/threads |

---

## 5. Composer, Attachments, Voice & Prompt Ergonomics

| Feature | OpenAgents Desktop | t3code Desktop |
| --- | --- | --- |
| Editor capabilities | Custom GPU multi-line editor (`components/zeron/editor.coax`), undo/redo, IME | Tiptap rich editor (`ComposerPromptEditorTiptap.tsx`): syntax-aware code blocks with language picker, list continuation, drag-to-reorder `@` mention pills, prompt length validation |
| Prompt stash & history | Draft persisted per chat | `promptStashStore.ts` (`ComposerStashMenu.tsx` — save/pop multiple draft prompts) + prompt history navigation |
| `@` Mentions | Ranked fuzzy `@file` popover (`git ls-files`, max 4,000 files) | `@file`, `@model`, and skill/thread mentions with drag-reordering |
| Slash commands | 4 built-in commands: `/goal`, `/model`, `/new`, `/fork` | Searchable command menu (`ComposerCommandMenu.tsx`), provider slash commands, provider skill search (`providerSkillSearch.ts`) with inline skill chips (`SkillInlineText.tsx`) |
| Attachments | Drag-drop, clipboard paste, `+` file picker; image thumbnails (macOS only) | Drag-drop, clipboard paste, file picker, **terminal selection chips** (`ComposerPendingTerminalContexts.tsx`), **SnapShot region captures**, **embedded browser DOM annotations** |
| Model, effort & speed pickers | Model picker (`Cmd+/`), `Low`/`Medium`/`High` effort cycle, Antigravity review mode (`Auto`/`Default`/`Plan`) | Rich multi-provider `ProviderModelPicker.tsx` with sidebar, effort glyphs (`EffortGlyph.tsx`), speed/trait pickers (`TraitsPicker.tsx`, `TraitsSpeed.tsx`), runtime mode config, **Fresh Session** toggle, **cross-provider model handoff with context budgeting** (`modelSwitch.ts`) |
| Multi-model fan-out | **Absent** | `useMultipleModelSend.ts` — send the same prompt simultaneously to multiple models/providers to compare results |
| Follow-up queue, steer & interrupt | **Strong:** `Return` steers mid-turn, `Shift+Return` queues persistent follow-ups (`CHAT.queue.json`), edit/reorder/promote/pause/clear tray, `Esc` interrupts | `QueuedRunsControl.tsx` — queue, edit, reorder, and cancel pending runs |
| Autonomous goals & budgets | **Strong (OpenAgents unique):** `/goal` multi-turn autonomous loop with mandatory `[GOAL_COMPLETE]` verification audit turn; per-chat USD budget cap with live spend chip & override dialog | Not a first-class composer primitive (t3code relies on scheduled tasks & triage instead) |
| Voice input / dictation | **Absent** in Desktop (`coder/tui/dictate.coax` exists in TUI, not wired to Desktop) | **First-class:** `ComposerSpeechButton.tsx` + `VoiceInputPill.tsx` + `GlobalVoiceInputIndicator.tsx` + native macOS `AppleSpeech` (`native/apple-speech`) + local Whisper/Silero VAD (`silero.ts`) + CloudSpeech + filler-word stripping + custom dictionary (`customWords.ts`) |

---

## 6. Transcript, Tool Rendering, Interactive Permissions, Plan Mode & Checkpoints

| Feature | OpenAgents Desktop | t3code Desktop |
| --- | --- | --- |
| Markdown & rich rendering | Virtualized Coax Markdown (`CP_DOCS = 48`), code blocks + Copy, clickable file paths & URLs, inline images, `OA-SHOW` / `OA-SHOW-LIVE` 3D Metal cards | Full Markdown (`ChatMarkdown.tsx`): **Mermaid diagrams** (`MarkdownMermaidCodeBlock.tsx`), **LaTeX math** (`MarkdownMath.tsx`), interactive tables (`MarkdownTable.tsx`), **sandboxed HTML render frames** (`HtmlRenderFrame.tsx`), **MCP Apps iframes** (`McpAppFrame.tsx`), clickable file & thread links (`ThreadChipHoverCard.tsx`) |
| Activity & tool display | Collapsible activity groups with live timers, Thought rows (up to 64 KB), expandable tool input/output, live Agent To-Do checklist tray (`page/todos.coax`) | `TimelineWorkGroupRows.tsx`, `ShellCommandBlock.tsx`, `CommandOutputPanel.tsx`, `WorkLog.tsx`, `ChangedFilesTree.tsx`, `AssistantCitationChip.tsx` + inline text-selection citation comments (`AssistantSelectionToolbar.tsx`) |
| Interactive permissions & questions | **Recently landed (`#13831`, `page/permit.coax`)**: GPU-rendered permission & `interaction_*` question cards for Antigravity + iOS Simulator permission card (`simpane.coax`); Microcoder `"gated":true` commands still auto-approve | Full `TimelinePendingApprovalRow.tsx`, `ComposerPendingApprovalActions.tsx`, **`ChildThreadApprovals.tsx`** (approve subagent requests inline), `ComposerPendingUserInputPanel.tsx`, **`SecretRequestCard.tsx`** (1Password/secret vault prompts), `LinkRequestCard.tsx`, `McpSignInAction.tsx` |
| Plan Mode | Antigravity `Plan` mode toggle in Settings (no dedicated interactive plan-approval card) | First-class **`ProposedPlanCard.tsx`** with structured plan rendering, approve-and-implement, and edit/revise loop |
| Per-turn Checkpoints & Rewind | **Absent** (no git snapshot per turn, no revert-to-turn UI; only `/fork` conversation) | **First-class (`CheckpointStore.ts`, `useTimelineRewind.ts`)**: Git-backed per-turn checkpoints capturing tracked + untracked files (`checkpointUntrackedBlobs.ts`), diff any turn range, one-click rewind of both conversation and working tree |
| Transcript navigation | `Jump to latest` / `New reply` pill, fold long user messages (>5 lines), click reply for model attribution | **`TimelineMinimap.tsx`** (visual scrollbar minimap), **`ThreadFindBar.tsx`** (in-thread search with match jumping), `ThreadDetailsPanel.tsx` (PRs, issues, usage, automations, dev servers) |
| Sub-agents & side threads | Sub-agent cards (up to 12/turn, 6 parallel) opening right-hand resizable transcript pane with individual Stop + 100-quiet-call pause check-in | `SideThreadPane.tsx` & `LinkedThreadPane.tsx` (side-by-side linked/delegated/remote threads), `ChildThreadApprovals.tsx` |

---

## 7. Diff Viewer, File Explorer, Integrated Terminal & Preview Surfaces

### 7.1 Diff & Code Review
- **OpenAgents Desktop (`page/layout/review.coax`):** Shows a "Changes" tab in the right pane listing up to 64 files / 256 edits **captured from the agent's edit tool calls only** (changes made by shell commands or external editors do not appear). Collapsible unified hunks, no file tree, no inline review comments.
- **t3code Desktop (`DiffPanel.tsx`, `components/diffs/`):** Shows real **Git working-tree and per-turn checkpoint diffs** powered by a dedicated `DiffWorkerPoolProvider`, with `DiffFileTree.tsx`, `useHideTestFiles.ts`, syntax-highlighted `StyledDiffCodeView.tsx`, and **`AnnotatableCodeView.tsx` / `DiffCommentAnnotation.tsx`** allowing users to leave line-level review comments that attach directly back to the composer (`reviewCommentContext.ts`).

### 7.2 File Explorer & Document Previews
- **OpenAgents Desktop (`page/filepane.coax`):** No directory tree browser. Opens single clicked/Finder files in a right-hand pane with line numbers, **in-place editing up to 16 KB / 512 lines** (`Cmd+S`, dirty badge, unsaved prompt), read-only up to 256 KB, "Open in Default App", and native **3D Metal viewer** for `.ple-scene.json` / `.ifc`. No syntax highlighting in the editor, no multi-tab open files.
- **t3code Desktop (`components/files/`):** Full `FileExplorerPane.tsx` / `FileBrowserPanel.tsx` directory tree + specialized read-only viewers for **source code, large text files, PDFs (`PdfViewer.tsx`), Office/OpenXML docs (`OfficeViewer.tsx`), Jupyter notebooks (`NotebookPreview.tsx`), Mermaid (`MermaidViewer.tsx`), CSV/TSV tables (`DelimitedTablePreview.tsx`), fonts (`FontViewer.tsx`), audio (`AudioPreview.tsx`), and binary files**. Plus `OpenInPicker.tsx` to open local or remote (SSH/WSL) files in Cursor, VS Code, Zed, Xcode, Ghostty, etc.

### 7.3 Integrated Terminal
- **OpenAgents Desktop:** **Absent.** No embedded terminal or PTY pane exists.
- **t3code Desktop:** Full **Ghostty VT WASM** integrated terminal (`apps/web/src/terminal/ghostty/` + `apps/server/src/terminal/NodePtyAdapter.ts`) with multi-tab drawers (`ChatTerminalDrawers.tsx`), 5,000-line / 8 MB retained scrollback, bundled Symbols Nerd Font, clickable file/URL links (`terminal-links.ts`), **send selected terminal output directly to composer**, automatic **dev-server port scanning** (`PortScanner.ts`), and **native process-tree CPU/RAM telemetry** (`native/resource-monitor`).

### 7.4 Preview, Simulators & Visual Capture
- **OpenAgents Desktop:**
  - **Native iOS Simulator pane (`page/simpane.coax`, `Window+Simulator.swift`, `oasim`):** Live 15 fps Metal framebuffer, Xcode device chrome, touch/drag/pinch/scroll/keyboard forwarding, light/dark toggle, screenshots to `.openagents/shots/`, recording, and interactive per-chat agent permission card.
  - **3D Metal PLE Gym Workbench (`GymWorkbenchView.swift`, `SceneView.swift`):** Semantic 3D layers, camera presets, raycast picking, clash inspector, baseline diffing, timeline snapshots.
  - **No embedded web preview browser** and **no global OS screen capture tool**.
- **t3code Desktop:**
  - **Embedded Chromium Preview Browser (`apps/desktop/src/preview/`, `apps/web/src/browser/`):** Full `<webview>` browser with device viewport presets & resize handles, **DOM element picker & visual annotation overlay** (`PickPreload.ts`) with voice annotations fed straight into the agent prompt, **CDP relay** for agent browser automation, **cookie import** from Chrome/Firefox/Safari (`native/browser-secret`), WebAuthn passkeys, and screen recording + PiP.
  - **Global SnapShot (`apps/desktop/src/snapShot/`):** Global hotkey screen/region capture on macOS, Hyprland, KDE, Niri, and GNOME (via bundled GNOME Shell extension) with accessibility metadata (`@crowecawcaw/xa11y`) attached directly to chat.
  - **Multi-platform Device Hosts (`apps/server/src/device/`):** Local, SSH, EAS, and Simlock device hosts for iOS/Android/Expo simulators.

---

## 8. Git, Source Control, Pull Requests & Issue Trackers

| Capability | OpenAgents Desktop | t3code Desktop |
| --- | --- | --- |
| Automatic worktrees | Yes (`Worktree.swift`, CoW hydration, dirty-worktree protection on delete, auto-restore) | Yes (`VcsProvisioningService.ts`, checkout cache, restore missing worktree, setup scripts) |
| Git status / branch / commit / push UI | Branch name chip only; zero Git action buttons | **`GitActionsControl.tsx`**: commit, push, pull, create PR, live git-hook output streaming, default-branch safety guards, **AI-generated commit messages & PR descriptions** (Conventional Commits / repo conventions) |
| Source control forges | None in Desktop UI | 7 forge adapters: **GitHub, GitLab, Bitbucket, Azure DevOps, Forgejo, GitCafe, Cursor Origin** |
| Pull Requests view | None | Dedicated **`_chat.pull-requests.tsx`**: PR inbox, saved views, stacked PRs, files-viewed tracking, **AI-generated guided code review tours** (`pullRequest/guide/`), PR watch notifications (`pullRequestWatch.ts`), materialize PR into thread |
| Issue trackers | None | Dedicated **`_chat.issues.tsx`**: Linear integration supporting **multiple Linear teams per project** (`103_IssueTrackerAdditionalScopes.ts`), spawn threads from issues |

---

## 9. Providers, Accounts, Usage/Quota Limits, MCP & Skills

### 9.1 Providers & Drivers
- **OpenAgents Desktop (5 providers):**
  1. **Microcoder (Gemini 4 Neon/Carbon/Hydrogen/Argon)** via Vertex AI / ADC or Gemini API key.
  2. **Microcoder (OpenRouter)** with live model validation against `openrouter.ai/api/v1/models`.
  3. **Microcoder (Local Metal — Inkling 2 Mini GGUF)** via managed `curl` download + `llama-server` on 32 GB+ RAM Macs.
  4. **Claude Code** (`claudeworker` + `gitguard`, used as sub-agents: Haiku/Sonnet/Opus/Fable).
  5. **Antigravity CLI (`agy` >= 1.5.0)** (`agyworker` over ACP, isolated profiles, OAuth `--login` launcher, interactive permissions `#13831`, detached subagents `#13832`, MCP injection `#13833`, multipart/UTF-16/read-repair `#13834`).
  - *Notable omissions in Desktop:* **Codex** (exists in `coder/tui/codex.coax` but not wired into Desktop), **Cursor** (exists in `coder/tui/cursor.coax` but not wired into Desktop), OpenCode, Grok, Kiro, Pi, generic ACP Registry.
- **t3code Desktop (11 built-in drivers + ACP Registry):** Codex, Claude, Cursor, Grok, Fx, OpenCode, Antigravity, Kiro, Pi, Muse, plus `AcpRegistryDriver` for arbitrary ACP-compliant agents. Supports **multi-instance accounts** (`ProviderInstanceRegistry.ts`) and **load balancing** (`LoadBalancingSettings.tsx`).

### 9.2 Usage & Quota Visibility
- **OpenAgents Desktop:** Shows **per-chat USD spend vs budget** and a **context-window fullness ring** in the composer. Although `coder usage` / `/usage` in the TUI probes Claude, Codex, and Antigravity 1.5.x quotas (`#13838`), **OpenAgents Desktop has no Usage page or Settings → Usage view**.
- **t3code Desktop:** Dedicated **`_chat.usage.tsx`** (`UsagePage.tsx`, `UsageLimits.tsx`, `UsageLimitsPooled.tsx`, `UsageProviderChart.tsx`, `UsagePriceOverrides.tsx`) + inline `ComposerUsageLimits.tsx` + `UsageLimitRecoveryBanner.tsx` + `UsageLimitRecoveryWorker.ts` (automatically resumes/rotates accounts on quota reset) + LiteLLM live pricing table with user overrides across 8 usage-capable drivers.

### 9.3 MCP, Skills & Secrets
- **OpenAgents Desktop:**
  - Scans `~/.claude/skills/*/SKILL.md` read-only and injects up to 16 skills into Microcoder's system note; global `~/.openagents/instructions.md` editable in Settings.
  - **No MCP configuration UI, no MCP server status/OAuth UI, no skills/plugins browser/manager UI.**
  - Secrets stored in `0600` files under `~/Library/Application Support/Coder/` + Secure Enclave-bound refresh tokens for OpenAgents Account (Passkey / Email OTP / Nostr NIP-98).
- **t3code Desktop:**
  - **Built-in MCP HTTP server (`McpHttpServer.ts`) with 14 toolkits** (orchestrator, worktree, preview, device, issues, pullRequests, html, theme, thread, toolSearch, etc.), **MCP OAuth sign-in flow (`McpOAuth.ts`, `McpSignInAction.tsx`)**, and **MCP Apps iframe UI (`McpAppFrame.tsx`)**.
  - **Secrets (`SecretResolver.ts`, `SecretRequests.ts`, `secret-provider-onepassword`):** Interactive secret request cards in chat + 1Password integration.
  - **Skills:** Searchable in composer (`providerSkillSearch.ts`) with inline chips.

---

## 10. Settings, Keybindings, Command Palette, Themes & Notifications

| Dimension | OpenAgents Desktop | t3code Desktop |
| --- | --- | --- |
| Settings breadth | **5 pages** (`General`, `Models`, `Foreman`, `Account`, `About & updates`), live search across 25 entries | **26 dedicated settings routes** (`general`, `appearance`, `chat`, `threads`, `archived`, `projects`, `worktrees`, `diffs`, `providers`, `connections`, `access`, `devices`, `integrations`, `source-control`, `secrets`, `browser`, `voice`, `snap-shot`, `scheduled-tasks`, `keybindings`, `storage`, `maintenance`, `diagnostics`, `labs`, `beta`, `about`, `open-source-licenses`) |
| Themes & appearance | Fixed monochrome Zeron skin; 8 zoom steps (`80%`–`200%`) | 5 built-in palettes (`t3-chat`, `grove`, `ocean`, `ember`, `iris`), light/dark/system, **VS Code & OpenVSX theme import** (`vscodeThemeImport.ts`, `openVsxThemes.ts`), custom CSS, font & contrast controls, live theme inspector |
| Keybindings | Hardcoded macOS shortcuts (`Cmd+N`, `Cmd+K`, `Cmd+B`, `Cmd+/`, `Cmd+1..9`, `Ctrl+Tab`, `Cmd+\``, `Shift+Cmd+.`, `Option+Cmd+S`) | Full **customizable keybindings editor** (`KeybindingsSettings.tsx`) with VS Code-style `when`-clause AST (`terminalFocus`, `previewFocus`, `editableFocus`, `isDesktop`, `isWeb`) and conflict detection |
| Command Palette | `Cmd+K` searches sessions (titles), 24 commands, and settings | `Cmd+K` with 3 modes (`command`, `files`, `content`), plus theme switcher, port-forward, add-project, and new-thread-in sub-flows |
| Notifications & sounds | APNs account-security notices only; **no turn-finished / needs-input notifications or sounds** | 4 modes (`off`, `notifications`, `sound`, `notifications-and-sound`), Web Audio completion & input-needed chimes, OS notifications with deduplicated tags, Dock/taskbar badge counts |

---

## 11. Remote Environments, Mobile Sync & Federation

| Capability | OpenAgents Desktop | t3code Desktop |
| --- | --- | --- |
| Local-to-mobile pairing | **Zero-config Tailscale + Bonjour (`_openagents-coder._tcp`)** (`Tailnet.swift`), verified via `tailscale whois --json`, sealed NIP-SESS NDJSON forwarded to `coder-host`; OpenAgents Mobile streams live turns, steers, queues, and stops | Pairing grants (`PairingGrantStore.ts`) + DPoP-bound tokens (`dpop.ts`) + `pair.tsx` / `connect.tsx` / `AuthorizedClientsSettings.tsx` / `ConnectDeviceApprovalSettings.tsx` |
| Remote SSH environments | **Absent** in Desktop (`coder/tui/remote.coax` / Boats exist in CLI/TUI, not wired into Desktop) | **First-class (`DesktopSshEnvironment.ts`)**: discovers `~/.ssh/config` hosts, manages SSH tunnels + password/passphrase prompts, bootstraps remote `t3` server, remote Open-in-Editor |
| Windows WSL2 integration | **Absent** | **First-class (`apps/desktop/src/wsl/`)**: run backend inside WSL2 alongside or instead of native Windows, with transparent path translation |
| LAN / Tailscale Serve exposure | Tailnet listener only (no UI toggle; hidden `defaults write` to disable) | In-app `DesktopServerExposure.ts` UI switching between Local-only (`127.0.0.1`), LAN (`0.0.0.0`), and Tailscale Serve |
| Cloud Relay, Peer Federation & Guest Sharing | **Absent** in Desktop | Managed Cloud Relay (`CloudLink.ts`), Peer-to-Peer environment federation (`PeerLinks.ts`, `RemoteDelegation.ts`, `PeerThreadRelay.ts`), and Guest thread sharing (`guest.tsx`) |

---

## 12. Where OpenAgents Desktop Is Ahead of t3code Desktop

Any roadmap should preserve and amplify the genuine advantages OpenAgents Desktop already holds over t3code Desktop:

1. **Native GPU performance & zero-Chromium footprint:** Coax + Metal/Vulkan renders at 120 Hz ProMotion with frosted-glass backdrop blur, sub-second startup, and a fraction of Electron's RAM/battery footprint.
2. **Shared single-language core (`coax/lib/agent/desktop/`):** One Coax codebase drives macOS, Linux, and Windows UI + core logic without maintaining separate TypeScript/React/Node/native stacks.
3. **Resident daemon outliving UI quit (macOS):** `coder-host` keeps up to 12 agent runs alive after the Desktop window/app quits and reattaches losslessly from byte offsets.
4. **Autonomous `/goal` loops with verification audit:** First-class multi-turn goal state machine (`Active` / `Paused` / `Complete` / `Blocked` / `Budget limit`) that forces a verification audit turn before accepting `[GOAL_COMPLETE]`.
5. **Per-chat USD spend & budget guardrails:** Hard per-chat spend limits with live spend chip and explicit approval/override when reached.
6. **Foreman multi-chat orchestrator (`Foreman.swift`, `foreman-ctl`):** Built-in operator agent with tiered permissions (`Observe` / `Managed` / `Autonomous`), loopback HTTP control API, event bus, and JSONL audit log.
7. **Native iOS Simulator pane (`oasim` + `simpane.coax`):** Direct 15 fps Metal framebuffer streaming with device chrome, touch/pinch/keyboard forwarding, and per-chat agent permission cards.
8. **3D Metal PLE Gym Workbench & `OA-SHOW-LIVE`:** Native embedded 3D scene inspection directly inside chat turns and the Gym workbench.
9. **Local Metal LLM inference (`Inkling-2-Mini-IQ4_NL.gguf`):** Built-in resumable download and local `llama-server` hosting on 32 GB+ Macs with zero external accounts.
10. **Zero-config Tailnet/Bonjour mobile pairing & sovereign identity:** Automatic Tailscale `whois`-verified iPhone pairing plus Passkey + Nostr NIP-98 + Secure Enclave auth.
11. **CoW worktree hydration (`.coder/worktree.json`):** APFS `cp -c` / `--reflink=auto` / `robocopy` cloning of heavy build folders and shared `COAX_TOOL_CACHE` / `CARGO_TARGET_DIR`.
12. **Cryptographically pinned Ed25519 update manifests:** `openagents.desktop.update.v1` verifies Ed25519 signature, pinned team ID, exact size, SHA-256, and `codesign --verify --deep --strict` before atomic swap.

---

## 13. Prioritized Gap Closure Roadmap

Ordered by day-to-day developer impact and architectural fit with OpenAgents' Coax-first model:

### P0 — Highest-Impact Daily-Driver Gaps
1. **Integrated Terminal Pane (PTY):** Embed a multi-tab terminal pane in OpenAgents Desktop (using Coax terminal cell layout `components/backend/terminal.coax` + native PTY host) with clickable file paths and "Send selection to chat". This is the single biggest workflow gap forcing users to alt-tab out of OpenAgents Desktop.
2. **Real Git Diff Viewer, Git Actions & Per-Turn Checkpoints/Rewind:**
   - Upgrade `page/layout/review.coax` from edit-tool-only hunks to real `git diff` (staged + unstaged + per-turn checkpoint diffs) with a changed-files tree and inline review comments that attach back to the composer.
   - Add a Git Actions bar (Commit, Push, Pull, Create PR, Branch switcher, Worktree vs Local mode toggle).
   - Add Git-backed per-turn checkpoints (`CheckpointStore` equivalent in `coax/lib/agent/desktop/worktree.coax`) with one-click turn + working-tree rewind.
3. **Full Interactive Permission, Question & Plan Cards for All Workers:**
   - Extend `page/permit.coax` (landed for Antigravity in `#13831`) to Microcoder's `"gated":true` commands and sub-agent approvals (instead of auto-approving everything in `README.md`).
   - Add a dedicated **Plan Mode** approval card (`ProposedPlanCard` equivalent).
4. **Completion / Needs-Input Notifications, Audio Chimes & Dock/Taskbar Badges:**
   - Fire native macOS/Linux/Windows notifications + optional subtle completion/input-needed sound when a background or unfocused chat finishes or requests permission, plus unread/attention Dock & taskbar badge counts.

### P1 — Workspace Organization, Providers & Cross-Platform Parity
5. **Project-Grouped Sidebar, Archiving, & Full-Text Transcript Search:**
   - Group chats by repo/working folder in `page/layout/sidebar.coax` (keeping Pinned at top), lift the 50-chat visibility cap, add Archive/Restore UI (wiring the existing daemon `archived` field), and add in-thread find (`Cmd+F` inside chat) + full-text cross-chat search in `Cmd+K`.
6. **Desktop Usage & Quota Dashboard (Settings → Usage + Composer Popover):**
   - Surface the existing Coax quota probes (Claude, Codex, Antigravity `#13838`) and aggregate token/USD spend across chats in a dedicated Settings → Usage page and composer ring popover.
7. **Wire Codex & Cursor Workers into Desktop + MCP & Skills UI:**
   - Expose the already-implemented `coder/tui/codex.coax` and `coder/tui/cursor.coax` engines in Desktop's model picker and Settings → Models.
   - Add Settings → MCP (server list, status, OAuth sign-in, enable/disable) and Settings → Skills (browse/toggle `~/.claude/skills` and project skills).
8. **Close Linux & Windows Desktop Parity Gaps:**
   - Wire `coder-host` resident daemon, Antigravity worker (including `#13839` Windows `.cmd` launcher + live Wine smoke test), image attachment decoding, in-app update feed download, and Windows Authenticode signing + installer (`apps/openagents-desktop/windows/`).

### P2 — Power-User Polish & Remote Workflows
9. **File Explorer Tree & Syntax-Highlighted Editor:** Add a collapsible repo file tree tab in the right pane, lift the 16 KB edit cap in `page/filepane.coax`, add syntax highlighting (reusing `components/core/` syntax roles), and add "Open in Editor" (`VS Code`, `Cursor`, `Zed`, `Xcode`, `Ghostty`).
10. **Voice Dictation in Desktop:** Wire the existing Coax voice/dictation pipeline (`coder/tui/dictate.coax`, `metaverse/voice.coax`) + macOS native speech / local Whisper into a composer mic button and global shortcut.
11. **Remote Environments (SSH / Boats) & Keep-Awake:** Wire `coder/tui/remote.coax` and Boats into a Desktop environment picker (SSH host discovery + remote daemon tunnel), and add OS sleep-prevention (`IOPMAssertion` / `systemd-inhibit` / `SetThreadExecutionState`) while runs are active.
12. **Rich Markdown & Composer Upgrades:** Add Mermaid diagram rendering and LaTeX math to `components/core/` Markdown, prompt stashing/history, and multi-model fan-out send.

### P3 — Shell Polish & Deferred Items
13. **Custom Protocol (`openagents://`), CLI Activation & Shell Polish (`#13889`):** Custom `openagents://` protocol handler, single-instance CLI activation socket, CLI shim installer, quit-hold protection, and multi-display bounds polish.
14. **Deferred / Closed as Won't Do (2026-11-20, see `docs/audits/2026-11-20-neon-transcripts-and-release-vision-audit.md`):**
    - ~~Pull Requests inbox, multi-forge source control (GitLab/Bitbucket/Azure/Forgejo), AI review tours, and Issues view (`#13886`)~~ — deferred; GitHub commit/push/PR actions remain in `#13871`.
    - ~~Full VS Code/OpenVSX theme engine, customizable `when`-clause keybindings, and 10-locale i18n (`#13887`)~~ — deferred from short-term trajectory.
    - ~~Embedded Chromium web Preview Browser & global SnapShot (`#13888`)~~ — closed as won't do to preserve OpenAgents Desktop's zero-Chromium native Metal/Vulkan architecture (`oasim`, native 3D/Game Builder viewports, and external headless Chrome/CDP cover preview/testing needs).

---

## 14. Deliberate Non-Goals (Do Not Copy from t3code)

- **No Electron / Chromium / Node.js runtime:** Keep OpenAgents Desktop native Coax + Metal/Vulkan.
- **No Clerk or third-party auth lock-in:** Keep OpenAgents' sovereign Passkey + Nostr NIP-98 + Secure Enclave identity.
- **No 1Password SDK dependency:** Keep zero-dependency `0600` local key files / OS Keychain / Secure Enclave.
- **No heavy SQLite ORM/migration stack in Desktop unless needed:** Coax-native indexed store or lightweight SQLite FTS for search is sufficient without adopting Node/Effect-TS orchestration layers.
