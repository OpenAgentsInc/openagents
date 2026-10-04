# AgentCraft parity

Status: gap analysis, October 4, 2026. The owner asked for parity with
[AgentCraft](https://github.com/blendi-remade/agentcraft) (studied at
`0be815d`). This page maps AgentCraft's user-facing features onto the
[Agent Studio](agent-studio.md) in [Everglade](everglade.md) and names the
issue that closes each gap. AgentCraft runs Claude agents only; the studio
keeps every Coder route, so parity here means the same experience with any
engine.

Legend: **Done** works end to end today; **Partial** exists with a named
limit; **Missing** does not exist.

## Doing real work safely

| AgentCraft | Studio today | Gap issue |
| --- | --- | --- |
| A worktree and branch per task (`agentcraft/<agent>/<task>`) | Done: each released task gets `studio/<seat>/<task>-<slug>` under the host's state | #10542 |
| Review the real diff of a worker's task | Done: the coordinator saves the run record a review reads | #10542 |
| Merge only on approval, off-tree, fast-forward into your branch, refused on a dirty checkout | Done: a local merge; nothing is pushed | #10542 |
| Push blocked inside git, not just in the prompt | Done: Git refuses every transport for a studio task's processes | #10542 |
| Agents commit under their own name; the approved merge carries yours | Done: seats commit as `Studio <Seat>`; the merge carries the person's identity | #10542 |
| Permission prompts for risky steps with a risk chip and a scoped "always allow for this agent" | Partial: approvals arrive as decisions; no risk chip or standing rule | Permissions |

## Orchestration

| AgentCraft | Studio today | Gap issue |
| --- | --- | --- |
| Lead plans read-only and creates tasks with dependencies | Partial: the plan is read from the lead's reply automatically; read-only is prompt text | Lead review and CI loop |
| Task graph, statuses, progress | Done | |
| CI after each task; one automatic fix round; then review with the failure noted | Missing: a red check fails the task and waits for a manual retry | Lead review and CI loop |
| Lead reviews the diff before the merge decision | Missing | Lead review and CI loop |
| Merge conflicts go back to the worker to resolve | Partial: the simulated team only | Lead review and CI loop |
| Pause, resume, stop, spawn; hand-off of committed work on reassign | Partial: Verse sends every intent; reassign moves only a task that has not started | Studio panels |
| Spend per agent, task, and goal, shown in status | Done: kept per task across restarts; in the snapshot, `studio goal list`, and the Verse console and seat panel | |
| Session resume and restart recovery | Done | |

## Launch

| AgentCraft | Studio today | Gap issue |
| --- | --- | --- |
| One command to launch everything on a repository | Missing: seven manual steps | One-command launch |
| A free simulated team for demos | Partial: a test fixture and `verse --studio-sim` | One-command launch |
| Graceful authentication failure with a banner | Missing | One-command launch |

## The world

| AgentCraft | Studio today | Gap issue |
| --- | --- | --- |
| Distinct characters per agent that sit at desks, with postures and head look | Done: the Ranger in each seat's outfit color, with postures authored on its skeleton at load | |
| Pathfinding around the studio | Done: routes around the zone's blockers, run when long, with skip-ahead | |
| Speech bubbles for messages; a "!" over the agent that owns a decision | Partial: bubbles for questions to you and the lead's task hand-outs; seats send each other no other messages | Characters and life |
| State particles (thinking, working, error, done) | Done | |
| Waiting agent walks to the podium or to you | Done | |
| Goal atrium with a progress ring; HUD goal bar; "n waiting" badge | Done on desktop; the phone has no panels for the badge to open | Signals |
| Bell for a new decision, chimes for task and goal done, toasts, desktop notifications | Partial: bell, chimes, mute (`V`), and notices on macOS and Linux; no in-world toasts, and Windows stays silent | Signals |
| Live monitors and Task Wall | Done | |

## Screens

| AgentCraft | Studio today | Gap issue |
| --- | --- | --- |
| Console with prefixes, completion, history, acknowledgments in place | Partial: the history lasts the session, not across runs | Studio panels |
| Decision screen: priority order, number keys, accident guards | Done | |
| Diff review with keys and the worker's summary | Partial: no line wrap; the summary is the worker's newest log line | Studio panels |
| Agent card: state, task, decisions, recent log, actions | Done | |
| Task details with retry, prioritize, reassign, cancel | Partial: the snapshot carries no checks or branch | Studio panels |
| Memory library | Done: shared memory with the plan pinned, in the snapshot | |

## Not pursued

AgentCraft's Minecraft-specific tooling (DevBridge, the shot recorder) maps to
Verse's existing capture examples. Per-repository wings were specified but
not built in AgentCraft either.
