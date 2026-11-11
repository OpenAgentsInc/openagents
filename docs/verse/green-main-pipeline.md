# The green-main pipeline

This document describes how changes from many agents reach `main` and
production without blocking each other. Agents push `main` directly. An
after-landing check runs on the remote build hosts on every new head, testing
only what changed since the last head that passed. A last-known-good commit,
`main-green`, is what releases ship and what new work starts from. When a
check fails, the pipeline finds the commit that caused it, tells the agent
that wrote it, and reverts the commit if it still fails 30 minutes later.

To see the state of `main`:

```sh
coax/remote/after.sh state    # main, main-green, the last runs, the red list
```

The design follows a merge-queue layout that Kit Langton published on
2026-11-09: builders hand off to an integrator, a fast check runs before
landing, a full check runs after, and a failure becomes a task that names the
culprit. Here it is fitted to Coax, the remote build hosts
([remote-builds.md](remote-builds.md), #13077), `coax/web/release.sh` and our
push rate. The integrator and its queue are built and optional: an agent can
submit a branch instead of pushing, and the integrator lands it after its
check. The pipeline doesn't depend on it.

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
        │  finished commits, each with an Agent: trailer
        ├──────────────► git push origin HEAD:main
        ▼ (optional)                       │
 submit ──► queue ──► integrator:          │
   checks each branch on the head it       │
   lands on, lands the green ones ─────────┤
                                           ▼
 after-landing check, on the remote hosts, on every new head
   fast run: only what changed since main-green
   full run, every three hours: every suite · every wasm budget · room · parity
       │ green                                │ red
       ▼                                      ▼
 main-green moves to this commit        bisect from main-green to the head;
       │                                issue and message to the culprit's agent;
       ▼                                revert if it still fails 30 minutes on
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

### 2. Builders push, or submit

When a branch is done and its own tests pass, the builder pushes `main`
(`git push origin HEAD:main`), and the after-landing check (step 4) takes it
from there. A builder that wants its branch checked on the head it lands on
before it lands submits it instead:

```sh
coax/remote/submit.sh
```

This pushes the branch to `refs/heads/queue/<agent>/<topic>` and adds it to
the queue.

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

A submitted branch was tested on exactly the code it landed on. A landing
that loses the race to a direct push is stacked and checked again on the new
head.

### 4. The after-landing check runs on every new head

`coax/remote/after.sh --watch` checks the newest head of `main` whenever the
previous run has finished. Each run covers every landing since the last one.

Most runs are fast runs. A fast run checks head `H` against `main-green` `G`,
and runs only what the commits between them can have changed:

- the suites whose sources changed since `G` (`coax/web/tool.sh suites
  --since G`, from the `#load` graph), in three parts on three host slots;
- the web modules the change touched, each against its budget;
- `tools-check`, `everglade-parity` and the committed shaders against their
  Coax;
- the room build, when the change reaches one of its units, as its own job
  (it takes about 25 minutes);
- `coax/tests/run.sh` in eight parts, when compiler sources changed.

Every other check passed at `G` and reads no file that changed, so it passes
at `H`. A head that changes only documents is green with no job. A head that
changes the compiler (`coax/src/`, `coax/bootstrap/`, `coax/build.sh`) gets a
full run instead, since a changed compiler can change any check.

A full run is the backstop: every suite, every web module against its budget,
the report checks, the room build, and the compiler's units when its sources
changed since the last full green run. It runs when there is no `main-green`,
and at most every three hours after that (`OA_AFTER_GAP`). If a full run fails
where the fast runs passed, it bisects from the last head a full run passed
and moves `main-green` back to that head.

When a run passes, it moves `main-green`:

```sh
git push origin <commit>:refs/heads/main-green
```

Then, in the background and at most once an hour, it builds that commit's
pages once into the shared store (`coax/remote/pages.sh build`, #13119), so
the release of the commit takes its modules instead of building them again,
and agents fetch them with `coax/remote/pages.sh`
([remote-builds.md](remote-builds.md#the-shared-cache)). `OA_AFTER_PAGES=0`
turns this off.

Results go under `releases/after/<commit>/`. The per-commit history of
compile-time totals and wasm sizes shows which commit made a unit grow.

A job whose host stops under it (the hosts are spot machines) runs again, up
to two more times, before the run counts as unfinished.

### 5. A failure finds its culprit and goes back to its author

When a run fails, `after.sh` bisects between `main-green` and the failing
head. It runs only the failing check, several commits at once on parallel
host slots, so a run of eight commits takes one or two rounds. It then:

1. Files an issue that names the culprit commit, the failing check and its
   output, and the agent from the commit's `Agent:` trailer.
2. Messages that agent with the issue and the command that runs the check
   alone.
3. Adds the failure to the red list (`releases/after/red.json`), so other
   agents know a failure on `main` is already owned.

A check that fails at the head and passes when run again is flaky. It is
recorded in `releases/after/flaky.txt` and has no culprit.

Each later run that still sees the failure stamps the red-list entry. When a
run sees it 30 minutes or more after the entry opened, the integrator lands a
revert of the culprit commit and reopens its issue. The revert's check is a
branch's check plus the check the culprit broke, which must pass with the
revert, so a wrong culprit is never reverted. The revert's message says which
check failed, and the agent gets a message that names the revert.

### 6. Releases ship `main-green`

`release.sh` already takes `--ref COMMIT`. Its default becomes
`origin/main-green` instead of `origin/main`, so a fresh failure on `main`
never blocks a release. The release rule (a check that fails on staging and
passes on production stops it) stays as it is.

## What changes for agents

- Start worktrees from `origin/main-green` when it exists. A failure in that
  worktree is your own change's.
- Put `Agent: <id>` in every commit message.
- Run your own touched tests on the remote hosts while you work (#13077), then
  push `main`. Don't run whole suites: the after-landing check does.
- Before you investigate a failure on `main`, run `coax/remote/after.sh
  state`. A failure on the red list is owned by the agent it names.
- When a red-list entry names your commit, fixing it comes before new work.
  You have 30 minutes before the commit is reverted.

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

## What is built

Issue [#13102](https://github.com/OpenAgentsInc/openagents/issues/13102).

| Part | Script |
| --- | --- |
| Submit and the queue refs | `coax/remote/submit.sh` (`status`, `withdraw`) |
| The integrator | `coax/remote/integrate.sh` (`--once`, `status`) |
| One check on a host | `coax/remote/check.sh` (`branch BASE`, `since BASE I/N`, `web BASE`, `room-since BASE`, `suites I/N`, `only PATHS`, `compiler I/N`, `unit UNIT`, `modules I/N`, `module NAME`, `wasm`, `reports`, `room`, `tools`) |
| The after-landing check, bisect and red list | `coax/remote/after.sh` (`--once`, `--watch`, `state`, `red`, `history`) |
| The revert | `coax/remote/integrate.sh --revert`, which `after.sh --watch` runs each minute |
| Releases from `main-green` | `coax/web/release.sh` |
| Shared settings | `coax/remote/pipeline-lib.sh` |

`remote.sh run --commit REV --out DIR` runs a check on a commit's own files
instead of a snapshot of the checkout, so the integrator and `after.sh` check
exact commits without a worktree. `coax/web/tool.sh suites --part I/N` splits
the suites across host slots.

The coordinator runs one watcher on the Mac. It runs the after-landing check
on each new head, and one integrator round a minute for the 30-minute revert
and for any branch an agent submitted:

```sh
coax/remote/after.sh --watch        # --no-revert: report culprits, revert nothing
```

Measured on 2026-11-09 on today's tree: a branch check takes 8 to 42 s of
host time when it touches a few suites or modules, about $0.001 to $0.003 of
host time (remote.sh's ledger bills each job at a whole host's price, three
times that). A full run is 11 jobs and 11,712 slot-seconds, about $0.30 of
host time ($0.90 on the ledger); with the hosts shared with agents' own
builds it took 31 minutes. Logs, each
job's output and the red list are under `~/.openagents/pipeline/`
(`releases/after/COMMIT/`, `releases/after/red.json`,
`releases/after/history.tsv`). A message to an agent goes to
`~/.openagents/pipeline/inbox/AGENT.log`, which `submit.sh status --agent ID`
prints, and to `OA_PIPELINE_NOTIFY` when it is set.

Measured on 2026-11-10: a fast run of a change to nine files took 153 s on
four host slots (three suite parts of about 55 s each, and 153 s for the web
modules, the report checks and the shaders), 315 slot-seconds, under $0.01 of
host time. When compiler sources change, the compiler's units add eight jobs
of 6 to 30 minutes, and a change that reaches the room adds its 25-minute
build. The first fast run on `main`, on a loaded evening (every slot of
eight hosts busy), covered 12 commits that touched 26 suites, the room and
the compiler's units: 13 jobs, 49 minutes, and one unit that timed out under
load and passed when run again. The time to a verdict is set by the slowest
job and by free slots, not by the number of commits. A full run of all 980 suites, 55 modules, the reports,
the room build and the compiler's units took 41 minutes on shared hosts and
20,315 slot-seconds, about $0.52 of host time; eight a day is about $4.

### Differences from the design

- Agents push `main` directly (owner, 2026-11-10). Submitting is optional, so
  a landed commit was not always tested on the code it landed on. The fast
  after-landing run takes that role: it checks each new head within minutes
  and names the culprit.
- The queue runs in the order of each branch's newest commit time. A
  resubmitted branch keeps its place only if its commits are unchanged.
- A branch that conflicts only with a branch ahead of it waits for the next
  round instead of going back.
- Compile-limit totals come from the web build's near-limit notes (75% and
  up). `tool.sh suites` doesn't pass a suite's notes on, so a touched test
  suite's totals aren't reported.
- No run includes the drift check: it needs a browser, and the hosts have
  none.
- `suites --since` picks suites from the `#load` graph. A suite's checks run
  at compile time, and the compiler reads only `#load`, `#import_proto` and
  itself, so a changed file that isn't Coax matters only when it is a `.pb`
  descriptor, a compiler source (`coax/src/`, `coax/bootstrap/`,
  `coax/build.sh`) or the C a native suite links. Then every suite asks
  `coax deps`, which runs most of them. Before 2026-11-10 any changed file
  that wasn't Coax (a Swift source, a C face, a script), and any changed Coax
  file that nothing loads, did that: a three-commit range took over five
  minutes to pick its suites, and now takes three seconds.
- The web modules a change touched come from the same graph, plus the C faces:
  a changed `.c` or `.h` file touches the modules whose build command names
  it or a file that includes it. Any other changed file under the trees the
  web build reads builds every module.
- A failure that passes when run again on the same head is flaky: it is
  recorded in `releases/after/flaky.txt`, and if every failure of a run was
  flaky, `main-green` moves.
- Before the first green run there is no `main-green` to bisect against.
  Each failure of a full run goes on the red list owned by nobody, under one
  issue, so later runs don't bisect it, and it is never reverted. A fast run
  then compares each new head against that full run's head, and runs the
  full run's failures again (up to eight; more wait for the next full run).
  When they and everything the change touched pass, the head is green and
  `main-green` is set. A failure the full run didn't have is bisected from
  the full run's head and gets its culprit.
- The full run builds the web modules in three parts, each module as
  `build.sh` runs it. It leaves out `project` and `projects`, which need the
  tables `build.sh` bakes from Nostr first.
- The red list and the messages to agents are files on the coordinator's Mac.
  An agent on another machine reads `origin/main-green` and the issues.

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
