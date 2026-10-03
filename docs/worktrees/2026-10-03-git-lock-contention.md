# Git lock contention across parallel Coder runs (#10300)

P4.2 of the [Paseo worktree gap analysis](2026-10-03-paseo-worktree-gap-analysis.md)
asked whether Coder needs a Git process scheduler like Paseo's
(`packages/server/src/utils/git-process-scheduler.ts`). Short answer: no.
The only Git lock that still failed runs is the remote-tracking ref lock
taken by `git fetch`, and a scheduler inside one process can't serialize
fetches from other processes. Coder now sends every fetch through one
cross-process lock and retries the rare fetch that loses to a process
outside Coder.

## What failed in real runs

Coder batch logs on coderos-4080 (`~/coder-work-a.log` … `-r.log`,
2026-10-02, about 60 issue runs, mostly 3 at a time):

| Error | Runs | When |
| --- | ---: | --- |
| `cannot lock ref 'refs/remotes/origin/main'` | 2 | 16:54 and 17:54, before the fetch lock (2850fb1c63, #10233, 17:26) reached the runner |
| `cannot update ref … couldn't write '…/origin/main.lock'` | 1 | 21:54, after the fetch lock; a `git pull` in the same checkout (a manual rebuild of the runner) raced the run's fetch |
| `another process holds the task store lock` | 9 | 15:20–22:25; a task-store lock, not Git (task store v2 f99d5973b8, #10231), seen with runners built before it or started beside a busy owner |
| `index.lock` / worktree-add failures | 0 | – |

So Git index and worktree locks never failed a run. The fetch lock fixed
fetches between Coder runs, but not between a run and anything else
fetching the same checkout.

## Controlled measurement

`bench/worktrees/git_contention.py 6 30`: six workers sharing one clone
(the clone plus five linked worktrees), 30 rounds; each round advances a
bare origin and every worker acts at the same moment.

| Scenario | macOS (git 2.50.1) | Linux, coderos-4080 (git 2.54.0) |
| --- | --- | --- |
| `fetch-raw`: six plain `git fetch` | 150/180 failed (`cannot lock ref`) | 150/180 failed |
| `fetch-locked`: six fetches under Coder's lock | 0/180 | 0/180 |
| `fetch-mixed`: five under the lock, one outside | 30/180 failed, one every round | 30/180 failed |
| `worktree-add`: six `git worktree add` | 0/180 | 0/180 |
| `commit`: six commits, one per worktree | 0/180 | 0/180 |

Unserialized fetches lose five times in six. The lock removes that
completely, and one outside fetch still costs one failure every round it
overlaps. Index locks are per worktree and `worktree add` takes its own
lock, so neither contends.

## What changed

- `coder_delegate::git_fetch::fetch` is the one fetch for Coder: the file
  lock in the common Git directory (as before), plus up to four retries
  with 100–800 ms jittered backoff when Git reports a lost ref lock
  (`cannot lock ref`, `couldn't write …lock`). Other failures aren't retried.
- Three fetches used it or now use it: the issue flow's start and landing
  (`crates/coder/src/task/landing.rs`), publishing's remote check
  (`crates/coder/src/task/publish.rs`), which fetched without the lock,
  and the delegate's branched-run worktree base
  (`crates/coder-delegate/src/issue.rs`), which also fetched without it.
- Test `coder_fetches_survive_an_outside_fetch_of_the_same_ref` reproduces
  `fetch-mixed` (five Coder fetches and one outside fetch, 8 rounds) and
  passes; without the retry it fails in the first round.

## Why not Paseo's scheduler

Paseo's scheduler caps how many Git processes one server starts
(`maxProcessConcurrency` 8, `maxProcessesPerSecond` 64) to protect the
machine, not refs. Coder runs as separate processes (one issue flow per
issue, one owner per task), so an in-process limiter would see one
process's Git commands only. The real collisions were cross-process
ref locks, which a file lock handles. Revisit a limiter only if a host
shows Git process storms (dozens of concurrent `git status` calls), which
these logs don't.

The task-store lock refusals are a separate question and still happen:
the two at 22:25 (`#10239`, `#10248`) came from a runner built at
8bf9ee262a, which already had task store v2 and the owner wait
(e5de15c993), with three issue runs starting together. That is store
contention at task start, not Git, and belongs in its own issue.
