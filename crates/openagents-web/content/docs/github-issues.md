# Work on a GitHub issue

Ask Coder to work an issue and it takes the issue from claim to close: it
reads the issue, does the work, runs your checks, lands the change, and
reports on the issue.

## Before you start

- Run `coder` in a checkout of the issue's repository.
- Install the GitHub CLI and sign in: `gh auth login`.

## Ask for it

In a chat on the Mac, in the Terminal, or from a shell:

```sh
openagents chat "work on #10051"
```

"Take OpenAgentsInc/openagents#10051" works too.

## What Coder does

1. **Claims it.** Reads the issue, its comments, and a few issues it links,
   and posts a comment saying Coder is working on it.
2. **Works.** Starts from the latest default branch (`origin/main`), in its
   own worktree, with the issue as its task.
3. **Checks.** Runs your repository's checks for what it changed: the
   tests of each package it touched, plus checks for style and broken
   links. If they fail, it fixes and checks again, a few rounds at most.
4. **Lands.** Commits with the issue's title and a link, then either opens
   a pull request that closes the issue (the default) or rebases and pushes
   to `main`, as the repository says.
5. **Closes.** Comments the commit, the files, the checks it ran, and the
   agent it used, and closes the issue.

It never pushes a change whose checks fail. If it can't finish, it
comments what it tried and the failing output, releases its claim, and
leaves the issue open with the change in its worktree, so you can pick up
from there.

## Keep it running

The issue flow runs in the program that started it. Keep the Terminal
screen or the command open until it lands; if you close it partway, the
claim stays without a closing comment. Esc, Ctrl-C, or **Stop** stops it
and says so on the issue.

## Several issues

```sh
openagents chat work --issues 10052,10053           # one after another
openagents chat work --issues bug --parallel 2      # a label's open issues, two at once
```

Each issue gets its own thread and worktree. Coder skips closed issues and
issues someone claimed recently.

## The repository's policy

A file at `.openagents/coder-issues.json` tells Coder how to land:

```json
{ "land": "main", "claim_hours": 6, "fix_rounds": 3, "fmt": true, "clippy": true }
```

Without the file, Coder opens a pull request and runs the tests and the
diff checks. `--land main` or `--land pr` overrides it for one command.

Next: [Ship an iPhone build from your phone](/docs/ship-from-your-phone).
