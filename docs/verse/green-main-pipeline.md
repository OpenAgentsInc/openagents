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
coax/remote/after.sh state    # the watcher, main, main-green and its lag, the allowlist counts,
                              # Everglade II against production, the last runs, the red list, the flaky list
```

The first line says whether the watcher is running. When it is not, no head
is checked and `main-green` does not move. It names the watcher's machine
(coderos-4080; see "What is built" for how to check it there and how to move
it).

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

### The pre-push check

Issue [#13265](https://github.com/OpenAgentsInc/openagents/issues/13265).
Every `git push` to `main` runs a check on the pusher's machine before the
commits leave it. It is not a queue: the pusher runs it and pushes. It
refuses a push when:

1. **A commit undoes newer work on `main`.** For each file a pushed commit
   changes, the check reads that file's changes on `main` in the 24 hours
   before the commit's parent. If the commit's new content, or the file's
   absence, equals the content the file had before one of those changes, the
   commit undoes them. That is what a tree written from a stale snapshot
   does, and only rarely what an edit does. The rule doesn't count changes by
   the commit's own `Agent:`, commits earlier in the same push, and a deleted
   file that git sees as moved. A file the message names is allowed: its
   path, its file name, one of its folders of two or more levels with the
   slash (`coax/web/room/`), or the hash of the commit it undoes.
2. **The push isn't a fast-forward of `main`**, or deletes `main`.
3. **A commit has no `Agent:` line.** A person writes `Agent: human-NAME`, or
   pushes with `OA_PREPUSH_HUMAN=1`.
4. **A check fails:** `tools-check`, `everglade-parity`, `wire-check` and
   `layer-check` (the last two with `--base` set to the push's merge base),
   and `coax test` of the suites the push changed (`check.sh only`, on the
  pushed commit itself), on a build host when
   `remote.sh status` shows one with a free slot. A check that fails on
   `main` already (an open entry on the red list) isn't counted. The checks
   run only when the pushed commit is the checkout's `HEAD`, and only when
   the push changes something that isn't a document.

The checks share a time limit of 120 seconds (`OA_PREPUSH_SECONDS`).
Anything still running then is stopped and named as skipped; the
after-landing check runs it. With no free host, the touched suites are
skipped with a note. `OA_PREPUSH_TESTS=0` skips them.

A checkout with no `coax/bin/coax` (a fresh worktree before `coax/build.sh`)
skips `wire-check`, whose registry rows name that file, and the suites, with a
note; the other report checks still run. A missing compiler never refuses a
push. Until 2026-11-11 21:00 it did: `wire-check` failed on the missing file
and the hook refused every push from a fresh worktree (reported on #13265).

Measured on 2026-11-11 for a one-commit push of a new tool: the rules took
0.1 to 0.3 seconds, the four report checks 1 to 3 seconds each in parallel,
and `check.sh only` of the touched suite on a host 12 to 15 seconds (about
$0.001). The push paid 14 to 18 seconds of checks and about 21 seconds in
all. With `OA_PREPUSH_TESTS=0` it paid 3 seconds; with no free host, about
6 (the host status takes about 5). The first push from a new worktree also
builds the tools, a few seconds more. A deliberately failing assertion in a
touched suite was refused in 15 seconds.

**Why "since the base" means "since the commit's parent".** A push to `main`
must be a fast-forward, so a pushed commit always sits on `main`'s tip, and
nothing on `main` changed since its merge base. Both clobbers were like that.
`7cdf01e725`'s parent is `f3480dd580`, and `bc5c6a5845`'s is `0d81202ed9`,
yet their trees were written from older snapshots, so they undid the commits
just before them. The rule therefore compares each file's new content with
its own recent history.

**Replayed.** `coax/web/tool.sh prepush --replay-check` judges the historical
commits: `7cdf01e725` is refused (39 files undone) and `bc5c6a5845` is
refused (7 files). Their fixes `ed4754f5f5` and `75cdc077b5`, which name the
commits they undo, pass. An ordinary commit (`51a282ab2f`), a move of another
agent's new file (`a566a4fc73`) and a deletion whose message names the file
(`fb45a6170d`, "Zeron.metal is gone") pass too. The report checks run it
(`check.sh reports`). Over the 400 commits on `main` up to `51a282ab2f`,
the rule refused 5: `bc5c6a5845` and 4 intentional rewrites that deleted or
reverted another agent's new file without naming it (`1dcf86bbc2`,
`adefd2b821`, `a4bc225b22`, `ccf6427829`). Each would have passed with the
file named in its message. `coax/web/tool.sh prepush --replay COMMIT` judges
any commit.

**Where the hooks live and how they are installed.** The hooks are tracked in
`coax/remote/hooks/`: `pre-push`, and `post-checkout`, `post-commit` and
`post-merge`, which call `git lfs` as `git lfs install` writes them. They
are thin wrappers. The rules are Coax, in `coax/web/prepush.coax`, run by
`coax/web/tool.sh prepush`. A clone turns the hooks on once:

```sh
git config core.hooksPath coax/remote/hooks
```

`core.hooksPath` is in the repository's shared config, so it applies to
every worktree, and a relative path resolves against each worktree's root.
Each worktree runs the hooks and the tool of its own commit. The main
checkout on this Mac has it set. A worktree made from a commit before
`coax/remote/hooks/` existed runs no hooks until it rebases. No tracked file
uses Git LFS, so losing the LFS hooks there costs nothing.

The hooks aren't in `.githooks/`. The main checkout holds an untracked
`.githooks/` folder of the same four LFS hooks, which the config pointed at
before. Tracking files at those paths would make `git pull --ff-only` in the
main checkout refuse to overwrite them, even byte-identical ones. The
untracked folder stays as it is, unused.

In the after-landing watcher's sparse checkout (only `coax/remote`), the hook
finds no `coax/web/tool.sh` and lets the integrator's pushes go unchecked;
the integrator checks them itself. A fresh worktree with no compiler uses the
newest one in `~/.cache/coax-release`. If the tool can't build or run, the
hook prints a warning and lets the push go: a broken check never stops
every agent. The hook stops after 300 seconds whatever happens.

**Emergency bypass.** `OA_PREPUSH_BYPASS="why" git push ...` skips every rule
and appends a line (time, checkout, refs, reason) to
`~/.openagents/prepush-bypass.log`. Use it only when `main` itself needs the
push to get green again. Don't use `git push --no-verify`: it skips the LFS
hook too, and leaves no record.

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
- `tools-check`, `everglade-parity`, `wire-check` (every wire shape is in
  `proto/registry.json`; the allowlist is compared with `G`'s copy, #13236)
  and the committed shaders against their Coax;
- the room build, when the change reaches one of its units, as its own job
  (one to two minutes with its cache), and in it Everglade II against
  production ("The comparing gates", below);
- the allowlist counts against `main-green`, with `git` alone on the watcher;
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
compile-time totals, wasm sizes and allowlist counts (`after.sh history`)
shows which commit made a unit grow.

A job whose host stops under it (the hosts are spot machines) runs again, up
to two more times, before the run counts as unfinished.

An unfinished run leaves the head unjudged, neither green nor red. The same
is true when the hosts could not finish a bisect. The jobs that did not pass
run again at once on the same head, and the jobs that passed are kept. After
three such runs (`OA_AFTER_UNFINISHED`) the pipeline reports it, then tries
the head again every half hour. A new head is checked at once. So a failed
bisect never leaves `main-green` waiting for someone to push.

### 5. A failure finds its culprit and goes back to its author

When a run fails, `after.sh` first runs the failing check again on the head.
If it passes, the check is flaky (see below). If it fails again, `after.sh`
bisects between `main-green` and the head. It runs only the failing check,
several commits at once on parallel host slots, so a run of eight commits
takes one or two rounds. A commit the hosts could not run is left out of its
round, and the other commits still narrow the range. It then:

1. Files an issue that names the culprit commit, the failing check and its
   output, and the agent from the commit's `Agent:` trailer.
2. Messages that agent with the issue and the command that runs the check
   alone.
3. Adds the failure to the red list (`releases/after/red.json`), so other
   agents know a failure on `main` is already owned.
4. Comments on each issue the culprit's subject names (`#NNNN`), so the
   session working on that issue hears of the failure before any revert.

A check that fails at the head and passes when run again is flaky. It is
recorded on the flaky list and has no culprit. A check that is already on the
list gets three runs on the head, and one pass is enough. Each entry has an
owner and an issue, and counts as a failure after seven days ("Flaky checks",
below).

Each later run that still sees the failure stamps the red-list entry. When a
run sees it 30 minutes or more after the entry opened, the integrator lands a
revert of the culprit commit and reopens its issue. The revert's check is a
branch's check plus the check the culprit broke, which must pass with the
revert, so a wrong culprit is never reverted. That check must also still fail
on the head the revert lands on: when a fix landed first, the revert is
dropped and the entry closes. The revert's message says which
check failed, and the agent gets a message that names the revert.

### The comparing gates

Issue [#13266](https://github.com/OpenAgentsInc/openagents/issues/13266).
Two gates judge a head by comparing it, not by pass or fail alone: the
allowlist counts against `main-green`, and Everglade II against production.
`after.sh state` prints both, the flaky list, and `main-green`'s lag.

`coax/remote/test-after.sh` checks them in a scratch repository with a
stand-in for `check.sh` (no host, no network, no GitHub, about 9 seconds):
an ordinary head stays green; a head that raises `tools-port` is red and the
red list names the commit; taking the line out again is green and closes
the entry; a raise with a `Rebaseline:` trailer is green; a commit worse than
production in Everglade II, under two later commits, is red and the bisect
names it; a `main-green` as bad as the head against production is reported,
not red; a suite that fails once is flaky with an owned entry, which fails
`after.sh flaky` once it is eight days old. `check.sh reports`, and any fast
run that changes `coax/remote/`, runs it.

#### The allowlist counts

Five checks pass while an allowlist excuses what they would refuse. The
lists are meant to shrink, so their sizes are counted on every run
(`coax/remote/counts.sh show REV`, with `git show` alone, no host):

| Count | What it counts |
| --- | --- |
| `parity-known`, `parity-gaps` | `EF_KNOWN` and `EF_GAPS` in `everglade/features.coax` (`everglade-parity`) |
| `components-to-move` | `CC_TO_MOVE` in `coax/web/components-check.coax` |
| `wire-allowlist` | the lines of `proto/wire-allowlist.txt` (`wire-check`) |
| `tools-port` | the lines of `coax/tools-port.txt` (`tools-check`) |
| `layer-aethyr-up`, `layer-everglade-up`, `layer-tests` | the lines of `aethyr/layer-allowlist.txt` per rule (`layer-check`) |

As of 2026-11-11 22:00 on `main`: 117, 4, 22, 143, 1,238, 57, 7 and 126.

The rule (`counts.sh check BASE REV`). Walking the commits from `main-green`
to the head, a count's baseline starts at its value at `main-green` and
falls with it: it is the lowest value since. A head whose count is above the
baseline is red, and the culprit is the first commit after the lowest point
that went above it. The culprit is known from `git` alone, so a count needs
no bisect; it goes on the red list as `count NAME`, with an issue and a
message to its agent, and is reverted after 30 minutes like any failure (the
revert's check is `check.sh counts BASE`). A count that falls lowers the
baseline, and once `main-green` moves past it, so does the next run's start.

**A legitimate rise says why.** `tools-port` went from 755 to 1,243 lines on
2026-11-02 in `45bdff5358`: `tools-check` began to name every tracked tool
with no `Firm reason:` line, and the 488 it found were already in the tree.
That was wider coverage, not new tools, and the list has only shrunk since
(1,238 now). Such a commit carries a trailer in its message:

```
Rebaseline: tools-port: tools-check now reads the tools under lib/, which were all here before
```

The trailer names the count, and the reason after the second colon must be
at least ten characters. The commit's own value becomes the baseline. Adding
one allowlist line that `wire-check` or `layer-check` accept (with its reason
and issue) also raises a count, so it needs the same trailer; otherwise the
head is red. On `main` from 2026-11-11 19:26 the wire allowlist went 141, 142,
143 with no trailer (`6e25356a99`, `5856a19006`): under this rule each would
have been red until it said why.

#### Everglade II against production

The live game is Everglade II (`/worlds/everglade2`). The release's rule, a
check that fails on staging and passes on production stops it, applies here
one step earlier: a head that is worse than production beyond the check's
noise does not move `main-green`, and the red list names the commit. When the
head and production fail the same way, it is reported, not red.

The gate is `check.sh e2 PROD`. It runs inside the room job (`room-since BASE
PROD` in a fast run, when the change reaches the room; `tools PROD` in a full
run), which builds the head's room server anyway, and adds about 80 seconds
to it. On the host it builds production's room server and its client from
production's own tree, once a host, in `~/oa-e2/PROD` (60 to 70 s the first
time, nothing after). Then it plays the room's protocol pair on each, in turn:
two bots (`load.coax everglade2`, the real socket protocol: a hello, inputs at
30 Hz, a cast every 5 s, pings) in one Everglade II room of a local room
server, 10 s, three rounds a side. A side's numbers are its medians. The head
is worse than production when:

| Measure | Worse when | Margin from |
| --- | --- | --- |
| The load's own checks (open, seated, seen, state at 28 Hz or more, ACKs, casts heard) | the head fails in most rounds and production holds in most | correctness |
| The room's tick at the 95th percentile (`/health`, 100 µs steps) | over production's by more than 300 µs and half of production's | 22 rounds: 300 to 400 µs |
| Input to ACK at the 95th percentile | over production's by more than 25 ms | 34 rounds: 11 to 36 ms; a side's median of 3 within 13 ms of the other's |
| The room's bytes a player hears a second | over production's by more than 25% | 33 rounds: 3,165 to 3,359 B/s; medians within 1.5% |

Production's commit is the newest release that passed (`PASSED` in the
production bucket, as `release.sh` reads it), looked up at most every 10
minutes into `releases/after/production`. `OA_AFTER_PRODUCTION` names another;
`OA_AFTER_E2=0` turns the gate off. The settings are `OA_E2_ROUNDS` (3),
`OA_E2_SECONDS` (10), `OA_E2_PLAYERS` (2), `OA_E2_TICK_US`, `OA_E2_ACK_MS` and
`OA_E2_BYTES_PCT`.

A failing measure is a line `e2 NAME` (`e2 holds`, `e2 tick-p95`, `e2
ack-p95`, `e2 bytes`). It is run again on the head first, as every failure is,
so a noisy round alone is flaky, not red. Then `main-green` itself is checked:
if it is also worse than production, no commit of this run caused it, and the
failure is written to `releases/after/e2-reports.tsv` and the log, not the red
list. Otherwise the bisect finds the commit.

Noise, measured 2026-11-11 on `oa-build-1`, `-2` and `-11` (34 rounds,
production `4dd7b49d65` against heads a few commits later, so both sides ran
nearly the same server): the numbers in the table above. One round of the 34
failed the load's checks (one of the two bots was not seated in time), which
is why `holds` goes by most rounds and a failure is run again before it
counts. A gate takes 70 to 150 s on a host with production's room built, and
about 60 s more the first time on a host.

The seeded regression: 1.5 ms of busy work at the start of each Everglade
tick in `room-host.c` (never committed). Its tick p95 was 1,800 µs in all
three rounds against production's 300, and `check.sh e2` failed with `e2
tick-p95`; the ACK and bytes stayed inside their margins (30 against 17 ms,
3,297 against 3,290 B/s). The same check on the head without the seed passed
(tick 300 against 300 µs, ACK 18 against 21 ms, bytes 3,246 against 3,287).

**What stays a report, and why.** The 60 FPS check (`coax/web/perf.ts`, now
four places down Everglade II's path, `--v1` for Everglade's old scenes) and
the two-browser pair (`everglade2-latency`) need a page that draws. The build
hosts have no GPU. With Chromium from nixpkgs on a host, WebGPU gives a
SwiftShader adapter, but Everglade II never drew its first scene: `perf.ts`
failed three times out of three at its 30-second wait, and a probe waited
120 s twice without a scene. The frames it timed meanwhile were 16.7 ms each,
the headless frame clock with nothing drawn, so a frame time from a host
measures nothing. On the Mac a browser run would take one of its two heavy
browser loops on every head. So both run after each release, on production:
`after-release.sh` writes `AFTER` (goldens and `perf.ts`) and `PAIR`
(`everglade2-latency`, two headless Chromes in a `pair-e2` room) to the
release's folder in the bucket. A GPU host (`g2-standard-8` spot, about $0.30
an hour) could gate on frames; that is not built.

#### Flaky checks

The flaky list is `releases/after/flaky.tsv` on the watcher's machine, as the
red list is: one line a check, `KEY FIRST-HEAD SINCE LAST TIMES OWNER ISSUE`.
The key is the check with its base commit taken out, so the same suite part
is one entry from run to run. When a check first turns out flaky, the entry
gets its owner, the agent of the last commit that changed the check's file (a
suite's path, a compiler unit's), or `pipeline` when there is none, and one
GitHub issue, "Flaky check: KEY". The owner gets a message. An entry older
than seven days (`OA_AFTER_FLAKY_DAYS`) counts as a failure: `after.sh state`
and `after.sh flaky` print it as FAILING, and `after.sh flaky` exits 1. An
entry leaves the list when its issue is closed (the watcher looks once an
hour). The list before this, `flaky.txt` (head and check, no owner), is kept as
`flaky-before-13266.txt`.

#### `main-green`'s lag

The watcher writes the lag (the age of the oldest commit on `main` after
`main-green`) to `releases/after/lag.tsv` every five minutes. `after.sh state`
prints it now and, for the last seven days, the worst and the share of samples
under 30 minutes. That line is where the week-long observation of #13255
(under 30 minutes behind for a week) is read.

### 6. Releases ship `main-green`

`release.sh` already takes `--ref COMMIT`. Its default becomes
`origin/main-green` instead of `origin/main`, so a fresh failure on `main`
never blocks a release. The release rule (a check that fails on staging and
passes on production stops it) stays as it is.

## What changes for agents

- Start worktrees from `origin/main-green` when it exists. A failure in that
  worktree is your own change's.
- Put `Agent: <id>` in every commit message. The pre-push check refuses a
  commit without one.
- When the pre-push check refuses a commit for undoing newer work, rebase
  and restore the other agent's files; name a file in the message only when
  you mean to delete or restore it.
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
| Wire shape registry check | `coax/web/tool.sh wire-check` (#13236) |

## What is built

Issue [#13102](https://github.com/OpenAgentsInc/openagents/issues/13102).

| Part | Script |
| --- | --- |
| Submit and the queue refs | `coax/remote/submit.sh` (`status`, `withdraw`) |
| The integrator | `coax/remote/integrate.sh` (`--once`, `status`) |
| One check on a host | `coax/remote/check.sh` (`branch BASE`, `since BASE I/N`, `web BASE`, `room-since BASE [PROD]`, `coder-since BASE`, `coder`, `suites I/N`, `only PATHS`, `compiler I/N`, `unit UNIT`, `modules I/N`, `module NAME`, `wasm`, `reports`, `visuals`, `room [PROD]`, `tools [PROD]`, `e2 PROD`, `counts BASE`) |
| The after-landing check, bisect and red list | `coax/remote/after.sh` (`--once`, `--watch`, `install`, `uninstall`, `state`, `health`, `red`, `history`, `flaky`) |
| The allowlist counts (#13266) | `coax/remote/counts.sh` (`show [REV]`, `check BASE [REV]`) |
| The comparing gates' own check (#13266) | `coax/remote/test-after.sh` |
| The revert | `coax/remote/integrate.sh --revert`, which `after.sh --watch` runs each minute |
| Releases from `main-green` | `coax/web/release.sh` |
| The pre-push check (#13265) | `coax/remote/hooks/pre-push`, `coax/web/prepush.coax` |
| Shared settings | `coax/remote/pipeline-lib.sh` |

`remote.sh run --commit REV --out DIR` runs a check on a commit's own files
instead of a snapshot of the checkout, so the integrator and `after.sh` check
exact commits without a worktree. `coax/web/tool.sh suites --part I/N` splits
the suites across host slots.

One watcher runs at a time. It runs the after-landing check on each new head,
and one integrator round a minute for the 30-minute revert and for any branch
an agent submitted:

```sh
coax/remote/after.sh install        # start it, now and at every login or boot
coax/remote/after.sh uninstall      # stop it
coax/remote/after.sh --watch        # the same loop in a terminal (--no-revert: revert nothing)
```

**The live watcher is on `coderos-4080`**, as the systemd user service
`com.openagents.green-main`, since 2026-11-11 (#13255). It runs from its own
clone, `~/.openagents/pipeline/clone` (not `~/openagents`), uses the build
hosts' SSH key and ledger in `~/.openagents/pipeline/remote`
(`OA_REMOTE_STATE`), and pushes `main-green` and reverts with the GitHub
login there. It builds nothing itself: every check goes to the pool hosts.
From any machine:

```sh
coax/remote/after.sh state     # first line: "watcher running on coderos-4080, seen N s ago"
coax/remote/after.sh health    # status 1 when it is not running
```

On coderos itself:

```sh
systemctl --user status com.openagents.green-main
tail -f ~/.openagents/pipeline/logs/after.out
```

On macOS, `install` writes `~/Library/LaunchAgents/com.openagents.green-main.plist`.
On Linux, it writes the systemd user service
`~/.config/systemd/user/com.openagents.green-main.service` (`Restart=always`,
`RestartSec=60`, `KillMode=process`) and enables it; `sudo loginctl
enable-linger USER` keeps it running with no one logged in (coderos has it
on), and `OA_AFTER_INSTALL_PRINT=1` prints the unit without enabling it. The
unit keeps `OA_REMOTE_STATE`, `OA_REMOTE_DAILY_USD`, `CLOUDSDK_CONFIG`,
`OA_PIPELINE_SHARED` and `OA_PIPELINE_NOTIFY` when they are set at install.
Either way the job runs `after.sh --watch` in a checkout of its own,
`~/.openagents/pipeline/checkout`, which holds only `coax/remote`, and is
started again whenever it stops. When the pipeline's scripts change on `main`,
the watcher stops after its current run and starts again on the new scripts.
Its log is `~/.openagents/pipeline/logs/after.out`. coderos was installed
with:

```sh
cd ~/.openagents/pipeline/clone
OA_REMOTE_STATE=$HOME/.openagents/pipeline/remote coax/remote/after.sh install
```

#### The shared state

The watcher's files live on its machine. Each minute, beside its heartbeat,
it writes the files other machines read (the heartbeat, the red list, the
last runs, the flaky and lag lists, production, the newest Everglade II
measurement, the alarms and every agent's inbox) as one object,
`gs://openagentsgemini-coax-build-cache/pipeline/state/state.tar.gz`
(`pipeline-lib.sh`'s `pl_publish`; `OA_PIPELINE_SHARED` names another prefix,
`0` none). On a machine whose own watcher isn't running, `after.sh state`,
`red` and `health` first copy it into the same files under
`~/.openagents/pipeline` (`pl_mirror`, at most every 20 s). So everything that
reads those files reads the watcher's: the pre-push check's red list,
`status-data.coax`, and `submit.sh status --agent ID` for an agent's inbox.
`state` names the machine: "watcher running on coderos-4080". A machine
whose own watcher is running (a fresh heartbeat and no `watcher-host` file)
reads only its own files; `--watch` removes `watcher-host` when it starts.

A watcher can't report that it died. When a machine's `state` or `health`
sees the shared heartbeat 15 minutes old or more, that machine raises the
alarm: the log, its `ALARM` file and the one "Green-main pipeline needs
attention" issue, which every machine reuses (`pl_alarm` looks for an open
one first). `release.sh` runs `health`, so a release raises it too.

The build hosts' daily cap counts every machine. Each `remote.sh` copies its
ledger to `gs://openagentsgemini-coax-build-cache/pipeline/ledgers/HOST-ID.tsv`
and reads the others back into `~/.openagents/remote/ledgers/all/`, at most
every 5 minutes and in the background, and `spent()` adds them up
(`OA_REMOTE_SHARED_LEDGERS=0`: this machine's ledger alone). The cap can be
up to 5 minutes behind.

#### Moving the watcher

Two watchers on two machines would race: both would move `main-green` and
land reverts. To move it, on the old machine run `after.sh uninstall`, copy
its `~/.openagents/pipeline/releases/after/` state files (`red.json`,
`runs.tsv`, `flaky*`, `full-green`, `full-head`, `last-*`, `production`,
`history.tsv`) and `inbox/` to the new one, then run `after.sh install` there
and watch its first run in the log. Back to the Mac, for example:

```sh
ssh coderos-4080 'cd ~/.openagents/pipeline/clone && coax/remote/after.sh uninstall'
coax/remote/after.sh install      # on the Mac, from any checkout
```

A Mac watcher starts from the state the mirror last copied, so after
`after.sh state` has run there it starts where coderos stopped.

#### Why coderos, not a pool host

Issue [#13255](https://github.com/OpenAgentsInc/openagents/issues/13255)
asked to move the watcher off the Mac. The pool hosts can't hold it: each is
a spot machine that powers itself off after 15 idle minutes and can be
preempted, and none has a service account or a GitHub credential. coderos is
always on, already had the automation account's gcloud configuration and a
GitHub login, and costs nothing more; the owner approved it on 2026-11-11.
Only the build hosts' SSH key was copied to it (the hosts' metadata holds the
Mac's public key).

### When the pipeline itself fails

Until 2026-11-10 nothing ran the watcher: each run was started by hand with
`after.sh --once`. The last of them ended at 21:45 Chicago time, and for more
than an hour 40 commits landed with no check and `main-green` did not move.
The pipeline now reports its own trouble in three places: the log, the file
`~/.openagents/pipeline/ALARM` (which `after.sh state` prints), and one
GitHub issue titled "Green-main pipeline needs attention", which it opens,
comments on, and closes when `main-green` next moves. It reports when:

- the watcher starts after 15 minutes or more without running;
- `main-green` is three hours behind `main` (`OA_AFTER_STALE`), for any
  reason;
- the hosts could not finish the check of one head three times in a row.

Each kind is reported at most once every two hours
(`OA_PIPELINE_ALARM_GAP`). `release.sh` also prints a warning when the
watcher is not running (`after.sh health`).

The watcher's two locks are `flock`s on `after.lock` (a run) and
`watch.lock` (the watcher). A process that inherits either descriptor keeps
the lock after the watcher lets go. On 2026-11-11 an orphaned `sleep` from
`remote.sh`'s token refresher held both for 12 minutes: the log said "a run
is going" every 30 seconds and the restarted watcher said "another watcher is
running". Host jobs now run without the lock descriptors, and `remote.sh`
ends the refresher's children with it (ab67fe133c). If the log says either
line with no run in progress, `lsof ~/.openagents/pipeline/releases/after/*.lock`
names the holder.

On 2026-11-11 a full run failed three suite parts that the fast runs had
passed, bisected them to three commits, and moved `main-green` back three
hours, to the last full green run. None of the commits was at fault. The
suites had refused the host slot's compiler ("coax/bin/coax was not built
from this tree"): its record names the Nix toolchain's store paths, and
`nixpkgs-unstable` had moved since the slot built it, while `host-job.sh`
kept a slot's compiler by its sources alone. 36 jobs failed that way that day,
on four hosts, and two earlier false culprits (#13234, #13235) came from it
too. `host-job.sh` now keeps a slot's compiler only while
`provenance.sh verify` passes in the job's toolchain, and `check.sh` exits 97
(the host could not judge it) on that refusal, so the pipeline runs the check
again instead of bisecting it (2135f8731e).

The same evening the 30-minute revert of c4040e643d (#13309) was queued with
conflict markers in `apps/openagents-web/build.sh`: four later commits built
on the module it added, and `git merge-tree --write-tree` prints a tree on a
conflict too, with exit status 1, which `integrate.sh` didn't read. The
revert's check failed, so nothing bad landed. `integrate.sh` now queues a
revert only when the merge is clean; a conflicting one is left to its agent,
and its issue says so.

Measured on 2026-11-09 on today's tree: a branch check takes 8 to 42 s of
host time when it touches a few suites or modules, about $0.001 to $0.003 of
host time (remote.sh's ledger bills each job at a whole host's price, three
times that). A full run is 11 jobs and 11,712 slot-seconds, about $0.30 of
host time ($0.90 on the ledger); with the hosts shared with agents' own
builds it took 31 minutes. On 2026-11-11, with native checks and check
reuse, a full run of 18 jobs took 455 s and 3,222 slot-seconds (f6b7edd058,
the first full run to pass). Logs, each
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
job and by free slots, not by the number of commits.

That unit was `parser` (compiler 2/8, #13145). It builds the whole compiler
under ASan and UBSan, one translation unit that takes 173 s of one core on an
idle host, from nothing in every run, against a limit of 180 s on the clock;
any load put it over. The objects are now kept by what they are made from, on
the host and in the shared cache
([remote-builds.md](remote-builds.md#the-shared-cache)), and a compile that
does run is limited by the CPU it uses, not by the clock. A full run of all 980 suites, 55 modules, the reports,
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
- `check-visuals` is a report check since 2026-11-10. `build.sh` stops on it
  before any module, and the module jobs run `wasm.sh` alone, so no run saw
  it fail. A bisect runs `coax/web/check-visuals.sh` as itself, so it can
  judge commits older than the step.
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
  recorded on the flaky list (`releases/after/flaky.tsv`, with an owner and an
  issue), and if every failure of a run was flaky, `main-green` moves.
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
- The red list and the messages to agents are files on the watcher's machine,
  shared through the bucket (see "The shared state"). A machine without the
  automation account's gcloud reads `origin/main-green` and the issues.

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
