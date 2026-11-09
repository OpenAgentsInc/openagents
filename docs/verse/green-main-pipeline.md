# The green-main pipeline

This document describes how changes from many agents reach `main` and
production without blocking each other. Builders hand a finished branch to
an integrator instead of pushing. The integrator checks each branch against
the latest `main`, testing only what the branch touched, and lands branches
in order. The full suite runs after landings, on the remote build hosts. A
last-known-good commit, `main-green`, is what releases ship and what new work
starts from. When a check fails, the pipeline finds the commit that caused it
and sends the fix to the agent that wrote it.

The design follows a merge-queue layout that Kit Langton published on
2026-11-09: builders hand off to an integrator, a fast check runs before
landing, a full check runs after, and a failure becomes a task that names the
culprit. Here it is fitted to Coax, the remote build hosts
([remote-builds.md](remote-builds.md), #13077), `coax/web/release.sh` and our
push rate.

## Our push rate

Measured on `origin/main` on 2026-11-09:

| Window | Pushes | Commits |
| --- | --- | --- |
| Last 24 hours | 161 | 208 |
| Busiest hour (12:00–13:00 Chicago) | — | 27 |
| Last 7 days | — | 1,834 (163 to 388 a day) |

204 of the last 24 hours' 208 commits carry the same Git author, so the
author field can't say which agent wrote a commit.

At these rates:

- Pushing directly after a pre-push check is a race. A 3-minute check at
  the busiest hour sees one or two other pushes land under it, so most checks
  are already stale when they finish. Landing must go through one integrator.
- The integrator can't land one branch per check run. A sequential check of
  2 to 7 minutes per branch lands 9 to 30 branches an hour, below the
  busiest hour's rate. It has to check branches in parallel and land them in
  order.
- The full suite can't run once per push. One full run takes about 17
  minutes, so each run covers several landings, and a failure needs a
  bisect.

## Why

On 2026-11-09 most of the waiting came from five problems:

| What happened | Cost |
| --- | --- |
| The steering commit `c3f65bfb55` broke four Experimental checks. The Shapechange commit `7353346533` pushed the fivelab unit over its compile-time storage limit (E0400). | `release.sh` could not ship for several hours while we bisected by hand. |
| Agents started worktrees from a `main` that was already broken, so their own runs failed on someone else's errors. | Runs were thrown away and repeated. One fix's results were wrong because its base predated a compile fix. |
| fivelab grew to 2,747,624 bytes against a 2,725,000-byte budget, and nothing noticed until `release.sh` built it. | A failed release and a second release run. |
| fivelab sat at 97% of its compile-time traversal and value-slot limits, so each new feature tipped it over. | Shapechange, the two-hander blend checks and the Everglade II fixes each needed an extra round. |
| Agents could not build, so every run went through one coordinator and the Mac's two build slots. | Runs queued for 30 to 60 minutes. A check that takes 7 minutes on a remote host waited 40. |

The remote build hosts fixed the last problem. This pipeline fixes the other
four, and the landing races that our push rate would cause.

## The pipeline

```
 builder worktree (from main-green)
        │  finished branch, with an Agent: trailer
        ▼
 submit ──► queue
              │
              ▼
 integrator: checks every queued branch at once, each on its own host slot
   rebase on the head it will land on · compile and test the touched units
   · wasm sizes · compile-limit warnings
              │ green, in queue order                 │ red or conflict
              ▼                                        ▼
         lands on main                       back to its builder
              │
              ▼
 after-landing check, on the remote hosts, on the newest head
   every suite · every wasm budget · compile limits · room · parity
       │ green                                │ red
       ▼                                      ▼
 main-green moves to this commit        bisect from main-green to the head;
       │                                task to the culprit's agent;
       ▼                                revert if not fixed in 30 min
 release.sh ships main-green
```

### 1. Builders start from `main-green`

Every agent makes its worktree from the last commit that passed the full check:

```sh
git fetch origin
git worktree add ../oa-TOPIC origin/main-green
```

A failure in that worktree belongs to the agent's own change.

Every commit carries an `Agent:` trailer with the agent's ID, so a failure can
reach the agent that wrote it.

### 2. Builders submit; they don't push

When a branch is done and its own tests pass, the builder submits it:

```sh
coax/remote/submit.sh
```

This pushes the branch to `refs/heads/queue/<agent>/<topic>` and adds it to
the queue. The builder doesn't push `main`.

### 3. The integrator checks branches in parallel and lands them in order

The integrator, `coax/remote/integrate.sh`, runs as a long-lived watcher. Its
own work is light; checks run on the remote hosts. For the queue in order
`B1, B2, … Bn` on head `H`:

1. Check every branch at the same time, each on its own host slot. Branch
   `Bk` is rebased onto `H` plus `B1 … Bk-1`, the head it lands on if the
   branches ahead of it land. A rebase conflict goes back to the builder at
   once.
2. For each branch, find the touched units. For each entry unit (fivelab,
   everglade2, room, the libraries with tests), `coax deps FILE` lists its
   source inputs. A unit is touched when the branch changes one of its inputs.
3. Compile and test the touched units. Compiler check reuse (#13077) re-runs
   only checks whose inputs changed, so most branch checks take 2 to 4
   minutes.
4. Build each touched wasm target and compare its size with
   `apps/openagents-web/wasm-budgets.txt`.
5. Report each touched unit's compile-time totals (traversal, value slots,
   steps). A unit above 90% of any limit is a warning on the branch.
6. Land the longest green prefix of the queue on `main` in one push. A red
   branch goes back to its builder. The branches behind it are checked again
   without it.

With 12 host slots, one round checks up to 12 branches in about 4 minutes,
which lands up to 180 branches an hour. That is well above the busiest hour.

Only the integrator pushes `main`. Pushes no longer race, and every landed
commit was tested on exactly the code it landed on.

### 4. The full check runs after landing, on the newest head

The integrator also starts the full check, `coax/remote/after.sh`, on the
newest head of `main` whenever the previous full run has finished. Each run
covers every landing since the last one. At the busiest hour that is 6 to 8
commits a run.

A full run covers:

- every Coax test suite that `coax/build.sh test` covers, split across host
  slots;
- every wasm target against its budget;
- the room tests and the drift check's offline profile;
- `everglade-parity`, `tools-check` and the committed shaders against their
  Coax (`wgsl-module --site --check`);
- each unit's compile-time totals and wasm sizes, recorded per commit.

When a run passes, it moves `main-green`:

```sh
git push origin <commit>:refs/heads/main-green
```

Results go under `releases/after/<commit>/`. The per-commit history of
compile-time totals and wasm sizes shows which commit made a unit grow.

### 5. A failure finds its culprit and goes back to its author

When a full run fails, `after.sh` bisects between `main-green` and the failing
head. It runs only the failing checks, several commits at once on parallel
host slots, so a run of eight commits takes one or two rounds. It then:

1. Files an issue that names the culprit commit, the failing check and its
   output, and the agent from the commit's `Agent:` trailer.
2. Messages that agent with the issue.
3. Adds the failure to the red list (`releases/after/red.json`), so other
   agents know a failure on `main` is already owned.

If the culprit's fix hasn't landed within 30 minutes, the integrator lands a
revert of the culprit commit and reopens its issue. The revert is checked like
any other branch. Its message says which check failed and how to land the
change again.

### 6. Releases ship `main-green`

`release.sh` already takes `--ref COMMIT`. Its default becomes
`origin/main-green` instead of `origin/main`, so a fresh failure on `main`
never blocks a release. The release rule (a check that fails on staging and
passes on production stops it) stays as it is.

## What changes for agents

- Start worktrees from `origin/main-green`.
- Put `Agent: <id>` in every commit message.
- Run your own touched tests on the remote hosts while you work (#13077), then
  `coax/remote/submit.sh`. Don't push `main` and don't run whole suites.
- When a red-list entry names your commit, fixing it comes before new work.
- A failure on `main` that names someone else's commit is theirs. Check the
  red list before investigating.

## What we already have

| Part | State |
| --- | --- |
| Remote build hosts, up to four, three slots each, $15/day cap | Live (`coax/remote/remote.sh`, #13077, `6d8f7defc3`) |
| Check reuse across checkouts, so unchanged checks don't re-run | On main (`2a16b5caea`); reaches the Mac with the next compiler publish |
| Unit dependency lists | `coax deps FILE` |
| Compile-time totals per check | Printed by every `coax test` and `wasm.sh` as `note[E0400]` lines |
| Release from a chosen commit | `release.sh --ref COMMIT` |
| Release checks that compare against production | `release.sh` |
| Zone parity report | `coax/web/tool.sh everglade-parity` (#13075) |

## What to build

1. `coax/remote/submit.sh` and the queue refs.
2. `coax/remote/integrate.sh`: parallel branch checks on stacked rebases,
   touched units from `coax deps`, wasm sizes, compile-limit warnings, and
   in-order landing of the green prefix.
3. `coax/remote/after.sh`: full runs on the newest head, the `main-green` push,
   per-commit totals, the bisect, the issue, the agent message and the red
   list.
4. The revert step, with its 30-minute window.
5. `release.sh` defaulting to `origin/main-green`.
6. `AGENTS.md` and agent briefs: start from `main-green`, add the `Agent:`
   trailer, submit instead of pushing.

## Capacity and cost

Prices are for `c3d-highcpu-30` spot hosts at $0.275 an hour, three slots
each.

| Work | Load |
| --- | --- |
| Branch checks | 161 a day at 2 to 4 minutes on one slot: about 8 slot-hours a day. Each failed and re-checked branch adds to that. |
| Full runs | Back to back during the 20 busy hours a day. One run uses six slots for about 17 minutes: about 120 slot-hours a day. |
| Bisects | About one round of six slots for 17 minutes per failing run. |
| Agents' own runs while they work | Already about $1.60 a day. |

That is about 130 to 150 slot-hours a day, or 45 to 50 host-hours: about $12
to $14 a day, roughly $400 a month. It needs the host limit raised to six
(18 slots), so full runs, branch checks and agents' own runs don't queue
behind each other, and the daily cap raised to $25. The busiest days, at 388
commits, cost more, but full runs are batched, so the cost grows with busy
hours, not with commits. Check the total against the $1,500-a-month GCP limit
before raising the cap. Every host still stops itself after 15 idle minutes.

## Not included

GPU checks, screenshots, drift against live ports and `release.sh` itself need
the Mac's GPU or credentials and stay on the Mac. They are release checks,
not landing checks.
