# Bench: the Gym's harness lane

`bench/adapters/openagents_coder.py` is the Harbor installed-agent adapter
for `openagents coder` (OpenAgentsInc/openagents#35). It runs the working
tree's CLI, not the published npm version, so pack a tarball first:

```sh
cd packages/openagents-cli
pnpm build
pnpm pack --pack-destination ../../bench
```

Use `pnpm pack`, not `npm pack`: the manifest carries pnpm `catalog:`
versions that only `pnpm pack` rewrites into versions npm can install.

Run against the dev forge on the host (`PHX_LISTEN_ALL=true mix
phx.server` in openagents.com, plus a `chat:account` token):

```sh
PYTHONPATH=bench OPENAGENTS_TOKEN=... harbor run \
  --dataset terminal-bench@2.0 \
  --agent-import-path adapters.openagents_coder:OpenAgentsCoder \
  -m openai/gpt-5.6-luna \
  -i fix-git --n-concurrent 1 --jobs-dir /tmp/gym-jobs
```

`bench/post_gym_run.py` posts a completed job's graded result to the Gym.
Without a run id it posts one summary row to `POST /api/v3/gym/runs`. With
`--run-id` (or `OPENAGENTS_GYM_RUN_ID` in the environment) it finalizes a
run that `run-suite.sh` registered against the lifecycle API: it upserts
each trial's final state and patches the run to `graded`, or to `abandoned`
when no verifier ran. Post only runs whose verifier actually ran: a score
is a claim, and a crashed grader is not a grade.

If the dev server binds loopback only (another session started it
without `PHX_LISTEN_ALL`), bridge instead of fighting over the port: run
a forwarder from `0.0.0.0:4001` to `127.0.0.1:4000` and point the
adapter at it with `OPENAGENTS_CODER_API_URL=http://host.docker.internal:4001`.

Known constraint on Apple Silicon: Terminal-Bench task images are amd64,
and under qemu emulation the verifier's `uv`/`pytest` segfaults after the
agent phase completes. Enable Docker Desktop's Rosetta emulation
(Settings → General → "Use Rosetta for x86_64/amd64 emulation"), use a
cloud environment (`--env daytona` and peers), or run on amd64 hardware
for scored runs. The agent phase itself runs fine under qemu.

## Suite runner

`bench/run-suite.sh` runs a suite file through Harbor and reports the run
to the Gym.

```sh
bench/run-suite.sh <suite-file> --model <harbor-model> [options]
```

Examples:

```sh
bench/run-suite.sh bench/suites/tb2-cross-section.suite.json \
  --model openai/gpt-5.6-luna --lane proxy --n-concurrent 2

bench/run-suite.sh bench/suites/tb2-cross-section.suite.json \
  --model ollama/qwen3.8:27b-mtp-q8_0 --lane local --dry-run
```

## Suites

`bench/suites/*.suite.json` are the pinned suites, regenerated from Harbor's
registry and this tracker's closed issues by:

```sh
pnpm run effectiveness:suites -- \
  --registry ../projects/repos/harbor/registry.json \
  --issues <(openagents issue list -R OpenAgentsInc/openagents --state closed --limit 200 --json)
```

Add `--check` to rebuild and diff without writing, which is how a manifest is
kept from drifting away from the registry it claims to pin.

A manifest pins each task by content — dataset, git url, commit, path — rather
than by name, and `coder-effectiveness report --suite-manifest` scores a run
against it. A run that did not cover every pinned task is a smoke run and
cannot be recorded, whatever it was invoked as; see
`packages/coder-effectiveness/README.md`.

The plain `.txt` lists still run. They carry no pin, so a run of one cannot be
recorded as a score.

Options include `--lane`, `--api-url`, `--jobs-dir`, `--n-concurrent`,
`--timeout-multiplier`, and `--dry-run`. Set `OPENAGENTS_TOKEN` unless the
model starts with `ollama/`.

### Run lifecycle

When you set `OPENAGENTS_TOKEN`, the runner registers the run with
`POST /api/v1/gym/runs/start` before Harbor starts, so `/gym` shows the run
while its trials are still executing. The runner exports
`OPENAGENTS_GYM_RUN_ID` and `OPENAGENTS_GYM_API_URL` — the host-side API
URL, not the container-rewritten one — into the Harbor run, and the adapter
reports each trial to `POST /api/v1/gym/runs/{id}/trials` from the host:
state `running` when the agent phase starts, then again with the thread id
it parses from the coder's `[oa:thread <uuid>]` line in the trial's
`coder.txt`. The coder prints that line in `--plain` mode on both lanes
when it has a server thread; a trial whose coder ran without one is
registered and graded without linkage. A failed trial report never fails
the trial.

After grading, the runner finalizes through `post_gym_run.py --run-id`,
which upserts each trial's final state — `passed`, `failed`, or `ungraded`
when the verifier never ran — and patches the run to `graded`. When no
verifier ran at all, it patches the run to `abandoned` instead: a crashed
grader is not a grade, but it must not stay a forever-running row. When the
suite fails before grading, `run-suite.sh` patches the run to `abandoned`.

Without a token, for example an `ollama/...` dry run, the runner skips
registration and the trials run without live reporting. If registration
fails, the suite still runs and the post-hoc one-shot path posts the graded
result as before.
