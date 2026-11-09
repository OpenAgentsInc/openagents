# The green-main pipeline

This document describes how changes from many agents reach `main` and
production without blocking each other. Builders push quickly after a
short check of what they touched. The full suite runs after every push, on
the remote build hosts. A last-known-good commit, `main-green`, is what
releases ship and what new work starts from. When a check fails, the
pipeline finds the commit that caused it and sends the fix to the agent
that wrote it.

The design follows a merge-queue layout that Kit Langton published on
2026-11-09: builders hand off to an integrator, a fast check runs before the
push, a full check runs after it, and a failure becomes a task that names the
culprit. Here it is fitted to Coax, the remote build hosts
([remote-builds.md](remote-builds.md), #13077) and `coax/web/release.sh`.

## Why

On 2026-11-09 about twelve agents pushed to `main` in one day. Most of the
waiting came from five problems:

| What happened | Cost |
| --- | --- |
| The steering commit `c3f65bfb55` broke four Experimental checks. The Shapechange commit `7353346533` pushed the fivelab unit over its compile-time storage limit (E0400). | `release.sh` could not ship for several hours while we bisected by hand. |
| Agents started worktrees from a `main` that was already broken, so their own runs failed on someone else's errors. | Runs were thrown away and repeated. One fix's results were wrong because its base predated a compile fix. |
| fivelab grew to 2,747,624 bytes against a 2,725,000-byte budget, and nothing noticed until `release.sh` built it. | A failed release and a second release run. |
| fivelab sat at 97% of its compile-time traversal and value-slot limits, so each new feature tipped it over. | Shapechange, the two-hander blend checks and the Everglade II fixes each needed an extra round. |
| Agents could not build, so every run went through one coordinator and the Mac's two build slots. | Runs queued for 30 to 60 minutes. A check that takes 7 minutes on a remote host waited 40. |

The remote build hosts fixed the last problem. This pipeline fixes the other
four.

## The pipeline

```
 builder worktree (from main-green)
        │
        ▼
 pre-push check, ~2–4 min, on a remote host
   rebase on origin/main · compile the touched units · run their checks
   · size and compile-limit check for touched wasm targets
        │ green
        ▼
 git push origin HEAD:main ──────────────► main
                                            │
                                            ▼
                          after-push check, on the remote hosts
                            every suite · every wasm budget · compile limits
                            · room · everglade-parity · tools-check
                             │ green                      │ red
                             ▼                            ▼
                     main-green moves to          find the culprit commit
                     this commit                  (remote bisect, last green → head)
                             │                            │
                             ▼                            ▼
                     release.sh ships             task to the culprit's agent;
                     main-green                   revert if not fixed in 30 min
```

### 1. Builders start from `main-green`

Every agent makes its worktree from the last commit that passed the full check:

```sh
git fetch origin
git worktree add ../oa-TOPIC origin/main-green
```

A failure in that worktree belongs to the agent's own change. Agents rebase
onto `origin/main` only for the pre-push check, so they push on top of the
latest work.

### 2. The pre-push check covers only what changed

Before pushing, the builder runs one command:

```sh
coax/remote/precheck.sh
```

It runs on a remote host and does the following, in order:

1. Rebase the worktree onto `origin/main`. A conflict stops here, goes back to
   the builder, and never reaches `main`.
2. Find the touched units. For each entry unit (fivelab, everglade2, room,
   the libraries with tests), `coax deps FILE` lists its source inputs. A unit
   is touched when the diff changes one of its inputs.
3. Compile and test each touched unit, in parallel across host slots.
   Compiler check reuse (#13077) re-runs only the checks whose inputs changed.
4. For each touched wasm target, build it and compare its size with
   `apps/openagents-web/wasm-budgets.txt`.
5. Report each unit's compile-time totals (traversal, value slots, steps). A
   unit above 90% of any limit is a warning that names the unit and the change.

A green pre-push check is the only gate on a push. It should take 2 to 4
minutes on a host. Untouched units don't run.

### 3. The after-push check runs everything, once per new head

A watcher runs `coax/remote/after.sh --watch`. It is light work, so it can run
on the Mac as a LaunchAgent; all heavy work goes to the remote hosts. Every
minute it reads `origin/main`. When the head moves, it runs the full set on
the new head:

- every Coax test suite that `coax/build.sh test` covers, split across host slots;
- every wasm target against its budget;
- the room tests and the drift check's offline profile;
- `everglade-parity`, `tools-check` and the committed shaders against their
  Coax (`wgsl-module --site --check`);
- each unit's compile-time totals, recorded per commit.

If several pushes land while a run is going, the next run takes the newest
head. When a run passes, the watcher moves `main-green`:

```sh
git push origin <commit>:refs/heads/main-green
```

The after-push check never blocks a push. It records its results under
`releases/after/<commit>/`, and the history of compile-time totals and wasm
sizes shows which commit made a unit grow.

### 4. A failure finds its culprit and goes back to its author

When the after-push check fails, `after.sh` bisects between `main-green` and
the failing head. It checks only the failing checks, on parallel host slots,
so a run of eight commits takes about three rounds. It then:

1. Files an issue that names the culprit commit, the failing check, its
   output, and the commit's author agent (from the commit trailer or the
   claiming chat).
2. Messages that agent with the issue.
3. Adds the failure to the red list (`releases/after/red.json`) so other
   agents know a failure on `main` is already owned.

If the culprit's fix isn't on `main` within 30 minutes, `after.sh` reverts
the culprit commit and reopens its issue. The revert goes through the same
pre-push check as any other push. Its message says which check failed and
how to land the change again.

### 5. Releases ship `main-green`

`release.sh` already takes `--ref COMMIT`. Its default becomes
`origin/main-green` instead of `origin/main`, so a fresh failure on `main`
never blocks a release. The release rule (a check that fails on staging and
passes on production stops it) stays as it is.

## What changes for agents

- Start worktrees from `origin/main-green`.
- Run `coax/remote/precheck.sh` before every push and push when it's green.
  Don't run whole suites before pushing.
- Never run heavy builds on the Mac (#13077).
- When the after-push check names your commit, fixing it comes before new work.
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

1. `coax/remote/precheck.sh`: rebase, touched units from `coax deps`, parallel
   compile and checks, wasm sizes, compile-limit warnings.
2. `coax/remote/after.sh`: the watcher, the full run, the `main-green` push, the
   per-commit totals, the bisect, the issue, the agent message and the red list.
3. The revert step in `after.sh`, with its 30-minute window.
4. `release.sh` defaulting to `origin/main-green`.
5. `AGENTS.md` and agent briefs: start from `main-green`, run `precheck.sh`.

## Cost

A full fivelab test run on a remote host takes about 995 seconds and costs
about $0.08. A pre-push check of one touched unit with check reuse costs about
$0.02. At 50 pushes a day, with after-push runs batched to the newest head,
the pipeline costs about $5 to $8 a day, inside the $15/day cap and the
$1,500/month GCP limit.

## Not included

- An integrator that lands branches one at a time. Pushes stay direct because
  the pre-push check rebases and tests on the latest `main`. If conflicting
  pushes become common, add an integrator in front of `main` that runs the
  same pre-push check per branch.
- GPU checks, screenshots, drift against live ports and `release.sh` itself.
  These need the Mac's GPU or credentials and stay on the Mac.
