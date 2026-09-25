# Data profile and wide entry points offline: protocol

Issue [#9654](https://github.com/OpenAgentsInc/openagents/issues/9654).
Written on 2026-09-25 after the code was built and its unit tests passed,
and before any task was replayed or any label was read. Nothing below
changes after the first replay. A defect found after labels are read is
reported, and any rerun with a fix is reported apart, as in-sample.

## What is measured

Two components, both code, neither asking a model anything:

- `evidence.data_profile` with its default parameters.
- `evidence.baseline` entry-point discovery, before (`Discovery::Named`,
  issue #9633) and after (`Discovery::Wide`, issue #9654), on the same
  workspace and instruction.

## Sets

The split is fixed by task name, before any replay:

- **Source**: `embedding-drift-monitor`, the task the pattern was learned
  from (`docs/coder/design/pattern-components.md`). Reported apart and
  never counted as evidence for either component.
- **Anatomy**: the other 17 tasks of
  `docs/terminal-bench/2026-09-24-task-anatomy.md`. Their labels are the
  anatomy's "What the verifier tests" and "Decisive facts" sections.
- **Family**: the six v18 family tasks (`cumulative-layout-shift`,
  `live-database-cutover`, `mp-checkpoint-consolidation`,
  `payments-pipeline-fix`, `photonic-waveguide-routing`,
  `telecom-entity-resolution`). No defect is recorded for them, so they
  count for coverage, time, and cost only.
- **Wider**: every other task in the local Terminal-Bench task cache
  (`~/.openagents/terminal-bench/upstream/terminal-bench/tasks`). Coverage,
  time, and cost only.

## Workspaces

A task's workspace is its environment image's working directory, copied
with `docker cp` from a created container that never starts. The image is
the first local one of `accept-env/<task>:latest`,
`tbench-warm/<task>:environment-*`,
`truth9584-review/<task>:public-environment`, and
`<task>__*__env-main:latest`. A task with no local image, or whose working
directory holds more than 2 GiB, isn't replayed and is listed as such. No
image is pulled or built.

## Runs

Every entry point that discovery finds and doesn't refuse, before and
after, runs once in a fresh container of the task's image: `--network
none`, the image's working directory, `timeout 60`, stdout and stderr each
kept to 16 KiB. That is the baseline's bound and its task-container
confinement. A stated `python` that exits 127 reruns as `python3`, as the
baseline does. Containers are named `dp9654-*` and removed after each run.

## Coverage

A task has an entry point when discovery finds at least one it doesn't
refuse. Reported per set, before and after, with the kinds found and 95%
Wilson intervals. A run is observable when it reached an exit its program
chose that isn't 126 or 127 and didn't time out.

## Labels and exposure

Read only after every replay has finished, for the source and anatomy
sets, from the anatomy's two sections. Each decisive fact and each
verifier-checked condition there is one label.

- **The profile exposes a label** when a profile finding or line states a
  property of a shipped input that is the input condition the label says
  the solution must handle or the verifier checks, such as all-zero rows
  where the label is about zero-norm rows, or empty fields where it is
  about missing values. Naming the file isn't enough.
- **A baseline run exposes a label** when its output shows the wrong value
  or behavior the label describes, or names the code the label says must
  change (the #9633 rule).

Each exposure is recorded in `records/labels.json` with the line of output
that shows it. A task counts once when any of its labels is exposed.

## Time and cost

Discovery, profile, and run times come from the component records and the
replay script. Cost is zero: no Jev or Luna call is made. If a narrow Jev
question turns out to be needed, it is recorded with its cost, under
$0.05 in all.

## Amendments, made before any label was read

1. The first replay failed on four tasks because of the copy method:
   `docker cp` to a directory couldn't read files owned by root, and one
   image has no command to create a container with. The script now
   streams a tar archive, extracts it with readable modes, and creates the
   container with `/bin/sh` as its entry point, which never starts. The
   whole replay ran again.
2. A supplementary pass (`replay.py --reconstructed`) adds the four
   anatomy tasks with no local image, from the departures experiment's
   reconstructed workspaces, for profile and discovery only, with no runs.
