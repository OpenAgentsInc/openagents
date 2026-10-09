# Worktrees and changes

Coder never works in your checkout. Each task gets its own Git worktree: a
separate folder with a copy of your project at its last commit. Your
files, your branch, and any uncommitted changes stay as they were.

## Where the work is

- A task's worktree starts from your checkout's current commit
  (`HEAD`). Uncommitted changes in your checkout are not carried over, so
  commit what Coder should see first.
- Worktrees live in `~/.openagents/worktrees/`. When a run finishes,
  the chat names its worktree and the files it changed, with lines added
  and removed.
- A follow-up that asks for more work continues in the same worktree.
- Stopping a run, or a run that fails, leaves its worktree as it was, so
  you can look at it or pick up from there.

## Review the change

When a finished task recorded its change, the chat shows a **What changed**
card.

1. The card opens on a click, and the diff shows in a pane on the right
   (on the phone, in the chat). The pane is read-only.
2. The card names the exact commits it shows. If the worktree changes
   while you look, the card says the view is out of date and offers
   **Refresh**.

## Publish the change

**Publish** on the card commits exactly the change you reviewed and pushes
it:

- By default, to a new `coder/review-…` branch, with a draft pull request
  opened through the GitHub CLI (`gh`).
- If the repository's `.openagents/coder-issues.json` says
  `"land": "main"`, onto that branch directly, fast-forward only.

The card then links the pull request or the commit. Publish is refused if
the worktree moved past what you reviewed; refresh and look again.

## Bring a change into your checkout yourself

The worktree is an ordinary Git worktree. From your checkout you can, for
example, look at it with `git -C PATH diff`, or commit there and
cherry-pick the commit.

Next: [Work on a GitHub issue](/docs/github-issues).
