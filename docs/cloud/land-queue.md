# One landing queue, idle stop for environments

2026-10-10, [#11227](https://github.com/OpenAgentsInc/openagents/issues/11227).
Gaps 12 and 15 of [dogfood-dev-on-prod.md](dogfood-dev-on-prod.md).

Agents on any machine (the Mac, CoderOS, GCE pool hosts, Boat sandboxes,
cloud environments) hand their finished branches to one queue. One
integrator, on the cloud environment `oa-dev-env-1`, lands them on `main`
one at a time. The environment powers itself off when nothing has run for
30 minutes.

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

## What the integrator does with each entry

`openagents land work` (the `oa-land-worker` service on the environment):

1. Takes the oldest queued entry and marks it `landing`.
2. Fetches `main` and the entry's branch into its own worktree
   (`~/.openagents/land-queue/work`) and rebases the branch onto `main`.
3. Runs the touched-crate checks, the issue flow's own gate
   (`coder::task::issue_run::Gate`): the tests of each touched Rust package
   in a leased build slot with sccache on GCS, the diff checks, and
   `cargo fmt --check` when the repository's policy asks. Red checks bounce
   the entry.
4. Lands with `coder::task::landing::land`, the issue flow's `--land main`:
   a plain push, and on a lost race a jittered retry that rebases again and
   reruns the checks only when the newer commits can affect the change.
5. A rebase conflict gets the landing's one bounded repair turn (#10418), a
   headless `claude -p` in the worktree (`--repair COMMAND`, or `none`).
   Then the checks run again. A conflict that stays bounces the entry.
6. Closes or comments the issue. The issue flow's `--land queue`
   (`openagents chat work --issues N --land queue`, or `"land": "queue"` in
   `.openagents/coder-issues.json`) submits entries with `close: true`, so
   the integrator closes the issue it worked. A bounced entry's issue gets the reason and
   the branch stays on `origin` for its author to fix and submit again.
7. Writes a record of the try. Pushes that kept failing put the entry back
   in line, at most three times.

## The queue

The queue is JSON in Cloud Storage, which every machine with the project's
account reaches (`OPENAGENTS_LAND_QUEUE` or `--queue` names another prefix
or a folder):

```text
gs://openagentsgemini-coder-artifacts/land-queue/openagents/
  entries/<id>.json          one per branch: state, branch, issue, machine, commit or reason
  attempts/<id>/<NNN>.json   one per try: checks, landing attempts, repair, outcome
  worker.json                the integrator's heartbeat: machine, time, current entry, GCE instance
```

An id is `<UTC time>-<machine>-<suffix>`, so the queue sorts oldest first.
States: `queued`, `landing`, `landed`, `bounced`, `withdrawn`. The web and
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
  session and the queue picks up where it was.

`scripts/cloud/dev-env-agent.sh check` says busy or idle and why. Change the
limit from anywhere:

```sh
gcloud compute instances add-metadata oa-dev-env-1 --zone us-central1-b \
  --project openagentsgemini --metadata oa-dev-env-idle-minutes=60
```
