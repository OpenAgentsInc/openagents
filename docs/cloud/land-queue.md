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
(`--no-wake` skips that). When it can't (a pool host's account may not
start other instances), `submit` still queues the change, prints a
`warning:` line, and sets `integrator_down` in its JSON with the
`gcloud compute instances start` command to run. The briefed engine's run
result and an issue comment carry the same warning
([#12206](https://github.com/OpenAgentsInc/openagents/issues/12206)).

## App and SDK access

`client.land(&session)` provides `submit`, `status`, `show`, and `withdraw`
through `POST /v1/land`, `GET /v1/land`, `GET /v1/land/{id}`, and
`DELETE /v1/land/{id}`. The `/api/v1/land` aliases answer too. These routes
require an app bearer for an account allowed to run agent work. Submit a
branch already on origin with a stable `Idempotency-Key`; retry with the
same key and inputs. Only its submitter or an administrator can withdraw a
queued change. Withdrawal does not stop an integrator that has already taken
the change; check its status. `show` includes each landing attempt; `status` groups waiting,
landing, and failed changes and reports the integrator's heartbeat.

The web server uses its cloud identity to read and write the CLI's queue
objects. `OPENAGENTS_LAND_QUEUE` selects the same bucket prefix or local
folder as the CLI, and `OPENAGENTS_LAND_REPO` names the integrator's
repository (default `OpenAgentsInc/openagents`). Other repositories are
refused. The app's `land_status` tool reads this API through its signed-in
account, so any custom agent can use it.

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
- **Code slots.** One per eight cores, two to four (`--slots N` or
  `OPENAGENTS_LAND_SLOTS`; `oa-dev-env-1` sets 3 in the systemd drop-in
  `oa-land-worker.service.d/slots.conf`, since `dev-env-session.sh` rewrites
  `~/.openagents/dev-env.env` at each start), each with its own worktree (`work-code-1`,
  `work-code-2`, ...) and its own build slot of the task store (its own
  `CARGO_TARGET_DIR`), sharing the build cache through
  `~/.cargo/config.toml`'s wrapper.
- **Checks fail fast.** Each touched package runs `cargo fmt --check`,
  then `cargo check -q -p PACKAGE` (the library and binaries as they ship,
  without `cfg(test)`;
  [#12210](https://github.com/OpenAgentsInc/openagents/issues/12210)), then
  `cargo check --tests` ([#12198](https://github.com/OpenAgentsInc/openagents/issues/12198)),
  then the tests. A failed step skips the rest. The briefed agent's
  `verify` tool and the replayed checks in `scripts/work/work_issue.py`
  run the same shipped-build check before the tests.
- **Checks test default features.** Each touched package runs `cargo test
  -q -p PACKAGE` with its default features, never `--all-features`, which
  pulled the Verse stack and other optional crates into most checks
  ([#12162](https://github.com/OpenAgentsInc/openagents/issues/12162)).
- **Strict order where it matters.** Two code entries overlap when one's
  packages are among the other's packages, their dependencies, or their
  dependents (the relation the landing already uses to decide whether
  newly landed commits re-run checks), when they change the same file
  other than `Cargo.lock`, or when either changes a workspace-wide build
  file (the root `Cargo.toml`, `rust-toolchain.toml`, `.cargo/`). A code
  entry waits while any earlier open entry overlaps it; `land status` says
  which (`waits for <id>`).
- **`Cargo.lock` counts by package.** The plan compares the lockfile
  before and after the change and counts only the local packages whose
  locked entries changed or that reach a changed one: a version bump of
  `regex` counts as a change to every package that locks `regex`, and a
  hunk that only adds a new workspace member counts as a change to that
  member. A lockfile that does not parse, or a package graph that cannot
  be read, still overlaps everything. After a rebase, a change to
  `Cargo.lock` re-runs its checks only when the newly landed commits
  changed `Cargo.lock` too.

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
3. Code lane: runs `scripts/dev/regenerate.sh --changed origin/main`
   (below) and commits what it rewrote on top of the change as
   "Regenerate generated files".
4. Runs the checks. Code lane: the issue flow's gate
   (`coder::task::issue_run::Gate`), the tests of each touched package in a
   leased build slot with sccache on GCS, the diff checks, and
   `cargo fmt --check` when the repository's policy asks. Fast lane: the
   diff checks only. Red checks bounce the entry, except a test that also
   fails on the target the change was rebased onto: the gate runs it there
   by name and reports it in the record (`already failing on main: ...`)
   instead of bouncing ([#11300](https://github.com/OpenAgentsInc/openagents/issues/11300)).
   The first check and every re-check after a rebase compare with the
   target they ran on. Checks the worker could
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

`.openagents/generated.json` lists committed files generated from sources,
each with the test that fails when the file is stale (`check`) beside the
command that writes it again (`regenerate`): the CLI tree, the first-party
OpenAPI document, the SDK reference, the app API contract, the route-map
sources, and the Android core binding
([#12208](https://github.com/OpenAgentsInc/openagents/issues/12208)).
`scripts/dev/regenerate.sh` runs them:

```sh
scripts/dev/regenerate.sh                        # every generator
scripts/dev/regenerate.sh --changed origin/main  # those the change is due
scripts/dev/regenerate.sh --list                 # each check beside its command
```

A change is due a generator when it touches one of its `sources` (holding
one of its `markers`, when given) or any file of the package whose test
checks it. The integrator runs the script with `--changed` on each rebased
code entry before the checks (adding `--packages` for the packages the
change reaches without touching them) and commits what it rewrote as
"Regenerate generated files". A generator whose `requires` is missing is
skipped and named in the entry's result (`regenerate_skipped` in `land
show`): the Android binding needs `ANDROID_NDK_HOME` and `cargo-ndk`, which
the integrator lacks. Agents run the same script before they push.

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

When changes wait and the integrator's heartbeat is more than five minutes
old, `land status` (and the status API) starts with an `ALERT:` line that
names the integrator, how long ago it was seen, and the command that starts
it ([#12206](https://github.com/OpenAgentsInc/openagents/issues/12206)).

`LANE` is `fast` or `code`; `SLOT` the slot that has or had it; `ELAPSED`
how long the current try has run (or the last one took). `land show <id>`
gives each try's lane, slot and duration.

The queue also shows on openagents.com (Settings > Agents) and on the
phone's Agents screen (#11250): while `land work` runs and the integrator
computer is signed in (`OPENAGENTS_APP_TOKEN`, else `coder login`), it
reports every open entry, and those that ended in the last six hours, as
rows of kind `landing` on that computer's list
(`crates/openagents-cli/src/land_fleet.rs`). A row's title is `#N` and the
summary; its line is `queued`, `landing: checks` (or `quick checks` in the
fast lane), `landed <commit>`, `bounced: <reason>` or `withdrawn`. The rows
sit beside the issue runs `chat work` reports from the same computer
(`POST /v1/computers/{name}/activity?kind=landing` replaces only the
landing rows). They take no Stop or Message: the website answers with the
`openagents land withdraw ID` to run instead.

### The integrator's sign-in

The integrator signs in as its own service account, never the owner's
(AGENTS.md; [#12251](https://github.com/OpenAgentsInc/openagents/issues/12251)).
The account is a Nostr key's: a key that signs in through
`POST /v1/device/nostr` with a proof of work makes a new account
(`openagents_sdk::Client::sign_in_with_key_on`,
[Nostr sign-in](../auth/README.md#sign-in-with-a-nostr-key)). The key is
kept off the environment, in `~/work/.secrets/land-integrator-nostr.key`
on the owner's Mac. From that account's session, `openagents keys create
--name "Land integrator" --expires never` mints the login key (`oa_sk_…`)
the integrator uses. It lives in the Secret Manager secret
`dev-openagents-app-token`, which `scripts/cloud/dev-env-session.sh` writes
to `~/.openagents/dev-env.env` as `OPENAGENTS_APP_TOKEN` at each start of
`oa-land-worker`. `dev-env-session.sh --check` says whether it is set.

The landing rows then show on that account's Settings > Agents and phone
Agents screen, under the computer `oa-dev-env-1`. To rotate the key, run
`openagents keys rotate "Land integrator"` as that account, add the new key
as a new version of the secret, and restart the worker after draining it
(below).

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
  Coder run, the queue's busy marker, which the integrator holds while an
  entry lands or waits in the queue), an ssh session, or
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
