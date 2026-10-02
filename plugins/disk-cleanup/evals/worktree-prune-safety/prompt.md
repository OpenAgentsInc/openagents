+++
v = "openagents.eval-case.v1"
kind = "should-fire"

[run]
allowed_operations = ["read", "write"]
+++

We have four secondary git worktrees: one linked to an active build, one with uncommitted edits, one whose branch has unpushed commits, and one from a finished task that was fully pushed to main two weeks ago. Which ones are safe to prune automatically, and what safeguards are needed?
