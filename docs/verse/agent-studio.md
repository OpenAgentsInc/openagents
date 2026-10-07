# Agent Studio

Status: implemented, October 4, 2026. This document began as the
specification of a place in Verse where a team of coding agents does real work
on a real repository, and where people watch, steer, answer, and approve that
work by walking around. Any engine the Coder host can run works there, not
only Claude. The world borrows its idea from AgentCraft; the panels for
reading, answering, and reviewing are harvested from Zeron.

The [terminal workbench roadmap](../terminal/workbench-roadmap.md) adds a
shared application around these resources: Grid and standalone terminal
first, Everglade integration next. The studio stays the host-owned team
coordinator; opening a terminal at a station adds no execution or review
authority.

What runs today:

- A real goal ran end to end on a scratch host on October 4: a Codex lead
  (`codex/loop`) planned two tasks, a worker on the Claude Agent SDK
  (`claude/sdk`) asked two approvals and continued its turn after each, the
  change merged into the checkout without a push, and the lead's review
  finished the goal for $0.19.
- The host's studio coordinator releases planned tasks as their
  dependencies clear, gives each task its own worktree and branch, serves
  reviews at exact revisions, merges only finished changes, sends a
  conflicting merge back to its seat with the merge already started, and
  merges locally without pushing.
- `openagents studio` does every step from the command line (#10566), and
  [Everglade](everglade.md) does them on the desktop. Phones act through a
  paired computer (#10570, #10579).
- Seats choose their engine: `codex/loop`, `claude/loop`, `claude/session`,
  `codex/session`, or `claude/sdk` (#10568, #10571).
- The simulated team runs on a scratch host with no model spend
  (`openagents studio up --sim`). See [Simulated team](#simulated-team).

### Use it for day-to-day work

Seats edit files only. The host commits a seat's change when it lands, and
a conflict comes back with the merge already started, so seats never write
Git's shared directory.

```sh
OA=~/work/openagents-target-agent1/release/openagents   # a build from main
$OA studio up --repo ~/code/myproject --workspace myproject --full-access \
  --team "lead=codex/loop:gpt-6.1-sol,ada=claude/sdk:claude-opus-5-5,bob=codex/loop:gpt-6.1-sol"
$OA studio goal submit "Add a --verbose flag to the CLI" --workspace myproject
$OA studio watch                      # or walk Everglade: verse --everglade
$OA studio decisions                  # questions and approvals
$OA studio answer DECISION allow      # or text, or --always for a standing rule
$OA studio tasks                      # done tasks wait for review
$OA studio review TASK --diff
$OA studio merge TASK                 # lands in the checkout; push it yourself
$OA studio down
```

`--full-access` is needed for `session` and `sdk` seats; `loop` seats run
inside the host's boundary. Use a checkout no one else edits: a merge
refuses a dirty one.

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

## Why a place

> my strong suspicion is that the optimal way to manage multiple
> intelligences is that which is closet to our primordial state: wandering
> around a small town, seeing people we know well, physically embodied
>
> the cybersynesque dashboards feel productive, but i doubt they really are

— [Will Manidis](https://x.com/WillManidis/status/2106401558565134823), replying about AgentCraft

That is the bet behind the studio: managing several agents works better as
walking among them than as reading a wall of status. The world carries the
glanceable state (who is working, waiting, or stuck), and the panels hold the
detail for the one station the person is standing at.

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
| Coder V1 (`codex/coder:MODEL`, `claude/coder:MODEL`, or `vertex/coder:MODEL` for Coder on the OpenAgents Gateway with no login) | The default for a seat that names no engine (#10754): one turn of Coder V1 (`openagents coder chat --json`) in the task's own Coder session, `task-ID`, in its worktree; Coder chooses its provider (the Codex login, Claude Code's, then the OpenAgents Gateway). Full access only; under other access the seat runs Microcoder's loop, the loop Coder runs inside, under the host's boundary | [Workshop agent](workshop-agent.md#coder-v1-as-her-engine) |
| Codex (`codex:MODEL`) | Microcoder's loop on the Codex login by default (`coder.codex` is `loop`), or a `codex exec` session with `coder.codex session` | [Delegate door](../coder/runtime/delegate-door.md) |
| Claude Code (`claude:MODEL`) | A Claude Code session on the `claude` CLI login by default (`coder.claude` is `session`), or Microcoder's loop with Claude as the model | [Delegate door](../coder/runtime/delegate-door.md) |
| Devin | `devin acp` over the Agent Client Protocol | [Devin route](../coder/runtime/devin.md) |
| OpenCode | `opencode acp` | [OpenCode route](../coder/runtime/opencode.md) |
| Grok Build | `grok agent stdio` | [Grok route](../coder/runtime/grok.md) |

Microcoder's loop is not a route of its own. It is the loop mode of a `codex`
or `claude` route ([Microcoder](../coder/guides/microcoder.md)). Host settings
provide defaults; each seat can select its loop, session, or supported SDK
engine (#10568, #10571), or Coder V1 with `coder`, which a seat that names
no engine runs on (#10754). Session, SDK, and Coder seats require `full`
access. A seat's Coder session is in the Coder store, so `/resume` in your own
Coder, `coder --follow task-ID`, and `openagents coder sessions read task-ID`
show what the seat is doing. The
`claude/sdk` seat uses the in-repo
[`claude_agent_sdk`](../../crates/claude_agent_sdk/README.md) port and can
raise decisions that resume its running turn, as the scratch run above
records.

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

On the wire each intent's operation kind carries a `studio.` prefix, such as
`studio.goal.submit`, because `task.cancel` already names a NIP-HOST
operation. [NIP-HOST](../../nips/openagents/NIP-HOST.md#operations) lists the
operations, their fields, and their refusals.

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
path, which for a studio task merges locally: it builds the merge commit
off-tree, fast-forwards the branch the owner's checkout has checked out, and
pushes nothing. A dirty checkout, a detached one, or a conflict refuses the
merge with the reason and leaves the decision open
([`studio_git.rs`](../../crates/coder/src/task/studio_git.rs)); **Request changes** sends the reviewer's text back to the same seat as a
follow-up turn; **Reject** closes the task and keeps its worktree for
inspection until archive.

### Safety on real repositories

AgentCraft's rules carry over, enforced by the host rather than the view:

- Every task runs in its own worktree on its own branch
  (`studio/<seat>/<task>-<slug>`, under the host's state). No seat writes the
  owner's checkout.
- No seat lands or pushes. Git itself refuses every transport for a studio
  task's processes (`GIT_ALLOW_PROTOCOL`, `protocol.allow=never`,
  `GIT_CEILING_DIRECTORIES`, no inherited `GIT_DIR`-style variables). A
  landing happens only after a person with the
  `review` right chooses **Merge** at the reviewed revision. A host's existing
  auto-start policy does not imply auto-merge; studio landing is a separate,
  explicit decision.
- Steps that leave the worktree boundary become approval decisions through the
  existing interaction path. An approval answer never widens a grant.
- Seats commit as `Studio <Seat>`. Only the approved merge carries the
  person's identity, signed when their Git configuration signs commits.
- Smokes and tests run against a scratch host under a temporary `HOME`, and
  archive every task they create, as this repository's agent contract
  requires.

## The studio in Verse

The studio lives in [Everglade](everglade.md), a separately loaded
[zone](zones.md) under the [zone rules](zone-rules.md): a forest glade and
workshop built from CC0 Quaternius kits. Everglade's layout maps each station
below to a place in the glade:

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

The world shows where work is happening. Reading, answering, and deciding
happen in panels over the world, and those panels are the
[Zeron-derived interface](#interface-harvested-from-zeron) the desktop app
already uses, not new Verse HUD drawing:

- **Console.** The ported command palette and composer. Plain text starts a
  goal; `@seat text` messages a seat; `/answer`, `/diff`, `/status`,
  `/pause`, `/resume`, `/stop`, and `/repos` mirror the intents. Seat names
  complete with Tab on desktop.
- **Seat panel.** A seat's live transcript with tool rows, its plan, and its
  engine and usage strip, opened by selecting a seat or its desk.
- **Decisions.** The oldest open decision first, as a paged question flow with
  the asking seat, the question, its options, and free text.
- **Diff review.** The ported "What changed" card and diff pane, with the
  seat's summary, the lead's review notes, and line comments for **Request
  changes**.

On desktop, Everglade reads the studio of the host on the same computer
through the control socket the desktop app uses, only while the player is in
the glade (`verse::zones::everglade::studio::live`). That socket's peer is the
host's owner; a panel still offers only the intents the source's rights
allow, and every studio panel shows the host's last answer or refusal code.

When a decision opens and the client is not in the studio, the phone gets a
push through the existing [push gateway](../deployment/push-gateway.md) wake
constants, and the desktop app raises a notification.

### Multiplayer

The first release is single-owner: the people who see a studio are the
devices enrolled on its host. Seats appear to those devices only, through the
host connection. Publishing seats as NIP-MV `agent` entities on the shared
plaza, so other players see an owner's team at work without seeing its logs,
is a later step that needs its own disclosure decision.

## Interface: harvested from Zeron

The studio's 2D interface is harvested from
[Zeron](https://github.com/zeronsh/zeron) (MIT, studied at commit
`9e1a1115`), a Rust desktop app that controls Claude Code, Codex, Cursor,
Devin, Grok, Hermes, Pi, OpenCode, and Antigravity sessions. Zeron already
solves the 2D half of this problem for one session at a time: a transcript
that streams tool calls, a composer that answers agent questions, a sidebar
ranked by which session needs attention, and a diff view per checkout. The
studio puts many such sessions in one place and adds the parts Zeron lacks:
a coordinator, approvals, and merge decisions.

### What is already ported

Most of Zeron's chat interface is already reimplemented for the OpenAgents
desktop app, from the
[port audit](../research/2026-09-29-comet-desktop-ui-port-audit.md) to the
[fidelity closeout](../desktop/verification/2026-09-30-zeron-fidelity/closeout.md)
(#10029), against Zeron `50cf9e97`. The studio reuses these modules rather
than drawing its own:

| Zeron element | OpenAgents module | Shared with phones |
| --- | --- | --- |
| Palette, translucent inks, syntax colors, heading and code metrics | [`openagents-chat-app/src/visual.rs`](../../crates/openagents-chat-app/src/visual.rs) | Yes |
| "What changed" card and unified diff pane | [`openagents-chat-app/src/changes.rs`](../../crates/openagents-chat-app/src/changes.rs) | Yes |
| Engine and usage strip | [`openagents-chat-app/src/engine.rs`](../../crates/openagents-chat-app/src/engine.rs) | Yes |
| Transcript tool rows | [`openagents-chat-app/src/coder_run.rs`](../../crates/openagents-chat-app/src/coder_run.rs) | Yes |
| Composer editing model | [`rust-native/src/edit.rs`](../../crates/rust-native/src/edit.rs) | Yes |
| Composer field, Markdown painting, transcript | [`rust-native-desktop/src/composer.rs`](../../crates/rust-native-desktop/src/composer.rs), [`rich.rs`](../../crates/rust-native-desktop/src/rich.rs), [`transcript.rs`](../../crates/rust-native-desktop/src/transcript.rs) | Desktop renderer |
| Solar icons and menu shortcut badges | [`rust-native-desktop/src/solar.rs`](../../crates/rust-native-desktop/src/solar.rs) | Desktop renderer |
| Shell, sidebar, titlebar | [`openagents-desktop/src/chrome.rs`](../../crates/openagents-desktop/src/chrome.rs) | No |
| Command palette, chat and profile menus, archive confirmation | [`openagents-chat-app/src/command_panel.rs`](../../crates/openagents-chat-app/src/command_panel.rs), key caps in [`rust-native-desktop/src/paint.rs`](../../crates/rust-native-desktop/src/paint.rs) | Yes |
| Rename dialog, profile footer | [`openagents-desktop/src/chat.rs`](../../crates/openagents-desktop/src/chat.rs) | No |

Solar icons are the only Zeron-sourced assets in the repository, with
their notice. Text uses Paper Mono, the typeface every surface uses.

Verse shares none of this today. Its HUD
([`crates/verse/src/hud.rs`](../../crates/verse/src/hud.rs)) draws with its
own batch, atlas, and amber palette, and Verse uses Rust Native only for
surface lifetime. The studio closes that gap: studio panels are Rust Native
views from the modules above, composited over the world surface on desktop
and mounted by the phone hosts as the chat screens already are. Shell-only
pieces in `openagents-desktop` that the studio needs moved into a shared
crate instead of being copied into `verse`: the palette and menus are in
`openagents-chat-app` (#10466). The world geometry uses Everglade's own
textured palette, and the panels use the chat palette, as they do in the
desktop app.

### What to harvest next

These Zeron pieces are not ported yet. Paths are in the Zeron checkout.

| Zeron source | Studio use | Notes |
| --- | --- | --- |
| `crates/ui/src/shell/spaces.rs`, `crates/proto/src/entities.rs` (`ChatIndicator`) | Seat roster ordered by attention: awaiting input, errored, working, completed and unseen, idle | Drives the roster and the world's lamps from one value. |
| `crates/proto/src/view.rs` (`effective_indicator`, `SESSION_STALE_MS` of 45 seconds) | A seat with no event for a bounded time shows as stale, never working | Prevents a crashed engine from looking busy forever. The bound is ours to measure. |
| `crates/ui/src/composer.rs` (`QuestionFlow`, `Wizard`) | Decision panel: paged questions, number keys select, single-select advances | Over the host's `interaction.rs` questions. Ported in [`openagents-chat-app/src/decision.rs`](../../crates/openagents-chat-app/src/decision.rs) (#10469). |
| `crates/ui/src/comments.rs`, `comment_ui.rs` | Line comments on a review, sent with **Request changes** | The model is in [`review_comments.rs`](../../crates/openagents-chat-app/src/review_comments.rs) (#10470); no platform draws it yet. |
| `crates/ui/src/todo_panel.rs` | A seat's plan, and a Task Wall card's detail | Ported as [`openagents-chat-app/src/plan_panel.rs`](../../crates/openagents-chat-app/src/plan_panel.rs) over [`openagents-chat/src/plan.rs`](../../crates/openagents-chat/src/plan.rs) (#10471); the desktop chat shows it. |
| `crates/ui/src/shell.rs` (`RightSurface::Subagent`, `SideChat`) | Seat panel tabs, including an engine's own subagents | Tab state is in [`openagents-chat-app/src/subagents.rs`](../../crates/openagents-chat-app/src/subagents.rs), whose module documentation lists which engines' traces identify a spawn. |
| `crates/ui/src/change_requests.rs` | Landing and pull request state after **Merge** | Display only. |
| `crates/ui/src/sound.rs`, `notify.rs`, `assets/sounds/` | The podium bell and desktop notifications for done, request, and attention | Prefer original sounds; a copied file needs the MIT notice. |
| `crates/ui/src/loaders.rs`, `motion.rs` | Working indicators and panel motion | The closeout lists motion as missing. |

Not harvested, consistent with the audit's "don't port" list: the file editor
and tree, the git history graph, the embedded browser, harness pickers, theme
import, and Mermaid. A seat's terminal, if needed later, uses the existing
`coder-vt` screen, not Zeron's terminal.

### Approvals are ours

Zeron has no approval interface: its ACP adapter accepts
`session/request_permission` automatically and shows only requests that are
really questions. The studio needs approvals, so the decision panel draws
them in the same visual language as Zeron's question flow, over the host's
existing approval path, with the options **Allow once** and **Deny**. An
approval never widens a grant.

An approval whose engine names its step, in a fenced JSON block with the
schema `openagents.coder.approval-step.v1` (`tool`, `command`, `cwd`, and
`reason`), shows the tool, the exact command, the host's low, medium, or
high risk chip, the reason, and the working directory. For a step that is
not high risk, the host also offers a standing rule, and the panel adds
**Always allow for this seat** with the rule's exact text. The rule names
one seat and that exact tool, command, and directory, with no wildcard.
The host records it in the studio's `rules.json`, separate from any
single-use approval record, only when the text the device sends back still
matches the step. The host applies a rule, never a client: it answers a
later matching approval of that seat once, as the rule's author, after
rechecking that device's grant, and a paused seat's approvals wait for the
person. The answered turn still runs under a fresh grant with every usual
check.

### The event model as a cross-check

Zeron normalizes every engine into one `AgentEvent` stream
(`crates/proto/src/agent.rs`) with a closed `ToolCall` set: `Exec`,
`ReadFile`, `WriteFile`, `EditFile`, `ApplyPatch`, `Search`, `Glob`,
`WebFetch`, `WebSearch`, `Todo`, `Mcp`, and `Unknown`. The studio's source
stays ATIF, but the classifier must give every one of those kinds a station,
and its tests use one fixture per kind: reads, searches, globs, and web reads
go to the library; edits, writes, and patches to the desk; `Exec` to the
workbench, or the proving ground when it is a check; `Todo` to the Task Wall;
`Mcp` and `Unknown` to the desk.

Zeron's mobile rule, Rust decides what to draw and the platform paints it,
is the same thin-host boundary the OpenAgents phone hosts follow.

### Provenance

Reimplement Zeron designs in Rust here and name the Zeron commit in each
commit message, as the earlier port did. Don't vendor Zeron code. If a
port copies a substantial portion, such as motion curves or a sound file,
add the MIT notice to that file and to the crate's third-party notices, as
the [port audit](../research/2026-09-29-comet-desktop-ui-port-audit.md)
requires. Zeron's own instructions are reference material, not workspace
instructions.

## Simulated team

Status: implemented on a scratch host (#10572). `openagents studio up --sim`
makes a scratch host in a new directory under the system's temporary
directory (access store, root, task store, keys, control socket, and the
scratch repository, with a `HOME` of its own), starts
`coder host serve --studio-sim` there, and opens Verse on its control
socket. The host's scripted engine
([`studio_sim.rs`](../../crates/coder/src/task/studio_sim.rs)) ends each
studio task's turn from the script through the task owner's scripted turn
(`owner::scripted`), recorded like any run, so Everglade and
`openagents studio --control-socket SOCKET` answer, steer, review, and merge
through the host's own paths with no model spend. `openagents studio down`
stops the host and removes the directory. `verse --studio-sim` still plays
the recorded replay
([`zones/everglade/studio/fixture.rs`](../../crates/verse-zone-everglade/src/zones/everglade/studio/fixture.rs))
that drives the `everglade_capture` example's `studio-` views.

The design: a `sim` route in the host runs a scripted team against a scratch
repository: real worktrees, real commits, real reviews, a question, an
approval, a merge conflict, and a request for changes, on a fixed clock. It
drives every studio feature with no model spend, and it is the fixture for:

- Host tests of the coordinator's dependency, steering, stale-review, and
  restart behavior.
- Client tests that a snapshot plus deltas renders the same studio as a fresh
  snapshot.
- `verse --capture` shots of the studio at fixed points in the script, the
  Verse equivalent of AgentCraft's screenshot gallery.

The simulated route is a host test fixture, never a route an auto-start policy
or a person can select on a real repository. The scripted engine runs only
with `serve --studio-sim`, which refuses an access store, root, or task store
under `~/.openagents` and the keychain; only on a root a scratch host marked;
only on a task store that admits scripted turns, which this computer's own
store never does; and never while the root's auto-start policy is on. A real
engine's admission is unchanged.

The [shared-sheet acceptance receipt](verification/2026-10-06-studio-workbench/README.md)
retains a scripted goal, question, exact review, and local merge with matching
opening contexts and archived scratch tasks. Native and real-engine
qualification remain separate owner checks.

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
6. **Zeron panels.** Move the palette and menus out of `openagents-desktop`
   into a shared crate, composite Rust Native panels over the Verse surface,
   and harvest the attention indicator, stale rule, question flow, approval
   panel, review comments, and plan panel. Then mount them in the phone
   hosts.
7. **Mixed-engine acceptance.** One goal on a scratch repository with a
   Codex lead and Claude Code, OpenCode, and Microcoder-loop workers: at least two
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

A separately admitted worktree shell opens with `terminal-app --host HOST
--task TASK` or Verse's `--terminal-host HOST --terminal-task TASK`. The host
requires the current terminal grant and an interactive studio worktree binding;
read-only tasks refuse the shell. The pane shows its host, task, generation,
and admitted directory. Shell edits invalidate earlier exact tree reviews.
Detach or close affects the shell and does not cancel or steer the task.
