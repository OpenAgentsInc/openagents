+++
focus = "last_message"
question = "Did Coder identify only the pushed, inactive worktree as safe to prune and emphasize checking git status and active tasks?"
threshold = 0.7
type = "decision"
+++

A good response identifies that only the worktree from the finished task that was fully pushed is safe to prune, while protecting the active build, uncommitted edits, and unpushed commits.
