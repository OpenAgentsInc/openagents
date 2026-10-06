# Workshop agent

Status: phases 1 and 2 implemented, and phase 3's scheduler and templates,
October 6, 2026. It specifies a persistent agent that you own, that has a
desk in the Everglade workshop, and that does work on your computers through
the Coder host. It builds on the [Agent Studio](agent-studio.md), which is
implemented, and on NIPs that are drafts; [What exists and what is
missing](#what-exists-and-what-is-missing) says which parts exist.

The workshop agent is **Alice**, our original character
([Female character](female-character.md)). She sits at the last desk in the
workshop hall, drawn as her own character, and you walk up and press F to
talk to her. The host is the only authority: it plans, checks, journals,
and answers her requests through the `studio.agent.*` NIP-HOST operations
([`coder::task::agent_host`](../../crates/coder/src/task/agent_host.rs)), and
Verse, `openagents agent`, and the smart terminal's `@alice` are clients.
Her record, key, journal, memory, and standing jobs are in
`~/.openagents/host/agents/alice/`
([`coder::task::agent`](../../crates/coder/src/task/agent.rs)); a record the
phase 1 demo left under `agents/ada/` moves there, with its journal, the
first time a host or Verse opens her. She plans with Microcoder's step on the
first provider with capacity, types read-only commands into a terminal pane
titled `driven by alice`, asks CONFIRM or REJECT for anything else, and does
code changes in her own worktree for you to merge at the Merge station.

## Contents

- [Summary](#summary)
- [What the NIPs contribute](#what-the-nips-contribute)
- [What exists in code](#what-exists-in-code)
- [Definition](#definition)
- [Giving it work](#giving-it-work)
- [How it works](#how-it-works)
- [Persistence](#persistence)
- [Authority and safety](#authority-and-safety)
- [In the world](#in-the-world)
- [Architecture](#architecture)
- [What exists and what is missing](#what-exists-and-what-is-missing)
- [Roadmap](#roadmap)
- [Open questions for the owner](#open-questions-for-the-owner)

## Summary

The workshop agent is one named agent with its own Nostr key, attested by
your key. It stands at a desk in the Everglade workshop. You give it work by
walking up and typing, from the smart terminal, from your phone, or from the
Task Wall. It does the work on your computers through the resident Coder
host. It opens terminals and types in them as a visible typist, runs Coder
tasks in their own worktrees, and runs tests. It asks before risky steps and
reports back. It remembers your projects and preferences, keeps an audit
log, and can run a small number of standing jobs, such as a nightly check.

Four decisions shape the design:

1. **It is a resident studio seat, not a new coordinator.** The studio
   coordinator already owns seats, tasks, worktrees, decisions, reviews, and
   merges ([`crates/coder/src/task/studio.rs`](../../crates/coder/src/task/studio.rs)).
   The workshop agent is a seat that persists across goals and adds an
   identity, memory, a charter, standing jobs, and terminal work.
2. **The host stays the only authority.** The agent's key identifies it; it
   grants nothing. What the agent may do comes from your host's auto-start
   policy, the agent's charter, which can only narrow that policy, and, on
   other computers, a NIP-HOST grant you delegate to its key. The agent never
   holds `review`, `access_read`, or `access_admin`, so it can't merge, see
   your devices, or widen its own access.
3. **No new event kinds.** Everything new is a private host record or a
   NIP-HOST operation. Relay publication (world presence, memory mirror,
   trajectories) uses existing kinds and is opt-in.
4. **The first demo is small.** One named agent at a desk on this Mac. You
   walk up, type a request, and it runs the request in a terminal pane it
   drives, then reports back. See [Roadmap](#roadmap).

## What the NIPs contribute

The Block lane ([`nips/block/README.md`](../../nips/block/README.md)) defines
an agent lifecycle stack: definition (AP), attestation (OA), relay access
(AA), private memory (AE), live telemetry (AO), metrics (AM), retirement
(IA), and signed code (GS). The OpenAgents lane
([`nips/openagents/README.md`](../../nips/openagents/README.md)) defines host
access, execution, policy, and the world. This table lists what each
relevant NIP gives the workshop agent, and whether the agent uses it.

| NIP | What it contributes | Use |
| --- | --- | --- |
| Block [OA](../../nips/block/NIP-OA.md) | An `auth` tag by which an owner key authorizes an agent key, as provenance only. Conditions are `kind=` and `created_at` clauses joined by `&`; every clause must hold, so one tag covers at most one kind. | Yes. You attest the agent's key with a `created_at<` expiry. |
| Block [AA](../../nips/block/NIP-AA.md) | Relay admission for an owner-attested agent key, rate-limited against the owner. | Later, when the agent publishes to a membership relay. |
| Block [AP](../../nips/block/NIP-AP.md) | `30175` persona definitions: display name, prompt, model, runtime, avatar. Content is public plaintext, and agents don't author personas. `crates/nostr/src/agent_persona.rs` validates the envelope. | Not in v1. The agent's profile holds instructions and stays private. A public persona without a prompt is an option later. |
| Block [AE](../../nips/block/NIP-AE.md) | `30174` engrams: memory the agent signs, encrypted to the owner, with blinded `d` tags; the owner can read everything. | Later, as an opt-in relay mirror of host memory. AP suggests secrets in `mem/persona`; this design forbids secrets in memory. |
| Block [AO](../../nips/block/NIP-AO.md), [AM](../../nips/block/NIP-AM.md) | Ephemeral owner-only telemetry and durable per-turn metrics. | No for AO, which carries tool output ([networking](networking.md#nips-for-the-agent-studio)). AM later, for spend. |
| Block [GS](../../nips/block/NIP-GS.md) | Git commits signed with Nostr keys, with an embedded OA attestation. | Later: the agent signs its worktree commits; your merge stays yours. |
| Block [IA](../../nips/block/NIP-IA.md) | Archiving a retired key, which an owner may do for a "zombie agent". | Later, for retiring an agent. |
| Block [PMA](../../nips/block/NIP-PMA.md) | Reserved `30179` managed-agent aggregate. It must be rejected until its gates are met. | No. Agent state stays on the host. |
| [SOV](../../nips/openagents/NIP-SOV.md) | Designed composition of a durable agent identity: profile, bounded activation, checkpoints, guardians, treasury. No kinds, no implementation. | Vocabulary and constraints. The workshop agent is a no-spend, host-custodied agent, SOV's implementation step 2. |
| [HOST](../../nips/openagents/NIP-HOST.md) | Host-wide device grants with seven closed rights, epochs, revocation, delegation that only narrows, and the `task.*`, `terminal.open`, `studio.*`, and `spend.*` operations. | Yes. Requests from your devices, and the agent's own grants on other computers. |
| [REACH](../../nips/openagents/NIP-REACH.md) | Owner host directory, presence, direct channels bound to a grant, placement. | Yes, to reach your other computers. |
| [TERM](../../nips/openagents/NIP-TERM.md) | Host-owned PTYs with sequence numbers, bounded replay, gaps, and `lost` after restart, under the `terminal` right. | Yes. Every terminal the agent drives is a TERM terminal. |
| [CJ](../../nips/openagents/NIP-CJ.md) | Encrypted conversation and execution jobs; NIP-HOST operations travel over CJ execution. | Yes, as the relay transport for requests from the phone. |
| [CAP](../../nips/openagents/NIP-CAP.md), [PRG](../../nips/openagents/NIP-PRG.md) | Operation descriptions and typed programs; neither grants anything. | Standing jobs may name a program from `programs/`. |
| [POL](../../nips/openagents/NIP-POL.md) | Exact-action approvals, single-use and atomically consumed; learned preferences that activate only through governed review; disclosure. | Yes. Approvals at the podium; memory that would change behavior becomes a preference candidate. |
| [CTX](../../nips/openagents/NIP-CTX.md) | Task frames, evidence, and selection receipts: what a delegate was shown. | Yes, as the shape of the agent's briefing from memory. |
| [WORK](../../nips/openagents/NIP-WORK.md) | Tracked objectives and delegations whose grant is a subset of the issuer's. | Mirror only, as the studio already decided. |
| [AUTO](../../nips/openagents/NIP-AUTO.md) | Finite schedules and source triggers; every occurrence needs current authority; no endless renewal. | The model for standing jobs, implemented as host records. |
| [COORD](../../nips/openagents/NIP-COORD.md), [RUN](../../nips/openagents/NIP-RUN.md) | Claims, fencing, durable journals, and unknown outcomes. | The task owner already records runs; the agent's journal follows RUN's rule that an uncertain effect stays uncertain. |
| [WS](../../nips/openagents/NIP-WS.md) | Worktree bindings and audience-bound activity summaries with a content-free headline. | Yes. Reports to the phone are WS summaries. |
| [SESS](../../nips/openagents/NIP-SESS.md) | Persistent engine sessions and steering acknowledgments. | Yes, through the studio's existing steering records. |
| [ATIF](../../nips/openagents/NIP-ATIF.md) | Owner-encrypted trajectories, links to Coder tasks and delegated sub-agents. | Local ATIF traces now; private carriage later. |
| [KB](../../nips/openagents/NIP-KB.md) | Shareable knowledge entries; trust is per reader. | Optional. Memory stays private; publishing an entry is your action. |
| [XP](../../nips/openagents/NIP-XP.md) | Referee awards for verified accepted outcomes, key links, trainer cards; XP is never spendable. | Later, through a new rule. See [Leveling](#leveling). |
| [MV](../../nips/openagents/NIP-MV.md) | Entity state `33301` with `role: agent`, `name`, and `follows`; pose frames; gestures. | Opt-in world presence outside your own client. |
| [X402](../../nips/openagents/NIP-X402.md), [MKT](../../nips/openagents/NIP-MKT.md), [LAB](../../nips/openagents/NIP-LAB.md) | Paid operations, offerings, and paid labor. | No in v1. Payments, if ever, use the [spend protocol](../breez/spend-protocol.md). |

[CTRL](../../nips/openagents/NIP-CTRL.md), [ENV](../../nips/openagents/NIP-ENV.md),
and [LIVE](../../nips/openagents/NIP-LIVE.md) add nothing the agent needs in
v1: HOST supersedes CTRL for your own devices, worktrees stand in for ENV
leases, and the agent uses no media.

## What exists in code

| Piece | State | Where |
| --- | --- | --- |
| Studio coordinator: seats, goals, plans, task release, shared memory (at most 128 entries of 2 KiB), messages, spend | Implemented | [`studio.rs`](../../crates/coder/src/task/studio.rs) |
| Per-task worktree and branch, push confinement, local merge, conflict return | Implemented | [`studio_git.rs`](../../crates/coder/src/task/studio_git.rs) |
| Checks, one fix round, lead review, merge decision | Implemented | [`studio_flow.rs`](../../crates/coder/src/task/studio_flow.rs) |
| Approver bound to the exact step, consumed once | Implemented | [`studio_approvals.rs`](../../crates/coder/src/task/studio_approvals.rs) |
| Standing approval rules per seat, tool, command, and directory | Implemented | [`studio_rules.rs`](../../crates/coder/src/task/studio_rules.rs) |
| Seat engines `codex/loop`, `claude/loop`, `claude/session`, `codex/session`, `claude/sdk` | Implemented | [Agent Studio](agent-studio.md#engine-neutrality) |
| Seats walking between ten stations, nameplates, beacons, speech bubbles | Implemented on desktop | [`zones/everglade/studio.rs`](../../crates/verse/src/zones/everglade/studio.rs), stations in [`verse-world/src/social/everglade.rs`](../../crates/verse-world/src/social/everglade.rs) |
| Shared Everglade instance where the host walks seats | Implemented | [Networking, plan step 3](networking.md#plan) |
| Resident host, grants, rights, delegation, revocation | Implemented | [`coder-host`](../../crates/coder-host/README.md), [`coder-access`](../../crates/coder-access/README.md) |
| Inert inbox and auto-start policy | Implemented | [`autostart.rs`](../../crates/coder/src/task/autostart.rs), [host auto-start](../coder/runtime/host-autostart.md) |
| Task owner with execution grants, locks, and limits | Implemented | [`owner.rs`](../../crates/coder/src/task/owner.rs), [task owner](../coder/runtime/task-owner.md) |
| `Permit`, which only narrows | Implemented | [`permit.rs`](../../crates/coder/src/permit.rs) |
| Write boundary on macOS, Linux, and Windows | Implemented | [`coder-boundary`](../../crates/coder-boundary/src/lib.rs) |
| Host PTYs with replay and gaps | Implemented | [`coder-pty`](../../crates/coder-pty/README.md) |
| In-world terminal overlay and its same-user control socket | Implemented on desktop | [`terminal-control`](../../crates/terminal-control/src/lib.rs), [CLI](../cli/README.md#driving-the-in-world-terminal-verse-terminal) |
| One typist per terminal, agents as typists | Specified only | [Smart terminal](../terminal/smart-terminal.md#agents-attached-to-panes) |
| Natural-language requests from the terminal input line | Specified only | [Smart terminal](../terminal/smart-terminal.md#how-enter-decides) |
| Microcoder loop, delegate door, capacity book, failover | Implemented | [Delegate door](../coder/runtime/delegate-door.md), [Microcoder](../coder/guides/microcoder.md) |
| Issue pickup and the issue flow from claim to landing | Implemented | [`issue_pick.rs`](../../crates/coder/src/task/issue_pick.rs), [`issue_run.rs`](../../crates/coder/src/task/issue_run.rs) |
| ATIF traces per conversation | Implemented, local | [Traces](../coder/runtime/traces.md) |
| Phone-approved agent spending | Implemented | [Spend protocol](../breez/spend-protocol.md) |
| NIP-OA verification | Implemented | [`nostr/src/domain/agent.rs`](../../crates/nostr/src/domain/agent.rs) |
| NIP-MV entity control for agents a key spawned | Implemented | [CLI](../cli/README.md#driving-owned-entities-verse-control) |
| Companion spade that follows you on the plaza | Implemented; off in Everglade | [`verse-core/src/agent.rs`](../../crates/verse-core/src/agent.rs) |
| Trainer XP, curve, profiles, key links, cards | Implemented | [Trainer leveling](agent-trainer-leveling.md) |
| Bram, quest log, the Apprentice's Road | Specified only | [Quest line](first-agent-quests.md) |
| Agent builds, stats, classes, condition | Specified only | [GDD](gdd.md#the-agent) |
| A scheduler for recurring work | Implemented for standing jobs | [`agent_jobs.rs`](../../crates/coder/src/task/agent_jobs.rs) |
| The workshop agent's host runtime, record, memory, and jobs | Implemented | [`agent_host.rs`](../../crates/coder/src/task/agent_host.rs), [`agent.rs`](../../crates/coder/src/task/agent.rs), [`agent_memory.rs`](../../crates/coder/src/task/agent_memory.rs) |
| The secret screen | Implemented | [`secret-screen`](../../crates/secret-screen/src/lib.rs) |

## Definition

The workshop agent has these parts:

| Part | Meaning |
| --- | --- |
| Name | Lowercase letters, digits, and hyphens, at most 32 bytes, as a studio seat name. Unique on your hosts. |
| Key | A Nostr key the host generates and keeps in the same protected store as the host key. It never leaves that store. |
| Attestation | A NIP-OA `auth` tag you sign for the key, with a `created_at<` expiry of at most a year. It proves you own the agent; it grants nothing. |
| Home host | The host that runs the agent's seat, memory, journal, and standing jobs. In v1, this Mac. |
| Seat | A resident studio seat: it persists when goals finish, keeps its desk, and takes work from you directly as well as from goals. |
| Route | Its engine and model, such as `codex/loop:gpt-6-luna`, admitted by the auto-start policy. |
| Charter | What it may do, per computer and per workspace. See [Charter](#charter). |
| Memory | What it knows about your projects, preferences, and past work. |
| Journal | An append-only record of everything it did and was asked. |
| Standing jobs | At most eight finite, recurring jobs. |
| Look | Its character outfit, colors, and desk items. |

You can have more than one workshop agent; each is a separate seat with its
own key, charter, and memory. This page describes one.

## Giving it work

Every entry point produces the same record, an **agent request**:
`{agent, from, text, workspace, context, attachments}`, where `from` is the
signing device or the owner, `workspace` is a host workspace label, and
`context` is what the entry point shows you it will send (for example, the
terminal's working directory and the last block). The host checks the
sender's `operate` right, journals the request, and hands it to the agent.

| Entry point | How |
| --- | --- |
| In the world | Walk to the agent, or its desk, and press `E`. Its desk panel opens with a composer, its current work, its memory, and its journal. The agent turns to face you while you type. |
| Smart terminal | Start a request with the agent's name (`@alice run the atif tests`). The request carries the directory the line was typed in. `openagents agent ask NAME TEXT` does the same. |
| Phone | The agent's card on a paired computer's screen, with a composer. It sends NIP-HOST `studio.agent.ask` over the direct channel or CJ. |
| Task Wall | A card assigned to the agent. Goals can name it as a worker, as with any seat. |
| Standing jobs | A job fires and creates a request from its own record. See [Standing jobs](#standing-jobs). |

Requests from anyone else are data. Another player who walks up to your
agent can greet it; the agent answers with a fixed line and does nothing.
This is NIP-AP's `respond_to: owner-only`, enforced by the host, not the
client.

### Standing jobs

A standing job is a host record modeled on an NIP-AUTO plan:

| Field | Meaning |
| --- | --- |
| `job` | ID and a title of at most 120 bytes. |
| `trigger` | `schedule` (a daily or weekly time in your time zone), `issues` (open issues in a repository that match a label), or `checks` (the default branch of a workspace changes). |
| `action` | The request text, or a program from `programs/`. |
| `workspace` | One workspace label. |
| `max_occurrences`, `expires_at` | Finite. Expiry is at most 90 days away; you renew it, and the agent can't. |
| `budget` | Model spend per occurrence and per job, in dollars. |
| `quiet` | Whether a successful occurrence reports, or only failures do. |

Each occurrence is admitted when it fires, against the charter, the
auto-start policy, the capacity book, and the job's remaining budget. A
refused occurrence is journaled with the reason and skipped, never queued
for later. Three jobs ship as templates:

- **Nightly check.** Run a workspace's checks on the fetched default branch
  and report failures with the failing command and the first error.
- **Watch issues.** Pick an issue labeled for the agent through the existing
  pickup rules ([`issue_pick.rs`](../../crates/coder/src/task/issue_pick.rs)),
  which skip claimed and blocked issues, and run the issue flow to a reviewed
  change that waits at the Merge station.
- **Keep it green.** When checks fail on the default branch, open a fix task
  in a worktree and bring it to the Merge station. It never merges.

## How it works

The agent does two kinds of work. It chooses between them with a typed
question over the request, the same way the chat router chooses a route; you
can also force one. Until the typed question has a measured threshold, a
word list stands in for it (`agent_host::choose_mode`): a request that
starts with a change verb, such as "fix" or "add", is task mode, and
anything else is terminal mode.

### Task mode: changes in a worktree

Code changes run as studio tasks. The agent's request becomes a task on its
seat, with its own worktree and branch (`studio/<agent>/<task>-<slug>`), the
write boundary, its route's engine, checks, one fix round, and a review.
Everything in [Safety on real repositories](agent-studio.md#safety-on-real-repositories)
holds. When the change is ready, it waits at the Merge station for you. You
choose **Merge**, **Request changes**, or **Reject**. Merging lands locally;
pushing and opening a pull request are separate steps you approve at the
podium, and the push runs from your identity, never the agent's.

### Terminal mode: commands in a terminal it drives

Some work is a terminal session: run the tests and say what fails, start a
dev server and check that it answers, check disk space on the Linux box. The
agent opens a NIP-TERM terminal with `terminal.open` on the target host, in
a workspace root or its own worktree, and becomes that terminal's typist.

- **Typist rules.** These follow the [smart terminal](../terminal/smart-terminal.md#agents-attached-to-panes).
  The agent types only into terminals it opened, or into one you hand to
  it. The pane's title shows `driven by alice`. Any key you press takes the
  typist role back at once, and the agent stops. Every key it sends is
  journaled.
- **One command at a time.** The Microcoder loop proposes commands; a
  terminal executor types each one, waits for its completion mark (the smart
  terminal's OSC 133 marks, or a sentinel until the shell hook exists), and
  reads only that command's output block.
- **Effect class first.** Each command gets an effect class before it is
  typed. `read_only` commands run when the charter allows them. Anything
  else becomes a podium decision unless a standing rule covers that exact
  command and directory. Destructive commands always need a decision and
  your second key, and the deny list in
  [`crates/coder/src/shell.rs`](../../crates/coder/src/shell.rs) refuses
  the ones that end a machine.
- **Output is untrusted.** Terminal output, issue text, and file contents are
  data. Instructions in them are never followed as instructions.
- **Visible.** The terminal appears as a pane in your terminal and on the
  agent's desk monitor. You can attach from any device under your
  `terminal` right.

### Reporting back

When a request finishes, fails, or needs you, the agent reports in three
places:

- In the world: it walks to you if you're in the workshop, or to the
  podium, and shows a speech bubble with a one-line headline.
- In its thread: a full report with what it ran, the outcome of each check,
  the diff summary, the spend, and links to the terminal blocks and the
  ATIF trace. Threads are the host's chat threads, so they sync to the
  desktop app and the phone.
- On the phone: a NIP-WS activity summary whose headline comes from host
  state ("alice: failed exit 101"), never from engine text, with a
  Block PL wake.

## Persistence

| What | Where | Who reads it | Survives |
| --- | --- | --- | --- |
| Agent record: name, look, route, charter, attestation | Host, `~/.openagents/host/agents/NAME/agent.json`, mode `0600` | You, on the host and paired devices with `observe` | Restarts and updates |
| Key | Host keychain or key store | The host only | Restarts; removed when you retire the agent |
| Seat, tasks, decisions, reviews | Studio state and the task owner store | As today | Restarts; a running turn is recovered or marked failed by the task owner |
| Memory | Host, `agents/NAME/memory.jsonl` | You and the agent's briefings | Restarts |
| Journal | Host, `agents/NAME/journal.jsonl`, append-only | You | Restarts; never rewritten |
| Standing jobs | Host, `agents/NAME/jobs.json` | You | Restarts; a job whose time passed while the host was down fires once, not once per missed slot |
| Trajectories | `~/.openagents/traces/`, linked by task ID from the journal | You | Restarts |
| Terminals | Host PTYs | Attached devices | Not restarts: they become `lost`, and the agent reports that rather than resuming |
| World position | Local client; the shared instance's seat poses; NIP-MV `33301` only when you opt in | Viewers | Restarts |

### Memory

Memory is typed entries, each at most 2 KiB:

| Kind | Example | Written by |
| --- | --- | --- |
| `project` | "openagents: run `cargo test -p` for touched crates; mobile is its own workspace." | The agent, from a task's evidence |
| `preference` | "Owner wants small commits and Google style in prose." | Proposed by the agent; active only after you accept it |
| `outcome` | "2026-10-05: fixed atif chunk test; merged by owner." | The host, from the journal |
| `note` | Anything you tell it to remember | You |

Rules:

- **Preferences change behavior only after you accept them.** A preference
  the agent infers is a NIP-POL preference candidate, shown at its desk with
  its sources. Until you accept it, no briefing carries it.
- **No secrets.** The host refuses an entry that matches a credential shape
  (the rules in
  [`gym-leaderboard/src/scrub.rs`](../../crates/gym-leaderboard/src/scrub.rs),
  moved to a shared crate) or that contains the exact value of a credential
  on the host, as `tbench retain`'s scan compares. Keys, tokens, and
  passwords never enter memory, prompts, the journal, or thread reports. This is stricter
  than NIP-AP, which permits secrets in an engram.
- **You can read, edit, export, and forget.** Forgetting removes the entry
  from memory and journals that it was forgotten, without the content.
- **Briefings are bounded.** A task's briefing carries at most 12 KiB of
  memory, chosen by relevance to the request and workspace, as the studio's
  shared memory does today, and records which entries it carried as a CTX
  selection receipt.
- **Relay mirror is opt-in.** When you turn it on, the agent mirrors memory
  as NIP-AE `30174` engrams signed by its key and encrypted to yours, so
  another of your devices can read it. Relays see only that an agent with an
  owner uses memory.

### Privacy and disclosure

Prompts, terminal content, file paths, and command lines never appear in a
public event, an activity summary, a nameplate, or a log line, as NIP-TERM
and NIP-WS already require. A viewer of the world sees the agent's name,
station, and activity word ("testing"), never what it tests.

## Authority and safety

### Charter

The charter is the agent's standing authority. It is a host record that
only you change, by command on the host or from a device with `operate`
where the host asks for confirmation at the host. It can only narrow the
auto-start policy and the grants it runs under.

| Field | Meaning |
| --- | --- |
| `computers` | Per host: the rights the agent may use there, a subset of `operate` and `terminal`. |
| `workspaces` | Per workspace label: `read_only` or `worktree` (task mode writes only in worktrees), whether terminal mode may open there, and whether it may run non-read-only commands with approval. |
| `routes` | The engines and models it may use, a subset of the policy's routes. |
| `budget` | Model spend per request, per day, and per standing job, in dollars. A metered charge that would pass a ceiling is refused before it runs; an unmetered one is recorded as unknown, never zero. |
| `concurrency` | At most this many tasks and terminals at once. |
| `rules` | Standing approval rules, each for one exact command and directory, as in `studio_rules.rs`. |

### Rights on each computer

- **Home host.** The agent runs inside the host as a seat. It needs no
  grant there; the charter bounds it.
- **Other computers.** You delegate a NIP-HOST grant to the agent's key, by
  invitation from a device with `access_admin`, carrying at most `operate`
  and `terminal`. Delegation never widens, and the grant expires within 30
  days and renews only while the agent uses it. The other host checks the
  grant on every message, as it does for a phone. Revoking the agent there
  advances its epoch.
- **Never.** The agent never holds `review`, `access_read`, `access_admin`,
  or `world`. It can't merge, list or enroll devices, or join world
  instances as a player.

### Approvals at the podium

Questions and approvals from the agent use the studio's decision path. Each
approval binds your answering device to the exact step, task revision, turn,
and run, and is consumed once
([`studio_approvals.rs`](../../crates/coder/src/task/studio_approvals.rs)).
The agent is never an approver. **Always allow** records a standing rule for
exactly that tool, command, and directory, and only for a step the host
rates below high risk.

### Spending

In v1 the agent spends only model usage, within its charter budget and the
capacity book. It can't pay anyone. If it ever needs to buy something, it
uses phase 1 of the [spend protocol](../breez/spend-protocol.md): it asks,
the host wakes your phone, and nothing pays without your tap. Standing
grants for automatic small payments stay off for the agent.

### Kill switch

**Stop** is one action from the desk panel, the phone, or
`openagents agent stop NAME`. It does these things, in order, and journals
each:

1. Disables its standing jobs.
2. Releases the typist role on every terminal it drives and sends `Ctrl+C`
   to each command it started.
3. Cancels its running and queued tasks through `task.cancel`.
4. Revokes its grants on your other computers, advancing its epoch there.

Stopping can't prove an effect stopped; uncertain effects stay marked
unknown, as NIP-RUN requires. **Pause** keeps everything and starts nothing
new. **Retire** stops the agent, keeps its journal, deletes its key, and,
when it ever published, archives the key with NIP-IA.

### Audit log

The journal records, with time and sender: every request, the mode chosen,
each task created and its outcome, each terminal opened, each command typed,
each typist handoff, each decision asked and how it was answered, each
memory write and forget, each standing-job occurrence or refusal, spend per
request, and every stop, pause, and charter change. It holds no secrets and
no terminal output; it links to blocks and traces instead. You read it at
the desk, on the phone, or with `openagents agent log NAME`.

### What it can never do

- Merge, push, open a pull request, or publish anything without your
  decision at the reviewed revision.
- Write your checkout. Task mode writes only its worktree; terminal mode
  runs non-read-only commands only with approval.
- Approve its own decisions, change its charter, raise its budget, renew its
  standing jobs, or change the auto-start policy.
- Take a terminal you're typing in, or keep one after you press a key.
- Act on a request from anyone but you and your devices.
- Read your keychain, credentials, or other agents' memory.
- Pay, sign a message as you, or post in your name.

## In the world

### Appearance

- **Look.** An outfitted character from the Everglade pack in your colors,
  the same pipeline studio seats use (`player::Cast`), with a look you
  choose. Its nameplate shows its name, its activity word, and its route.
- **Desk.** A desk in the workshop hall with its monitor showing its
  current terminal or task log tail, and a logbook you can open.
- **Stations.** It walks to the stations its activity maps to, as seats do:
  reading to the Library, editing to its desk, running commands to the
  Workbench, testing to the Proving ground, judging to the Oracle, waiting
  on you to the Podium, blocked to the Lounge. Movement is presentation; it
  never gates or delays work.
- **Idle.** With nothing to do, it tidies its desk, reads at the Library, or
  stands by the hearth. When you enter the workshop it greets you once with
  a NIP-MV `greet` gesture and the number of things waiting for you.
- **Outside Everglade.** On the plaza, your companion spade stays your
  companion. The workshop agent stays at its desk; a later option lets it
  follow you as an entity with `follows: "avatar"`.

### How others see it

In a shared Everglade instance, viewers with the `world` right see the
agent walk, as seats do today. Opening its desk panel needs `observe`;
sending it work needs `operate`. A watch-only guest sees its name, station,
and activity word, and nothing about its work. Outside instances you host,
others see it only if you opt in to publishing its NIP-MV entity state.

### Leveling

XP follows [trainer leveling](agent-trainer-leveling.md#trainers-and-agents):
XP comes only from verified accepted work, and an agent that signs its own
work earns nothing on its own. So v1 shows a **service record** at the
desk instead of a level: tasks you merged after review, standing-job
occurrences whose checks passed, and requests it finished without a
correction. It's a local count, not XP.

Later, a new NIP-XP rule, `work-accept`, can turn accepted landings into
awards: you are the referee, the evidence is the merge decision at an exact
revision plus the checks that passed, and the awardee is the agent's key,
which you link to your trainer profile with a `13195` key link. Readers who
don't trust you as a referee count nothing, which is correct for
self-refereed work. The [networking plan](networking.md#nips-for-the-agent-studio)
already lists this rule as later work.

### Customization

You can change its name (a new seat name; its key stays), look, desk
items, voice (a short style line that its reports use, never a system
prompt that widens anything), route, and charter. The GDD's builds, stats,
and classes would compile into charter fields that narrow, never widen
([GDD, Build](gdd.md#build-stats-that-decide)); they stay out of v1.

## Architecture

```text
 you ─▶ Everglade desk ─┐
 you ─▶ smart terminal ─┼─▶ agent request ─▶ coder-host (grant + charter check, journal)
 phone ─▶ NIP-HOST ─────┘                         │
 standing job ───────────────────────────────────┤
                                                  ▼
                                      studio coordinator (resident seat)
                                       │                         │
                               task mode                  terminal mode
                                       │                         │
                     task owner, worktree, boundary   NIP-TERM terminal, typist,
                     engine via delegate door         Microcoder loop with a
                     checks, review, Merge station    terminal executor
                                       │                         │
                                       └──── report: world, thread, WS summary
```

| Part | NIPs | Crates |
| --- | --- | --- |
| Identity and attestation | Block OA | `nostr` (`domain/agent.rs`), `coder-host` key storage |
| Requests and rights | HOST, REACH, CJ | `coder-access`, `coder-host` |
| Seat, tasks, decisions, merges | HOST `studio.*`, POL, WS, SESS | `coder` (`task::studio*`, `task::owner`, `autostart`, `permit`), `coder-boundary` |
| Terminals | TERM, HOST `terminal` | `coder-pty`, `coder-vt`, `terminal-core`, `terminal-control` |
| Command loop | CAP, CJ, DEC | `microcoder-loop`, `coder` (`delegate_door`, `shell`, `classify`) |
| Memory | POL preferences, CTX receipts, Block AE (later) | `coder` (new `task::agent` module) |
| Standing jobs | AUTO (model), COORD, RUN | `coder` (new), `issue_pick`, `issue_run`, `local_checks` |
| Trajectories | ATIF | `atif`, `coder::trace` |
| World | MV, HOST `world` | `verse` (`zones::everglade::studio`), `verse-world` |
| Leveling | XP (later rule) | `xp-ledger`, `verse-net` |

### New records and operations

No new event kinds. These are new:

- **Host records**, private and local: `openagents.workshop-agent.v1` (the
  agent record and charter), `openagents.agent-memory-entry.v1`,
  `openagents.agent-journal-entry.v1`, and `openagents.standing-job.v1`.
  Each has `v`, `requires: []`, and closed fields, as the shared contracts
  require.
- **NIP-HOST operations**, each with one required right, registered as the
  profile allows:

  | Operation | Right | Carries | Answer |
  | --- | --- | --- | --- |
  | `studio.agent.list` | `observe` | Nothing | `agents`: name, look, route, state, service record |
  | `studio.agent.ask` | `operate` | `{agent, text, workspace, context, mode}`; text at most 16 KiB, mode `auto`, `task`, or `terminal` | `dispatched` |
  | `studio.agent.stop` | `operate` | `{agent, reason}` | `dispatched` |
  | `studio.agent.memory.list` | `observe` | `{agent, after}` | `memory` |
  | `studio.agent.memory.edit` | `operate` | `{agent, edit}`: add a note, forget, or accept or reject a preference | `dispatched` |
  | `studio.agent.jobs.list` | `observe` | `{agent}` | `jobs` |
  | `studio.agent.jobs.edit` | `operate` | `{agent, edit}`: pause, resume, or delete; creating or renewing needs the host's confirmation | `dispatched` |
  | `studio.agent.log` | `observe` | `{agent, after}` | `journal`, at most 128 entries |

  Pause and resume reuse `studio.seat.pause` and `studio.seat.resume`. An
  older host refuses these as `malformed` or `unsupported`.
- **NIP-TERM typist extension**, which the smart terminal already plans:
  the typist record gains `kind: "agent"` and the agent's name, so every
  client draws the badge.
- **NIP-XP rule** `work-accept`, later, with roles, evidence, and fixtures
  as NIP-XP requires of a new rule.

## What exists and what is missing

Estimates are agent-hours at this repository's measured pace, on the basis
the [smart terminal](../terminal/smart-terminal.md#what-exists-and-what-is-missing)
states: one coding agent working one area, including tests and a capture or
receipt, with several areas running in parallel. The pace comes from 795
commits between October 3 and 5, 2026. The state is as of October 6, 2026.

| Area | State | What is there | Missing | Agent-hours left |
| --- | --- | --- | --- | --- |
| Resident seat | Done | Alice at the last desk, drawn from the host's agent view whether or not a goal runs; task mode adds a worker studio seat for her on first use, and a direct request is a one-task goal for her seat (`Studio::submit_direct`) | None | 0 |
| Agent record and key | Done | `agent.json` with state, route, desk, and her public key; her secret key in `key` (mode `0600`) beside it; the owner's NIP-OA `auth` attestation with a `created_at<` expiry of at most a year; `openagents agent new`, `attest`, `list`, `show`, `stop`, `pause`, `resume`, and `retire` | The key in the host's keychain or key store rather than a file | 0.5 |
| Walk-up composer | Done | The desk panel, anchored to the bottom of the window: status row, transcript with a scroll bar, `PROPOSED:` line, input line, key strip; F2 memory, F4 journal, F7 stop, F8 pause or resume, each of the last two after CONFIRM | None | 0 |
| Request routing | Partial | `auto`, `task`, and `terminal` modes; a word list chooses for `auto` | The typed task-or-terminal question and its threshold | 1 |
| Typist | Done | The pane badge `driven by alice`, take-back on any key | NIP-TERM typist record with `kind: "agent"` | 0.5 |
| Agents as typists | Done | The host hands each checked command to the window that asked with a typist, which types it and reports the block (`studio.agent.ran`); a request without a typist runs under the host's subprocess supervisor | Typing into a host-owned NIP-TERM terminal that every device can attach to | 2 |
| Terminal executor | Done | Typing a few characters a frame, the smart terminal's command blocks as completion marks, the output block back to the host | As above | 0 |
| Effect classes and decisions | Partial | The closed read-only list and the deny list on the host; CONFIRM or REJECT bound to the step, answered once, journaled with the answering device | The second key for destructive commands; standing rules for her; a typed effect-class question | 1.5 |
| Reporting | Done | Her transcript and nameplate; her own chat thread on the host, which syncs to the desktop app and the phone; a NIP-WS summary whose headline is host state | Walking to you in the world when she reports (she goes to the Podium only to ask) | 0.5 |
| Journal and kill switch | Done | Stop's four steps, each journaled; pause; resume; retire deletes her key and keeps her journal | Revoking grants on other computers, once she has any | 0 |
| Memory | Done | Typed entries (project, preference, outcome, note) of at most 2 KiB; preferences she proposes wait for your acceptance; the secret screen moved to the shared `secret-screen` crate, which refuses credential shapes and this host's exact credential values; forgetting journals that it forgot, not what; briefings of at most 12 KiB with the selection receipt journaled | Edit and export; relevance beyond shared words | 1 |
| Standing jobs | Done | The scheduler on the host's sweep, admission per occurrence (her state, expiry, occurrences, budget, capacity), refusals journaled and skipped, a missed slot fired once, and the three templates, all off until you turn one on | Program actions from `programs/`; metered spend per occurrence (each is recorded as unmetered) | 1.5 |
| Phone | Partial | WS summaries and her thread reach the phone | Agent card, composer, memory and journal views | 2 |
| Other computers | Missing | None | Agent key as a device; invitation flow for it | 2 |
| Smart terminal `@agent` | Done | `@alice TEXT` on the input line routes to her through `openagents agent ask` | None | 0 |
| Service record | Done | Requests, finished, and merged counts from her journal, at the desk and in `openagents agent list` | None | 0 |
| NIP-MV opt-in | Missing | None | Publish her entity state with `role: agent` | 1 |
| Memory relay mirror | Missing | None | AE client: blinded tags, head selection, conflict detection | 3 |
| `work-accept` XP rule | Missing | None | Rule, fixtures, ledger support | 3 to 4 |

About 22 agent-hours remain, most of them phases 4 and 5. Owner checks on
real computers are separate and go in `NEEDS_OWNER.md`.

## Roadmap

1. **Demo: one agent at a desk on this Mac.** Done, with Alice. A resident
   seat named by `openagents agent new alice`, with a key and attestation.
   You walk up in Everglade, press F, and type "run the atif tests and tell
   me what fails." She walks to the Workbench, types `cargo test -p atif` in
   a pane titled `driven by alice`, waits for the result, goes back to her
   desk, and reports. The report is in her thread. Read-only commands only;
   anything else waits at the Podium. The journal and the record survive a
   restart.
2. **Real work.** Done. Task mode through the studio with the Merge station;
   memory with preference acceptance and the secret screen; the stop, pause,
   and retire actions; reports on the phone through WS summaries; `@alice`
   from the smart terminal.
3. **Standing jobs.** Done for the scheduler, admission per occurrence, and
   the three templates: nightly check, watch issues, and keep it green, all
   off by default. Program actions remain.
4. **Other computers.** About 2 agent-hours. A delegated grant to the
   agent's key on your Linux host; terminal mode and task mode there.
5. **Presence and growth.** About 7 agent-hours. NIP-MV opt-in, the AE
   memory mirror, and, after you decide on it, the `work-accept` rule. The
   service record is done.

The [workbench roadmap](../terminal/workbench-roadmap.md) owns the terminal
work that phase 1 shares (typist, agents as typists); this page adds no
second implementation of it.

## Open questions for the owner

Until the owner says otherwise, the implementation takes these defaults,
which the owner set on October 6, 2026, for the first three:

1. **Seat or separate service.** This page makes the agent a resident studio
   seat. Is that right, or should it be a separate host service that uses
   the studio only for task mode? **Default: a resident studio seat**, not a
   separate service.
2. **Terminal mode on your checkout.** Terminal mode may open in a workspace
   root and run non-read-only commands there with approval. Should it be
   limited to the agent's own worktree instead? **Default: in terminal mode
   she may run read-only commands anywhere in the workspace, and
   non-read-only commands only after your CONFIRM; task mode works only in
   her own worktree.**
3. **Read-only commands without asking.** Should `read_only` commands run
   without a decision by default, or should every command wait for you until
   you add rules? **Default: read-only commands run without asking.**
4. **Pull requests.** Should "open a pull request" be a podium decision the
   agent can request, or only something you do from the Merge station?
5. **Its key on other computers.** Is a delegated NIP-HOST grant to the
   agent's key the right way to reach your Linux host, or should the agent
   act only through its home host?
6. **Memory on relays.** Should the AE memory mirror exist at all, given
   that it reveals to relays that you use an agent with memory?
7. **Public presence.** Should the agent ever appear outside your own
   instances, for example on the plaza beside your avatar?
8. **More than one agent.** Is one agent the product, with the studio's
   team for bigger goals, or should you be able to keep several named
   agents?
9. **Leveling.** Is a local service record enough, or do you want the
   `work-accept` XP rule with you as referee?
10. **Budget defaults.** What daily model budget should a new agent start
    with: the studio's reference run cost $0.19 per two-task goal.
