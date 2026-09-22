# Terminal-Bench resilience

This page records how the harness handles a trial that goes wrong. Each
case below was observed on a real Harbor trial, or on a run the harness
refused before Harbor started, on 2026-09-22 on the x86_64 NixOS host
(Harbor 0.22.0, upstream task commit `3b5caaa4863d`). The evidence for
every case is in
[`bench/terminal-bench/samples/resilience-fix-git/`](../../bench/terminal-bench/samples/resilience-fix-git/).

The trials spent no inference. Each ran a contract probe from
`bench/terminal-bench/tests/fixtures/` through the `coder-v05` arm, with
placeholder door values, on the `fix-git` task. The probes implement the
`openagents.coder.episode.v1` command surface and misbehave in one way
each:

| Probe | Behavior |
| --- | --- |
| `coder-v05-probe` | Writes a full bundle and exits 7. |
| `coder-v05-slow-probe` | Writes a partial bundle, then sleeps until killed. |
| `coder-v05-no-trace-probe` | Writes a manifest and usage record, no trajectory, and exits 0. |
| `coder-v05-collect-fail-probe` | Deletes its own output directory and exits 0. |
| `coder-v05-macho-stub` | A 16-byte Mach-O header: pinned correctly, but not a Linux executable. |

Job names start with `resilience--`, so none collides with a benchmark
job. Other benchmark trials ran on the machine throughout; the checks
below look only at containers, networks, and processes whose names come
from a resilience trial.

## Summary

| Case | Job | Terminal status | Reward | Harness change |
| --- | --- | --- | --- | --- |
| Cancellation (Ctrl-C) | `resilience--cancel--fix-git` | `cancelled` | none | Fixed: Harbor was killed and left its container running. |
| Cancellation (SIGTERM) | `resilience--cancel-sigterm--fix-git` | `cancelled` | none | Fixed with the same change. |
| Timeout | `resilience--timeout--fix-git` | `timeout` | none | None needed. |
| Missing trace | `resilience--no-trace--fix-git` | `completed`, trace `absent` | 0.0 | None needed. |
| Wrong digest | `resilience--wrong-digest--fix-git` | `setup_failure` (refused) | none | Fixed: the Harbor job aborted without a record. |
| Missing binary | `resilience--wrong-binary--fix-git` | `setup_failure` (refused) | none | Fixed with the same change. |
| Wrong platform | `resilience--wrong-platform--fix-git` | `install_failure` | none | Fixed: recorded as `agent_error` before. |
| Missing credentials | `resilience--no-credentials--fix-git`, `resilience--no-credentials-claude--fix-git` | `setup_failure` (refused) | none | Added a refusal record. |
| Artifact-collection failure | `resilience--collect-fail--fix-git` | `completed`, bundle `collection_failed` | 0.0 | Added the `bundle` completeness flag. |
| Resume of an interrupted job | `resilience--resume--fix-git` | 3 × `timeout`, 1 × `cancelled` kept as `interrupted` | none | Fixed: resume deleted the attempt records and the cancelled trial. |

After every case ran, no container, network, or process from any
resilience trial remained
(`samples/resilience-fix-git/leftovers-after-all-cases.txt`).

## Cancellation

**What ran.** The slow probe wrote `manifest.json` and
`trajectory.atif.json` inside the container and slept. When the episode
was running, the driver sent SIGINT to the process group of the `tbench
run` session, which is what Ctrl-C in a terminal does. A second run sent
SIGTERM instead, which is what a supervisor sends.

**Before the fix.** `tbench` ran Harbor with `subprocess.run`, in the
same process group. On Ctrl-C, Python's `subprocess` gives the child a
quarter of a second and then kills it, so Harbor died in the middle of
its cleanup. The trial's container, `fix-git__jlraacz__env-main-1`, kept
running, Harbor wrote no `result.json`, and `tbench` collected nothing.
The first attempt at a fix forwarded SIGTERM to Harbor; Harbor's SIGTERM
handler raises `KeyboardInterrupt` immediately, asyncio reported `Task was
destroyed but it is pending!`, and the container was left running again.
Both transcripts are in `samples/resilience-fix-git/before-fix/`.

**What the harness does now.** `tbench` starts Harbor in a session of its
own, so a terminal's Ctrl-C reaches `tbench` only. On the first SIGINT,
SIGTERM, or SIGHUP, `tbench` sends Harbor one SIGINT, which asyncio turns
into a cancellation of Harbor's main task, and then waits for Harbor to
exit. Harbor records `CancelledError` for the trial, the adapter's
`finally` block downloads the bundle, and Harbor stops and deletes the
environment. `tbench` then collects the job and exits with an error that
names the signal. Later signals aren't forwarded, because a second
SIGINT makes asyncio skip the cleanup.

**Observed.**

- SIGINT at 19:51:44Z; every process that named the job exited by
  19:51:56Z. SIGTERM at 19:52:59Z; every process exited by 19:53:11Z.
- Terminal status `cancelled`, reward `null`, exception
  `CancelledError`. The verifier didn't run.
- The partial bundle was collected: `agent/episode/manifest.json` and
  `agent/episode/trajectory.atif.json`, with the trajectory converted to
  `agent/trajectory.json`. The attempt record says trace `present` and
  bundle `present`.
- No container, compose project, network, or process for the trial
  remained.

**Evidence.** `samples/resilience-fix-git/cancel-sigint/` and
`cancel-sigterm/`, each with the attempt record, the manifest, Harbor's
result and trial log, the collected bundle, and the transcript with the
leftover checks.

## Timeout

**What ran.** The slow probe with `--agent-kwarg episode_timeout_sec=20`.

**What the harness does.** The adapter runs the episode under the exec
deadline. When it expires, the adapter raises `EpisodeTimeoutError`,
downloads the bundle, and Harbor stops the environment.

**Observed.** Terminal status `timeout` after 20.2 seconds of agent
execution, reward `null` (Harbor didn't run the verifier after the
timeout), and the partial bundle collected as in the cancellation case.

**Evidence.** `samples/resilience-fix-git/timeout/`.

## Missing trace

**What ran.** The no-trace probe exits 0 without writing
`trajectory.atif.json`.

**What the harness does.** The adapter finds no trajectory to convert, so
Harbor writes none. The collector looks for a trajectory file and native
session logs; with neither, the attempt record says trace `absent` rather
than inferring one. The manifest now prefers `trajectory*.json` over any
other JSON file in the agent directory, so an unrelated file can't stand
in for a trajectory.

**Observed.** Terminal status `completed` with the verifier's reward of
0.0, `completeness.trace` `absent`, and `completeness.bundle` `present`
(the manifest and usage record arrived). `tbench compare` marks the cell
`1 without trace`.

**Evidence.** `samples/resilience-fix-git/missing-trace/`.

## Wrong binary or digest

Three variants, from cheapest to most expensive to catch:

- **Digest mismatch.** The probe with a pin of other bytes.
- **Missing binary.** An `artifact_path` that doesn't exist.
- **Wrong platform.** `coder-v05-macho-stub`, pinned by its own correct
  digest, which the Linux container can't execute.

**Before the fix.** The adapter checks the pin while Harbor constructs the
agent. A raise there aborted the whole Harbor job with a traceback: the
job left a trial directory without a `result.json` and no attempt record,
so the refusal was visible only on the terminal. A contract error inside
the environment was mapped to `agent_error`.

**What the harness does now.**

- `tbench run` and `tbench resume` check `artifact_path` and
  `artifact_sha256` on the host before Harbor starts. A missing file or a
  digest mismatch is refused, and the refusal is written to
  `<job>/tbench/refusals/<time>.json` as an
  `openagents.tbench.refusal.v1` record: stage `artifact`, terminal status
  `setup_failure`, the reason, and `spend: none`.
- A pinned binary that fails inside the environment still fails the
  install: `--version` exited 126 with `Exec format error`, the adapter
  raised `EpisodeContractError`, and the attempt record now maps
  `EpisodeContractError` and `ArtifactIdentityError` to `install_failure`.

**Observed.** The digest mismatch and the missing binary were refused in
under a second, with no job config, no environment, and no trial. The
wrong-platform trial reached agent setup (0.8 seconds of environment
setup), stopped before any episode ran, recorded `install_failure`, and
left no container.

**Evidence.** `samples/resilience-fix-git/wrong-digest/`,
`wrong-binary/`, and `wrong-platform/`.

## Unavailable credentials

**What ran.** `tbench run` for `coder-v05` without `OPENAGENTS_API_KEY`
and `OPENAGENTS_DOOR_URL`, and for `claude-code-opus` without
`CLAUDE_CODE_OAUTH_TOKEN`, `ANTHROPIC_API_KEY`, or `ANTHROPIC_AUTH_TOKEN`.

**What the harness does.** It checks each auth mode's variables by name,
never by value, and refuses the run before Harbor starts. The refusal now
names what each mode needs and leaves a refusal record with stage
`credentials`.

**Observed.** Both runs exited 1 in under a second. The message for
`claude-code-opus` reads: `known modes: api-key needs one of
ANTHROPIC_API_KEY or ANTHROPIC_AUTH_TOKEN; subscription-oauth needs
CLAUDE_CODE_OAUTH_TOKEN`. No environment started and no inference ran.

**Evidence.** `samples/resilience-fix-git/no-credentials/` and
`no-credentials-claude/`.

## Artifact-collection failure

**What ran.** The collect-fail probe deletes `/opt/openagents/episode`
and exits 0, so the adapter's `docker compose cp` has nothing to copy.

**What the harness does.** The adapter catches the failed download and
writes the error to `agent/episode-collection-failed.txt`. The collector
now reads that marker: the attempt record's `completeness.bundle` is
`collection_failed`, and the manifest lists the marker as
`evidence.collection_failure` with its digest. `bundle` is `present`,
`empty`, `collection_failed`, or `not_applicable` for an arm without an
episode bundle.

**Observed.** Terminal status `completed`, reward 0.0 from the verifier,
trace `absent`, bundle `collection_failed`. The marker holds Docker's
`Could not find the file /opt/openagents/episode/.` error.

**Evidence.** `samples/resilience-fix-git/collection-failure/`.

## Resume of an interrupted job

**What ran.** The `smoke-comparison` profile narrowed to `fix-git`:
three sequential slow-probe trials, each timing out after 15 seconds.
The driver sent Ctrl-C while the second trial's episode ran, then ran
`tbench resume` with the same arguments.

**Before the fix.** `harbor job resume`, and a `harbor run` of a job name
that already exists, deletes every subdirectory of the job without a
`result.json` as an unfinished trial. That includes `tbench/`: the
attempt records, the context, and the materialized config were deleted,
and the collector recreated only the records of trials still on disk.
Resume also deletes every cancelled trial by default before running it
again, so the interrupted attempt and its partial bundle were lost
(`samples/resilience-fix-git/before-fix/resume-wiped-tbench.out`).

**What the harness does now.**

- Before Harbor opens an existing job dir, `tbench` moves `tbench/` to
  `<jobs-dir>/.tbench-held--<job>/held/` and moves it back when Harbor
  exits. A hold left by a crash is restored on the next run.
- Before `harbor job resume`, `tbench` copies each trial Harbor is about
  to delete (cancelled, or without `result.json`) to
  `tbench/interrupted/<trial>/`. Its attempt record keeps `kind:
  interrupted`, and `tbench compare` counts it apart from the trials.
- A trial without `result.json` gets an attempt record with status
  `unknown` and a note, instead of no record.

**Observed.**

| Trial | Before the interrupt | After resume |
| --- | --- | --- |
| `fix-git__swLsRNt` | Finished, `timeout` | Same directory, same trial ID and start time, not rerun |
| `fix-git__WgzE2pn` | Running at the interrupt, `cancelled` | Preserved under `tbench/interrupted/`, record `kind: interrupted` |
| `fix-git__Y8H9YRf` | Didn't exist | New trial, `timeout` |
| `fix-git__VkdjH9h` | Didn't exist | New trial, `timeout` |

Harbor names new trials rather than reusing a cancelled trial's name, so
the two new trials fill the two places the profile still needed. The job
ends with the three trials the profile asks for, one interrupted attempt
kept as evidence, no held directory, and no container.

**Evidence.** `samples/resilience-fix-git/resume/`.

## Cold and warm image state

Every attempt record now has an `environment` block:

| Field | Values |
| --- | --- |
| `image` | The task's declared `docker_image`, or `null`. |
| `image_source` | `prebuilt`, `dockerfile`, or `unknown`. |
| `image_state` | `cold`, `warm`, or `unknown`. |
| `image_action` | `pulled`, `reused`, `built`, or `unknown`. |
| `image_state_method` | How the state was determined, or why it couldn't be. |

The evidence is Harbor's own trial log. Before `docker compose up`,
Harbor's Docker environment runs `docker inspect` on a prebuilt image,
and when the image isn't in the local cache it logs `Skipping image OS
validation for <image>: docker inspect returned 1`; `compose up` then
pulls the image. That line means the image was cold for that trial. No
such line after a finished environment setup means it was warm. A task
built from its Dockerfile is rebuilt in every trial, and the log doesn't
say whether the build cache served its layers, so its state stays
`unknown` with action `built`. A missing log, an environment setup that
didn't finish, or an inspect that couldn't run also stays `unknown`.

Applied read-only to the 45 trials on this host at the time: 29 warm, 5
cold, and 11 Dockerfile builds (`build-cython-ext`). The cold trials are
the first `fix-git` trial on this host, whose environment setup took
10.9 seconds against about 1 second warm, and four `headless-terminal`
trials that started while the image was absent
(`samples/resilience-fix-git/image-state.txt`). Run `tbench collect
<job>` to add the block to a job collected before this change.

## Repeated runs

`tbench compare` now reports each task and arm as one or more cells.
Repetitions pool into one cell across job names, such as a `-2` suffix,
only when their pins match: the task checksum, the observed agent
version, the model, and the artifact digest a contract arm ran. Each cell
reports:

- trials, and interrupted attempts counted apart;
- scored trials, passes, the pass rate, and a 95% Wilson interval from
  two scored trials up; a cell with one scored trial is labeled `single
  trial` and has no interval;
- the mean reward;
- agent time and cost as mean, minimum, and maximum over the trials that
  report them, with the cost provenance;
- trials without a trace and trials whose bundle wasn't collected.

An arm with more than one cell prints its pin under each row. From the
resilience jobs (`samples/resilience-fix-git/compare.txt`):

```text
arm                         n pass [95% Wilson]      reward agent time mean (min-max)  cost mean (min-max)                  status
----------------------------------------------------------------------------------------------------------------------------------
coder-v05                   1 0/1 single trial         0.00 0.2s                       unknown                              completed  [1 without trace]
    pin: version pinned-artifact, model v0.5-composite, artifact sha256 354d63107281, task e89f54f2fc73
coder-v05                   2 0/2 [0.00, 0.66]         0.00 0.1s (0.1s-0.1s)           unknown                              agent_errorx2
    pin: version pinned-artifact, model v0.5-composite, artifact sha256 3f171b2bbca9, task e89f54f2fc73
coder-v05                   6 no scored trial             ? 12.2s (3.9s-20.2s)         unknown                              cancelledx2,timeoutx4  [+1 interrupted]
    pin: version pinned-artifact, model v0.5-composite, artifact sha256 54b412b10ee3, task e89f54f2fc73
coder-v05                   1 no scored trial             ? unknown                    unknown                              install_failure  [1 without trace]
    pin: version pinned-artifact, model v0.5-composite, artifact sha256 ec4e6562d7cd, task e89f54f2fc73
coder-v05                   1 0/1 single trial         0.00 0.2s                       unknown                              completed  [1 without trace; 1 bundle not collected]
    pin: version pinned-artifact, model v0.5-composite, artifact sha256 f56345e097a4, task e89f54f2fc73
oracle                      2 2/2 [0.34, 1.00]         1.00 0.3s (0.2s-0.3s)           unknown                              completedx2  [2 without trace]
```

The `oracle` cell pools `resilience--repeat-oracle--fix-git` and
`resilience--repeat-oracle--fix-git-2`, and the `3f171b2bbca9` cell pools
the two `resilience--repeat-probe--fix-git` jobs the same way. The five
`coder-v05` probes never pool with each other, because each is a
different artifact. The pass rate counts scored trials only: a timeout or
a cancellation has no verifier reward, and the report shows it as a
status rather than as a zero.

## Reproduce

The drivers are shell scripts outside the repository; each case is one
`tbench` command. For example, the timeout case:

```sh
cd bench/terminal-bench
P=$PWD/tests/fixtures/coder-v05-slow-probe
OPENAGENTS_API_KEY=placeholder OPENAGENTS_DOOR_URL=http://127.0.0.1:9 \
  nix shell nixpkgs#uv -c uv run tbench run --profile smoke \
    --agent coder-v05 --task fix-git --job-name resilience--timeout--fix-git \
    --agent-kwarg artifact_path=$P \
    --agent-kwarg artifact_sha256=$(sha256sum $P | cut -d' ' -f1) \
    --agent-kwarg episode_timeout_sec=20
```

`tests/test_runner.py`, `tests/test_results.py`, and
`tests/test_compare.py` cover each behavior without Docker.
