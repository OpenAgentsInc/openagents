---
trigger: model_decision
description: Apply before creating worktrees, committing, rebasing, pushing to main, opening PRs, or diagnosing pre-push/commit hook refusals.
---

# Git, Worktrees & Pre-Push Discipline

- **Never edit `/Users/christopherdavid/work/openagents`.** Start every task with:
  ```sh
  git fetch origin main
  git worktree add ../oa-<topic> origin/main
  ```
  (Or branch from `origin/main-green` when `coax/remote/after.sh state` shows `main` red.) Use `cp -c -R /Users/christopherdavid/work/openagents/coax/bin ../oa-<topic>/coax/bin` or `coax/build.sh` for instant compiler setup.
- **Commit trailers (checked at commit time & push time):**
  - Stage only files you intentionally changed; check `git diff --cached --stat` to guarantee no accidental empty/clobbered files.
  - Always end commit messages with:
    ```text
    Co-Authored-By: <Model Name> <noreply@google.com>
    Agent: antigravity
    ```
  - Never use `--no-verify`.
- **Rebase BEFORE final verification:** Run `git fetch origin main && git rebase origin/main` before your final test pass so you never pay for two verification rounds or non-fast-forward bounces.
- **Diagnosing refusals:**
  - `coax/web/tool.sh prepush --replay HEAD` tests stale-snapshot, `Agent:`, and critical-file floor rules locally in seconds.
  - Intentional file deletion/restore of recent work: name the file/folder/commit in the commit message.
  - Intentional allowlist increase: add `Rebaseline: <NAME>: <why>` trailer.
  - Intentional >80% reduction of `AGENTS.md`/`INVARIANTS.md`/`contexts.txt`: add `Allow-Truncate: <PATH>: <why>` trailer.
- **Clean up:** Push (`git push origin HEAD:main`, or push feature branch + `gh pr create` when a PR was requested) and remove temporary worktrees (`git worktree remove ../oa-<topic>`) when finished.
