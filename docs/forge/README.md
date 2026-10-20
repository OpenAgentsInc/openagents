# Forge: the software factory

Status: proposal, 2026-10-20. Nothing in this folder is built yet. The
definition below is the owner's, from 2026-10-20. It's part of the
[Enter the Verse roadmap](../roadmap/2026-10-20-enter-the-verse.md).

## What the Forge is

The Agent Lab builds agents. The Forge puts them to work. The Gym makes them
better.

The Forge turns agent work into production-grade software. You hand it a
piece of software work. It runs agents from the Lab on that work, using the
harnesses, models and Pylons the agents are bound to. It collects evidence,
verifies the result, and delivers a change that a person can merge with
confidence. Every run's outcome goes to the Gym as a measured result, and
the Gym's findings go back to the Lab as better agent definitions.

```text
 Agent Lab ──agents──▶ Forge ──deliveries──▶ your repository
     ▲                   │
     │                   └──outcomes──▶ Gym
     └──────better agents──────────────┘
```

"Production grade" means a delivery carries:

- the change, as a draft pull request on the repository's existing host;
- the evidence the run produced: commands, tests, logs and the agent trace
  (ATIF);
- a verification report that judges that evidence against the work order's
  checks, kept separate from the evidence itself;
- a delivery receipt that records what was delivered, where, by which agents
  and runs, and under which verification.

The Forge is not:

- **A git host.** GitHub stays the source and review host. The earlier
  GitHub-replacement direction (GitAfter, Sarah.Forge, the Agent Forge of
  episodes 270–275) stays parked, as [episode 281](../transcripts/281.md)
  decided.
- **A new agent runtime.** Agents run through the harnesses, Coder, Microcoder
  and Pylons that already exist.
- **A separate service in another stack.** It's Rust over the crates we have,
  shown in the web app first and reused by the desktop app.

## Where this came from

The name has meant several things. This definition picks up the April one.

| When | Meaning | What happened | Sources |
| --- | --- | --- | --- |
| Jan 2026 | Any code host that receives patches (GitHub, GitAfter, NIP-34) | An adapter contract. The `Forge` trait in `crates/coder-task/src/task/publish.rs` still uses this sense: "the forge a draft pull request opens on." | ADR-0018 (archived) |
| Apr 2026 | **The internal software factory**: work orders, runs, workspaces, leases, evidence, verification, delivery receipts | A private Rust control plane with worker attach, queue and leases. It was validated locally but never run as a hosted loop. | `OpenAgentsInc/forge` (private), the April Forge spec, [episode 225](../transcripts/225.md) |
| Jun 2026 | Forge as a customer product in the software-factory category, with stages and factory metrics | A `/forge` dashboard on the TypeScript site. Deleted in the September cleanup. | `git show dabc08102f^:docs/blitz/forge/README.md` |
| Jun–Jul 2026 | An owned git coordination layer, with GitHub as a mirror | Partly built, then postponed. | `git show dabc08102f^:docs/forge/` |
| Aug 2026 | Sarah.Forge and the Agent Forge: a self-hosted git forge | Used live, then paused as "too big of an apple". | Episodes [270](../transcripts/270.md)–[275](../transcripts/275.md), [281](../transcripts/281.md) |

Private and earlier-stack code is reference only. We reimplement here; we
don't copy it.

## Objects

These come from the April design, cut down to what the first version needs.
Each is a typed Rust record with explicit state transitions, stored like the
other Agent Lab and task records.

| Object | Represents | Built on |
| --- | --- | --- |
| Work order | One requested unit of software work: target repository, scope, checks that define "done", budget, privacy | New. Created from chat, an issue, the Grid, or Foreman |
| Agent crew | The Lab agents assigned to a work order and their roles (lead, worker, reviewer) | Agent Lab definitions (`crates/agent-lab`, `openagents_sdk::agents`) |
| Run | One attempt at a work order. A work order can have several | Foreman fan-out and `coder::task` |
| Workspace | Where a run works: repository, branch or worktree, computer, harness, limits | Paired computers, cloud computers, `coder-boundary` |
| Evidence bundle | What the run produced: diffs, commands, test output, traces | `coder::task::owner` artifacts and ATIF traces |
| Verification report | The judgment over the evidence: required checks, a review agent's findings, waivers | Gym checks and gates, Coder's checks |
| Delivery receipt | What was delivered and where: the pull request, its verification, its cost | The `Forge` trait's draft pull request, `openagents land`, receipts |

The rules carried over from April:

- A work order exists before any run and outlives every run.
- Evidence and verification stay separate. The bundle is what was collected;
  the report is the judgment.
- A delivery is ready only when its verification passed or a person waived
  a check in writing.
- If a fact must survive a crashed worker or a restarted host, it belongs to
  the Forge's records, not to a harness session.

## How a work order runs

1. **Intake.** A person, Foreman, or an automation writes a work order with
   its checks and budget.
2. **Crew.** The Forge assigns Lab agents. Each step's model or harness
   follows the agent's policy: sensitive steps to an attested Pylon, bulk
   steps to network models such as DeepSeek on Pylons, hard steps to the
   person's own harness.
3. **Runs.** Workers run in their workspaces. The Forge holds a lease on each
   run, so a lost worker is noticed and its work isn't silently retried.
4. **Evidence.** Commands, tests and traces are collected as the run goes.
5. **Verification.** Required checks run and a reviewer agent reads the
   change. The result is a verification report.
6. **Delivery.** A passing run opens a draft pull request with its evidence
   and receipt. A person merges.
7. **Outcome.** The Gym records the result: pass or fail, cost, time, and
   which agents and models did the work.

## In the Grid

The Forge is a place in the Grid, beside the Agent Lab and the Gym. It's
where the repository view and watchable work from the roadmap live:

- The work order board shows queued, running and delivered work.
- Each loaded repository is a floor laid out from the code graph. Workers
  stand at the code they touch, and their messages to each other show as
  exchanges.
- Delivered work leaves through a delivery gate with its pull request link
  and receipt.
- Public runs show their work. Private runs show only that work is happening
  (see the [privacy decision](../roadmap/2026-10-20-enter-the-verse.md#decisions)).

The Grid is the web app first. The desktop app reuses the same GPUI and
Verse pieces.

## First version

1. **Records.** Work order, run, evidence bundle, verification report and
   delivery receipt types with transitions and tests, in a new `forge` crate
   that depends on `agent-lab` and `coder-task`.
2. **One loop.** Foreman takes a work order, fans out workers, collects
   evidence, runs the checks, and opens a draft pull request with a receipt.
   It runs on a test repository during checks.
3. **Web view.** The Forge in the GPUI web app: the board, a run's evidence
   and its delivery. Then the Forge place in the Grid.
4. **Gym link.** Each delivery writes a Gym result. A Gym board shows Forge
   outcomes by agent.

[The proposed issues](../roadmap/2026-10-20-enter-the-verse-issues.md#epic-f-the-forge)
break this down.

## Review addendum (2026-10-20)

The first Forge loop should prove one bounded delivery before building a
new control plane or depending on V4.1. Keep the new records as references
and state transitions over existing Coder tasks, host grants, issue claims,
workspaces/leases, ATIF traces, and Gym checks. Work-order scope and resource
permissions remain explicit; a model's tool output or a capability beacon
cannot expand them.

Bind every verification report to the repository, base and tested head,
work-order/agent-definition versions, exact check commands, results, and
artifact hashes. Keep observed check results separate from a reviewer
agent's judgment; a second agent is useful review, not independent proof
that software is production-ready. A waiver names its author and exact
scope. Rebase or a changed delivery head requires the affected checks to
run again. Opening a draft PR, publishing, merging, and deploying retain
separate authority; use the existing land path and manual/non-GitHub-billed
checks rather than inventing another merge queue or GitHub workflow.

Record an outcome for **every run**, including failure, cancellation,
timeout, and unknown cost. Delivery-only metrics hide failed attempts.
Keep private traces/code out of public Gym evidence and training exports
without a separate grant; preserve protected evaluation splits. A receipt
attributes the work and evidence, not the truth of a reviewer's verdict.

For the first web view, show the work order, its evidence and checks, and
its PR link in shared Rust Native cards rendered by GPUI. The broader PR
browser/preview scope conflicts with the earlier desktop-only decision;
see the [strategy addendum](../roadmap/2026-10-20-enter-the-verse.md#narrow-the-first-web-delivery-and-reuse-the-existing-app-boundaries).
Use a code graph tied to the run's repository revision for the floor. Show
reads, edits, commands, and messages as distinct observed events, with an
unmapped state when necessary; file proximity is not worker ownership.
