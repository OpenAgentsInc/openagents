# Daily Coding Capability Audit — Provider Session Archives vs OpenAgents Desktop

- **Date:** 2026-07-11
- **Author:** Fable (agent)
- **Status:** capability audit — not roadmap authority; sequencing stays `docs/sol/MASTER_ROADMAP.md`
- **Status snapshot superseded:** per-capability statuses here are the 2026-07-11 snapshot; the live authority is `apps/openagents-desktop/src/capability-registry.ts`, and the refreshed status + switch runbook is [`2026-07-12-daily-coding-cutover-status-and-switch-runbook.md`](./2026-07-12-daily-coding-cutover-status-and-switch-runbook.md)
- **Issue:** #8712 (EP250)
- **Owner directive (verbatim):** "deeply research all of the chats we make now in ~/.claude and ~/.codex, for day to day coding, build tests/evals to ensure we can do ALL of those things via UI and programmatically via OpenAgents app only. first write a detailed audit in docs/fable/, commit and push."
- **Sources:** local `~/.claude/projects` and `~/.codex/sessions` JSONL archives, mined 2026-06-11 → 2026-07-11 (30-day window, mtime-filtered); openagents source at `origin/main` HEAD `c95b260c22`
- **Privacy note:** archives were mined with local read-only scripts that emitted only counters, type names, and distributions. This document contains no prompt text, no assistant text, no thinking, no tool arguments, no session file paths, no session identifiers, and no repo names other than public OpenAgents ones. Shell-command evidence is limited to first-token verb frequencies (e.g. `git`, `gh`, `bun`). This follows the privacy boundary in `docs/teardowns/2026-07-10-claude-subagents-rendering-analysis.md`.

## 1. What this is

Deep-research audit of every distinct thing day-to-day coding actually consists of here — measured from one month of real Claude Code and Codex session archives — mapped against what the OpenAgents desktop app (`apps/openagents-desktop/`) can do today, ending in a test/eval matrix a follow-on lane can implement without re-mining. The deliverable chain is: (1) quantitative mining, (2) capability taxonomy with per-capability desktop status and code receipt, (3) UI + programmatic oracle per capability with six-rung targets, (4) the honest ranked gap list.

Several of the window's most recent sessions are the OpenAgents/EP250 build itself. That is representative, not contamination: the app must be able to build itself.

## 2. Method and privacy boundary

Three Python miners walked the archives read-only and emitted aggregate JSON only:

- **Claude:** per-record `type`, role, `tool_use` names, `isSidechain`, model, timestamps, slash-command markers, image blocks, interrupt markers, `Agent` tool-input flags (`subagent_type`, `run_in_background`, `isolation`), MCP name prefixes.
- **Codex:** record/`payload` types, `function_call`/`custom_tool_call` names, `turn_context` models, `session_meta` key names, `turn_aborted`, compaction, fork/parent lineage keys.
- **Supplemental:** first-token verb of each shell command (both providers) and `git`/`gh` subcommand frequency — first two tokens only, nothing else retained.
- **Sidecars, shapes only:** `~/.codex` SQLite table names (`threads`, `thread_spawn_edges`, `thread_goals`, `thread_dynamic_tools`, `agent_jobs`, `remote_control_enrollments`, …), `~/.claude/tasks` (18 task dirs), `~/.codex/archived_sessions` (38), `~/.codex/history.jsonl` (6,301 lines counted, not read).

Not mined: any message content, tool arguments beyond the two-token shell prefix, or per-session identity. One limit to note honestly: Claude-side resume/fork lineage is not reliably recoverable from the JSONL alone (continuations rewrite in place), so Claude resume rates are inferred from compaction/summary markers rather than measured directly; Codex lineage IS measured (`forked_from_id`, `parent_thread_id`, repeated `session_meta`).

## 3. Quantitative results

### 3.1 Archive overview (30-day window)

| Metric | `~/.claude` | `~/.codex` |
| --- | --- | --- |
| JSONL files scanned | 2,243 | 1,019 |
| Records parsed | 601,097 | 1,268,942 |
| Top-level sessions | 350 (69 interactive workspace, 281 automated/pylon-fleet) | 1,019 rollouts |
| Subagent transcripts | 1,891 sidechain files | 167 `parent_thread_id` spawn threads |
| Human/user turns | 7,350 | 4,916 |
| Turns/session p50 / p90 / max | 1 / 9 / 1,157 | 1 / 20 / 202 |
| Session entry points | CLI + SDK | Codex Desktop 979, codex-tui 371, codex_sdk_ts 312, khala_code_desktop 77, codex_exec 15, pylon stream 6 |
| Models seen | 4 Claude models + haiku utility calls (opus-4-8 dominant by assistant-record count, then sonnet-5, fable-5) | gpt-5.6-sol dominant, then gpt-5.5, 5.6-terra, 5.4 |

Sessions run around the clock (hour-of-day histograms are flat-ish with peaks 13:00–20:00 UTC for Claude and heavy overnight 02:00–10:00 UTC for Codex — the overnight autonomous loops are Codex-heavy). Session volume is bursty: quiet days of 1–8 sessions and assault days of 96–241 (fleet waves).

### 3.2 Tool-call frequency, ranked

**Claude (top-level + sidechain combined):**

| Tool | Calls | | Tool | Calls |
| --- | --- | --- | --- | --- |
| Bash | 109,408 | | WebFetch | 368 |
| Read | 32,080 | | Monitor | 263 |
| Edit | 21,728 | | SendMessage | 201 |
| Write | 4,680 | | WebSearch | 176 |
| Agent (subagent) | 1,845 | | EnterWorktree | 139 |
| ScheduleWakeup | 778 | | TaskStop | 130 |
| TaskUpdate/Create/List | 727 | | Skill | 52 |
| ToolSearch | 559 | | AskUserQuestion | 48 |

**Codex (function/custom tool calls):**

| Tool | Calls | | Tool | Calls |
| --- | --- | --- | --- | --- |
| exec_command | 130,465 | | web_search | 386 |
| exec | 23,030 | | view_image | 250 |
| apply_patch | 15,714 | | list_agents | 201 |
| write_stdin | 8,333 | | spawn_agent | 182 |
| update_plan | 1,617 | | followup_task | 137 |
| wait | 1,270 | | GitHub MCP (issue/PR ops) | ~144 |
| send_message | 843 | | goal tools | ~125 |
| wait_agent | 727 | | interrupt_agent | 19 |
| MCP doc/design fetch | ~800 | | | |

### 3.3 What the shell calls actually are (first-token verbs)

| Verb family | Claude | Codex | Combined |
| --- | --- | --- | --- |
| search (`grep`/`rg`) | 22,371 | 13,894 | 36,265 |
| file reading (`sed`/`nl`/`cat`/`head`/`tail`) | 11,319 | 42,553 | 53,872 |
| `git` | 12,859 | 29,044 | 41,903 |
| `bun`/`bunx` | 11,998 | 15,920 | 27,918 |
| `gh` | 2,549 | 7,394 | 9,943 |
| `find`/`ls` | 7,674 | 5,549 | 13,223 |
| `curl` | 414 | 1,699 | 2,113 |
| `python3`/`node` | 2,087 | 1,570 | 3,657 |
| `gcloud` | 455 | 430 | 885 |

`git` subcommands (combined): status 8,751 · diff 8,079 · fetch 2,838 · add 2,194 · push 1,581 · log 1,591 · worktree 1,079 · commit 871 · show 820 · rebase 437+. `gh` subcommands: issue 6,012 · pr 1,999 · api 239 · auth 107 · label 42 · release 15. GitHub work is issue-first, PRs second; the fetch→rebase→push retry loop and worktree isolation are structural habits, not occasional.

### 3.4 Capability signal counts

| Signal | Claude | Codex |
| --- | --- | --- |
| Image inputs | 97 user messages with image blocks (21 sessions) | 250 `view_image` calls (27 sessions) |
| Interrupts / steering | 118 interrupt markers (16 sessions) | 122 `turn_aborted` (106 sessions) |
| Plan usage | 2 `/plan` + plan-tool disabled here | 1,617 `update_plan` (76 sessions) |
| Slash commands | 302 uses; top: `/loop` 222, `/model` 30, `/compact` 7 | n/a |
| MCP calls | 58 (stripe 37, expo 13, Apollo 8) | ~800 incl. design-fetch + docs + GitHub servers |
| Web tools | 544 (WebFetch 368 + WebSearch 176) | 386 searches (65 sessions) |
| Subagent spawns | 1,845 (`general-purpose` 1,420, `Explore` 420); 1,811 background, 546 worktree-isolated | 182 spawns + 727 waits + 843 messages to agents |
| Compaction | 14 sessions | 5,648 events across 252 sessions |
| Resume / fork | continuation-in-place (not directly countable) | 741 appended `session_meta` beyond first; 158 `forked_from_id`; 167 `parent_thread_id` |
| Background tasks | TaskCreate 281 / ScheduleWakeup 778 / Monitor 263 | `wait` 1,270, `followup_task` 137, automations dir present |

## 4. Capability taxonomy and desktop status

Status enum: **ui_available** (exercisable from the app UI today, incl. agent-mediated via the fable-local lane's full toolset) · **programmatic_only** (typed surface exists, no UI affordance) · **partial** · **missing**. Receipts are against `apps/openagents-desktop/src/` at `c95b260c22`. Key structural fact: `fable-local-runtime.ts` imposes no `allowedTools` restriction (allow-all `canUseTool`, disallow-list only `Skill`, `EnterPlanMode`, `ExitPlanMode`), so the local agent has the full SDK toolset — Bash, Read, Write, Edit, Grep, Glob, Agent children, WebSearch, WebFetch — plus the `mcp__codex__delegate` tool.

### A. Conversation & steering

| ID | Capability | Observed | Status | Receipt |
| --- | --- | --- | --- | --- |
| A1 | Multi-turn streaming chat | 12,266 human turns | ui_available | `renderer/shell.ts` transcript + `fable-local-contract.ts` `text_delta` |
| A2 | Mid-turn interrupt / steer | 240 interrupts | partial | `FableLocalInterruptChannel` + `renderer/boot.ts:298` plumbed; no renderer Stop-button call site; signed-in `conversation.interrupt` exists (`runtime-gateway-contract.ts:181`) |
| A3 | Queue follow-up while turn runs | 137 `followup_task` + queue-operation records | partial | signed-in `conversation.append`; no local-lane queue |
| A4 | Model selection / mix | 9 models, 30 `/model` | partial | model pinned `FABLE_LOCAL_MODEL`; `model_effective` event exists, no picker |

### B. Code reading & search

| ID | Capability | Observed | Status | Receipt |
| --- | --- | --- | --- | --- |
| B1 | File reading | 85,952 (Read + reader verbs) | ui_available | agent Read/Bash + `workspace-contract.ts` `workspace-read`, files workspace |
| B2 | Code search (grep/rg) | 36,265 | ui_available | agent Bash/Grep; no direct search box in files workspace (agent-mediated only) |
| B3 | Structure navigation (glob/find/ls) | 13,226 | ui_available | agent tools; files workspace tree |

### C. Editing & patching

| ID | Capability | Observed | Status | Receipt |
| --- | --- | --- | --- | --- |
| C1 | Targeted edits | 37,442 (Edit + apply_patch) | ui_available | agent Edit; `renderer/tool-cards.ts` renders typed cards |
| C2 | New-file creation | 4,680 Write | ui_available | agent Write, tool card |
| C3 | Human file edit + save | habitual | ui_available | `workspace-save` with SHA-256 `expectedRevision` conflict guard (`workspace-contract.ts`) |

### D. Execution & terminal

| ID | Capability | Observed | Status | Receipt |
| --- | --- | --- | --- | --- |
| D1 | Shell execution | 262,903 calls | ui_available | agent Bash; tool cards |
| D2 | Background processes + monitoring | Monitor 263, `wait` 1,270, bg spawns 1,811 | partial | delegate children async w/ caps; no general background-process surface |
| D3 | Interactive terminal / stdin steering | `write_stdin` 8,333 | partial | `workspace.terminal` command exists (`desktop-command-contract.ts`); no agent-attached PTY stdin path |

### E. Git & GitHub

| ID | Capability | Observed | Status | Receipt |
| --- | --- | --- | --- | --- |
| E1 | Repo inspection (status/diff/log) | ~18,400 | ui_available | review workspace: `workspace-git-status`/`workspace-git-diff` + agent |
| E2 | Commit / push (incl. fetch-rebase-push retry) | 5,290 (commit+push+rebase+fetch) | programmatic_only | agent Bash only; no commit/push UI or gateway command |
| E3 | Worktree / branch isolation | 1,079 `git worktree` + 546 isolated spawns + 139 EnterWorktree | programmatic_only | agent-driven only |
| E4 | GitHub issues (`gh issue`) | 6,012 | programmatic_only | agent Bash only |
| E5 | GitHub PRs (`gh pr`, PR create) | 2,006 | programmatic_only | agent Bash only |

### F. Web research

| ID | Capability | Observed | Status | Receipt |
| --- | --- | --- | --- | --- |
| F1 | Web search | 562 | ui_available | SDK WebSearch available in fable-local (no allowedTools restriction) |
| F2 | URL fetch | 368 | ui_available | SDK WebFetch available in fable-local |

### G. Delegation & multi-agent

| ID | Capability | Observed | Status | Receipt |
| --- | --- | --- | --- | --- |
| G1 | Subagent spawn (same provider) | 2,027 | ui_available | SDK Agent tool + `tool-cards.ts:280` Agent card |
| G2 | Cross-provider delegation | delegate habit is new (EP250) but codex spawn_agent 182 | ui_available | `mcp__codex__delegate` (`fable-local-runtime.ts:137`), `codex-child-runtime.ts`, child_* events |
| G3 | Background agents + completion notify | 1,811 bg spawns, 727 wait_agent | partial | children async with caps/timeouts; no notification-on-complete surface |
| G4 | Steer/message running children | 843 send_message + 201 SendMessage + 19 interrupt_agent | missing | no child-steer channel in `fable-local-contract.ts` |
| G5 | Scheduled / fleet automation | ScheduleWakeup 778, automations dir, cron tools | partial | fleet workspace + pylon registry; no local scheduling |

### H. Session lifecycle

| ID | Capability | Observed | Status | Receipt |
| --- | --- | --- | --- | --- |
| H1 | Resume / continuation | 741 Codex resume-appends; Claude continuation habitual | partial | SDK `resume: resumeSessionId` per thread (`fable-local-runtime.ts:870,930`); automatic only, no picker; Codex children never resume |
| H2 | Session fork | 158 `forked_from_id` | missing | no fork surface |
| H3 | History import / browse | 1,019 Codex + 2,243 Claude files | partial | `codex-history.ts` + history workspace import `~/.codex/sessions`; no `~/.claude` import |
| H4 | Session search | 6,301-line codex history index; constant re-finding | partial | structured coding-session search (`shell.ts:1846`, `project:`/`repository:`/`state:` grammar); no free-text transcript search |
| H5 | Context compaction | 5,662 events | partial | SDK auto-compacts; no UI marker or control |

### I. Context & inputs

| ID | Capability | Observed | Status | Receipt |
| --- | --- | --- | --- | --- |
| I1 | Image input (screenshots) | 347 (97 msgs + 250 view_image) | missing | no composer image path; `main.ts:866` disallows webview attachments |
| I2 | User-configured MCP servers | ~858 MCP calls across 4+ servers | missing | only internal delegate SDK-MCP server (`fable-local-runtime.ts:673`); no config UI |
| I3 | Skills / slash commands | 354 (Skill 52 + slash 302, `/loop` dominant) | missing | `Skill` in `FABLE_LOCAL_DISALLOWED_TOOLS`, `skills: []` |
| I4 | File attachments / mentions | habitual in CLI | missing | no attachment path |

### J. Interactive control

| ID | Capability | Observed | Status | Receipt |
| --- | --- | --- | --- | --- |
| J1 | Agent asks user a question | 48 AskUserQuestion | ui_available | question cards end-to-end (`fable-local-contract.ts` question_pending/resolved, `shell.ts` QuestionCardInteraction) |
| J2 | Plan mode / plan review | 1,693 (update_plan 1,617 + plan sessions) | missing | `EnterPlanMode`/`ExitPlanMode` disallowed; no plan surface (signed-in `runtime.decideInteraction` is tool-approval, not plan) |
| J3 | Tool approval / permission modes | pervasive (CLI permission prompts) | partial | local lane allow-all `canUseTool`; signed-in `runtime.decideInteraction` exists |
| J4 | Task/todo progress tracking | 727 Task* + 1,617 update_plan | missing | no todo/plan-progress rendering |

### K. Workspace & observability

| ID | Capability | Observed | Status | Receipt |
| --- | --- | --- | --- | --- |
| K1 | Multi-repo / workspace switching | sessions span many cwds; 18 task dirs | partial | `workspace.choose` command; single active workspace |
| K2 | Usage / token observability | 477,474 token_count events | ui_available | `usage-ledger-contract.ts` snapshot/rows per (provider, accountRef); fleet workspace renders |

**Totals: 33 capabilities — 13 ui_available, 4 programmatic_only, 10 partial, 6 missing.**

## 5. Test/eval matrix (spec for the follow-on lane)

Every capability gets two oracles: a **UI oracle** (live-proof step or renderer/DOM test) and a **programmatic oracle** (typed surface driven headlessly — fable-local events, gateway commands, workspace channels, usage ledger). Six-rung target says the highest rung the eval must reach initially (`fixture` = rung 2 fixture-proven via `bun test`/smoke; `live` = rung 4 live-proven via live-proof receipt). Existing infra to build on: `src/live-proof.ts` (typed `LiveProofStepName` union, journal.json + PNG receipts), smoke mode fixtures (`tests/fixtures/codex-smoke/`, scripted fable/codex-child spawns, question-card fixture), `bun:test` renderer suites, `usage-ledger.test.ts`, `runtime-gateway.e2e.test.ts`, behavior-contract sweep (`packages/behavior-contracts`).

| ID | UI oracle | Programmatic oracle | Rung target |
| --- | --- | --- | --- |
| A1 | live-proof `fable-turn` (exists): streamed text visible in DOM | assert `turn_started`→`text_delta`+→`turn_completed` ordering on `FableLocalEvent` stream | live (exists — keep) |
| A2 | NEW live-proof step `interrupt-stop`: click Stop mid-turn, transcript shows interrupted state | invoke `FableLocalInterruptChannel` mid-fixture-turn; assert `turn_failed` reason `interrupted` | live (blocked: Stop button) |
| A3 | renderer test: composer accepts input while turn streaming; queued indicator | fixture: send second prompt mid-turn; assert ordered delivery post-turn | fixture (blocked: queue) |
| A4 | renderer test: model chip shows effective model | assert `model_effective` event matches pinned model; substitution → `model_substituted` failure | fixture (exists partially) |
| B1–B3 | smoke fixture turn whose script emits Read/Grep/Glob tool_use; tool cards render humanized (extend `tool-cards.test.ts`) | fixture transcript contains `tool_use`+`tool_result` pairs for each tool kind | fixture |
| C1–C2 | tool-card render for Edit/Write incl. path redaction check (live-proof `redaction-check` exists) | fixture: Edit/Write tool_result success recorded; workspace-read confirms content | fixture, then live |
| C3 | NEW live-proof step `file-save`: edit in files workspace, save, reread | `workspace-save` with stale `expectedRevision` → typed conflict; fresh revision → success | live |
| D1 | live-proof `fable-turn` already exercises Bash; add assertion a Bash card rendered | fixture Bash tool_use/result pair | live (exists) |
| D2 | renderer: background/child activity indicator visible while child runs | delegate fixture: `child_started` → `child_activity`+ → `child_completed` | fixture |
| D3 | NEW: terminal workspace opens, echoes a command | typed PTY seam (to be defined at implementation) | fixture (blocked: PTY seam) |
| E1 | NEW live-proof step `git-review`: review workspace shows real dirty-file diff | `workspace-git-status` + `workspace-git-diff` against a seeded temp repo | live |
| E2 | NEW live-proof step `commit-push` (agent-mediated): prompt drives commit in seeded repo; review workspace shows clean | assert seeded repo HEAD advanced + remote ref updated (local bare remote) | live |
| E3 | agent-mediated: prompt creates worktree in seeded repo; transcript card visible | assert worktree dir exists then removed; `isolation` flag on Agent spawns recorded | fixture |
| E4–E5 | agent-mediated `gh` against a fixture/replay transport (no live GitHub in CI) | assert issue/PR command family invoked and outcome card rendered; live rung = one real `gh issue comment` receipt on a designated test issue | fixture, one live receipt |
| F1–F2 | tool cards for WebSearch/WebFetch render query/domain only | fixture tool_use pair; redaction assert (no full URL leak into card) | fixture |
| G1 | Agent card renders with child summary (extend `tool-cards.test.ts:280` coverage) | fixture Agent tool_use; child transcript linked | fixture |
| G2 | live-proof `codex-turn`-adjacent: delegate card + child completion visible | delegate fixture end-to-end: `child_started/completed`, `CodexChildResult` decode, usage rows for provider `codex` | live (exists in smoke; promote) |
| G3 | child-completion notification appears after user navigates away | assert `child_completed` event delivered while different workspace active | fixture |
| G4 | (blocked) steer-child affordance | (blocked) child-steer channel to be added to `fable-local-contract.ts` | — until landed |
| G5 | fleet workspace lists pylon registry (live-proof `fleet-workspace` exists) | usage-ledger + registry snapshot decode | live (exists) |
| H1 | NEW live-proof step `thread-resume`: second turn on same thread references first-turn state | assert SDK `resume` used (session id stable across turns) vs history-window fallback | live |
| H2 | (blocked) fork affordance | (blocked) fork seam | — until landed |
| H3 | history workspace renders codex-smoke fixture sessions (exists) | `codex.history.catalog`/`codex.history.page` gateway queries decode fixture | fixture (exists); extend with `~/.claude` importer when landed |
| H4 | search field filters coding catalog (renderer test exists — extend) | catalog query grammar unit tests (`project:`/`repository:`/`state:`) | fixture |
| H5 | compaction indicator rendered when session compacts | long-fixture: assert transcript integrity across a compaction boundary | fixture (blocked: indicator) |
| I1 | (blocked) NEW live-proof step `image-attach`: paste image, turn references it | composer accepts image block; fable-local input schema carries it | — until landed |
| I2 | (blocked) MCP config UI lists a configured server | configured server's tools appear as `mcp__*` tool_use in fixture turn | — until landed |
| I3 | (blocked) slash/skill invocation from composer | skill invocation event recorded | — until landed |
| J1 | smoke question-card fixture (exists, `main.ts:779`): option click resolves | `question_pending`→answer via `FableLocalAnswerQuestionChannel`→`question_resolved` | fixture (exists); add live rung |
| J2 | (blocked) plan review card + approve/reject | (blocked) plan surface | — until landed |
| J3 | signed-in interaction approval card renders (runtime-interactions tests exist) | `runtime.decideInteraction` command round-trip (`runtime-gateway.e2e.test.ts` seam) | fixture (exists) |
| J4 | (blocked) task/plan progress rendering | (blocked) | — until landed |
| K1 | workspace.choose via palette switches root | command host routing test (exists: `desktop-command-host.test.ts`) | fixture |
| K2 | live-proof `fleet-usage-check` (exists) | `usage-ledger.test.ts` exact token accumulation (exists) | live (exists) |

Matrix size: **33 capabilities × 2 oracles = 66 oracles** (10 already exist in some form; ~14 buildable immediately on current code; 9 blocked on missing capabilities).

### Harness shape

1. **New file `apps/openagents-desktop/tests/capability-evals.test.ts`** (bun:test) plus a typed registry `src/capability-registry.ts`: `{ id: "A1"…"K2", group, status, uiOracle, programmaticOracle, rungTarget, blockedOn? }`, validated the same way `packages/behavior-contracts` validates registries (a status of `ui_available` REQUIRES both oracles named and green; `missing` requires `blockedOn`). The eval suite iterates the registry so a capability cannot silently regress from ui_available without a red test.
2. **Extend `LiveProofStepName`** in `src/live-proof.ts` with `interrupt-stop`, `file-save`, `git-review`, `commit-push`, `thread-resume` (seeded temp repo + local bare remote so `commit-push` needs no network), each journaling into the existing `journal.json` + PNG receipt format under `docs/receipts/`.
3. **Extend smoke fixtures** under `tests/fixtures/`: scripted fable turns emitting each tool kind (B/C/F groups), a delegate child script (G2/G3), a long transcript crossing a compaction boundary (H5), a second question-card variant (J1).
4. **Ordered build plan:** Phase 1 — registry + evals for all existing capabilities (pure fixture work, no product change). Phase 2 — the five new live-proof steps (rung-4 receipts for what already works). Phase 3 — missing capabilities in the priority order of §6, each landing WITH its two oracles in the same change (per the behavior-contract discipline).

## 6. Honest gaps — what the archives prove we do daily that the app cannot do yet

Ranked by observed frequency (the owner's real priority list):

1. **Interactive terminal stdin / PTY steering** — 8,333 `write_stdin` calls. Driving REPLs, TUIs, long processes interactively is a daily Codex habit; the desktop has a terminal workspace command but no agent-attached PTY seam. (partial)
2. **Plan/task progress surface** — 2,420 observations (update_plan 1,617 + Task* 727 + plan sessions). Both providers externalize plans constantly; the app renders none of it, and plan mode is explicitly disallowed in the local lane. (missing, J2+J4)
3. **First-class commit/push and PR/issue flows** — 13,300 git-write + `gh` operations. Agent-mediated Bash covers it today (so not a hard blocker), but there is zero typed surface or UI: no commit affordance, no push status, no issue/PR view. The review workspace stops at diff-viewing. (programmatic_only, E2–E5)
4. **Steering running children** — ~1,063 (send_message 843 + SendMessage 201 + interrupt_agent 19). Multi-agent supervision is routine; the app can spawn children but cannot talk to or stop one. (missing, G4)
5. **User-configured MCP servers** — ~858 calls across Stripe/Expo/Apollo/docs/design servers. No config surface; only the internal delegate server exists. (missing, I2)
6. **Resume/fork as a user affordance** — 741 measured Codex resume-appends + 158 forks + pervasive Claude continuation. The local lane resumes automatically per thread, but there is no "pick up that session" or fork control, and `~/.claude` history (2,243 files this month) is not imported at all. (partial/missing, H1–H3)
7. **Image input** — 347 observations, and the workspace's own "no ship without pixel proof" rule makes screenshots load-bearing for UI work. Blocked outright in the composer. (missing, I1)
8. **Skills / slash commands** — 354 uses; `/loop` alone is 222 (recurring supervision loops are a core operating pattern). Disallowed in the local lane. (missing, I3)
9. **Mid-turn Stop button** — 240 interrupts observed across providers; the channel is fully plumbed and untested UI-side because no control invokes it. Cheapest gap on this list. (partial, A2)
10. **Model selection** — 9 distinct models used in the window, 30 explicit switches. The local lane pins one model; the mix in the archives is deliberate (heavy/light routing). (partial, A4)
11. **Free-text session search** — structured catalog search exists, but the daily reality is "find that conversation where…" across 3,262 session files; nothing searches transcript content. (partial, H4)
12. **Message queueing during a turn** — 137 `followup_task` + queue records; steer-by-queueing is habitual in both CLIs. (partial, A3)

Items 1–5 and 7–8 are the ones the archives show heavily that agent-mediation cannot substitute for; item 3 is high-frequency but already achievable through the agent, so it ranks below its raw count for urgency of UI work.

## 7. Follow-on lane pointers

- Implement §5 harness shape phases 1–2 against `apps/openagents-desktop/` (bun:test + live-proof; `verify` script stays the gate).
- Each Phase-3 capability change updates the capability registry and `src/contracts/ux-contracts.ts` in the same change (behavior-contract discipline).
- Re-mining is not needed: the counts above are the frequency authority for prioritization; refresh only when the owner asks for a new window.
