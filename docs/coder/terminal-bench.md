# Terminal-Bench comparisons

The `bench/terminal-bench` package runs local [Harbor](https://github.com/harbor-framework/harbor)
trials that compare Claude Code, Codex, and Coder Terminal v0.5 on
identical task pins. It is benchmark infrastructure: Harbor owns the task
environments, and the package owns the pins, the attempt records, and the
reports.

This page is the runbook. The v0.5 side of the integration is specified in
the [episode contract](terminal-bench-contract.md).

## What you need

- macOS or Linux with Docker running. The Docker VM must fit the panel's
  largest declared task (2 CPUs and 4 GiB today); `doctor` says so when it
  does not.
- `uv` and `git`.
- Credentials for the arm you plan to run, in your shell environment.
- For v0.5, a pinned Linux artifact and its sha256.

Nothing here calls a provider or pulls an image until you say `run`.

## Install

```sh
cd bench/terminal-bench
uv sync
```

`uv.lock` pins Harbor `0.22.0` and the resolved dependency tree. A
different Harbor version is a different benchmark; `doctor` fails on the
mismatch rather than running anyway.

## Check the machine

```sh
uv run tbench doctor
```

`doctor` is read-only. It reports, line by line:

- the pinned Harbor, `uv`, `git`, and `docker` tools;
- the Docker daemon's architecture, CPU, and memory against the panel's
  largest declared task;
- emulation conditions — on Apple Silicon, `fix-git` runs its
  `linux/amd64` image under emulation and every run records that;
- the upstream task checkout and its pinned commit;
- which authentication mode each arm could use, by variable name only;
- host agent installs, marked informational. A host login does not
  authenticate or version the container Harbor builds.

`uv run tbench doctor --smoke` adds the checks that cost something:
registry reachability and image-manifest inspection. Manifest availability
is not a runtime test.

## Get the pinned tasks

```sh
uv run tbench tasks checkout
```

This clones the upstream `harbor-framework/terminal-bench` repository into
the persistent cache at `~/.openagents/terminal-bench/upstream/` and
checks out commit `3b5caaa4…`, the same commit `profiles/tasks.json` pins.
A moved or dirty checkout fails `doctor`; do not run against one.

## The panel

`profiles/tasks.json` is the checked record. It carries the eight design
tasks, each pinned to one upstream revision and one architecture record:

| Task | Path | Notes |
| --- | --- | --- |
| `fix-git` | `archive/fix-git` | Smoke; `linux/amd64` image, emulated on arm64. |
| `build-cython-ext` | `archive/build-cython-ext` | Smoke; image resolved at build time. |
| `fix-code-vulnerability` | `archive/fix-code-vulnerability` | Panel. |
| `cancel-async-tasks` | `archive/cancel-async-tasks` | Panel. |
| `headless-terminal` | `archive/headless-terminal` | Panel. |
| `vllm-deepseek-streaming` | `tasks/vllm-deepseek-streaming` | Panel; dual-arch CPU image. |
| `batched-eval-parity` | `tasks/batched-eval-parity` | Panel; verifier includes a timing check. |
| `math-eval-grader` | `tasks/math-eval-grader` | **Excluded**: requires an H100. Never substitute a CPU or MPS runtime. |

This panel is development material, not a held-out confirmation set. Do
not report its pass rates as Terminal-Bench scores.

## Authenticate an arm

Credentials travel by name. The materialized job config holds `${VAR}`
templates that Harbor resolves inside its own process and redacts in
everything it persists. A run never writes a credential value to the
repository, a log, a fixture, or an artifact.

### Claude Code

Two modes:

- **API key** — set `ANTHROPIC_API_KEY` (or `ANTHROPIC_AUTH_TOKEN`).
  Direct API billing; the attempt record marks cost `provider_reported`.
- **Subscription** — run `claude setup-token`, set `CLAUDE_CODE_OAUTH_TOKEN`,
  and let the profile force `CLAUDE_FORCE_OAUTH=1`. The attempt record
  labels this a reference price, not an observed incremental bill.

### Codex

Two modes:

- **API key** — set `OPENAI_API_KEY`. Codex reports token usage; the cost
  field is a Harbor price estimate and the record says so.
- **Auth file** — set `CODEX_AUTH_JSON_PATH` to a readable `auth.json`, or
  set `CODEX_FORCE_AUTH_JSON=1` to use `~/.codex/auth.json`. The auth file
  itself is never copied into retained public artifacts.

A run with no configured mode stops before any environment spend with a
credential error, which is a setup failure, not a task failure.

### Coder v0.5

The door credential pair: `OPENAGENTS_API_KEY` and `OPENAGENTS_DOOR_URL`.
The episode contract defines what the artifact does with them; the
benchmark forwards them by name only.

## Run

Run the controls first. An oracle failure is an environment or task
problem; a `nop` pass means the verifier is not discriminating. Neither
says anything about an agent.

```sh
uv run tbench run --profile smoke --agent oracle
uv run tbench run --profile smoke --agent nop
uv run tbench run --profile smoke --agent claude-code
uv run tbench run --profile smoke --agent codex
```

`smoke` is the two-task profile (`fix-git`, `build-cython-ext`) at one
attempt each. `smoke-comparison` runs the same two tasks three times per
arm — the minimum for a labeled development comparison. `panel` is the
seven CPU-capable tasks. `install-check` builds the environment and
installs the agent without running it — the cheapest proof that an arm's
in-container installation works.

For v0.5, pass the pin explicitly:

```sh
uv run tbench run --profile smoke --agent coder-v05 \
  --agent-kwarg artifact_path=/path/to/coder-v05 \
  --agent-kwarg artifact_sha256=<hex>
```

The adapter refuses a missing binary, a wrong digest, an unknown contract
version, and missing assets before inference can be spent. There is no
fallback to the `coder` on `PATH` — that binary is v0.4 and is never
benchmarked accidentally.

`tbench materialize` writes the resolved job config without starting
Harbor, so you can review the exact pin set a run will use.

## Resume

Harbor keeps per-trial results under the job dir. A rerun of the same job
name resumes it:

```sh
uv run tbench resume --profile smoke --agent claude-code
```

Collection is idempotent: attempt records and episode manifests are
rewritten from whatever `result.json` files exist, so a resume never loses
a failed attempt.

## Inspect

```sh
uv run tbench inspect smoke--claude-code
```

shows each trial's reward and terminal status from its `result.json`.

## Compare

```sh
uv run tbench compare
```

scans the jobs dir and writes `tbench-report.json`, an
`openagents.tbench.report.v1` document grouping attempts by task and arm
on identical pins, plus a printed table. Mixed commits inside one task
name are flagged as warnings, never pooled. The default label is *small
development sample* — keep it until a genuinely unexposed confirmation set
exists.

## What a run retains

Every attempt writes, under `<jobs-dir>/<job>/tbench/`:

- `attempts/<trial>.json` — the `openagents.tbench.attempt.v1` record:
  trial identity joined to task pin and checksum; arm, auth mode, and
  observed agent/model identity; reward separate from terminal status;
  per-phase timing (environment setup, agent setup, agent execution,
  verification, total wall); usage with `full`/`partial`/`unknown`
  coverage; cost with its provenance (`provider_reported`,
  `price_estimate`, `billing_verified`, `none`, `unknown`); ATIF step and
  call counts; and completeness flags.
- `manifests/<trial>.json` — the episode manifest: every evidence file
  with a sha256, marked resolved or unresolved.

Unknown stays unknown. Missing usage is never zero-filled, a subscription
reference price is never reported as an observed bill, and a failed or
unverifiable attempt is retained with its reason.

## Failure behavior

The harness distinguishes, and records, these outcomes: task failure
(reward 0), setup failure, install failure, timeout, cancellation,
provider refusal, verifier failure, missing credentials, and unverifiable
trials. An adapter timeout surfaces as `timeout`, not a fabricated error;
a stopped job leaves whatever evidence its trials already wrote, and
`resume` continues from it.

Bulk logs, provider payloads, and task workspaces stay under
`~/.openagents/terminal-bench/` — never in the repository.

## Test

```sh
cd bench/terminal-bench && uv run pytest
```

The suite covers the profile loaders, credential templating, the
no-fallback artifact contract, attempt-record accounting, the comparison
report, and the ATIF-to-Harbor schema contract.
