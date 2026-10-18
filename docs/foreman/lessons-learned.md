# Foreman: lessons learned

These lessons come from the
[2026-10-17 close-timeline audit](../audits/2026-10-17-nostr-identity-close-timeline.md)
and the [review of the two Foreman chats](chat-review.md). Each one names the
failure, what should have caught it, and what is missing. Items marked
**Proposal** don't exist yet.

## Summary

| # | Failure | Cost | Should have caught it | Missing |
| --- | --- | --- | --- | --- |
| L1 | Coordination ran in a terminal session that no one could see or steer. | 71-minute stall; 4 h 45 min from code complete to close. | Foreman, the phone, the website. | Coordinators as app chats, or a way to steer a terminal session. |
| L2 | Foreman's own turn hung for 4 h 12 min. | Foreman absent for the disk-full event and the whole integration window. | A host watchdog. | Per-call timeouts and a host-level turn watchdog. |
| L3 | Foreman saw the stall at 19:13 and only offered. | About 20 more minutes, until the owner asked. | The stall rule. | A stall rule with authority to act. |
| L4 | Check-ins were never turned on; no decision was logged. | No host-driven wake-ups, no memory between turns. | The #11965 defaults. | On by default, with a visible "not set up" state. |
| L5 | Foreman's watch lived in the harness process and died with it. | Missed alerts 14 or more times. | The host. | Host-owned watches. |
| L6 | Agents handed off code that had never compiled. | About 55 min of serial compile fixes. | `verify` in the briefed agent. | Dispatch through `chat work`. |
| L7 | Branches were merged one at a time by hand. | About 60 min. | The landing queue. | Dispatch through `land submit`. |
| L8 | Every build ran on the owner's Mac; the disk hit 0 bytes. | 16 min dead, about 35 min total, plus three near misses. | The GCE pool, the build sweep. | Builds off the Mac by default; disk in Foreman's state. |
| L9 | A red `main` had to be compared failure by failure. | About 50 min. | A typed "is this on `main` too" judgment. | System One questions for integration. |
| L10 | Rules given to chats were wrong (build count, one global queue). | Stalled queues for 10–30 min. | Typed checks instead of hand-written shell. | Shared, tested probes. |
| L11 | Probes changed production (visitor tokens, analytics). | Polluted `app_visitor_started` from about 12:50 to 14:11. | Read-only probes. | A probe contract. |
| L12 | Foreman acted with its own hands outside a recorded decision. | Unlogged cache deletions; a stopped deploy script. | The decision log. | Decisions recorded for every out-of-band action. |
| L13 | Nothing measured Foreman. | No way to see whether it helped. | A metric. | Code-complete-to-close and stall minutes. |

## L1. Coordination ran where no one could see it

**What happened.** The terminal Claude Code session `claude-code:a9366ded`
(pid 98355, started 00:39) filed, claimed, and integrated the Nostr identity
issues. Its subagents were Task-tool children inside that session. At 18:25
it ended its turn with a rebase stopped on a conflict, and nothing ran until
19:37 (audit). Foreman saw only the claim markers: "It runs outside the app,
so I can't see its chats" (19:15). The owner couldn't reach it from the
phone.

The same pattern ran the night before: Foreman 1's two epic subagents were
Task-tool children and died at a session restart ("2 background agents
didn't finish before the previous session ended", 2026-10-16 20:33).

**What should have caught it.** Foreman's `list_chats` and `send_message`,
the phone, and the website, all of which work on app chats
(`apps/openagents/crates/mcp/src/tools.rs`).

**What is missing.**
- A rule and a default: a session that coordinates issues is an app chat
  (Proposal; the audit addendum's step 1).
- For the sessions that still run in a terminal, a way for Foreman to read
  their progress (the Claude Code session file's last write, the run record
  in `~/.openagents/runs/<id>/run.json`, `~/.openagents/activity/<pid>.json`)
  and either nudge them or refuse to treat their claims as live (Proposal).

## L2. Foreman's own turn hung

**What happened.** At 14:11:56 Foreman ran one shell command: `curl` (no
`-m` timeout) against production, then `lsof` for each cargo process. Its
result was never recorded. The session file has nothing until 18:24:00,
when the session ended and resumed. Foreman's template says "No single tool
call blocks for more than 2 minutes" (`crates/agent-lab/src/templates/foreman.md`),
but only `wait_for_turn` enforces a cap (`MAX_WAIT_SECS`, #11969).

**What should have caught it.** The host. `Foreman::due` already knows
Foreman's chat is busy, and returns nothing in that case
(`apps/openagents/crates/engine/src/foreman.rs`). That is exactly the
condition under which no one is watching.

**What is missing.**
- A watchdog on Foreman's own chat: busy with no new output for N minutes
  (for example 10), interrupt the turn and start a check-in that says why
  (Proposal).
- A default timeout on every shell call the harness runs for Foreman, and
  `timeout`/`-m` in every probe it is told to run (Proposal).

## L3. The stall rule only observed

**What happened.** At 19:13 Foreman wrote "no builds and no commits for over
20 minutes while 17 issues are open", and at 19:15 decided to wait for the
6-hour stale mark (about 20:00–20:30). The claim rule ("leave an issue
another claim holds", `AGENTS.md`) and the 10:06 chain of command ("you
report to me") both told it to wait.

**What should have caught it.** The audit's proposed stall rule: a claimed
issue whose chat or queue entry shows no progress for 20 minutes gets a
message; if that doesn't move it, Foreman restarts the chat and tells the
owner.

**What is missing.**
- Progress as a first-class signal, separate from the claim's age. The
  engine knows only `ChatFinished`, `ChatErrored`, `ChatAwaitingInput`,
  `IssueOpened`, `IssueClosed`, and `StaleClaim` after six hours
  (`STALE_CLAIM_MS`, `engine/src/foreman.rs`). There is no "working with no
  output" event and no "claimed with no commit" event (Proposal).
- A rule that a stall gets an action, not an offer, and that a terminal
  holder that can't be reached loses the claim after the stall window
  (Proposal; changes `AGENTS.md`'s claim rule for Foreman).

## L4. The autonomy that shipped was never switched on

**What happened.** #11965 landed by 10:14. Foreman itself said at 10:05:
"Once you rebuild the dev app and turn on Foreman check-ins in Settings →
General, I'll wake on a schedule". The app was rebuilt by 18:21, but there
is no `foreman/` state on disk and Foreman made zero `record_decision`
calls. `ForemanSettings::default()` has `chat_id: None` and
`authority: false` (`apps/openagents/crates/proto/src/foreman.rs`).

**What should have caught it.** The After-close check for #11965
(master delegation), and a visible "Foreman isn't set up" state.

**What is missing.**
- When a saved agent named Foreman exists, the host picks its chat and turns
  check-ins on by default, and the owner turns authority off rather than on
  (Proposal).
- Foreman's own chat says when check-ins are off, and the phone shows it
  (Proposal).

## L5. Foreman's watch lived in the harness

**What happened.** Foreman's heartbeats came from a `Monitor` shell loop it
started itself (29 `Monitor` calls in one day). The loop died at each
harness restart (at least 14 times on 2026-10-17) and at its 30-minute
expiry. At 11:10 the owner found everything stopped: "My session restarted
again, which killed my watch."

**What should have caught it.** The host's check-in schedule and event
wake-ups (#11965), which run in the engine, not in the harness.

**What is missing.** Disk, build count, production health, and main-commit
events as host events that wake Foreman, so Foreman needs no shell loop
(Proposal; extends the `Event` enum).

## L6. Agents handed off code that never compiled

**What happened.** At 14:33 the rule was "agents write code only and the
coordinator does every build". Integration then found compile errors at
15:05, 16:08, 16:22, 17:00, 17:12, and 17:53, one at a time (audit).

**What should have caught it.** The briefed agent's `verify`
(`scripts/work/work_issue.py`, [`docs/cloud/work-on-issues.md`](../cloud/work-on-issues.md)),
which compiles, tests, and formats on the agent's own host and replays the
checks on the real diff.

**What is missing.** Foreman dispatching through `openagents chat work
--issues N --on gce` instead of `create_chats` with a hand-written brief
(Proposal). The command exists in the installed CLI (`openagents chat work
--help`).

## L7. Hand merges, one branch at a time

**What happened.** The coordinator rebased, built, and tested each branch
separately until the owner ordered one combined pass at 16:54 (audit).
`openagents land status` on 2026-10-17 shows: "integrator: oa-dev-env-1 (not
running, seen 5d ago …) Nothing waiting to land." The queue wasn't used.

**What should have caught it.** The landing queue
([`docs/cloud/land-queue.md`](../cloud/land-queue.md)): documents in seconds,
unrelated code side by side, overlapping code in order, `--issue N` closes
the issue.

**What is missing.** Foreman telling every chat "submit with `openagents
land submit --issue N`, don't push", and reading `land status` on each
check-in (Proposal).

## L8. Builds on the owner's Mac

**What happened.** Free disk on the Mac went 110 → 63 GiB in 16 minutes
(08:58), 33 GiB (about 10:30), 0 bytes (15:42), and 262 → 32 GiB while
Foreman was frozen (14:06–18:25). Foreman 1's evening had the same problem
(2.5 GiB at 19:44, 130 MiB at 20:22 on 2026-10-16). The GCE pool grant was
live all day with no hosts: `openagents cloud status` shows "Pool pb22a59
… granted (epoch 4). 0 host(s)".

**What should have caught it.** The GCE pool
([`docs/cloud/gce-pool.md`](../cloud/gce-pool.md)) and, as a backstop, the
build sweep that now keeps 100 GiB free (`58559c745b`,
[`docs/coder/runtime/leases.md`](../coder/runtime/leases.md)).

**What is missing.**
- Placement as Foreman's default: issue work goes to the pool, and the Mac
  builds only what the owner runs (Proposal).
- Disk, build count, and the capacity book (`openagents capacity`) in the
  facts every check-in carries, so a check-in sees them without a shell
  loop (Proposal).

## L9. A red `main`

**What happened.** `main` had 21 failing tests, so every branch's failures
had to be compared with a `main` baseline (about 50 minutes, #12012).

**What should have caught it.** A typed judgment, "is this failure also on
`main`?", over the two test outputs. System One (`POST /v1/systemone`,
`crates/jev`) answers this kind of question with receipts.

**What is missing.** A question set for integration judgments in
`questions/`, and Foreman or the integrator asking it (Proposal).

## L10. Hand-written rules were wrong

**What happened.** The first build-count command counted shell wrappers,
so #11968's wait loop "never fired" (08:47–08:56). The single push queue
held chats that touched different files (corrected at 09:29). Foreman's
first worktree in-use check matched every open file (11:30).

**What should have caught it.** Shared, tested facts: the build lease
counts (`openagents lease`, `crates/coder-lease`) and `changed_paths`
(#11972).

**What is missing.** Foreman reads these from host tools, not from shell
lines it writes into briefs (Proposal: a `machine_status` tool, see
[sdk-additions.md](sdk-additions.md#s9-machine-and-capacity-facts)).

## L11. Probes that write

**What happened.** Foreman's watch sent `POST /v1/visitors` every minute,
which "mints a real visitor token each time", tripped the per-address rate
limit, and inflated `app_visitor_started` from about 12:50 to 14:11.

**What should have caught it.** A probe contract: health checks are
read-only `GET`s. Foreman saved this as a memory at 18:52.

**What is missing.** A shared list of read-only health probes, so no agent
writes its own (Proposal).

## L12. Out-of-band actions without a record

**What happened.** Foreman deleted build caches and worktrees (11:24–11:31,
14:01, 18:29) and stopped another session's deploy script (13:56). Its
template says "I don't edit files, run cargo, or push commits" and lists
deploys among what needs the owner's approval. None of these went into a
decision log.

**What should have caught it.** `record_decision`, and the rule that
anything outside the standing authority's scopes (`Message`, `Create`,
`Steer`, `Stop` in `proto/src/foreman.rs`) is either delegated or asked.

**What is missing.** Machine actions (prune, stop a process) as explicit,
logged scopes, or a rule that they go to a chat. Built in #12033: the owner
grants `build.prune` and `process.stop` per agent ID, the host refuses
them otherwise, and every action and refusal is logged
([leases.md](../coder/runtime/leases.md), "Machine actions"). Still
missing: machine log lines in Foreman's decision log.

## L13. No measurement

**What happened.** Nothing recorded how long issues waited between code
complete and close, or how long any chat sat stalled. The only numbers are
from the audit, after the fact.

**What should have caught it.** The decision log and the board.

**What is missing.** Two metrics computed from GitHub and chat status:
minutes from code complete (last push to the issue's branch, or a "ready"
event) to close, and stall minutes per chat (Proposal; roadmap item 13).

For comparison, on 2026-10-17 the coordinator's 11 issues took 4 h 47 min to
5 h 17 min from code complete to close. Foreman's six parallel chats,
dispatched at 19:58, closed #12018 in 21 minutes, #12022 in 33, and #12019 in
35.
