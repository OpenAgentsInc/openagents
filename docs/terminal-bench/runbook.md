# Terminal-Bench runbook: operating notes

This page collects what the other Terminal-Bench documents don't say: the
host, the credentials and their failure modes, the rate limits, the version
pins that matter, and how to turn a finished trial into a results row. Read
it before you run anything, then follow the procedure in the document for
the arms you run.

- Harness mechanics, profiles, and resume: [the harness runbook](../coder/terminal-bench.md).
- The artifact contract Coder One implements: [`openagents.coder.episode.v1`](../coder/terminal-bench-contract.md).
- Coder One's delegate arms: [the delegate runbook](coder-one-delegate-runbook.md).
- Every result: [the results page](README.md).
- Cancellation, timeouts, refusals, resume, image state, and repeated-run statistics: [the resilience page](resilience.md).
- Inspect attempts and evidence: [the Gym TUI](../gym/terminal-bench-tui.md).
- Inspect or run from the command line: [the Gym CLI](../gym/terminal-bench-cli.md).

## The host

Current trials run on an x86_64 NixOS host with 28 CPUs and native Docker.
Earlier baselines (Fable 5.1, Sonnet 4.5, Codex 0.153.3, Devin) ran on an
arm64 Mac, so agent times from the two machines aren't strictly
comparable. The results page marks times that come from a trajectory
rather than Harbor's phase timings with †.

- `uv` isn't installed globally. Run the harness through nix:

  ```sh
  cd bench/terminal-bench
  nix shell nixpkgs#uv -c uv sync --frozen
  nix shell nixpkgs#uv -c uv run tbench doctor --agent coder-one
  ```

  `nix-ld` is enabled, so uv-managed Python and manylinux wheels work.
- The pinned task checkouts live under
  `~/.openagents/terminal-bench/upstream/`: `terminal-bench` at commit
  `3b5caaa4863d` for the `smoke`, `panel`, and `extended` profiles, and
  `terminal-bench-v4.0.0` at tag `v4.0.0` (commit `452bf305c6da`) for the
  `tb4` profile. `uv run tbench tasks checkout` creates or pins both;
  `--catalog tb4` or `--catalog panel` pins one.
- Jobs land in `~/.openagents/terminal-bench/jobs/<job>/`. Failed setup
  attempts that aren't results are moved to
  `~/.openagents/terminal-bench/failed/` so they don't pool with real
  trials.

## Credentials

Never print a credential, paste it into a command, or copy it into a trace.
Export each one straight from its file, and scan retained evidence for it
before a commit (see [Retain the evidence](#retain-the-evidence)).

| Arm family | Variable | Where it comes from | Notes |
| --- | --- | --- | --- |
| Coder One generation | `OPENAGENTS_API_KEY` | `~/.openagents/bearer`, an openagents.com session token | Serves `https://openagents.com/v1/responses`. |
| Coder One Jev | `TYPESAFE_API_KEY` | `api_key` in `~/.openagents/jev.json` | Hosted TypeSafe, `jev-1.13.0`. |
| Claude Code, and Coder One's Opus delegate | `CLAUDE_CODE_OAUTH_TOKEN` | `claudeAiOauth.accessToken` in `~/.claude/.credentials.json` | Expires after about eight hours; check `expiresAt` and run `claude -p ok` to refresh it. Unset `ANTHROPIC_API_KEY` and `ANTHROPIC_AUTH_TOKEN` for `--auth-mode subscription-oauth`. |
| Codex | `CODEX_FORCE_AUTH_JSON=1` (host-side selector) | `~/.codex/auth.json` | Sign in once with `codex login --device-auth`; it needs a person at a browser. Use `--auth-mode auth-json`. The selector never reaches the container, because Harbor scrubs the values of credential-named variables from evidence, and a forwarded `1` once redacted every digit in a result file. |

```sh
export OPENAGENTS_API_KEY="$(tr -d '\n' < ~/.openagents/bearer)"
export TYPESAFE_API_KEY="$(jq -r .api_key ~/.openagents/jev.json)"
export CLAUDE_CODE_OAUTH_TOKEN="$(jq -r .claudeAiOauth.accessToken ~/.claude/.credentials.json)"
export CODEX_FORCE_AUTH_JSON=1
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN OPENAI_API_KEY
```

A host login doesn't authenticate the container. Harbor installs each agent
inside the task environment and forwards only what the arm's profile names.

## Version pins that matter

- **Opus 5.5 needs Claude Code 2.1.280 or newer.** On 2.1.278 the API
  answers `400 claude_code_version_too_old` and the trial ends in about a
  second with no inference. `claude-code-opus` pins 2.1.280, and so do the
  Coder One Opus delegate arms.
- **GPT-6 Sol and Luna appear only in Codex's live catalog.** The catalogs
  bundled in Codex 0.155.1 and 0.157.0-alpha.9 list `gpt-6-astra` but not
  `gpt-6-sol` or `gpt-6-luna`. After `codex login`, `codex debug models`
  lists all three for a ChatGPT account. The `codex-gpt-6-*` arms pin CLI
  0.155.1. The old `codex` arm keeps 0.153.3 so earlier results stay
  attributable.
- **ChatGPT-account Codex can't serve `gpt-5.2-codex` or `gpt-5.1-codex`**
  (HTTP 400). Use the GPT-6 or GPT-5.6 names the live catalog shows.

## The free lane's rate limit

Coder One generates on the openagents.com `free` lane, which admits **20
generations a minute for one account**. The limit is in the production
service (`bins/coder-serve/src/responses_endpoint/free_limit.rs` in the
private `OpenAgentsInc/coder` repository). A Coder One trial generates
roughly 10 to 15 times a minute, so:

- **Run Coder One trials one at a time**, and don't run a delegate arm's
  explorer alongside another Coder One trial. On 2026-09-22 eight explorers
  ran at once, every one of them hit the limit, and four trials ended as
  `generation_failed`. Those rows are listed as invalid, not as losses.
- Coder One since commit `9f31630ac6` retries a 429 up to six times,
  honors `Retry-After`, and backs off to 65 seconds, so a single trial
  waits out the window instead of failing. The waiting still counts as
  agent time. Check a trial for `generation failed: the free lane takes 20
  generations a minute` in its trajectory before you analyze its time.
- Commit `7e810c3a5` in `OpenAgentsInc/coder` exempts operator admins
  (`CODER_ADMINS`, `AtlantisPleb` when unset) from the limit. It is live
  since `0773a7fdf` (2026-09-22): 50 free-lane calls in a minute all
  answered. With the operator's session token, Coder One trials can run in
  parallel; any other account still needs them one at a time.

## Pin the Coder One artifact

Build a static Linux binary from a clean tree:

```sh
./scripts/build-coder-one-linux.sh
```

It prints `artifact_path`, `artifact_sha256`, and the version with the
commit it was built from. It needs a musl C compiler; on this host the
script fetches one through `nix`. Set `CODER_ONE_TARGET_DIR` to keep a
build out of another worktree's target directory.

When you rebase a branch after benchmarking it, the commit the artifact
names disappears from history. Tag the benchmarked commit first and push
the tag, as `coder-one-speed-99e647a7a974` was, so the version string in a
result still resolves.

Coder One's own switches reach an episode as environment variables, which
an arm's profile sets:

| Variable | Arm that sets it | Effect |
| --- | --- | --- |
| `CODER_ONE_DEEP=on` | `coder-one-deep` | A parallel Jev survey of up to 100 files before the first step, a "done and checked" question every step, and repeated-command hints. |
| `CODER_ONE_JEV=off` | `coder-one-no-jev` | The ablation: no Jev calls. |
| `CODER_ONE_DELEGATE=always` or `auto` | the delegate arms | Delegate mode; see the delegate runbook. |
| `CODER_ONE_EXPLORE_STEPS=0` with `CODER_ONE_DEEP=on` | `coder-one-jevbrief-opus`, `coder-one-jevbrief-luna` | Jev-brief: no Gemini explorer; the Jev survey's files go straight into the delegate's briefing. |
| `CODER_ONE_PROBES=on` with `CODER_ONE_DEEP=on` and explore 0 | the `coder-one-jevprobe-*` arms | Jev-probe: read-only probes run in parallel, Jev keeps the relevant outputs, and they join the survey in the briefing. |
| `CODER_ONE_PROBE_V2=on` or `v3` | the `jevprobe2` and `jevprobe3` arms | Probe v2: a Jev-gated setup pack, git probes in named repositories, whole edit targets, a 40-file survey, and batch-mode directions. `v3` swaps the single final check for directions to test every changed code path. |
| `CODER_ONE_DELEGATE_TOOLS`, `CODER_ONE_DELEGATE_EFFORT` | the lean and low-effort arms | Claude Code's `--tools` list and `--effort`, or Codex's `model_reasoning_effort`. |
| `CLAUDE_CODE_PROMPT_CACHE_TTL=5m` | `coder-one-jevprobe2-opus-lean-low-5m`, `coder-one-jevprobe3-opus-lean-low` | Read by Claude Code itself: the five-minute prompt cache instead of the one hour a subscription token gets. Writes cost 1.25 times the input rate instead of 2 times. |

### Configure an arm with a policy manifest

A policy manifest names every component's implementation and parameters in
one JSON file with the schema `openagents.coder-one.policy.v1`. The
reference manifests live in `crates/coder-one/policies/`:
`jevprobe3-luna.json` and `jevprobe2-opus-lean-low-5m.json` reproduce the
switches those two arms set in the table above, and their profiles now
point at them with the `policy` kwarg. The adapter reads the file on the
host, installs the executor and version the manifest pins, and passes the
manifest to the episode as `CODER_ONE_POLICY`.

The episode resolves its configuration once, before anything runs: the
manifest, or the built-in default when there is none, with each switch
above applied on top as an override. The bundle's `manifest.json` records
the resolved manifest, its digest, and every override under `policy`, and
the Gym groups comparisons by that digest. The `protected` part (the
isolation boundary, the effect policy, the acceptance rule, and the
resource ceilings) is outside any candidate's search space, and a field
is searchable only when a canary test in `coder_one::policy` shows that
changing it reaches the invoked executor.

```sh
gym coder policy list
gym coder policy diff coder-one-jevprobe3-luna coder-one-jevprobe2-opus-lean-low-5m
```

### Read cost by call and the episode deadline

`evaluation/usage.json` has a `ledger` that lists every generation, Jev
request, and delegate dispatch with its charge: `priced`, with a cost and
its provenance; `zero`, a known zero, such as a request the deadline
skipped or a 4xx refusal; or `unknown`, such as a timed-out Jev request or
a session its deadline cut off. A component's cost is known only when
none of its calls is unknown, and `cost.lower_bound_usd` sums what is
known either way. Each dispatch keeps its own agent, model, credential,
provenance, and units: native turns, model calls, and completed items,
which Codex reports instead of model calls.

While an episode runs, the adapter copies the new lines of its episode log
to `agent/live/` under the trial every `live_interval_sec` seconds (10 by
default; set it to 0 to turn the copy off). `gym coder live --follow` and the
Gym terminal's live view read that copy, so you can follow a trial before
its bundle is collected.

When the adapter has an exec timeout (`episode_timeout_sec`), it sets
`CODER_ONE_EPISODE_DEADLINE` 60 seconds inside it. The episode then gives
each dispatch, Jev request, retry, wait, setup command, probe, and command
at most what is left after the policy's reserve, and the manifest's
`deadline` records every cut. A dollar bound stays soft
(`CODER_ONE_SPEND_SOFT_USD`): no executor adapter reserves a known maximum
charge, so the policy refuses a hard one. `gym terminal-bench attempt`
and the Gym's attempt view show the ledger and the deadline's use.

## Install agents from prebuilt layers

Before 2026-09-23, each trial installed its delegate CLI over the network:
`apt-get install nodejs npm`, nvm, `nvm install 22`, then `npm install -g`
or Claude Code's bootstrap download. Over v3 Luna's 24 trials, Harbor's
`agent_setup` phase averaged 273 seconds, against 52.5 seconds of agent
work. With eight to sixteen trials installing at once, some installs
passed Harbor's 360-second limit and ended in `AgentSetupTimeoutError`
before any inference: 3 of 16 trials at sixteen at a time, and 12 of 24
attempts when eight Claude Code arms started together.

The Coder One delegate arms now install from prebuilt toolchain layers by
default. A layer is one finished install, built once on the host from
pinned public downloads and cached under
`~/.openagents/terminal-bench/toolchains/<key>/`:

| Layer | Source | Check |
| --- | --- | --- |
| `node-22.23.2-<platform>` | The Node build `nvm install 22` resolved on 2026-09-22 | The sha256 pinned in `tbench/toolchain.py` |
| `codex-<version>-<platform>` | `@openai/codex` and its platform package, laid out as `npm install -g` lays them out | The npm registry's sha512 integrity |
| `claude-code-<version>-<platform>` | Claude Code's native binary | The release manifest's sha256 |

The platform is `linux-x64` or `linux-arm64`, with a `-musl` suffix on a
musl image. The layers depend on the task image only through the C
library, so the platform is the whole image identity in the key. The
Coder One artifact was already a host upload pinned by its sha256.

Each trial copies its layers into `/opt/openagents/toolchain/<key>/` and
links `node`, `npm`, `npx`, and `codex` or `claude` into `/usr/local/bin`.
The trial runs no package manager and makes no network request to install
anything. Layers hold public release files only. Each build scans its layer
for your credential values and discards the layer if it finds one.

Build the layers before a run so no trial pays the cold build:

```sh
uv run tbench toolchain build --executor codex --version 0.155.1
uv run tbench toolchain build --executor claude-code --version 2.1.280
uv run tbench toolchain list
```

Each adapter writes `agent/toolchain-setup.json` with the mode
(`prebuilt` or `network`), the cache state (`warm` when every layer was
reused, `cold` when a layer was built during the trial, `none` for a
network install), each layer's key and tree digest, and the install
phases. If no layer fits the image, such as a musl image for Codex, the
adapter falls back to the network install and says why in `note`. To
keep the network install, pass `--agent-kwarg toolchain=network`. It pins
Node 22.23.2 and runs at most `install_concurrency` installs at a time
(two by default) in each Harbor process.

To use layers for Harbor's own Claude Code and Codex arms, set an arm's
`harbor_import_path` to `tbench.prebuilt:PrebuiltClaudeCode` or
`tbench.prebuilt:PrebuiltCodex` in place of `harbor_name`. The existing
`claude-code-*` and `codex-*` arms keep Harbor's stock install, so their
results stay comparable.

A standalone check on 2026-09-23, with no trial and no container network
(`--network none`), measured the toolchain install alone:

| Executor | Cold, one container | Warm, eight containers at once |
| --- | --- | --- |
| Codex 0.155.1 with Node 22.23.2 | 11.8 s (10.6 s of it the host build) | 1.7 to 11.6 s |
| Claude Code 2.1.280 with Node 22.23.2 | 5.9 s (5.0 s of it the host build) | 1.8 to 9.2 s |

These times cover the toolchain only. Harbor's `agent_setup` phase also
covers the Coder One artifact upload and the episode doctor, so compare
trial setup times in the Gym, which labels each boundary. A screen at
eight concurrent trials hasn't run yet.

Harbor's `agent_setup` phase is what the Gym reports as setup:
`gym terminal-bench compare` shows setup time by cache state beside agent
and total time, counts setup failures beside the graded attempts, and
prints which clock each number uses.

With the network install, still run at most eight trials at a time, no
more than about four of them Claude Code arms. Retry a setup timeout once
after moving its job directory to `failed/`. A setup timeout is never a
result.

## Run the Terminal-Bench 4.0 suite

The `tb4` profile is Terminal-Bench 4.0: all 66 tasks under `tasks/` at
tag `v4.0.0`. The panel's pin at `3b5caaa` has the same task names, but
137 files differ, so `tb4` draws from its own catalog
(`catalogs.tb4` in `profiles/tasks.json`) and its own checkout. Each task's
entry records what its `task.toml` declares: CPUs, memory, storage, GPUs,
the 8-hour agent timeout, the build and verifier timeouts, a separate
verifier environment's budget, and the base image of the Dockerfile's final
stage. No task has a prebuilt image; each one builds from its Dockerfile.

| Resources | Tasks |
| --- | ---: |
| 2 CPUs | 44 |
| 4 CPUs | 14 |
| 8 CPUs | 6 |
| 16 CPUs | 2 |
| A GPU (`fp8-rmsnorm-gemm`, `jax-speedrun-gpu`, `math-eval-grader`) | 3 |

List the tasks and their budgets:

```sh
uv run tbench tasks checkout --catalog tb4
uv run tbench tasks list --profile tb4
```

### Schedule a suite

`tbench suite run` runs one arm over a profile's tasks, one Harbor job per
task and attempt, as many at once as the budgets allow:

```sh
uv run tbench suite plan --profile tb4 --agent <arm> --attempts 5
uv run tbench suite run --profile tb4 --agent <arm> --attempts 5 --detach
uv run tbench suite status --profile tb4 --agent <arm>
uv run tbench suite stop --profile tb4 --agent <arm>
```

`plan` starts nothing: it prints what is finished, what is skipped and why,
and the first wave a run would start. `--tasks a,b` narrows a suite, and
`--auth-mode` and `--agent-kwarg` pass through to every job.

- **Budgets.** A trial reserves its task's CPUs and memory, the larger of
  the agent and separate verifier environments. The defaults are
  `--max-cpus 24 --max-mem-gb 100`, which leave four CPUs and 25 GiB for
  Docker builds and the host. The scheduler starts the largest pending
  trials first and backfills smaller ones; a trial that has waited 30
  minutes stops the backfill so the large tasks get their turn. A task
  larger than the whole budget runs alone. `--max-concurrent` caps the
  trial count as well.
- **GPU.** A GPU task needs Docker to pass a GPU through. The `tb4`
  profile's environment, `tbench.gpu_docker:CdiDockerEnvironment`, is
  Harbor's Docker environment with GPU support declared when an NVIDIA
  Container Device Interface (CDI) spec exists in a directory Docker reads,
  such as `/var/run/cdi/nvidia-container-toolkit.json`. For a task with
  `gpus > 0`, it adds `devices: ["nvidia.com/gpu=all"]` to the task's main
  service. Without a spec, `tbench run` refuses a GPU task and records why
  under the job's `tbench/refusals/`, and the suite marks the trial
  `skipped` with the same reason. GPU trials take GPU slots, one by
  default (`--max-gpus`), because this host has one 16 GB RTX 4080. The
  tasks declare an H100; `fp8-rmsnorm-gemm` builds for `sm_90a`, which the
  RTX 4080 (`sm_89`) can't run, so expect that task to fail here whatever
  the agent does.
- **Disk.** No trial starts while the Docker volume has less than
  `--min-free-disk-gb` free (40 by default); the scheduler prunes and waits
  instead. Harbor removes each trial's built image
  (`<trial>__env-main`) when the trial ends, so what grows is the base
  images and Docker's build cache. When a task's trials have all finished
  and free space is within `--prune-margin-gb` (20) of the floor, the
  scheduler removes any image a crashed trial of that task left. Below the
  floor it also drops Docker's unused build cache. Other agents' Cargo
  target directories count against the same disk.
- **Images.** A task's later attempts wait until its first attempt has
  built its image, so they build from Docker's layer cache rather than all
  from nothing at once.
- **Names.** Jobs use the runbook's names: `tb4--<arm>--<task>`, then
  `tb4--<arm>--<task>-2` through `-5`. A name that already holds a finished
  trial is never run again.
- **Retries.** A setup timeout (`AgentSetupTimeoutError` or
  `EnvironmentStartTimeoutError`) is never a result. The scheduler moves
  the job directory to `failed/<job>-setup-timeout-<unix time>` and runs the
  trial once more; a second timeout marks it `failed`. A trial whose process
  exits without a finished result is resumed twice, then marked `failed`.
- **Restarts.** The scheduler keeps no state the job directories don't. On
  every start it reads each job directory, adopts any trial whose `tbench`
  process still runs, and resumes interrupted ones. `suite stop` (or
  Ctrl-C) stops new starts and interrupts the running trials once, so
  Harbor cancels them cleanly; the next start resumes them. A second
  scheduler for the same suite is refused.

Everything a suite writes, apart from the job directories, is under
`~/.openagents/terminal-bench/suites/<profile>--<arm>/`: `status.json`
(schema `openagents.tbench.suite-status.v1`: counts, passes, resources in
use, free disk, every trial's state and reason, prunes, and recent events),
`scheduler.log`, and one process log per job under `logs/`.

### Compare against the leaderboard

`bench/terminal-bench/reference/tb4-leaderboard.json` holds the public
Terminal-Bench 4.0 leaderboard, per task: each row's successes over its
five trials, errors, mean agent time, and cost. `uv run tbench reference`
refreshes it from the Harbor Hub without credentials. A task's cost comes
from the source job's per-task aggregate for the row's agent and model; the
file flags a row whose per-task counts or costs don't add up to its own
metrics (`per_task.consistent`, `per_task.cost_consistent`).

The Gym groups `tb4` attempts under their profile, so they never pool with
the panel's tasks of the same name:

```sh
gym terminal-bench overview --profile tb4
gym terminal-bench compare --profile tb4 [--task terminal-bench/<task>]
gym coder matrix --profile tb4 [--reference-rows 5] [--json]
```

The overview lists each arm's passes over graded trials beside every
leaderboard row. A comparison shows the leaderboard's result on the
group's task. The matrix lists all 66 tasks, with the best-ranked
leaderboard rows under each task's cells, even before any arm has run it.

### Check the prebuilt install against TB4 images

The TB4 tasks start from 26 base images, among them Debian, Ubuntu,
Fedora, Conda, CUDA, and `node:20` images. Check the prebuilt toolchain
layers against each one before a Coder One suite:

```sh
uv run tbench toolchain check --profile tb4 --executor claude-code --version 2.1.280
uv run tbench toolchain check --profile tb4 --executor codex --version 0.155.1
```

Each check probes the image's architecture and C library, mounts the layers
read-only where a trial copies them, links them, and runs each version
command with no container network. The layers link `node`, `npm`, and
`npx` only when the image has none of its own, so a task built on `node:20`
keeps Node 20; Codex runs through a wrapper that starts the pinned Node by
path, and each version check runs the layer's own binary.

## Name jobs so results don't collide

The job name is `<profile>--<arm>--<task>`, and running the same name again
**resumes** that job instead of adding a trial. That includes a run with a
new artifact. Pass `--job-name` for anything that must be a separate trial:

- a new artifact on an existing arm, for example
  `--job-name smoke--coder-one-v2--fix-git`;
- a rerun after a failed or invalid attempt, for example
  `smoke--coder-one-deep2--fix-git`;
- a repetition, for example `smoke--claude-code-opus--fix-git-2`.

Record which artifact each job name ran in the results page.

## Watch a long run

Start trials in the background and wait for them to exit. Don't write a
`while pgrep -f '<pattern>'` loop: `pgrep -f` matches the loop's own
command line, so it never ends. Wait on the job's process instead, or
check for the trial's `result.json`.

Harbor warns that an arm's `extra_allowed_hosts` are ignored because the
effective network policy is public. That is expected for these tasks; the
list only matters under a restricted policy.

## Turn a trial into a results row

```sh
cd bench/terminal-bench
python3 tools/trial_metrics.py smoke--coder-one--fix-git smoke--claude-code-opus--fix-git
```

The tool prints, for every trial in each job: reward, exception, Harbor's
agent and total time, and Harbor's usage and cost. For a Coder One trial it
adds the outcome, calls by kind, the generation, Jev, and delegate
components, the exact Jev cost, and time by kind of step. For a Codex GPT-6
trial it prints a manual cost.

Pricing rules the results page follows:

- **Jev** is exact: reported Jev input tokens × $0.042 per million. Output
  is free.
- **GPT-6** costs are manual, from OpenAI's standard short-context pricing
  that the operator supplied on 2026-09-22, because Harbor records none for
  these models. Uncached input, cached input, and output each at their rate:

  | Model | Input | Cached input | Output |
  | --- | --- | --- | --- |
  | `gpt-6-astra` | $10.00 | $1.00 | $50.00 |
  | `gpt-6-sol` | $2.00 | $0.20 | $10.00 |
  | `gpt-6-luna` | $0.10 | $0.01 | $0.50 |

  Pricing the two Codex 0.153.3 Astra trials this way reproduces Harbor's
  estimates exactly. Mark these costs ‡ and label them manual.
- **Claude Code** reports `total_cost_usd`. On a subscription token that is
  a list-price figure, not a bill.
- **Devin** shows `—`: it doesn't report what a run would cost to buy.
- **Coder One generation** is the door's `cost_microusd` per call. The
  `free` lane is described as metered at zero but still reports a figure;
  the page uses the reported figure and says so.

## Retain the evidence

For each job you add to the results page, run the retention tool from
`bench/terminal-bench`:

```sh
uv run tbench retain <job> [<job>...]
```

It copies each trial's full evidence closure into
`bench/terminal-bench/traces/<job>/`:

| Path | What it holds |
| --- | --- |
| `<trial>.json` | Harbor's normalized ATIF trajectory |
| `<trial>.episode/manifest.json`, `trajectory.atif.json`, `evaluation/`, `artifacts/` | Coder One's episode directory as the agent wrote it: the raw ATIF, usage, state, briefings, and the native delegate streams (`artifacts/delegate-*.stream.jsonl`) |
| `<trial>.episode/native/` | Other executors' native traces, such as `claude-code.txt`, `codex.txt`, and session JSONL |
| `<trial>.episode/verifier/` | The reward, the CTRF report, and the verifier's per-test output (`test-stdout.txt`) |
| `<trial>.episode/produced/` | Files Harbor collected from the task container |
| `<trial>.episode/setup/toolchain-setup.json` | How the agent was installed: prebuilt or network, cold or warm, and the install phases |
| `<trial>.episode/harbor-result.json` | Harbor's result, trimmed to phase timings, the verifier result, the exception, and `agent_result`'s usage and cost |
| `<trial>.episode/tbench-attempt.json`, `tbench-manifest.json` | The harness's attempt record and episode manifest |
| `<trial>.episode/retention.json` | The retention record: each copied file and its digest, each manifest digest checked, each missing reference with its reason, how the raw ATIF relates to the normalized one, and the credential scan |

The tool reports every reference it can't copy instead of dropping it: a
file the episode manifest names but the job doesn't hold, a file over the
size bound (4 MiB a file and 16 MiB a trial by default; change them with
`--max-file-bytes` and `--max-trial-bytes`), and a verifier that left no
per-test output. Harbor collects only `/logs/artifacts` from the
container, so files a task writes elsewhere, such as `/app/summary.csv`,
aren't retained; the record notes this.

Before it writes anything, the tool scans every staged file for the
values of `OPENAGENTS_API_KEY`, `TYPESAFE_API_KEY`,
`CLAUDE_CODE_OAUTH_TOKEN`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`,
`~/.openagents/bearer`, `api_key` in `~/.openagents/jev.json`, the OAuth
tokens in `~/.claude/.credentials.json`, and the tokens in
`~/.codex/auth.json`. A trial with a match isn't written, and the error
names the file and the credential's source, never its value. Run with
`--dry-run` to check a job without writing, and `--trial` to retain one
trial.

Then check the result in the Gym. A newly retained attempt shows
`verified files`; any gap is listed by:

```sh
gym terminal-bench evidence --missing --no-jobs
```

Three Codex `fix-git` trajectories from the Mac
(`smoke--codex--fix-git`, `-2`, and `-3`) aren't valid JSON because
Harbor's scrubber rewrote literal values in them. Keep them, and don't
count them.

## After each run

1. Add the row to the task's table and to its **Cheapest first** table,
   in cost order.
2. For a Coder One arm, add a run analysis to the results page: cost by
   component, calls by kind against the other arms, tokens per call, where
   the time went, and what explains the difference. The delegate runbook
   lists what a delegate analysis covers.
3. Note any invalid attempt, such as a rate-limit or setup failure, under
   **Data problems** rather than as a result.
4. Commit and push to `main`. The repository runs no gate before a push.
