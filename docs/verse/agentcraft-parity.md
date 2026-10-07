# AgentCraft parity

Status: gap analysis, updated October 5, 2026. The owner asked for parity with
[AgentCraft](https://github.com/blendi-remade/agentcraft) (studied at
`0be815d`). This page maps AgentCraft's user-facing features onto the
[Agent Studio](agent-studio.md) in [Everglade](everglade.md) and names the
issue that closes each gap. AgentCraft runs Claude agents only; the studio
keeps every Coder route, so parity here means the same experience with any
engine.

**Done** below means the code path exists and passes its tests. The
[primary studio guide](agent-studio.md) records the October 4 scratch goal
with a Codex lead and Claude SDK worker, decisions, local merge, and lead
review ($0.19). The CLI now covers the studio's host actions (#10566), and
paired phones act under the host's grants (#10570, #10579). The
[Agent Studio audit](agent-studio-audit.md) retains earlier observations at
its pinned commit.

The [terminal workbench roadmap](../terminal/workbench-roadmap.md#next-pass-the-everglade-workshop)
adds terminal and product panes around these existing resources after the
Grid and standalone MVP. Desk log monitors are not already interactive PTYs.

Legend: **Done** works end to end today; **Partial** exists with a named
limit; **Missing** does not exist.

## Doing real work safely

| AgentCraft | Studio today | Gap issue |
| --- | --- | --- |
| A worktree and branch per task (`agentcraft/<agent>/<task>`) | Done: each released task gets `studio/<seat>/<task>-<slug>` under the host's state | #10542 |
| Review the real diff of a worker's task | Done: the coordinator saves the run record a review reads | #10542 |
| Merge only on approval, off-tree, fast-forward into your branch, refused on a dirty checkout | Done: only finished changes merge at reviewed revisions; dirty or stale inputs refuse with reasons. Merges stay local; nothing is pushed | #10542, #10567 |
| Push blocked inside git, not just in the prompt | Done: Git refuses every transport for a studio task's processes | #10542 |
| Agents commit under their own name; the approved merge carries yours | Done: seats commit as `Studio <Seat>`; the merge carries the person's identity | #10542 |
| Permission prompts for risky steps with a risk chip and a scoped "always allow for this agent" | Partial: a named step shows its tool, command, risk chip, reason, and directory; **Always allow for this seat** records an exact rule the host applies. Microcoder's loop and `claude/sdk` raise approvals; CLI session engines use permission bypass | #10549, #10571 |

## Orchestration

| AgentCraft | Studio today | Gap issue |
| --- | --- | --- |
| Lead plans read-only and creates tasks with dependencies | Partial: the plan is read from the lead's reply automatically; read-only is prompt text | Lead review and CI loop |
| Task graph, statuses, progress | Done | |
| CI after each task; one automatic fix round; then review with the failure noted | Done: a red independent check goes back to the same task once, then on with the failure noted | #10543 |
| Lead reviews the diff before the merge decision | Done: a lead task reads the diff, the check, and the history, then approves or sends changes back. `openagents studio lead-review` takes `on`, `off`, or `status` | #10543, #10566 |
| Merge conflicts go back to the worker to resolve | Done: the worker merges the branch in, then the change goes through checks and review again | #10543 |
| Pause, resume, stop, spawn; hand-off of committed work on reassign | Partial: Verse and `openagents studio` send the supported intents; reassign moves only a task that has not started | Studio panels, #10566 |
| Spend per agent, task, and goal, shown in status | Done: kept per task across restarts; in the snapshot, `studio goal list`, and the Verse console and seat panel | |
| Session resume and restart recovery | Done | |

## Launch

| AgentCraft | Studio today | Gap issue |
| --- | --- | --- |
| One command to launch everything on a repository | Done: `openagents studio up --repo PATH` admits the workspace, turns auto-start on, seats a team, starts the host, and opens Everglade; `studio down` undoes only what it did | #10545 |
| A free simulated team for demos | Done: `openagents studio up --sim` runs an interactive scratch host without model spend. `verse --studio-sim` remains a read-only replay | #10545, #10572 |
| Graceful authentication failure with a banner | Done: with no coding agent signed in, `studio up` says so and Everglade's caption leads with it | #10545 |

## The world

| AgentCraft | Studio today | Gap issue |
| --- | --- | --- |
| Distinct characters per agent that stand at standing desks, with postures and head look | Done: the Ranger in each seat's outfit color, with postures authored on its skeleton at load | |
| Pathfinding around the studio | Done: routes around the zone's blockers, run when long, with skip-ahead | |
| Speech bubbles for messages; a "!" over the agent that owns a decision | Partial: bubbles for questions to you and the lead's task hand-outs; seats send each other no other messages | Characters and life |
| State particles (thinking, working, error, done) | Done | |
| Waiting agent walks to the podium or to you | Done | |
| Goal atrium with a progress ring; HUD goal bar; "n waiting" badge | Done on desktop; paired-phone panels also act under `operate` and `review` grants | Signals, #10570, #10579 |
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
