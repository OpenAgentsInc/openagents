# S1 selection, frozen 2026-10-10 16:58 UTC

This file was committed before the first run of this batch. It is the S1 rule
from [self-improving-codebases.md](../self-improving-codebases.md): the issues
are chosen before execution, and the inventory
([2026-10-10-inventory.md](2026-10-10-inventory.md)) records every attempt on
every selected issue, including failures. Nothing here is edited after the
first run starts. Corrections go in the inventory, with the reason.

## Source

GitHub project 22 ("V1 Launch — Oct 9"), items with Status **Todo**, read
2026-10-10 16:51 UTC (`gh project item-list 22`). 118 items: 100 Done,
6 In Progress, 6 Blocked, 6 Todo. In Progress and Blocked items are held by
other agents or by owner steps and are not in the source.

## Exclusion rules

An issue is excluded when any of these holds. The rule is read from the
issue's labels, body and comments as they stood at the snapshot.

| Rule | Excludes |
| --- | --- |
| E1 | Owner-only: the issue, or what remains of it, needs the owner's own click, key, approval or sign-off to be done |
| E2 | Payments or money movement |
| E3 | Releases: a versioned release, TestFlight, App Store or Play builds, store submissions |
| E4 | Needs an external console (Cloud console, App Store Connect, a vendor's dashboard) |
| E5 | Explicitly deferred past V1 ("After 1.0") |
| E6 | Held by another agent: a comment or commit within the claim window (6 h) says the remaining work is under way elsewhere |
| E7 | What remains is not a code change (a retrain, a data run, a deploy) |

Loop gaps found while preparing the batch, before any run, are filed on
project 22 and put at the head of the selection, so the loop fixes itself
first. They are marked **gap**.

## Decisions

| Issue | Title | Decision | Why |
| --- | --- | --- | --- |
| #11113 | Interactive answers: typed component catalog, Rust compiler, native renderers | excluded | E5: "After 1.0"; phase 1 landed, phases 2-4 are post-V1 |
| #11220 | Google first: every model and embedding call on our keys goes to Vertex AI first | excluded | E6 + E7: the remaining step is a filefind scorer retrain "running now" by another agent, and a worker release |
| #11226 | Web Environments: push to the repository with the owner's GitHub connection | **selected** | Code: hand runs a repo-scoped token from the GitHub App broker (`crates/oa-auth/src/repos`) |
| #11228 | Fleet rows for cloud environment runs | **selected** | Code: read the land queue and environment runs into `AgentRow` |
| #11233 | Claude plan usage from Claude Code itself | **selected** | Code: `rate_limit_event` / `get_usage` into the usage book and status surfaces |
| #11234 | Pick Claude Code, then connect | **selected** | Code: the remaining web sign-in-in-environment card and the phone card |
| #11242 | Issue flow: `--land queue` hands green changes to the landing queue | **selected, gap** | Filed 16:55 UTC: `chat work --land` accepts only `main` or `pr`, so issue-flow runs cannot land through the #11227 queue |
| #11243 | Trace replay: capture changes the issue flow and landing queue landed | **selected, gap** | Filed 16:55 UTC: `traces.py capture` reads only `coder issue-run` and A/B folders, so issue-flow landings cannot be replay-verified |

## Order and how each runs

Every run: on `oa-dev-env-1` (GCE `c3-standard-22`, spot, 300 GB, image
`oa-coder-host`, project `openagentsgemini`), after
`eval "$(scripts/cloud/dev-env-session.sh)"`, with
`openagents chat work --local --issues N` and Claude Code (the owner's
subscription token) as the engine. At most 2 runs at once. Each issue flow
gets its own worktree and a leased build slot.

| Batch | Issues | Landing |
| --- | --- | --- |
| 1 | #11242, #11243 | `--land main` (the queue mode does not exist until #11242 lands) |
| 2 | #11228, #11233 | `--land queue` through `oa-land-worker`; `--land main` if #11242 did not land, recorded as such |
| 3 | #11226 | as batch 2 |
| scale | #11234, then any Todo issue that passes the rules on a later snapshot (recorded with its snapshot time) | as batch 2 |

The first five are #11242, #11243, #11228, #11233 and #11226.

## Counting

- A distinct issue counts once, however many attempts it takes.
- Briefed-agent (`coder issue-run`) runs count only after #11229 closed
  (it closed before this selection).
- Costs that the run does not record stay unknown, never 0.
- A label (landed and checks pass) counts only with a replay verdict from
  `scripts/bench/traces`; without one the issue is listed as unverified.
