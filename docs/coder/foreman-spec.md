# Foreman for Coder Desktop — Specification

**Status:** Design draft
**Date:** 2026-11-19
**Scope:** `apps/coder-desktop`, `coax/lib/agent/engine/mcworker-wire.coax`, `coder/tui/mcworker.coax`, `coder/book/ch043-the-foreman.md`

---

## 1. Purpose and Definition

Coder Desktop (`apps/coder-desktop`) already lets a user run multiple concurrent chats in the sidebar, each backed by its own headless `CoderPage` (`cd_headless`) and worker subprocess (`mcworker` or `claudeworker`), and lets any single chat spawn ephemeral parallel sub-agents (`cd_agent_task`) inside its own transcript. Once a user has more than three or four active chats across a repository, however, the bottleneck shifts from model execution to human coordination: checking which chats finished, remembering what a chat from two days ago decided, starting follow-up chats in the right folder, and unblocking gated approvals.

The **Foreman** is a dedicated, persistent supervisory chat pinned to the top of the Coder Desktop sidebar. It is the single thread a user talks to when they want to coordinate work across the whole workspace rather than inside one isolated task. Specifically, the Foreman can:

1. **Command other chats** — inspect the state and transcripts of any chat in the sidebar, send instructions to existing chats, pause/stop/resume them, adjust their budgets or thread goals, and (when permitted) resolve gated permission prompts.
2. **Create and spawn its own chats** — open new first-class sidebar chats with a title, working folder, initial prompt, budget, and optional thread goal, so the user can watch, steer, or take over any work the Foreman initiates.
3. **Search across the whole repository** — run fast, bounded repository-wide code, file, and git history searches across the active workspace (or multiple registered workspace roots) without polluting its own context window.
4. **Search through previous chats** — query the local chat store (`~/Library/Application Support/Coder/chats/`) by keyword, folder, date, title, or status, retrieving bounded excerpts and exact chat/turn references.
5. **Operate with scoped elevated permissions** — hold explicit, auditable orchestration privileges beyond a regular chat, governed by clear safety boundaries, user-controlled elevation tiers, and a global kill switch.

### 1.1 Relationship to Existing Mechanisms and History

| Mechanism | Scope | Visibility | Lifetime | Tools / Authority |
|---|---|---|---|---|
| **Regular Chat** (`CoderChat`, Ch. 40) | One task / one folder | Sidebar row | Persistent (`<id>.json`) | Repo tools within its own working folder; no access to other chats |
| **Sub-agent** (`cd_agent_task`, Ch. 41) | One delegated subtask | Inline card inside parent chat | Ephemeral (ends with turn) | Bounded worker turn; cannot appear in sidebar or command other chats |
| **Headless Router** (`coder router`, Ch. 17) | Event-driven issues / CI | Headless CLI / outbox | Daemon / poll loop | Routes external triggers to detached workers; no shell/file tools |
| **Foreman** (this spec, Ch. 43) | Whole workspace + all sidebar chats | Pinned top entry in desktop sidebar | Singleton persistent chat | Cross-chat control, chat spawning, repo-wide search, chat-history search, scoped elevated permissions |

In OpenAgents' earlier Era 1 Rust monorepo, "Foreman" referred to an external orchestration service (`OPENAGENTS_FOREMAN_*` env vars, deprecated in `docs/2026-10-28-coder-service-deprecation.md`). In the current Coax/Swift Era 2 architecture, the Foreman is not a separate cloud service or daemon: it is a local, native capability of Coder Desktop built on the same `CoderStore`, `CoderRun`, `page.coax`, and `mcworker-wire.coax` primitives.

---

## 2. Desktop UI and Sidebar Experience

### 2.1 Pinned Top Entry in the Sidebar

The Coder Desktop sidebar (`apps/coder-desktop/page.coax`) currently has three regions: the top primary action (`New Chat`), the `Pinned` section (user-pinned chats with drag-and-drop reordering), and the `Recents` section (chronological by creation time).

The Foreman occupies a **dedicated, permanently pinned slot above `Pinned` and `Recents`** (directly below `New Chat` or paired in the top header cluster):

- **Always present, non-deletable, non-unpinnable:** Clicking **Foreman** switches the main pane to the Foreman conversation. The user cannot accidentally delete or unpin it; a "Reset Foreman Context" / "New Foreman Thread" action archives the current Foreman transcript into history and starts a fresh Foreman context while keeping the pinned slot.
- **Distinct visual identity:** Uses a distinct icon/glyph and subtle surface accent so the user never confuses a regular worker chat with the supervisory Foreman chat.
- **Live status badge:** Displays compact, real-time operational state on the Foreman row:
  - Running indicator when the Foreman itself is thinking/executing.
  - Count of active Foreman-managed chats (e.g. `3 running`).
  - Attention badge when any chat has a pending gated approval (`ask`) or has hit its budget/error state.
- **Keyboard shortcut:** `⌘0` (or `⌘⇧F`) jumps directly to the Foreman chat from anywhere in the app.

### 2.2 Lineage and Visibility of Foreman-Managed Chats

When the Foreman creates or commands a chat, that chat is a **real sidebar chat** stored in `CoderStore`, not a hidden background blob:

- Foreman-spawned chats appear in the sidebar (with a small subtle badge or metadata tag indicating `Foreman` lineage, e.g. `managed_by: "foreman"`).
- The user can click into any Foreman-spawned chat at any time to read its full transcript, watch tool calls live, type a message directly, pause it, or stop it.
- If the user intervenes directly in a Foreman-managed chat, the Foreman sees the user's message on its next inspection and yields to the user's explicit instruction.
- Clicking a chat reference in the Foreman's transcript (e.g. `chat:<id>` or a rendered chat card) navigates directly to that chat in the sidebar.

### 2.3 Oversight Strip and Kill Switch

Inside the Foreman's main pane, a compact header strip shows:

- **Permission tier badge** (e.g. `Supervised`, `Delegated`, `Elevated`) — clickable to change the Foreman's permission mode.
- **Active fleet summary** — count of running chats, pending approvals, and aggregate session spend across Foreman-spawned chats.
- **Pause All / Stop All** control — immediately halts the Foreman and every currently running Foreman-managed chat.

---

## 3. Core Capabilities

### 3.1 Commanding Other Chats

The Foreman can inspect and drive any chat known to `CoderStore` and `App.swift`'s active `CoderRun` map:

- **List and inspect chats:** Read the sidebar inventory (`id`, `title`, `folder`, `created`, `updated`, `pinned`, `status` — idle / running / waiting-for-approval / error / goal-active, spend vs. budget, and last-turn summary).
- **Read chat transcripts:** Fetch bounded slices of any chat's transcript (last N turns, specific turn range, or final diff/summary) without loading entire multi-megabyte JSON files into the Foreman's context.
- **Send a message to an existing chat:** Enqueue a user-equivalent instruction into an idle chat (starting or waking its `CoderRun` headlessly via `cd_headless` if not currently focused) or queue a follow-up for a running chat. Messages sent by the Foreman are clearly attributed in the target chat's transcript (e.g. `From Foreman`) so the user can distinguish human messages from Foreman messages.
- **Lifecycle control:** Pause, resume, or stop a running chat; rename a chat; pin/unpin a chat; update a chat's thread goal (`cd_goal_set` / `cd_goal_set_status`) or USD budget (`cd_set_budget_usd_micros`) within the global Foreman budget ceiling.
- **Approval mediation:** Inspect pending gated `ask` prompts (`MWW_ASK`) from worker chats and either surface them to the user in a consolidated prompt or approve/deny them according to the Foreman's active permission tier (Section 4).

### 3.2 Creating and Spawning Its Own Chats

The Foreman can create new persistent chats in `CoderStore`:

- **Parameters:** `title`, `folder` (must be within allowed workspace roots unless user-approved), `prompt` (initial task message), optional `goal` (autonomous multi-turn objective), optional `budget_usd`, optional `policy` / `gated` mode, and optional `pin` flag.
- **Execution:** `App.swift` creates a new `CoderChat` entry in `index.json` with `managed_by = "foreman"` and `parent_id = <foreman_chat_id>`, allocates a headless `CoderPage` (`cd_headless`) and `CoderRun`, starts its worker, and immediately streams updates back to `CoderStore` and the sidebar.
- **Parallelism and caps:** A configurable concurrency cap (default: e.g. 4 concurrent Foreman-spawned runs) prevents runaway fan-out. Additional spawned chats queue cleanly or return a backpressure error to the Foreman.
- **Completion notifications:** When a Foreman-managed chat finishes, errors, hits its budget, or blocks on an `ask`, the host posts a compact status event back to the Foreman (waking the Foreman if it is waiting on children, or appending a status notice for the next turn).

### 3.3 Searching Across the Whole Repository

Unlike a regular chat scoped to a single task or subfolder, the Foreman maintains a workspace-level view:

- **Workspace roots:** Defaults to the current folder's git top-level root, with optional support for multiple registered repositories/folders in the Foreman's workspace list.
- **Fast bounded search:** Uses git-aware search (`git grep -n`, `rg` when available, `git ls-files`, `git status`, `git log`, `git diff`) with strict output caps (line count, byte limit, and file-range pagination) so searches across large monorepos remain fast and never blow up the Foreman's context window.
- **Cross-chat workspace awareness:** Can inspect `git status` and uncommitted diffs across all workspace roots (and worktrees, when used) before assigning two chats to edit overlapping files, reducing merge conflicts.

### 3.4 Searching Through Previous Chats

Coder Desktop persists every chat as `~/Library/Application Support/Coder/chats/<id>.json` alongside `index.json`.

- **Metadata filtering:** Filter chats by `folder`, date range (`created` / `updated`), `title`, `pinned`, `managed_by`, or active status.
- **Full-text search:** Search across past chat transcripts (user messages, assistant replies, tool summaries, file paths touched) returning ranked matches with `chat_id`, `title`, `updated`, `folder`, turn index, and a short bounded snippet.
- **Incremental local index:** To keep search instant as chat history grows to hundreds of chats and tens of megabytes, `CoderStore` maintains a lightweight local search index (e.g. SQLite FTS5 or an incrementally updated token/inverted index under `~/Library/Application Support/Coder/chats/search.idx`), updated atomically whenever `CoderStore.keep(...)` writes a chat and rebuilt automatically if missing or stale.
- **Recall by reference:** Once the Foreman identifies a relevant prior chat via search, it reads only the needed turn range via `foreman_read_chat` rather than dumping whole transcripts.

---

## 4. Permissions, Elevation, and Safety Model

Because the Foreman can control other chats, spawn work, and search broadly, its permissions must be **explicit, tiered, and auditable** rather than an all-or-nothing bypass.

### 4.1 Permission Tiers

| Tier | Read/Search Repo & Chats | Create & Command Chats | Approve Worker `ask` Prompts | Direct Shell / File Edits by Foreman |
|---|---|---|---|---|
| **1. Supervised** *(default)* | Allowed (read-only across workspace + chat store) | Allowed, but spawning or sending to a chat prompts user confirmation (or runs spawned chats in `gated: true` mode) | User must approve all gated prompts | Read-only; edits/commands gated |
| **2. Delegated** | Allowed | Allowed freely within concurrency & aggregate budget caps | May auto-approve low-risk read/build/test commands matching an explicit allow-list; destructive or external actions still ask the user | Gated for destructive commands; normal edits allowed |
| **3. Elevated** *(explicit opt-in)* | Allowed | Allowed freely within aggregate budget caps | May auto-approve worker commands within workspace roots (excluding hard-blocked invariants) | Unrestricted within workspace roots (still subject to hard invariants) |

### 4.2 Hard Safety Invariants (Never Bypassed in Any Tier)

Even in **Elevated** mode, the host (`App.swift` + `mcworker.coax`) enforces non-negotiable invariants:

1. **Aggregate budget ceiling:** The Foreman has a hard USD cap (`foreman_budget_usd_micros`) covering itself plus all chats it spawns or commands in a run. Reaching the cap pauses the Foreman and all managed chats.
2. **Workspace boundary:** Accessing or modifying paths outside configured workspace roots (or sensitive credential paths such as `~/.ssh`, `~/.aws`, keychain, or API keys) always requires explicit human confirmation.
3. **Irreversible / external actions:** Force-pushing (`git push --force`), deleting branches/repos, `rm -rf` outside build caches, or sending external network/API mutations require explicit human confirmation unless individually whitelisted by the user for that session.
4. **Human override & kill switch:** A single click (`Stop All`) or menu shortcut immediately terminates the Foreman's worker and every running Foreman-managed worker.
5. **No self-escalation:** The Foreman cannot programmatically raise its own permission tier, raise its own budget ceiling, or disable the audit log. Only the human user via native AppKit UI can change these settings. A program outside the Foreman chat holds a separate operator credential (`docs/coder/foreman-control-api.md`): it can lower the tier at any time, and can raise it only after a person turns on **Let Programs Raise the Tier** in the Foreman menu. The Foreman worker's own credential is refused on every tier and Foreman-lifecycle route (checked by `apps/coder-desktop/foreman-control-test.sh`).
6. **Complete audit log:** Every Foreman control action (chat created, message sent, chat stopped, approval granted/denied, budget changed) is appended to `~/Library/Application Support/Coder/foreman-audit.jsonl` with timestamp, Foreman turn ID, action, target chat ID, and outcome.

---

## 5. Architecture and Integration with Coder Desktop

Coder Desktop follows a strict architectural split (`apps/coder-desktop/README.md`, Ch. 40): **Coax owns UI layout, transcript state, and wire-protocol encoding (`page.coax`, `mcworker-wire.coax`); Swift (`App.swift`) is a thin native host owning AppKit windows, `CoderStore`, subprocesses, and OS capabilities.** The Foreman preserves this split.

### 5.1 Storage Extensions (`CoderStore` in `App.swift`)

Extend `CoderChat` in `index.json` with optional, backward-compatible fields:

```json
{
  "id": "foreman",
  "title": "Foreman",
  "role": "foreman",
  "created": 1763560000,
  "updated": 1763561200,
  "folder": "/Users/alice/work/openagents",
  "renamed": true
}
```

For chats created by the Foreman:

```json
{
  "id": "a1b2c3d4-...",
  "title": "Fix sidebar scroll jitter",
  "created": 1763561210,
  "updated": 1763561400,
  "folder": "/Users/alice/work/openagents",
  "managed_by": "foreman",
  "parent_id": "foreman",
  "foreman_archived": false
}
```

All new fields are optional so existing `index.json` files decode cleanly with zero migration loss.

### 5.2 Host-Bridged Foreman Control Plane

Rather than giving the Foreman raw filesystem write access to `~/Library/Application Support/Coder/chats/` (which would race with `CoderStore` and bypass permissions), the Foreman interacts with chats through **typed host-handled control requests** over the existing JSON-lines worker protocol (`coax/lib/agent/engine/mcworker-wire.coax`), analogous to how `MWW_DELEGATE` (`mww_delegate`) already hands sub-agent tasks from the worker to `App.swift` and receives results back.

Proposed Foreman tool / control surface (routed through `App.swift`):

| Operation | Arguments | Returns |
|---|---|---|
| `foreman_list_chats` | `filter` (`all` / `running` / `managed` / `pinned`), `folder`, `limit` | Array of `{id, title, folder, status, updated, spend_usd, budget_usd, pending_ask, managed_by}` |
| `foreman_read_chat` | `chat_id`, `from_turn`, `max_turns`, `max_bytes` | Bounded transcript slice + current status + pending `ask` details |
| `foreman_search_chats` | `query`, `folder`, `since`, `limit` | Ranked array of `{chat_id, title, folder, updated, turn, snippet}` |
| `foreman_search_repo` | `query`, `root`, `glob`, `mode` (`grep` / `files` / `git_log` / `git_status`), `limit` | Bounded matches with path, line number, and text |
| `foreman_create_chat` | `title`, `folder`, `prompt`, `goal`, `budget_usd`, `gated`, `pin` | `{chat_id, status}` (or error if cap/policy exceeded) |
| `foreman_send_chat` | `chat_id`, `message`, `wake` | `{ok, status}` |
| `foreman_control_chat` | `chat_id`, `action` (`stop` / `pause_goal` / `resume_goal` / `set_budget` / `rename`), `value` | `{ok, status}` |
| `foreman_resolve_ask` | `chat_id`, `ticket`, `approve`, `why` | `{ok}` (subject to active permission tier; returns `needs_human` if tier forbids auto-approval) |

### 5.3 Worker Protocol (`mcworker-wire.coax` and `mcworker.coax`)

- A Foreman worker is started with an explicit role flag in `mww_request` (e.g. `"role":"foreman"`, `"tier":"supervised"|"delegated"|"elevated"`).
- Regular worker runs omit `"role":"foreman"` and are never exposed to Foreman control tools — enforcing privilege separation at both the prompt/schema level and the host execution level.
- When the Foreman invokes one or more control operations in a turn, `mcworker` emits a structured control request line to `App.swift`, waits for the host's JSON response line (matching the request `ticket`), and continues its turn — reusing the exact ticketed request/response pattern already proven by `MWW_ASK` and `MWW_DELEGATE`.

### 5.4 UI Bridge (`page.coax` and `host.h`)

Minimal additions to the C/Coax bridge (`apps/coder-desktop/host.h` and `page.coax`):

- Reserve the pinned Foreman slot at the top of the sidebar layout in `page.coax`, fed by a dedicated `cd_foreman_status(page, running_count, pending_asks, tier, active)` call.
- Add a sidebar action code (`CP_ACT_OPEN_FOREMAN`) and stop-all action (`CP_ACT_FOREMAN_STOP_ALL`).
- Render `From Foreman` attribution badges on messages injected by `foreman_send_chat`, and clickable chat-link cards in the Foreman's transcript that emit `CP_ACT_OPEN_CHAT`.

---

## 6. Failure Modes and Edge Cases

- **Concurrent edits to the same files:** Two Foreman-spawned chats editing the same file in one working directory can overwrite each other. Mitigation: the Foreman's system prompt and `foreman_search_repo(mode: "git_status")` instruct it to serialize overlapping file edits or assign separate git worktrees when parallelizing.
- **Recursive Foreman spawning:** A Foreman-spawned chat is always a regular chat (`role != "foreman"`) and cannot invoke Foreman control tools or spawn another Foreman.
- **Large chat history performance:** Searching 500+ chats synchronously on the main thread would freeze the UI. Mitigation: chat search runs on a background queue against the incremental local index, with strict per-query time and byte budgets.
- **Orphaned runs on app quit:** Existing `applicationShouldTerminate` in `App.swift` already warns when agent tasks are running; Foreman-managed runs are counted in that same active-run set and cleanly terminated on quit.
- **Deleted or archived target chat:** If the user deletes a chat while the Foreman holds its `chat_id`, `foreman_read_chat` / `foreman_send_chat` return a clean `not_found` error rather than crashing or recreating it silently.

---

## 7. Phased Rollout and Verification

### Phase 1 — Read-Only Foreman (Sidebar + Search + Inspection)
- Pinned Foreman row in `page.coax` + `App.swift`, `⌘0` shortcut, persistent `foreman` chat in `CoderStore`.
- `foreman_list_chats`, `foreman_read_chat`, `foreman_search_chats` (with incremental index), and `foreman_search_repo`.
- Verification: unit tests on `CoderStore` search/index, smoke test verifying Foreman can locate a prior chat decision and a symbol across the repo without modifying any chat.

### Phase 2 — Supervised Orchestration (Spawn + Command)
- `foreman_create_chat`, `foreman_send_chat`, `foreman_control_chat`, `From Foreman` transcript attribution, `managed_by` sidebar badge, and `Stop All` kill switch.
- Aggregate budget tracking and `foreman-audit.jsonl` logging.
- Verification: end-to-end desktop eval (following `docs/coder/desktop-evals-spec.md`) where the Foreman spawns two parallel chats, monitors completion, and summarizes their results.

### Phase 3 — Tiered Permissions and Approval Mediation
- Permission tier selector (`Supervised` / `Delegated` / `Elevated`), `foreman_resolve_ask`, allow-list rules for `Delegated` mode, and hard invariant enforcement tests.
- Verification: negative tests proving the Foreman cannot self-escalate, exceed its aggregate budget, or auto-approve hard-blocked commands.
