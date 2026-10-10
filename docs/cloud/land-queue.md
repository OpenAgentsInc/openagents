# One landing queue, idle stop for environments

2026-10-10, [#11227](https://github.com/OpenAgentsInc/openagents/issues/11227).
Gaps 12 and 15 of [dogfood-dev-on-prod.md](dogfood-dev-on-prod.md).

Agents on any machine (the Mac, CoderOS, GCE pool hosts, Boat sandboxes,
cloud environments) hand their finished branches to one queue. One
integrator, on the cloud environment `oa-dev-env-1`, lands them on `main`
through three lanes ([#11248](https://github.com/OpenAgentsInc/openagents/issues/11248)):
documents in seconds, unrelated code changes side by side, and related ones
one at a time in the order they came. The environment powers itself off
when nothing has run for 30 minutes.

## Submit a branch

From a checkout with the change committed:

```sh
openagents land submit --issue 1234          # push HEAD to land/<id>, queue it
openagents land submit --branch my-branch    # a branch already on origin
openagents land status                       # the integrator and what waits
openagents land show <id>                    # every try at one entry
openagents land withdraw <id>                # take a queued entry out
```

`--issue N` closes issue N once the change lands and moves the project board
to Done (through `scripts/dev/issue-board.sh`); `--no-close` only comments.
When the integrator's environment is stopped, `submit` starts it
(`--no-wake` skips that).

## Lanes

Before an entry lands the integrator plans it (`coder::task::land_plan`) in
a worktree of its own (`~/.openagents/land-queue/plan`):

- **Fast lane.** Every changed path is a document or an image (`.md`,
  `.txt`, `.png`, `.svg`, `.pdf`, ...) under `docs/`, under `nips/`, or at
  the top of the repository, and no file the build can read names it. The
  build is skipped; the diff checks run (conflict markers, broken links,
  figures with no source), then rebase and push. One fast slot, so a
  document never waits behind code.
- **Code lane.** Everything else. When unsure, it is code: a document
  inside a crate or app, anything under `knowledge/`, `fixtures/`,
  `plugins/` and the other data folders, `Cargo.toml`, `Cargo.lock`,
  `build.rs`, and any document that a quoted path in a `.rs`, `.toml`,
  `.json`, script or workflow file names (an `include_str!`, a test's
  fixture, a folder a test reads). A changed file outside every package
  that a package's sources name counts as that package's change, and its
  tests run too.
- **Code slots.** Two by default (`--slots N` or `OPENAGENTS_LAND_SLOTS`),
  each with its own worktree (`work-code-1`, `work-code-2`) and its own
  build slot of the task store (its own `CARGO_TARGET_DIR`), sharing the
  sccache bucket through `~/.cargo/config.toml`'s wrapper. Each check run is
  a full `cargo test` that uses every one of the environment's 22 vCPUs,
  and the host broker counts two build leases, so a third slot would wait.
- **Strict order where it matters.** Two code entries overlap when one's
  packages are among the other's packages, their dependencies, or their
  dependents (the relation the landing already uses to decide whether
  newly landed commits re-run checks), when they change the same file, or
  when either changes a workspace-wide build file (`Cargo.toml`,
  `Cargo.lock`, `rust-toolchain.toml`, `.cargo/`). A code entry waits while
  any earlier open entry overlaps it; `land status` says which
  (`waits for <id>`).

Pushing to `main` stays strictly one at a time: every slot takes one lock
for fetch → rebase → push only, never while checks run. When `main` moved
while a slot's checks ran, the landing rebases and re-runs the checks only
when the new commits touch the entry's packages, what they depend on, or
what depends on them; otherwise it pushes at once. That is what keeps a
busy `main` from re-running every check run behind it.

### Measured on 2026-10-10

First run of the lanes on `oa-dev-env-1`, from 23:44 UTC:

- A docs-only entry (`20261010T234929Z-...-500e`) went from submit to
  `main` in 38 s: 24 s until the worker picked and planned it, 14 s to
  rebase, run the diff checks and push. Both code slots were busy the
  whole time. Under the one-at-a-time worker it would have waited for
  every code entry ahead of it, 25 to 35 minutes each.
- Two code entries with unrelated packages checked side by side: the
  lanes change itself in `code-1` (23:45 to 00:13) and a
  `coder-cloud-web` change in `code-2` (23:49 to 23:59). `main` moved twice
  during the second one's checks; the newer commits could not affect it, so
  it pushed without checking again, and landed about 24 minutes before it
  could have started under the old worker.
- Two entries that touch crates depending on `coder` waited for the lanes
  change, as they should (`waits for ...` in `land status`).

## What the integrator does with each entry

`openagents land work` (the `oa-land-worker` service on the environment),
in the entry's slot:

1. Marks the entry `landing` with its lane, slot and start time.
2. Fetches `main` and the entry's branch into the slot's worktree and
   rebases the branch onto `main`.
3. Code lane: writes known generated files again when the change touches
   their sources (below) and folds them into the change's last commit.
4. Runs the checks. Code lane: the issue flow's gate
   (`coder::task::issue_run::Gate`), the tests of each touched package in a
   leased build slot with sccache on GCS, the diff checks, and
   `cargo fmt --check` when the repository's policy asks. Fast lane: the
   diff checks only. Red checks bounce the entry. Checks the worker could
   not run (no cargo, a boundary that would not start) put it back in line
   instead, at most three times.
5. Lands with `coder::task::landing::land` under the push lock: a plain
   push, and on a lost race a jittered retry that rebases again and reruns
   the checks only when the newer commits can affect the change.
6. A rebase conflict only in generated files takes `main`'s copy and writes
   them again. Any other conflict gets the landing's one bounded repair
   turn (#10418), a headless `claude -p` in the worktree (`--repair
   COMMAND`, or `none`). Then the checks run again. A conflict that stays
   bounces the entry.
7. Closes or comments the issue. A bounced entry's issue gets the reason and
   the branch stays on `origin` for its author to fix and submit again.
8. Writes a record of the try (lane, slot, checks, landing attempts,
   regenerated files, outcome). Pushes that kept failing put the entry back
   in line, at most three times.

## Generated files

`.openagents/generated.json` lists committed files generated from sources:
today the CLI tree (`crates/coder/src/cli_route/tree.json`, from the
`openagents` help text) and the first-party OpenAPI document
(`docs/api/openapi.first-party.json`, from the gateway's routes). When a
code entry changes one of a generator's sources (holding one of its
markers, such as `USAGE` for the CLI tree), the integrator runs its
command before the checks and folds the result into the entry. A new
`--flag` no longer turns `main` red because nobody ran
`OPENAGENTS_WRITE_CLI_TREE=1`.

Making `tree.json` a build output instead was considered and left out: the
tree comes from `openagents-cli`'s help strings, `coder` bundles it, and
`openagents-cli` depends on `coder`, so a build script in `coder` cannot
see the strings it would generate from. Pinned digests of large assets
(the Grid and Everglade packs) keep their own queue
(`docs/coder/runtime/artifact-queue.md`), since rebuilding them needs
release builds and private packs.

## Status

```text
$ openagents land status
integrator: oa-dev-env-1 (running, seen 3s ago, fast lane + 2 code slot(s))
  code-1 (code): 20261010T231502Z-... for 12m
  code-2 (code): 20261010T231610Z-... for 11m
ID  STATE  LANE  SLOT  ELAPSED  FROM  ISSUE  AGE  TRIES  RESULT
```

`LANE` is `fast` or `code`; `SLOT` the slot that has or had it; `ELAPSED`
how long the current try has run (or the last one took). `land show <id>`
gives each try's lane, slot and duration.

## Deploying a new integrator

The worker never loses an entry on a restart: one it was landing is still
`landing` under its name and is taken again first. To restart without
cutting a check run short, drain it:

```sh
touch ~/.openagents/land-queue/drain       # take nothing new
openagents land status                     # wait until no slot is busy
cp <new build> ~/.openagents/bin/openagents
sudo systemctl restart oa-land-worker
rm ~/.openagents/land-queue/drain
```

## The queue

The queue is JSON in Cloud Storage, which every machine with the project's
account reaches (`OPENAGENTS_LAND_QUEUE` or `--queue` names another prefix
or a folder):

```text
gs://openagentsgemini-coder-artifacts/land-queue/openagents/
  entries/<id>.json          one per branch: state, branch, issue, machine, commit or reason
  attempts/<id>/<NNN>.json   one per try: checks, landing attempts, repair, outcome
  worker.json                the integrator's heartbeat: machine, time, busy slots, GCE instance
```

An id is `<UTC time>-<machine>-<suffix>`, so the queue sorts oldest first.
States: `queued`, `landing`, `landed`, `bounced`, `withdrawn`. An entry also
records its `lane`, `slot`, `started_at` and, while it waits on an
overlapping earlier entry, `waiting_for`. The web and
phone fleet view (#11228) reads the same objects.

## Idle stop

`sudo scripts/cloud/dev-env-agent.sh install` on the environment sets up:

- `oa-dev-env-idle.timer`, every minute. The environment is busy while any of
  these exist: a job (`cargo`, `rustc`, `claude`, `codex`, `microcoder`, a
  Coder run, the queue's busy marker), an ssh session, or
  `~/.openagents/keep-awake`. After `oa-dev-env-idle-minutes` (instance
  metadata, default 30, `0` never stops) with none of them, it powers off.
  A stopped instance bills only its 300 GB disk, about $0.04 an hour.
- `oa-land-worker.service`, the integrator. It signs in with
  `dev-env-session.sh` at each start, so starting the VM again restores the
  session and the queue picks up where it was. Its PATH names rustup's
  `~/.cargo/bin` after the login profile (which otherwise drops it), the
  unit will not start without cargo there, and `land work` refuses to take
  entries when `cargo --version` fails: entries stay queued instead of
  bouncing with the worker's problem.

`scripts/cloud/dev-env-agent.sh check` says busy or idle and why. Change the
limit from anywhere:

```sh
gcloud compute instances add-metadata oa-dev-env-1 --zone us-central1-b \
  --project openagentsgemini --metadata oa-dev-env-idle-minutes=60
```
