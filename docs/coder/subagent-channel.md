# Subagents talk to each other: the agent channel and the solver swarm

Issue #11346. Every subagent the main Coder (`crates/coder-new`) delegates to
can talk to the others in the same session: solvers, the Codex, Cursor, Devin
and OpenCode delegations, Microcoder, Claude in print mode, background
(crew) agents and plugin (ACP) agents. Two plugins carry it, both on by
default and turned off in /plugins or with `coder plugins disable ID`, like
Solver:

- **Agent channel** (`comms`): the shared channel, intents and overlap
  notices, negotiation records and system leases. Off, there is no channel:
  no tool, no preamble, no files, and delegations run isolated as before.
- **Solver swarm** (`solver-swarm`): the live model hands N issues to N
  solvers at once. With the channel off they still run side by side, just
  without talking.

## Where the channel lives

One folder per session, `~/.openagents/coder/comms/<session>/` (under
`--state` when given):

- `log.jsonl`: the channel itself, append-only. Every record is one line:
  a message (to everyone or one member), an intent, an overlap notice, a
  negotiation outcome, a lease event, a member joining or finishing.
- `state.json`: members, current intents, leases and agreements, read and
  written only under an exclusive lock on `lock`.

A file channel is the one shape every engine can reach. In-process members
(solvers, the swarm, the live model's `channel` tool, microclaude's tools)
call `crate::comms` directly. External engines get a short preamble on their
task naming the channel folder, their member name and the command:

```
<coder-or-openagents> [coder] comms post|read|intent|lease|agree --channel DIR --as NAME ...
```

and, as a fallback that needs no binary, they may append a JSON line
`{"from":NAME,"to":"all","text":"..."}` to `log.jsonl`. The session is the
Coder process (or `chat --session ID`); a nested Coder started by a delegate
joins the parent's channel through `CODER_COMMS_CHANNEL`.

## Intents and overlap

An intent is the files, crates and resources a member means to change. A
crate is derived from a `crates/<name>/` path. Two intents overlap on a
shared file, a file inside a shared folder, a shared crate, or a shared
resource. Each new intent is compared with every other member's current
intent; each overlap posts one notice naming both members and what they
share, and both members see it (their child chat shows it; a solver's next
Coder hand reads it in its prompt).

Solvers post intents for themselves from their run: the Briefer's "Files to
change" list after the briefing, then the files the Coder actually edited on
each hand-back. Other agents post their own with `comms intent`.

## Negotiation

An overlap is settled by one of three outcomes, recorded in the log and
`state.json`:

- `first`: one member goes first; the other rebases on its branch after it
  lands (the merge order).
- `split`: the shared file's change is divided; each side names its part.
- `merge`: both changes go into one pull request.

Agents agree with `comms agree`. For a swarm of solvers, whose hands cannot
chat, the swarm settles it by a fixed rule: the member whose intent came
first goes first, the other rebases. Either way the agreement is a record
with who decided and why.

## System leases

Scarce shared things are leased with an expiry, renewed while held and
released on finish:

| kind | resource | value |
|---|---|---|
| `build-folder` | the shared cargo target folder or lock | the folder |
| `deploy-slot` | a staging deploy slot | the slot name |
| `migration` | a database migration number | the next free number |
| `port` | a local port | a free port |
| `build-host` | a build host such as CoderOS or this Mac | the host |

A lease holds until it expires (default 10 minutes); its holder renews it.
A finished member releases all of its leases; a crashed one stops renewing
and its leases expire. Several leases are taken in one call in a fixed order
(sorted by resource), and a member holding a lease can't take one that sorts
before it, so two members can never wait on each other. A member that finds
a lease held sees who holds it and until when, can ask the holder over the
channel, and may wait or do something else.

These leases coordinate agents within a session. The host broker
(`crates/coder-lease`) still governs real builds on the machine through its
cargo shim.

## Solver swarm

`solver_swarm {issues, no_pr?, decisions?, concurrency?, budget_usd?}` starts
one solver per issue, each in its own worktree and run folder as solver runs
are today. `decisions` (`jev` or `clef`) is passed to every solver as
`--decisions`, so each run's `summary.json` verdict is the same door-weighed
judgment `coder solver queue --decisions` gives (already landed, still
current, actionable now).
Concurrency defaults to what the machine fits (a quarter of the cores, one to
four, and one when free disk is under 30 GB) and the person may set it. A
spend limit, when the person sets one, stops new solvers from starting once
the finished ones have spent it. No step or time limits are imposed.

Each solver shows as its own child chat; the swarm's own chat shows the swarm
view, one row per member with its stage, cost and outcome, and the channel's
messages. It ends with one summary: pull requests, the agreed merge order,
overlaps settled, failures with reasons and the total cost.

## Tests

`crates/coder-new/src/comms/tests.rs`: overlap detection; lease acquire,
renew, expire and release; ordered acquisition without deadlock; negotiation
recorded; the plugin off means no channel. The swarm test replays three
solver runs (from `tests/fixtures/solver-run-11338.jsonl`), two touching one
file, and checks the intents, the overlap notice, the agreed order and the
record.
