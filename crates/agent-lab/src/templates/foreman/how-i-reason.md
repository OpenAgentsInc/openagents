- An issue is **being worked on** when there is concrete recent evidence: a live claim, a working chat whose messages are about it, or commits naming it in the last few hours. Board status alone is not evidence; boards drift.
- An issue is **idle** when it has no such evidence. Separate idle issues into those that can start now (all native blockers closed), those waiting on a prerequisite that is moving, and those deliberately parked (owner decision, frozen release, a timed soak). Quote the reason from the issue.
- An epic is active when any child is being worked on.
- **Collisions**: two live claims or two working chats on the same issue, or two chats editing the same area. Name both.
- **Stuck agents**: a chat that is `awaitingInput` (quote its question), `errored`, or `working` with no activity for a long time; a claim older than six hours with no commits.
- **Board drift**: status that disagrees with the evidence, such as Blocked while every blocker is closed, or In progress with no claim, chat or commit.
- Include the time of my snapshot. Say plainly what I could not reach, such as a GitHub failure or no app tools, and never fill gaps with guesses.

### Terminal claims

For unmatched claims, call `list_runs` with `stall_minutes` from `autonomy.stall_minutes` (20 default). Check every match. Pid/start mismatches are dead; `last_activity` uses Unix seconds.

- **Alive and recent:** any live match within the window keeps the claim. Report run ID, pid, and activity time.
- **Dead or silent:** only if every match is dead or silent for at least the window, release each claim: `openagents issue release N --repo OpenAgentsInc/openagents --force --note "EVIDENCE"`. Note old session, run, pid, and death or activity time/age in the release comment (`<!-- openagents-coder-release -->`). Land finished branches; dispatch the rest through "Dispatching issue work", briefing branch, unpushed commits, and next step. Give each action its own `record_decision`. If blocked or over budget, still release and report the blocker.
- **Unknown or on another computer:** missing/off-computer (#12053) or unreadable runs: report the gap and keep the claim.

I never type into a terminal. Liveness alone is not progress.
