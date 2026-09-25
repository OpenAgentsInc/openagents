# Where trial setup time goes

Issue #9631, measured on 2026-09-25. This page answers two questions for
the Microluna profile: how much of a trial is spent before and after the
agent, and how much of that a cache removes. Nothing here ran an agent, a
Luna session, a Jev call, or a verifier.

## Summary

- Setup is not why Microluna trials are slow. Across 91 retained Microluna
  trials (23.0 trial-hours), the agent's own time is 85.4% of trial time.
  Everything before and after the agent is 9.5%: environment start 3.4%,
  agent setup 0.2%, the handoff to the verifier 1.9%, and the verifier's
  environment 4.0%.
- The largest repeatable cost was a stop, not a start. Harbor's `main`
  service runs `sh -c "sleep infinity"` as PID 1, which ignores SIGTERM,
  so each `docker compose down` waited Docker's full 10-second grace. A
  task with a separate verifier pays that twice: about 20 seconds a trial.
- The second repeatable cost is building the task's images. A task's first
  trial built them in a median 22.5 seconds and up to 479 seconds; later
  trials rebuilt them from Docker's build cache in about 6 seconds, until
  the suite pruned the cache under disk pressure.
- After this change, environment-only startups of three tasks spend 9.5 to
  35.7 seconds on environments, down from 32.6 to 70.6 seconds. The
  allowlist held on all 15 probed startups.

## Per-phase breakdown from retained trials

The 91 trials are every retained Microluna trial under
`~/.openagents/terminal-bench/jobs/` that reached its agent (policies
`microluna-v1` to `microluna-v15`, 23 tasks). The phases come from
Harbor's `result.json` timestamps. The verifier split uses the CTRF report's
own start and stop, where one exists (82 trials).

| Phase | What it holds | Median | Range | Repeatable |
| --- | --- | --- | --- | --- |
| Trial start | Harbor's trial setup before the environment | 0.1 s | 0.0 to 0.2 s | No |
| Environment setup | Egress sidecar image, image build, `docker compose up` | 6.4 s | 2.5 to 479.1 s | Yes: the build |
| First trial of a task (23) | The same phase, first retained trial of each task | 22.5 s | 2.5 to 479.1 s | Yes: a cold build |
| Later trials (68) | The same phase, from Docker's build cache | 6.2 s | 2.8 to 71.6 s | Yes: the build |
| Agent setup | Coder One upload, digest check, doctor, Codex login | 1.5 s | 1.1 to 2.4 s | Partly |
| Agent execution | The episode | 484.3 s | 80.5 to 3,627.4 s | No |
| Handoff | Artifact collection and the agent environment's stop | 12.6 s | 11.2 to 111.6 s | Yes: the 10-second stop |
| Verifier | The whole phase | 43.8 s | 14.7 to 710.9 s | Partly |
| Verifier tests | The tests' own run (CTRF) | 6.2 s | 0.0 to 447.6 s | No |
| Verifier environment | Build, start, artifact upload, and stop | 17.6 s | 14.5 to 512.9 s | Yes: the build and the 10-second stop |
| Total | Trial start to finish | 651.8 s | 256.9 to 3,671.7 s | |

No Harbor phase counts the handoff: it falls between the agent's end and
the verifier's start. Its floor of 11.2 seconds is the 10-second stop
grace plus artifact collection. The verifier environment's floor of 14.5
seconds is the same grace plus the verifier image's build and start.

Coder One's own install is not worth caching as an image layer. The
environment-only startups timed the binary's upload and in-environment
digest check at 0.2 to 0.4 seconds. An image per task and binary digest
would save that and cost a 32 MB layer per task for every new build, so
the binary keeps its per-trial upload and digest check.

The switch to the agent phase's allowlist takes a steady 2.2 seconds,
inside Harbor's agent execution phase. It reconfigures the egress sidecar,
so this change leaves it alone.

## What changed

- The `tb4` job profile starts every environment through
  `tbench.warm_docker:WarmDockerEnvironment`. A task's images are kept
  after its first trial, tagged `tbench-warm/<task>:<role>-<hash>` by
  Harbor's content hash of the build context, so a changed task revision
  never reuses a stale image. Later trials of the same task, in the same
  suite or a later one, build nothing.
- `WarmDockerEnvironment` stops with a one-second grace (`docker compose
  down --timeout 1`, `stop --timeout 1`). Harbor collects the artifacts
  and the verifier's result before it stops an environment, so nothing
  that the grace would have saved is read afterward.
- Each start and stop is recorded in the trial's `tbench-environment.jsonl`
  with its phases: `sidecar_image`, `build`, `down`, `up`, `keep`, and
  `other`. `tbench.warm_docker.TimedDockerEnvironment` records Harbor's own
  start and stop the same way, unchanged, for comparisons.
- The attempt record's `timing` block adds `setup_ms` (environment and
  agent setup, everything before the agent's first command), `handoff_ms`
  (the agent's end to the verifier's start), and
  `verifier_environment_ms` (the verifier environment's start and stop,
  from the environment records). Its `environment` block reads the image
  state from the same records, so a Dockerfile task is `warm` or `cold`
  rather than `unknown`, and lists each start and stop.
- The Gym shows setup beside agent time: `gym terminal-bench compare`
  prints each attempt's setup with its environment and install parts and
  a group's setup range, `gym terminal-bench attempt` prints setup first,
  the TUI's comparison shows the setup spread, and the experiment pulse
  adds a `mean setup` column.
- The suite scheduler counts a task's kept images among its images: its
  later attempts start once one exists, and when free disk comes within
  the prune margin of its floor, a finished task's kept images go.
- `tbench envstart` runs the environment-only startups below.

The network allowlist is unchanged. The sidecar, its internal networks,
and the overlay from `tbench.egress_compose` apply to kept images the same
way, and every probed startup reached the three allowed hosts and none of
the four blocked ones.

## Environment-only startups

`tbench envstart --probe --artifact <coder-one>` with the
`coder-one-microluna-v15` arm's allowed hosts, one startup at a time.
Each figure is the agent environment's start and stop plus the separate
verifier environment's start and stop. `harbor` is Harbor's own start and
stop. `warm` is a startup where every image was already kept; `first` is
the startup that built and kept them.

| Task | Harbor, median (range) | First warm startup | Warm, median (range) |
| --- | --- | --- | --- |
| `embedding-drift-monitor` | 40.2 s (32.2 to 45.8 s, 3 runs) | 11.4 s | 9.5 s (9.1 to 13.6 s, 3 runs) |
| `sound-change-cascade` | 32.6 s (31.7 to 33.5 s, 2 runs) | 13.9 s | 9.8 s (1 run) |
| `payments-pipeline-fix`, 4 services under the allowlist | 70.6 s (53.0 to 88.2 s, 2 runs) | 45.9 s | 35.7 s (1 run) |

By phase, for `embedding-drift-monitor`: the agent environment's start
went from 5.8 to 3.2 seconds, its stop from 12.7 to 2.6 seconds, the
verifier environment's start from 6.3 to 1.4 seconds, and its stop from
12.2 to 2.1 seconds.

`payments-pipeline-fix` keeps 21 to 28 seconds of `docker compose up`
with warm images. That time is the task's own startup: its seeder loads
1.2 million transactions into Kafka before `main` is healthy. A cache of
images can't remove it.

The Harbor startups rebuilt from a warm build cache. After the suite
prunes that cache, a Harbor startup rebuilds from nothing, as the first
retained trials of `vpp-loss-divergence` (479 s), `freecad-impeller`
(388 s), and `formal-crypto` (345 s) did; a kept image avoids it.

Reports: `~/.openagents/terminal-bench/envstart/20260925T060450Z`,
`20260925T060537Z`, `20260925T060612Z`, and `20260925T061029Z`.

## The cache

The kept images live in Docker's image store under `tbench-warm/*`. On
2026-09-25 this host held 48 of them, 40.9 GB before shared layers, most
from earlier `tbench try` and replay runs; these measurements added seven
(`payments-pipeline-fix`'s four, `sound-change-cascade`'s two, and
`embedding-drift-monitor`'s environment image), 2.2 GB before shared
layers.

```sh
uv run tbench images list
uv run tbench images prune --match payments-pipeline-fix
uv run tbench images prune
```

A changed task revision gets a new tag. The old image stays until you
prune it or the suite removes it near its disk floor.
