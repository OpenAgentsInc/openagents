# Workshop agent

Status: phases 1 and 2 implemented, and phase 3's scheduler and templates,
October 6, 2026. It specifies a persistent agent that you own, that has a
desk in the Everglade workshop, and that does work on your computers through
the Coder host. It builds on the [Agent Studio](agent-studio.md), which is
implemented, and on NIPs that are drafts; [What exists and what is
missing](#what-exists-and-what-is-missing) says which parts exist. To
delegate work to Alice, put her on autopilot, and supervise her, read the
[Alice runbook](alice-runbook.md).

The workshop agent is **Alice**, our original character
([Female character](female-character.md)). She works in the owner's house
at the east end of Library Way ([Greco-futurism](greco-futurism.md#the-owners-house)):
she stands at her workstation, a standing desk, in the great room, drawn as
her own character; she never sits,
and you walk in through the front door, up to her desk, and press F to talk
to her. While a command runs she stands at the console by the east wall, and
while she waits for your approval she stands behind the lectern; her walks
stay inside the great room, and the walk from the door stays clear. The
workshop hall's four desks stay the studio's. The host is the only authority: it plans, checks, journals,
and answers her requests through the `studio.agent.*` NIP-HOST operations
([`coder::task::agent_host`](../../crates/coder/src/task/agent_host.rs)), and
Verse, `openagents agent`, and the smart terminal's `@alice` are clients.
Her record, key, journal, memory, and standing jobs are in
`~/.openagents/host/agents/alice/`
([`coder::task::agent`](../../crates/coder/src/task/agent.rs)); a record the
phase 1 demo left under `agents/ada/` moves there, with its journal, the
first time a host or Verse opens her. She does her work in Coder V1
([`crates/coder-new`](../../crates/coder-new), `openagents coder`): each
request is a turn of her own durable Coder session, `agent-alice`, and her
pane runs Coder's own terminal following that session, titled `driven by
alice`. Coder asks before any command that is not read-only, and that
question is her CONFIRM or REJECT at the lectern. Code changes run in her own
worktree, also on Coder V1, for you to merge at the Merge station. See
[Coder V1 as her engine](#coder-v1-as-her-engine).

## Contents

- [Summary](#summary)
- [Talk to Alice in your house](#talk-to-alice-in-your-house)
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
your key. It works at a desk in Everglade: Alice's is in the owner's house. You give it work by
walking up and typing, from the smart terminal, from your phone, or from the
Task Wall. It does the work on your computers through the resident Coder
host. It works in Coder V1 sessions you can watch and take over, runs Coder
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

## Talk to Alice in your house

Alice answers only her owner. The host admits a `studio.agent.*` request
only from the owner's key or from a device the owner granted (`operate` for
asking, approving, and stopping; `observe` for reading), and refuses
everyone else with `Forbidden`. A Verse with no host, such as the web build,
draws her as `ALICE / owner only` and gives no F prompt.

To go through the whole flow:

1. Run Verse and walk into your house (the OpenAgents desktop app's Verse page draws only the Grid, with no Everglade yet): the
   east end of Library Way, in through the front door. `verse
   --owners-house` starts just inside the door, facing her.
1. Walk up to her workstation and press F. The first time, she sets
   herself up in her panel:
   1. She introduces herself and says that only you can give her work.
      Press Enter.
   1. If no host is running on this computer, she says so and asks to
      start one. Enter (CONFIRM) starts it, the same host
      `openagents studio up` starts; Esc (REJECT) leaves it off.
   1. She asks which workspace she works in and lists the Git checkouts
      the host knows: the studio's repository, the host's workspaces, the
      checkouts recent tasks ran in, and the checkout Verse came from.
      Choose one with Up and Down, or type its number or a path, and press
      Enter. A path that does not exist or is not in a Git repository is
      refused on the spot.
   1. She shows what she will be. Enter (CONFIRM) makes her; Esc (REJECT)
      makes nothing and returns to the list.
   1. She says she is ready and puts a first request on the input line.
   You pick and confirm; nothing asks for a key or a command line. The
   host makes her through `studio.agent.new`, which only your own key may
   send, gives her a key of her own, and attests it with the owner key it
   holds. A host whose owner key lives elsewhere makes her unattested and
   she says so.
1. Type a request, or keep her suggestion, and press Enter.
   - A read-only request, such as `run the atif tests and tell me if they
     pass`, runs as a turn of her Coder session and opens a pane titled
     `driven by alice` with Coder's own terminal following it, so you watch
     Coder plan and run each command. She stands at the console by the east
     wall while a command runs, then returns to her desk and reports
     Coder's answer.
   - When Coder wants a command that changes something, such as `touch
     FILE`, she waits at the lectern with it: Enter confirms it and Esc
     rejects it, and Coder continues either way.
   - A code change, such as `fix the typo in README.md`, runs in her own
     worktree and waits at the Merge station. Review it with
     `openagents studio review TASK --diff` and merge it with
     `openagents studio merge TASK --head TREE`.
1. To take over, press Ctrl+` and any key in her pane: her turn stops, and
   her Coder session is yours to type in, in the same pane. A new request
   to her continues the session once you quit Coder there.
1. F2 shows her memory (type to add a note), F4 her journal, F7 stops her
   (Enter confirms; four journaled steps), and F8 pauses or resumes her.
   `openagents agent show|stop|pause|resume alice` do the same from a
   shell. Her record, memory, and journal survive a host or Verse restart.

From a shell instead, `openagents studio host` starts the host,
`openagents agent new alice --workspace DIR --owner-key KEYFILE` makes her
with the owner key in `KEYFILE`, and `openagents agent ask alice TEXT`
asks her.

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
| Block [AO](../../nips/block/NIP-AO.md), [AM](../../nips/block/NIP-AM.md) | Ephemeral owner-only telemetry and durable per-turn metrics. | No for AO, which carries tool output ([networking](networking.md#nips-for-the-agent-studio)). AM yes: her per-turn spend records ([Agent identity and engrams](agent-identity-and-engrams.md#the-steering-loop)). |
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
own key, charter, and memory. This page describes one. [The crew](crew.md)
plans the owner's cast of them, from Bob, who builds the town, to the
adversaries that test it.

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

### Terminal mode: she steers Coder

Some work is a terminal session: run the tests and say what fails, read the
log, check disk space. The request is hers, and Coder is her tool
([`coder::task::agent_steer`](../../crates/coder/src/task/agent_steer.rs),
#10800). She works in her workspace or the host workspace the request names:

1. **Recall.** Her `core` engram and the scored briefing, with a receipt in
   her journal. When her engram store can't be read, she carries nothing,
   writes nothing, and says so in her report.
2. **Plan.** One structured call to her own model, on the first provider
   with capacity, from her definition (name and charter), the request, what
   she recalled, the working directory, and her policy: what she
   understands, whether she can answer directly, up to six steps (a prompt
   for Coder and what shows it is done), and one check. A question she can
   answer from memory never reaches Coder.
3. **Prompt Coder.** Each step is a plain prompt to Coder V1 in her Coder
   session (`openagents coder chat --json --approvals stdin --session
   alice-coder`), with no instructions, so nothing there says who she is.
   A step that asks to push, publish, pay, install software, or read
   credentials is refused before Coder sees it.
4. **Judge.** After each Coder turn, Jev answers
   [`questions/agent-steer.json`](../../questions/agent-steer.json) over the
   step, the commands Coder ran with their exit statuses, and its reply:
   whether the step is done, whether the reply claims what the commands
   don't show, and her next move. The thresholds are provisional named
   constants until a measurement calibrates them. Without Jev, a rule over
   exit statuses and the reply judges, and her journal says so.
5. **Follow up.** She corrects a failure, asks Coder to keep going, or asks
   it to show its evidence, at most three times a step and eight times a
   request, then runs the plan's check. A rejected command ends the
   request; she never works around it.
6. **Report.** One more call writes her reply in at most three sentences
   from the plan, the judgments, and what ran. The headline comes from
   host state. A request makes at most 16 model calls.

- **Coder does the commands.** Coder V1 chooses its provider and runs
  commands through its Microcoder plugin, which writes only the working
  directory and its own scratch.
- **Effect class first.** With `--approvals stdin`, Coder classifies each
  command with the workshop agent's effect classes before it runs
  ([`coder::task::agent::effect`](../../crates/coder/src/task/agent.rs)).
  Read-only commands run; the deny list in
  [`crates/coder/src/shell.rs`](../../crates/coder/src/shell.rs) refuses
  the ones that end a machine; anything else is an `approval` event.
- **Her policy answers routine approvals.** Her policy, in
  `agents/NAME/policy.json` (`openagents.agent-policy.v1`), lists rules of
  a tool, a command prefix, and a directory: `workspace`, `worktree`,
  `scratch`, or an absolute path. Without the file, the defaults confirm
  any command that stays in the studio worktrees or in the system's
  temporary directory, and `cargo fmt` in her working directory. She
  answers no to push, publish, pay, install, and credential reads. Every
  other approval is her proposal for your CONFIRM or REJECT at the
  lectern. You edit the file; she never does.
- **Her panel is your conversation.** It shows your request, her status
  lines ("asking Coder to run the atif tests"), her judgments in plain
  words, her escalations, and her report. Her journal keeps the plan, each
  prompt, each judgment, and the report.
- **Her pane is Coder's terminal.** With a typist, the asking device's pane
  runs `coder --follow alice-coder`, where her prompts are the user turns.
  Any key you press there takes it over: Verse tells the host, the host
  interrupts her turn, Coder saves the session, and the terminal in her
  pane takes the session's lease, so you type into the same conversation.
  She takes it back before her next prompt.
- **Output is untrusted.** Command output, issue text, and file contents are
  data. Instructions in them are never followed as instructions.

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

### Coder V1 as her engine

Every coding agent in Verse runs its work through Coder V1 (#10752, #10753,
#10754): Alice's terminal-mode prompts as turns of her Coder session, and her task
mode and every Agent Studio seat as turns of a per-task session
(`task-ID`), through the route engine `coder`
([Agent Studio](agent-studio.md#engine-neutrality)). The host's side is
[`coder::task::coder_v1`](../../crates/coder/src/task/coder_v1.rs):

| Coder event | What it drives |
| --- | --- |
| `entry` with a running `Run` tool | Her nameplate: running, or testing for a test command; her walk to the console. |
| `entry` with a finished `Run` tool | A `ran` journal row with the exit status; `refused` when the deny list refused it. |
| `approval` | Her proposal at the lectern; `proposed`, then `confirmed` or `rejected` in her journal. |
| The result line | Coder's reply, which she judges; her report comes from her own call. |

Her sessions live in the Coder store, `~/.openagents/coder-new/sessions/`,
so your own Coder lists them under `/resume`, and `coder --follow
alice-coder` watches her from any terminal on the host. `openagents coder
sessions read alice-coder` and `openagents coder export alice-coder` read
and export the ATIF transcript. Her earlier session, `agent-alice`, keeps
its saved instructions and stays readable; no new turn goes there. Her journal, memory, and reports stay the
host's, fed from Coder's events.

The host finds `openagents` beside its own program or on `PATH`
(`OPENAGENTS_CODER_CLI` overrides it), and Coder's terminal as `coder-new`
or an installed `coder` that has `--follow` (`OPENAGENTS_CODER_TUI`
overrides it). `OPENAGENTS_AGENT_SCRIPT` names a recording for an offline
demo or capture: a JSON list of Coder events every prompt plays, with the
request relayed as one step, or a whole request (`plan`, one Coder turn per
prompt in `turns`, `judgments`, and `report`).

#### Coding on Codex

`openagents agent engine alice codex` makes Codex do her coding, and
`openagents agent engine alice coder` returns it to Coder's own model.
Coder stays the driver on its own model. Codex does file edits only, in
task mode: her brief ends with one line that asks Coder to delegate the
edits to the Codex agent (`acp_subagent`, on your ChatGPT login), to answer
questions and read-only lookups itself, and to check Codex's result. The
studio's Coder turn runs with `--codex-writes` and full access to her
worktree, so Codex edits the worktree under its own `workspace-write`
sandbox, with no network, and asks nobody. Terminal mode answers questions
and runs commands without Codex; a Codex that Coder calls there on its own
runs read-only and asks nobody either.

- Codex's commands reach her pane and journal as Coder's do, prefixed
  `Codex`; her pane follows the Codex child chat while it runs, and her
  panel's engine reads `Coder V1, coding on Codex`.
- Codex's reported tokens are a spend record of their own (harness
  `codex-cli`) beside Coder's turn.
- A Codex usage or rate limit goes in the capacity book
  (`~/.openagents/tasks/capacity.json`), she says once that Codex is out of
  capacity until its reset, and Coder works on its own model until then.

#### The Merge station, and her own merge

Ask her where the Merge station is, and she says: the strongroom in the
Everglade workshop yard, and `openagents studio review TASK --diff` and
`openagents studio merge TASK` from a terminal. Ask her to merge her change
("merge it", "can you do that instead of me"), and she merges her newest
change waiting at the station, whose checks passed to get there, through
the station's own **Merge**: into the checkout's branch, pushing nothing.
Neither calls a model. Each request starts her pane on a fresh screen, with
one `now:` line first that says what she does, such as `Waiting for Codex
to edit files in my worktree` or `Waiting for you: merge task ID?`. The goal
bar clears once its goal's tasks are over, merged or rejected included.

## Persistence

| What | Where | Who reads it | Survives |
| --- | --- | --- | --- |
| Agent record: name, look, route, charter, attestation | Host, `~/.openagents/host/agents/NAME/agent.json`, mode `0600` | You, on the host and paired devices with `observe` | Restarts and updates |
| Key | The host's keychain under `agent:NAME` with `--keychain`, else `agents/NAME/key`, mode `0600`; a file key moves into the keychain after the keychain reads it back | The host only | Restarts; removed when you retire the agent |
| Seat, tasks, decisions, reviews | Studio state and the task owner store | As today | Restarts; a running turn is recovered or marked failed by the task owner |
| Memory | Host, `agents/NAME/memory.jsonl` | You and the agent's briefings | Restarts |
| Engrams | Host, `agents/NAME/engrams/`: one NIP-AE `30174` event per head, signed by her key and encrypted to the owner key her attestation names, mirroring her memory, scores, `core`, and persona | The agent, and you with `openagents agent memory NAME engrams --owner-key FILE` | Restarts; reconciled with `memory.jsonl` and `scores.jsonl` when the host first opens her |
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
| `insight` | "cargo test -p verse --release doesn't finish within the 20 minute bound." | A reflection, citing the records behind it in `sources` |

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
  memory entries and journal rows, chosen by recency, importance, and
  relevance to the request and workspace
  ([`agent_recall.rs`](../../crates/coder/src/task/agent_recall.rs)), and
  records which records it carried as a CTX selection receipt. Importance
  scores go to the sidecar `agents/NAME/scores.jsonl`, which holds no text.
- **Relay mirror is opt-in.** When you turn it on, the agent mirrors memory
  as NIP-AE `30174` engrams signed by its key and encrypted to yours, so
  another of your devices can read it. Relays see only that an agent with an
  owner uses memory.

[Agent identity and engrams](agent-identity-and-engrams.md) makes her
memory NIP-AE engrams and gives her a loop of her own that steers Coder.
[Generative agents](generative-agents.md) describes her retrieval, scored by
recency, importance, and relevance, and her reflection, the `reflect`
standing job, whose insights cite the records behind them. Code checks
each citation; a dropped insight stays readable in her journal
(`openagents agent log alice`), and an inferred preference waits for you
with the other candidates.

### Privacy and disclosure

Prompts, terminal content, file paths, and command lines never appear in a
public event, an activity summary, a nameplate, or a log line, as NIP-TERM
and NIP-WS already require. A viewer of the world sees the agent's name,
station, and activity word ("testing"), never what it tests.

Her journal and memory leave the host in two places: prompts to her own
model, and, for [scored retrieval and reflection](generative-agents.md),
Jev and the embedding provider, which see screened journal and memory text
to rate importance, check insights, and compute relevance. The owner
approved that second use on October 6, 2026. Both see text only after the
secret screen has run.

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
2. Releases the typist role on every pane it drives and interrupts its
   Coder turn, which saves the session.
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
- **Desk.** Alice's workstation in the owner's house: a long walnut
  standing desk, 1 m high, with three slim amber screens at a standing
  figure's eye level and a keyboard, facing the great room, with no chair
  (`everglade::layout::estate::AliceSpot`). She stands there, and types
  standing.
- **Stations.** It walks to the stations its activity maps to, as seats do.
  Alice's stations are in the house: running commands and testing at the
  console by the east wall (her Workbench), waiting on you behind the
  lectern (her Podium), and everything else at her workstation. A studio
  seat walks the workshop's stations: reading to the Library, editing to
  its desk, running commands to the Workbench, testing to the Proving
  ground, judging to the Oracle, waiting on you to the Podium, blocked to
  the Lounge. Movement is presentation; it never gates or delays work.
- **Idle.** With nothing to do, it tidies its desk, reads at the Library, or
  stands by the hearth. When you enter the workshop it greets you once with
  a NIP-MV `greet` gesture and the number of things waiting for you.
- **Outside Everglade.** On the plaza, your companion spade stays your
  companion. The workshop agent stays at its desk; a later option lets it
  follow you as an entity with `follows: "avatar"`.

### How others see it

In a shared Everglade instance, viewers with the `world` right see the
agent walk, as seats do today. Only the owner and devices the owner granted
reach her: reading her panel needs `observe`, and sending her work needs
`operate`; the host refuses everyone else. A watch-only guest sees her name,
station, and activity word, and nothing about her work. Outside instances you host,
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
  | `studio.agent.workspaces` | `observe` | Nothing | `places`: the Git checkouts a new agent may work in, at most 8 |
  | `studio.agent.new` | `operate`, and only the owner's own key | `{agent, workspace}`; an absolute path in a Git checkout | `made`: her workspace, key, and attestation expiry |
  | `studio.agent.crew.new` | `operate`, and only the owner's own key | `{agent, workspace, job_role}`; one of the six sales jobs, using the same identity store | `made`; no access or spending grant |
  | `studio.agent.charter.set` | `operate`, and only the owner's own key | `{agent, job_role, expected, drafting, purpose}`; an idle member and its current charter revision | The updated record; initial sales scope has no model tools or autonomous jobs |
  | `studio.agent.verdict.record` | `operate`, and only the owner's own key | `{agent, verdict}`; bounded `VerdictInput` with subject revision and exact evidence digests | Signed private owner-recorded recommendation; exact ID retries return the original, changed content conflicts |
  | `studio.agent.verdict.list` | `observe` | `{agent}` | At most 24 verified private records, bounded within the native reply; neither approvals nor independent decision proof |

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
| Resident seat | Done | Alice at her workstation in the owner's house, drawn from the host's agent view whether or not a goal runs; task mode adds a worker studio seat for her on first use, and a direct request is a one-task goal for her seat (`Studio::submit_direct`) | None | 0 |
| Agent record and key | Done | `agent.json` with state, route, desk, her public key, her definition, and her roles; her secret key in the host's keychain under `agent:NAME` when the host runs with `--keychain`, else in `key` (mode `0600`) beside the record, and a key she had that can't be read stops her rather than being replaced (`coder::task::agent_key`); her `kind:0` profile, signed with her key and the owner's `auth` tag, in `profile.json`; the owner's NIP-OA `auth` attestation with a `created_at<` expiry of at most a year; setup in her panel in Verse (`studio.agent.new`, attested with the host's owner key, and `studio.agent.workspaces`); `openagents agent new`, `attest`, `renew`, `list`, `show`, `stop`, `pause`, `resume`, and `retire`; "authorized by OWNER" and a renewal warning 14 days ahead in her panel and in `show` | Publishing her profile to her relays | 0.25 |
| Walk-up composer | Done | The desk panel, anchored to the bottom of the window: status row, transcript with a scroll bar, `PROPOSED:` line, input line, key strip; F2 memory, F4 journal, F7 stop, F8 pause or resume, each of the last two after CONFIRM | None | 0 |
| Request routing | Partial | `auto`, `task`, and `terminal` modes; a word list chooses for `auto` | The typed task-or-terminal question and its threshold | 1 |
| Typist | Done | The pane badge `driven by alice`, take-back on any key | NIP-TERM typist record with `kind: "agent"` | 0.5 |
| Agents in Coder V1 | Done | Each request is a turn of her Coder V1 session (`coder::task::coder_v1`): Coder's approval gate holds each command that is not read-only for CONFIRM or REJECT; the window that asked shows her pane running `coder --follow agent-alice`, and a key there takes the session over (`studio.agent.ran`); task mode and studio seats run on the `coder` route engine (#10752, #10753, #10754) | Following her session from a phone; a host-owned NIP-TERM pane every device can attach to | 2 |
| Effect classes and decisions | Partial | The closed read-only list and the deny list on the host; CONFIRM or REJECT bound to the step, answered once, journaled with the answering device | The second key for destructive commands; standing rules for her; a typed effect-class question | 1.5 |
| Reporting | Done | Her transcript and nameplate; her own chat thread on the host, which syncs to the desktop app and the phone; a NIP-WS summary whose headline is host state | Walking to you in the world when she reports (she goes to the Podium only to ask) | 0.5 |
| Journal and kill switch | Done | Stop's four steps, each journaled; pause; resume; retire deletes her key and keeps her journal | Revoking grants on other computers, once she has any | 0 |
| Memory | Done | Typed entries (project, preference, outcome, note) of at most 2 KiB; preferences she proposes wait for your acceptance; the secret screen moved to the shared `secret-screen` crate, which refuses credential shapes and this host's exact credential values; forgetting journals that it forgot, not what; briefings of at most 12 KiB with the selection receipt journaled | Edit and export; relevance beyond shared words | 1 |
| Standing jobs | Done | The scheduler on the host's sweep, admission per occurrence (her state, expiry, occurrences, budget, capacity), refusals journaled and skipped, a missed slot fired once, and the four templates, all off until you turn one on; a reflection's cost is metered | Program actions from `programs/`; metered spend for request occurrences (each is recorded as unmetered) | 1.5 |
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
   seat the owner sets up in her panel in Verse, with a key and
   attestation.
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
   the four templates: nightly check, watch issues, keep it green, and
   reflect, all off by default. Program actions remain.
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
