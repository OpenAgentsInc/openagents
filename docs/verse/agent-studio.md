# Agent Studio

Status: proposed specification, October 4, 2026. Nothing in this document is
implemented yet. It describes a place in Verse where a team of coding agents
does real work on a real repository, and where people watch, steer, answer,
and approve that work by walking around. Any engine the Coder host can run
works there, not only Claude.

## Reference: AgentCraft

[AgentCraft](https://github.com/blendi-remade/agentcraft) (MIT, studied at
commit `0be815d`) is a Minecraft mod plus a Node and TypeScript orchestrator
called the Foreman. It turns a multi-agent Claude run into a building the user
walks through:

1. The user types a goal into an in-game console.
2. A lead agent reads the repository, writes a plan into shared memory, and
   pins tasks with dependencies to a Task Wall.
3. Worker agents walk to desks and code in their own Git worktrees. Each
   desk's monitor streams the worker's live log: reads, tool calls, test runs,
   and diffs.
4. When a decision belongs to the user, the agent walks to a podium, a bell
   rings, and a marker appears. Questions, permission prompts, and merges
   share one decision queue.
5. The user reviews the real diff in a review screen and chooses **Merge**,
   **Request changes**, or **Reject**. Nothing lands without that choice, and
   nothing is pushed.

The design rules that make it usable for daily work, and that this
specification keeps:

- **The orchestrator is the source of truth; the game is a view.** Agents keep
  working while the game is closed. On reconnect the view receives a full
  snapshot and rebuilds.
- **The view sends intents, never state.** It holds no business logic.
- **Location is derived from activity.** Reading and searching put an agent in
  the library, editing at its desk, tests at the test bench, and a question at
  the podium. Nobody scripts the walking.
- **Information reads at three distances.** Lamps and beacons from far away,
  nameplates and cards closer, monitors and diffs up close.
- **A deterministic simulated team** exercises every feature with real Git
  edits, for demos and screenshot checks, without model spend.

What this specification does not take from AgentCraft: Minecraft, Fabric,
Java, Node, TypeScript, the Claude Agent SDK as the only engine, its
persistence format, or its code. The design is reimplemented here in Rust
against existing OpenAgents crates. The AgentCraft checkout's own instructions
are reference material, not workspace instructions.

## Goals

- A goal typed in Verse becomes a plan, tasks, worktrees, reviewed diffs, and
  landed commits on a repository the host owner admitted.
- Every engine the Coder host already routes is a studio worker: the
  Microcoder loop, Codex, Claude Code, Devin, OpenCode, and Grok Build. A team
  can mix engines, and a seat can change engine between tasks.
- The studio is a view over the host's durable task state. Closing Verse,
  losing the network, or restarting the host loses no work.
- The same studio works on desktop, iOS, and Android, and in the desktop app's
  Play mode, through the shared Verse runtime.
- Watching needs only the `observe` right. Starting and steering work needs
  `operate`. Approving a landing needs `review`.

Non-goals for the first release: a public multiplayer studio shared with
strangers, paid labor, agents outside the owner's admitted hosts, and arbitrary
creator-built studios.

## Engine neutrality

AgentCraft wires its team to one SDK. Here the studio never talks to an engine.
It talks to the Coder host, and the host runs whatever route the task records,
through the existing paths:

| Engine | How the host runs it today | Reference |
| --- | --- | --- |
| Microcoder loop | In process, on the first provider with capacity | [Microcoder](../coder/guides/microcoder.md) |
| Codex | Codex login and the Codex Responses transport, or the loop | [Delegate door](../coder/runtime/delegate-door.md) |
| Claude Code | `claude` CLI login, or the loop | [Delegate door](../coder/runtime/delegate-door.md) |
| Devin | `devin acp` over the Agent Client Protocol | [Devin route](../coder/runtime/devin.md) |
| OpenCode | `opencode acp` | [OpenCode route](../coder/runtime/opencode.md) |
| Grok Build | `grok agent stdio` | [Grok route](../coder/runtime/grok.md) |

The studio adds no engine adapter. A new engine becomes a studio worker by
becoming a Coder route. Routes, capacity, and failover stay in the
[auto-start policy](../coder/runtime/host-autostart.md#routes-and-capacity)
and the capacity book. The studio shows which route a seat uses and when a
provider refused for a usage or rate limit; it never chooses one around the
host's policy.

To animate any engine the same way, the studio reads one normalized activity
stream instead of engine-specific logs. Each running task already records an
[ATIF trajectory](../coder/runtime/traces.md), and every engine writes the
same step and call shapes. The studio classifies each step into an
**activity**:

| Activity | ATIF evidence | Station |
| --- | --- | --- |
| `reading` | File reads, searches, knowledge retrieval | Library |
| `editing` | Patches and file writes | Desk |
| `running` | Shell commands that are not checks | Workbench |
| `testing` | Acceptance tests, verifiers, local checks | Proving ground |
| `judging` | A Jev, Kev, or Lev decision call | Oracle |
| `thinking` | A model step with no tool call yet | Desk |
| `waiting` | The task's summary phase is `Waiting` | Podium |
| `blocked` | A refusal, a missing grant, or no provider with capacity | Lounge |
| `done`, `failed` | The task's summary phase is final | Task Wall |

This generalizes the mapping in
[`crates/verse/src/replay.rs`](../../crates/verse/src/replay.rs), which
already places a retained Microcoder run's model steps at the workbench, Jev
questions at the oracle, knowledge at the library, and tests at the proving
ground. A live studio and a replay must classify a step the same way, so the
classifier moves to one shared function that both call. An unknown step kind
classifies as `thinking`, never as a guess at another station.

## Architecture

```text
 Verse client (desktop, phone, desktop Play)        Coder host (coder host serve)
 ├─ studio zone: stations, seats, Task Wall    ◄──► ├─ task owner: tasks, worktrees, grants
 ├─ HUD: console, decisions, diff review            ├─ studio coordinator: goals, plan, deps
 └─ engine presentation over studio snapshots       ├─ engines through existing routes
                                                    └─ ATIF traces, reviews, landing
            NIP-HOST over NIP-REACH direct channel or the relay
```

### The host owns state

The **studio coordinator** is a new module in the Coder host, built on
`coder::task`. It owns only what the task store does not already own:

- **Goals.** A goal is the text a person submitted, the repository, the lead
  seat, and the goal's tasks. Progress is the fraction of its tasks that are
  final.
- **Dependencies.** A task starts only when the tasks it depends on are
  `done`. The coordinator submits a task to the existing inbox when its
  dependencies clear; until then it holds a plan entry, not a queued task.
- **Seats.** A seat is a named studio role (lead or worker) bound to a route,
  a look, and a desk. Seats are configuration, not engines.
- **Shared memory.** The lead's plan and decisions, and repository
  conventions, as entries every seat's briefing includes. These are local
  first and can later publish as [NIP-KB](../../nips/openagents/NIP-KB.md)
  entries.
- **Messages.** Seat to seat and seat to person. A message to a running task
  arrives through the existing steer path, never by editing its prompt.

Everything else is already a host responsibility and stays there: worktrees
and branches ([`worktrees.rs`](../../crates/coder/src/task/worktrees.rs)),
questions and approvals
([`interaction.rs`](../../crates/coder/src/task/interaction.rs)), reviews at
exact revisions ([`review.rs`](../../crates/coder/src/task/review.rs)),
landing ([`landing.rs`](../../crates/coder/src/task/landing.rs)), execution
grants, `coder-boundary` write limits, and task archive. The coordinator
persists with the task store's atomic private files and survives restart like
a task does.

The lead is an ordinary task whose output is a plan: a bounded, typed list of
tasks with titles, descriptions, dependencies, and suggested seats. The
coordinator validates the plan (acyclic dependencies, a task count bound,
known seats) before it creates anything; a plan that fails validation becomes
a decision for the person, not a partial plan. Any engine can lead.

### The client is a view

Verse never runs an engine, opens a worktree, or reads a repository. It sends
intents and draws snapshots, under the
[Verse Engine authority rule](engine/architecture.md#runtime-and-authority-contracts)
that human, local agent, and remote agent input submit the same commands and
presentation never changes state.

Studio intents, each a NIP-HOST operation checked against the device's grant:

| Intent | Right | Effect |
| --- | --- | --- |
| `goal.submit` | `operate` | Start a lead task for a goal on an admitted repository. |
| `seat.message` | `operate` | Message a seat, or all seats. |
| `seat.pause`, `seat.resume`, `seat.stop` | `operate` | Pause keeps the task; stop returns it to the board. |
| `task.reassign`, `task.cancel`, `task.retry`, `task.prioritize` | `operate` | Steer a planned or running task. |
| `decision.answer` | `operate` | Answer a question or an approval with the existing `answer` command. |
| `review.open` | `observe` | Read a task's review: files, hunks, counts, revisions. |
| `merge.decide` | `review` | **Merge**, **Request changes**, or **Reject** a task at the reviewed revision. |

Host to client, one full `studio.snapshot` on connect and then
`studio.update` deltas, each with a monotonic sequence so a client that misses
one asks for a fresh snapshot. A snapshot carries goals, seats (activity,
station, task, route, look), tasks (status, dependencies, assignee),
decisions, repository summaries, and a bounded log tail per seat. Log entries
are display text derived from ATIF steps under the host's disclosure policy;
the client never receives raw tool output that `observe` does not allow.

A merge decision binds to the review's three revisions. If the worktree
changed after the review was read, `merge.decide` is refused as stale and the
client reloads the review. **Merge** hands the task to the existing landing
path; **Request changes** sends the reviewer's text back to the same seat as a
follow-up turn; **Reject** closes the task and keeps its worktree for
inspection until archive.

### Safety on real repositories

AgentCraft's rules carry over, enforced by the host rather than the view:

- Every task runs in its own worktree on its own branch. No seat writes the
  owner's checkout.
- No seat lands or pushes. A landing happens only after a person with the
  `review` right chooses **Merge** at the reviewed revision. A host's existing
  auto-start policy does not imply auto-merge; studio landing is a separate,
  explicit decision.
- Steps that leave the worktree boundary become approval decisions through the
  existing interaction path. An approval answer never widens a grant.
- Seats commit under a seat identity. Only the approved landing carries the
  person's identity, as the landing path already decides.
- Smokes and tests run against a scratch host under a temporary `HOME`, and
  archive every task they create, as this repository's agent contract
  requires.

## The studio in Verse

The studio is a building on the plaza, reached like the Gym building, with its
interior as a separately loaded [zone](zones.md) under the
[zone rules](zone-rules.md). It reuses the replay landmarks where they already
exist and adds the rest:

| Station | Shows | Interaction |
| --- | --- | --- |
| Goal atrium | Goal text, progress ring, open decision count | Open the console |
| Task Wall | Columns for planned, running, review, done, and blocked | Select a card for details and task intents |
| Desks | One per seat, with a monitor streaming its log tail | Select the seat for its card |
| Workbench | Seats running commands | None |
| Library | Shared memory and the lead's plan | Read entries |
| Oracle | Seats waiting on a decision model | None |
| Proving ground | Check lamps per task | Read the last check result |
| Podium | Seats waiting on a person; a beacon and a sound when one arrives | Answer the oldest decision |
| Merge station | Tasks ready for review | Open the diff review and decide |
| Lounge | Paused, stopped, and blocked seats, with the reason | Resume |

Seats are drawn as Verse agents, with the existing companion pipeline and
original character assets, and walk between stations with the shared
navigation. A seat's nameplate shows its name, activity, and route, so a
Codex seat and a Claude Code seat on the same goal are distinct at a glance.
Movement is presentation of the activity stream; it never gates or delays
work. When a seat changes activity faster than it can walk, it skips ahead to
the latest station.

The HUD adds three panels, built as Rust Native views so the phone hosts use
them without new platform code:

- **Console.** Plain text starts a goal; `@seat text` messages a seat;
  `/answer`, `/diff`, `/status`, `/pause`, `/resume`, `/stop`, and `/repos`
  mirror the intents. Seat names complete with Tab on desktop.
- **Decisions.** The oldest open decision first, with the asking seat, the
  question, its options, and free text.
- **Diff review.** File list, line numbers, collapsed context, the seat's
  summary, and the lead's review notes, from the host's review read.

When a decision opens and the client is not in the studio, the phone gets a
push through the existing [push gateway](../deployment/push-gateway.md) wake
constants, and the desktop app raises a notification.

### Multiplayer

The first release is single-owner: the people who see a studio are the
devices enrolled on its host. Seats appear to those devices only, through the
host connection. Publishing seats as NIP-MV `agent` entities on the shared
plaza, so other players see an owner's team at work without seeing its logs,
is a later step that needs its own disclosure decision.

## Simulated team

A `sim` route in the host runs a scripted team against a scratch repository:
real worktrees, real commits, real reviews, a question, an approval, a merge
conflict, and a request for changes, on a fixed clock. It drives every studio
feature with no model spend, and it is the fixture for:

- Host tests of the coordinator's dependency, steering, stale-review, and
  restart behavior.
- Client tests that a snapshot plus deltas renders the same studio as a fresh
  snapshot.
- `verse --capture` shots of the studio at fixed points in the script, the
  Verse equivalent of AgentCraft's screenshot gallery.

The simulated route is a host test fixture, never a route an auto-start policy
or a person can select on a real repository.

## Delivery

Each stage lands as its own issue, with its own checks.

1. **Shared classifier.** Move the replay's event-to-place mapping into one
   function over ATIF steps that returns an activity and a station. Replay and
   tests use it.
2. **Coordinator.** Goals, plan validation, dependencies, seats, messages, and
   shared memory in the host, with persistence and restart tests, driven by
   `openagents` CLI commands before any Verse work.
3. **Studio protocol.** The intents and the snapshot and update shapes as
   NIP-HOST operations, with rights checks and refusal codes, in the host and
   its client feature.
4. **Simulated team.** The scripted route and scratch repository fixture.
5. **Studio zone.** The building, stations, seats, and walking, from
   snapshots, on desktop.
6. **HUD panels.** Console, decisions, and diff review as Rust Native views,
   then the phone hosts.
7. **Mixed-engine acceptance.** One goal on a scratch repository with a
   Codex lead and Claude Code, OpenCode, and Microcoder workers: at least two
   tasks run in parallel, one question and one approval are answered in Verse,
   one change is requested and fixed, and the owner merges from the review
   screen. The client restarts mid-run and the host restarts mid-run without
   losing state.

## Open questions

- Whether the lead's plan should be its own typed question set answered by
  Jev before tasks are created, for example "is this task independently
  testable", with a measured baseline.
- How many seats a host runs at once by default, given that the auto-start
  policy caps running tasks at eight.
- Whether a goal can span hosts, with seats on the Mac and on a CoderOS
  machine, and how a merge then serializes across them.
- Whether seat identities become NIP-XP-earning agents, so studio work counts
  toward an agent's level the way Gym work does.
- The disclosure rule for showing seats to other players on the plaza.
