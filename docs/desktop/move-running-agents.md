# Move running agents without stopping them

Status: built on code, 2026-10-17 (see [As built](#as-built)). Owner request: updating or restarting the desktop app
must not kill every running agent. Running agents can be moved, all at once, to
the background on this computer or to a cloud computer through the Computers SDK.
Delivery: [#11930](https://github.com/OpenAgentsInc/openagents/issues/11930)–[#11933](https://github.com/OpenAgentsInc/openagents/issues/11933).

## Today

| Fact | Where |
| --- | --- |
| The desktop app connects to an engine already serving the IPC port; otherwise it **embeds the engine in its own process**. The owner's everyday app embeds. | `apps/openagents/crates/ui/src/state.rs` (`InProcessEngine`, `connect_ws`) |
| Every agent (Claude Code, Codex, ACP, SDK runner) is a child of the engine, in its own process group, killed when the engine drops it. | `apps/openagents/crates/harness/src/process/owned.rs`, `kill_on_drop` |
| `openagents headless` plus `openagents daemon install` already runs the engine as a LaunchAgent or systemd user unit. An installed service restarts into a new binary only when no run or terminal is live. | `apps/openagents/apps/openagents/src/daemon.rs`, `crates/update/src/lib.rs` `restart_when_superseded` |
| On boot, the engine marks interrupted runs "Run interrupted by engine restart — resuming". It re-sends the last user message on the remembered harness session, at most three times and only within 12 hours. | `apps/openagents/crates/engine/src/sessions.rs` `recover_stale` |
| Cloud computers, files, commands, stop and wake exist in the Computers API and Rust client. SDK file writes go up to 4 MiB each. | [Computers API](../sdk/computers.md) |
| A chat has one home, and changing it creates a linked continuation. A headless engine on a cloud computer and credentials by reference are open work. | #11898, #11852, #11813 |

So quitting the app ends every turn in flight. Auto-resume restarts the turn
from the last user message, so finished tool work is redone, side effects such
as pushes or deploys can repeat, and the work is lost after three restarts or 12 hours.

## Goals

1. Quit, update or crash the desktop window without touching running agents.
2. Update the engine itself without ending running turns.
3. Move every running chat, or a chosen set, to a cloud computer in one action,
   and bring it back later.
4. Agents, including Foreman, can do all of the above through tools, not only the UI.

Not goals: moving a live process's memory from this Mac to another machine,
copying provider logins or other secrets onto cloud disks, and changing the frozen
Coder 1.0.0 binaries.

## Design

### 1. Run keepers: agents outlive the engine

Spawn each harness run under a small **keeper** process: the same app binary with
a hidden `keeper` subcommand. It isn't a new install.

- The keeper starts in its own session (`setsid`), outside the engine's process
  group, and holds the harness's stdin and stdout and its process group.
- It appends every harness frame, with a sequence number, to the run's journal,
  and serves one Unix socket in the data folder (mode 0600). A token in a 0600
  file next to the socket authenticates the engine.
- The engine talks to the harness only through its keeper: it sends stdin frames,
  receives frames from an acknowledged sequence number, and can request interrupt
  or kill. Harness protocols are already line-framed stdio, so adapters don't change.
- When the engine goes away, the keeper keeps the agent running and keeps
  journaling. A new engine on boot lists keeper sockets, adopts each run, replays
  frames after its last acknowledged sequence, and re-shows any pending permission
  question. `recover_stale` skips adopted chats and stays the fallback for runs
  whose keeper also died.
- With no engine for 24 hours, a keeper interrupts its agent gracefully and
  exits, keeping the journal.
- The injected `openagents mcp` bridge reconnects to the IPC port when the engine
  restarts, so agents' app tools keep working across the handoff.
- Version skew: the keeper protocol is small and versioned, and an engine must
  adopt keepers from its previous release. Terminals (PTYs) get the same
  treatment as a second step.

Engine update becomes a handoff. The old engine stops taking new dispatches,
flushes, releases the data-folder lock and exits. The new engine takes the lock
and adopts the keepers. A failed adoption falls back to today's recovery,
reported per chat.

This alone fixes the owner's case for the embedded engine too: quitting the app
ends the engine but not the agents, and the next launch adopts them.

### 2. Background engine by default

On launch, the desktop app makes sure the background engine is installed and
running for the same data folder, then attaches over IPC instead of embedding.
That means installing the LaunchAgent with the app's environment and the data
folder written into the bundle.

- Closing the last window leaves agents running. A menu-bar item shows "N agents
  running" with **Open**, **Stop all and quit**, and **Move all to a cloud computer**.
- **Move to background**, for an app that is still embedding: start the background
  engine. The embedded engine then hands off as in section 1, so its keepers move
  to the background engine with no interruption.
- The service needs the same OpenAgents sign-in (the Keychain entry for the app's
  bundle) and the harness logins. A login it can't read is shown as a plain error
  before the handoff, never discovered mid-run.

### 3. Move to a cloud computer

**Move all to a cloud computer** (or a selection in the chat list) builds a
**move plan**: one durable record per chat, saved before anything is sent, with
retry-safe request keys for every Computers API call. Each chat moves through
these states, and any failure leaves that chat on this Mac and says why:

| State | What happens |
| --- | --- |
| Planned | Destination chosen: an existing cloud computer, or a new one sized from `GET /computers/capabilities`. One computer can hold several projects. |
| Pausing | A working agent is steered to "finish the current step and stop". After a bounded wait (default 10 minutes) its turn is interrupted. A chat waiting for an answer moves as it is. |
| Captured | **Workspace:** git projects commit the working tree, including untracked files but not ignored ones, to a hidden ref `refs/openagents/move/<chat>` and push it to the project's remote. Without a remote or push rights, a `git bundle` is uploaded in pieces of 4 MiB or less. **Agent state:** the harness's own session file (Claude Code session JSONL, Codex rollout) is uploaded. |
| Provisioned | The computer runs the headless engine (#11852), signed in as an account device. Harness logins come by credential reference (#11813), never as files. A move that needs a login the computer lacks stops here with that reason. |
| Restored | The project is fetched at the same absolute path, so native resume finds the session. Where the path can't be reproduced, the chat continues from a bounded summary plus recent turns, as in #11894. |
| Continued | A linked continuation (#11898) on the cloud engine, marked "Continued on Cloud computer". If the turn was interrupted, it is sent "Continue where you left off". The original chat becomes read-only. |
| Released | Only after the remote run has started: the local keeper stops. The local workspace is left as it was. |

**Bring back** reverses the plan. It fetches the computer's commits onto a local
branch, continues the chat on this Mac, and then stops or deletes the computer as
the person chooses. Paid time shows in the plan before it starts, and usage
appears under `GET /computer-usage`.

SPACE computers that preserve memory across suspend (#11812) make later moves
between cloud computers cheap. They don't change the move from this Mac, which
always happens at a turn boundary.

### 4. For agents

Add app tools (the MCP crate, alongside `list_chats`): `plan_move` (chats and a
destination, returning the plan and its estimated cost), `start_move`,
`move_status` and `bring_back`. They need the same confirmation as the UI action:
a tool call can't spend money or move a chat the person hasn't approved. Foreman
then can say "these six agents would keep running if moved to a cloud computer"
and do it on request.

## Delivery

| Issue | Scope | Depends on |
| --- | --- | --- |
| [#11930](https://github.com/OpenAgentsInc/openagents/issues/11930) Run keepers | Keeper subcommand, journal sequence numbers, socket protocol, engine adoption, MCP bridge reconnect, update handoff. Test: kill the engine mid-turn and confirm a fake harness keeps streaming and the new engine replays without duplication. | — |
| [#11931](https://github.com/OpenAgentsInc/openagents/issues/11931) Background engine | Ensure and attach to the service from the app, the menu-bar item, Move to background. Test: quit and relaunch the app during a turn, and the turn completes. | #11930 |
| [#11932](https://github.com/OpenAgentsInc/openagents/issues/11932) Cloud move: capture and restore | Workspace ref or bundle, harness session upload, same-path restore, seeded fallback. Test on a dedicated account with a disposable repository. | #11852, #11813, #11898 |
| [#11933](https://github.com/OpenAgentsInc/openagents/issues/11933) Cloud move: plans, UI and tools | Durable move plan, sheet and progress card, Bring back, MCP tools. Test: move three running chats, follow them on the phone, bring one back. | #11932, #11931 |

Until the keepers ship, restarting still interrupts agents, but the existing
auto-resume picks each one back up from its last message.

## As built

#11930–#11933 are on `main`. Where the build differs from the design above:

- **Where the agent runs.** The move starts the harness on the cloud computer
  as one Computers API command (Claude Code `--resume`, `codex exec resume`, or
  the seeded prompt), not through a headless engine there. The chat's linked
  continuation is an account chat (home `Cloud`) made from the chat's seed.
  It doesn't show the cloud agent's output yet.
- **Logins.** The engine reads which registered credential source and rule
  each agent uses from `<data>/moves/logins.json` (`claude_code`, `codex`,
  `git`, each `{ "source": "<uuid>", "rule": "<rule>" }`). The API has no
  list of sources. Without an entry the chat stays here and says why.
- **Waiting chats.** A chat waiting for an answer has its turn stopped before
  it moves, because a pending question can't be continued elsewhere. It
  resumes with "Continue where you left off", and the agent asks again.
- **Codex.** It resumes with `--dangerously-bypass-approvals-and-sandbox`, as
  the local driver runs it with full access and the cloud computer is the
  sandbox. Claude Code resumes in its default permission mode.
- **Bring back.** The cloud computer's work, committed or not, comes back as
  `openagents/cloud-<chat>`. The chat continues in a new local chat linked to
  the account part, with a first message that names that branch.
- **UI.** Move all is in the macOS menu-bar item, and one chat moves from the
  chat menu. The Move sheet shows the destination, the estimated paid time,
  each chat's state and reason, Try again, Bring back with stop, delete, or
  keep, and the approval for anything an agent asked for.
- **Terminals (#11951).** Each terminal's shell runs on a PTY held by a
  terminal keeper, which journals its output (the most recent ~1.5 MiB) and
  takes input and resizes. Quitting or updating the engine leaves the shell
  running, and the next engine shows it under the same id with its
  scrollback, so an open terminal no longer holds back an update.
  `OPENAGENTS_KEEPERS=0` opens terminals directly, and those still hold it.
