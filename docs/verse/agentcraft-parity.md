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
| A worktree and branch per task (`agentcraft/<agent>/<task>`) | Missing: real studio tasks share the goal's one checkout; only the simulated team makes worktrees | Worktrees, review, local merge |
| Review the real diff of a worker's task | Missing for real tasks: review needs a local run record autostarted tasks never write | Worktrees, review, local merge |
| Merge only on approval, off-tree, fast-forward into your branch, refused on a dirty checkout | Partial: merge publishes (push and draft pull request) | Worktrees, review, local merge |
| Push blocked inside git, not just in the prompt | Missing: prompt text only | Worktrees, review, local merge |
| Agents commit under their own name; the approved merge carries yours | Missing | Worktrees, review, local merge |
| Permission prompts for risky steps with a risk chip and a scoped "always allow for this agent" | Partial: approvals arrive as decisions; no risk chip or standing rule | Permissions |

## Orchestration

| AgentCraft | Studio today | Gap issue |
| --- | --- | --- |
| Lead plans read-only and creates tasks with dependencies | Partial: the plan is read from the lead's reply automatically; read-only is prompt text | Lead review and CI loop |
| Task graph, statuses, progress | Done | |
| CI after each task; one automatic fix round; then review with the failure noted | Missing: a red check fails the task and waits for a manual retry | Lead review and CI loop |
| Lead reviews the diff before the merge decision | Missing | Lead review and CI loop |
| Merge conflicts go back to the worker to resolve | Partial: the simulated team only | Lead review and CI loop |
| Pause, resume, stop, spawn; hand-off of committed work on reassign | Partial: on the host; Verse lacks retry, reassign, prioritize, cancel | Studio panels |
| Spend per agent, task, and goal, shown in status | Missing | Spend |
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
| Distinct characters per agent that sit at desks, with postures and head look | Missing: tinted boxy figures | Characters and life |
| Pathfinding around the studio | Partial: straight walks with skip-ahead | Characters and life |
| Speech bubbles for messages; a "!" over the agent that owns a decision | Missing | Characters and life |
| State particles (thinking, working, error, done) | Missing: one attention lamp | Characters and life |
| Waiting agent walks to the podium or to you | Partial: walks to the podium | Characters and life |
| Goal atrium with a progress ring; HUD goal bar; "n waiting" badge | Missing | Signals |
| Bell for a new decision, chimes for task and goal done, toasts, desktop notifications | Missing in Verse | Signals |
| Live monitors and Task Wall | Done | |

## Screens

| AgentCraft | Studio today | Gap issue |
| --- | --- | --- |
| Console with prefixes, completion, history, acknowledgments in place | Partial: no completion, history, or several commands | Studio panels |
| Decision screen: priority order, number keys, accident guards | Partial | Studio panels |
| Diff review with keys and the worker's summary | Partial | Studio panels |
| Agent card: state, task, decisions, recent log, actions | Partial: the seat panel | Studio panels |
| Task details with retry, prioritize, reassign, cancel | Missing | Studio panels |
| Memory library | Missing: the library is a place only | Studio panels |

## Not pursued

AgentCraft's Minecraft-specific tooling (DevBridge, the shot recorder) maps to
Verse's existing capture examples. Per-repository wings were specified but
not built in AgentCraft either.
