# 43. The Foreman

> **Status: unimplemented — design draft.** The desktop app ([Chapter 40](ch040-the-desktop-app.md)) already runs multiple concurrent chats in the sidebar, and the worker protocol ([Chapter 41](ch041-worker-protocol-and-sub-agents.md)) already lets one chat delegate ephemeral sub-agents. This chapter designs the **Foreman**: a persistent supervisory chat pinned to the top of the Coder desktop sidebar that commands other chats, spawns new chats, searches across the whole repository and across previous chats, and operates under scoped elevated permissions. The full engineering specification lives in [`docs/coder/foreman-spec.md`](../../docs/coder/foreman-spec.md).

---

## Why a Foreman

[Chapter 40](ch040-the-desktop-app.md) gives you concurrent chats in a sidebar, and [Chapter 41](ch041-worker-protocol-and-sub-agents.md) gives each chat parallel sub-agents. Those two mechanisms still leave a gap once you have five or ten threads of work in flight across a repository:

- **Regular chats are isolated.** Chat A cannot see what Chat B is editing, what Chat C decided yesterday, or whether Chat D is blocked waiting for an approval.
- **Sub-agents are ephemeral.** A sub-agent spawned by `cd_agent_task` lives inside one turn of one chat. It has no sidebar row of its own, no durable transcript you can click into and steer, and no way to outlive its parent turn.
- **The Router ([Chapter 17](ch017-one-agent-many-tasks.md)) is headless.** It listens to GitHub issues and CI triggers in the background, not to a developer sitting in front of a desktop sidebar full of active chats.

The **Foreman** closes that gap. It sits at the top of the Coder desktop sidebar as a single, always-available supervisory chat. Instead of context-switching across six tabs to check progress, recall earlier work, or spin up follow-up tasks, you talk to the Foreman—and the Foreman coordinates the sidebar for you.

---

## Where the Foreman Lives in the Desktop App

In `apps/coder-desktop/page.coax`, the sidebar currently stacks `New Chat`, user-pinned chats (`Pinned`), and chronological chats (`Recents`). The Foreman adds a permanent, non-deletable slot pinned above `Pinned`:

```text
┌──────────────────────────────────┐
│  + New Chat                      │
├──────────────────────────────────┤
│  ★ Foreman            3 running  │  ← always pinned at top (⌘0)
├──────────────────────────────────┤
│  PINNED                          │
│    Fix Metal glyph cache         │
│  RECENTS                         │
│    [F] Update wire tests    ●    │  ← spawned by Foreman
│    [F] Audit book links     ●    │  ← spawned by Foreman
│    Refactor sidebar scroll       │
└──────────────────────────────────┘
```

Two design rules keep the Foreman legible rather than magical:

1. **Foreman-spawned chats are real sidebar chats.** Every chat the Foreman creates is stored in `CoderStore` (`~/Library/Application Support/Coder/chats/`) with its own `<id>.json` transcript and sidebar row, tagged with `managed_by: "foreman"`. You can click into any of them at any time to watch tool calls live, type a correction, or take over.
2. **Human intervention wins.** Messages the Foreman injects into another chat carry explicit `From Foreman` attribution, and a **Stop All** kill switch in the Foreman header immediately halts the Foreman and every running chat it manages.

---

## Five Capabilities

### 1. Commanding Other Chats

The Foreman can inspect every chat in `CoderStore` and every live `CoderRun` in `apps/coder-desktop/App.swift`:

- Read the status of all chats (idle, running, blocked on a gated `ask`, hit budget, or errored) and read bounded turn ranges from any chat's transcript.
- Send follow-up instructions to an existing chat—waking it headlessly via `cd_headless` if it is not the currently focused window chat.
- Pause, stop, rename, or adjust the thread goal (`cd_goal_set`) and USD budget (`cd_set_budget_usd_micros`) of managed chats within the Foreman's aggregate budget ceiling.

### 2. Spawning Its Own Chats

When a task breaks into independent workstreams—say, updating a wire parser, writing tests, and auditing docs—the Foreman creates new sidebar chats (`foreman_create_chat`) with a title, working folder, initial prompt, optional goal, and budget. A concurrency cap keeps fan-out bounded, and when a child chat finishes, errors, or blocks on an approval, the host notifies the Foreman.

### 3. Searching Across the Whole Repository

Regular chats focus on one task; the Foreman maintains workspace-wide situational awareness. It can run fast, bounded git-aware searches (`git grep`, file listings, `git status`, `git log`, `git diff`) across the active repository (or multiple registered workspace roots). Checking `git status` across the repo also lets the Foreman avoid assigning two parallel chats to edit the same files at once.

### 4. Searching Through Previous Chats

Every desktop chat is already persisted locally in `~/Library/Application Support/Coder/chats/` (`index.json` plus `<id>.json`). The Foreman queries an incrementally maintained local search index over that store—filtering by folder, date, title, or full-text content—and receives ranked snippets with exact `chat_id` and turn references. It then reads only the relevant turns instead of dumping whole transcripts into context.

### 5. Scoped Elevated Permissions

Orchestration requires more authority than a single worker chat, but never an unchecked blank check. The Foreman uses three user-selected permission tiers:

- **Supervised** *(default)*: Read-only search across repo and chat history; spawning or commanding chats is gated or requires confirmation; human approves all gated `ask` prompts.
- **Delegated**: Freely spawns and commands chats within concurrency and budget caps; may auto-approve allow-listed low-risk read/build/test commands from worker chats while escalating destructive or external actions to the user.
- **Elevated** *(explicit opt-in)*: Broad workspace execution and worker approval authority within configured workspace roots.

Hard invariants hold in every tier: an aggregate USD budget cap across the Foreman and all its chats, workspace-root boundaries, human confirmation for irreversible or external actions (such as force-pushes or credential access), no self-escalation of permission tier or budget, a one-click `Stop All` kill switch, and an append-only audit log (`foreman-audit.jsonl`).

---

## How It Fits the Existing Architecture

Coder Desktop keeps Coax in charge of UI state and wire encoding (`page.coax`, `mcworker-wire.coax`) and Swift (`App.swift`) in charge of native windows, `CoderStore`, and worker subprocesses. The Foreman extends that exact architecture without adding a background daemon:

- **Role separation on the wire:** `mww_request` in `coax/lib/agent/engine/mcworker-wire.coax` tags the Foreman worker with `"role":"foreman"` and its active permission tier. Regular chats never receive Foreman control tools, and a Foreman-spawned chat cannot spawn another Foreman.
- **Host-bridged control plane:** Instead of letting the Foreman touch `chats/*.json` directly on disk, Foreman control actions (`foreman_list_chats`, `foreman_read_chat`, `foreman_search_chats`, `foreman_search_repo`, `foreman_create_chat`, `foreman_send_chat`, `foreman_control_chat`, `foreman_resolve_ask`) travel over the same ticketed JSON-lines request/response channel already used by `MWW_ASK` and `MWW_DELEGATE` ([Chapter 41](ch041-worker-protocol-and-sub-agents.md)).
- **Backward-compatible storage:** `CoderChat` in `index.json` gains optional fields (`role`, `managed_by`, `parent_id`), so existing chat histories load without migration.

---

## What Comes Next

The rollout in [`docs/coder/foreman-spec.md`](../../docs/coder/foreman-spec.md) proceeds in three phases:

1. **Read-only Foreman:** Pinned sidebar entry, `⌘0` shortcut, repo-wide search, and indexed search/inspection across previous chats.
2. **Supervised orchestration:** Spawning and commanding sidebar chats, `From Foreman` attribution, aggregate budget enforcement, `foreman-audit.jsonl`, and `Stop All`.
3. **Tiered permissions:** `Supervised` / `Delegated` / `Elevated` selector, approval mediation for worker `ask` prompts, and negative security tests for hard invariants.
