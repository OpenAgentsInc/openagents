# MirrorCode × Khala runner (Phase 0)

Repeatable runner that points the **Epoch Research MirrorCode** benchmark at
`openagents/khala` and runs ONE public task end-to-end, writing a public-safe
result JSON in the shared gym contract.

- Epic: **#6376** (MirrorCode-powered Khala Gym + MirrorCode-as-a-Service)
- Phase 0 issue: **#6377**
- Design: [`docs/benchmarks/2026-06-27-mirrorcode-khala-gym-integration-analysis.md`](../../../../docs/benchmarks/2026-06-27-mirrorcode-khala-gym-integration-analysis.md)

## Gym backstop runner (`backstop-run.sh` / `backstop_eval.py`, issue #6710)

The fleet-saturation **prio:4 backstop** (`prio:4-backstop-burn`) standing task
needs a runner that does **genuine own-capacity ($0) high-density work** whenever
the higher priority tiers are clear, so no fleet slot ever idles. That runner is
`backstop-run.sh` (live) / `backstop_eval.py` (core).

What it does, per problem:

1. Asks a model (live: **Khala**, OpenAI-compatible, own capacity, $0) for a
   solution to a small coding problem.
2. **Executes** the generated code against hidden test cases in a bounded
   isolated subprocess (`python -I`, wall-clock timeout) — real pass/fail.
3. Records per-problem and aggregate **pass rates** and writes **execution
   traces** (`results/backstop/traces/<id>.json` + a run summary).

```sh
./backstop-run.sh                 # mint a free Khala key, run the bounded batch
OPENAI_API_KEY=oa_agent_xxx ./backstop-run.sh --limit 8
MC_BACKSTOP_DRY_RUN=1 ./backstop-run.sh   # diagnostic: no model call, no spend
```

Problem source: the built-in **public-domain fixture set** (classic toy
functions — sum, reverse, palindrome, factorial, gcd, FizzBuzz, …). These are
**NOT MirrorCode tasks**, so there is no benchmark contamination, and the load is
tagged `demand_kind=internal` / `demand_source=gym_backstop`. The read-only
MirrorCode clone is detected and surfaced (`mirrorcodeClonePresent`,
`mirrorcodeSTargets`) in the result when present, but its tasks are **not** run
here.

Honesty: this is a real measurement of our own model against public-domain toy
problems; it is the smallest **real slice** that does genuine model work + records
traces. It is **not** the MirrorCode paper benchmark and must never be published
as a MirrorCode score (`grade: "backstop"`, `decisionGrade: false`).

**Follow-up (#6710):** wire the full Docker MirrorCode harness (`run.sh`) into the
backstop as an opt-in escalation path that pulls a bounded batch of real S-bucket
tasks when Docker + the clone are present and token/wall-clock budget allows. The
full harness burns ≥1B tokens per sample, so it is intentionally out of the
default backstop loop; the fixture slice is the always-available density burner.
Solution execution is currently subprocess-isolated with a timeout but not
container-sandboxed — harden to Docker/firecracker before running untrusted
(non-fixture, non-own-model) candidates.

Tests: `python3 backstop_eval_test.py` (extraction, real execution of correct /
wrong / empty / missing-entrypoint / exception candidates, aggregate pass-rate
math, trace writing, fail-soft model errors).

## Approach (Option A — zero provider code)

MirrorCode is built on [Inspect](https://inspect.aisi.org.uk/) and Khala is
**OpenAI-compatible**, so there is **no custom adapter**. We point Inspect's
`openai` provider at Khala and run a public task:

- `OPENAI_BASE_URL=https://openagents.com/api/v1`
- `OPENAI_API_KEY=<a Khala key>` (mint a free one:
  `curl -s -X POST https://openagents.com/api/keys/free | jq -r .credential.token`)
- model id `openai/openagents/khala` (Inspect `openai` provider + our model name)

Khala's OpenAI-compatible chat-completions endpoint was verified to support the
full agent loop: multi-turn **tool calling** (`bash`, `text_editor`,
`evaluate_testcases`, `submit`), `finish_reason=tool_calls`, and `role:tool`
result round-trips.

Requests are tagged `internal` / `gym_mirrorcode` via Khala's demand-attribution
headers (`x-openagents-demand-kind`, `x-openagents-demand-source`,
`x-openagents-client`) so the load is auditably an eval (#6298) and stays
preemptible behind real external demand (#6318).

## Hard constraints (read before running)

- The MirrorCode clone (default `~/work/projects/repos/MirrorCode`) is
  **READ-ONLY**. Do not modify it. **Never train or RAG on the tasks.** Respect
  both canary strings (MirrorCode + BIG-Bench).
- We run **PUBLIC tasks only** (the paper's private set is not shipped). Always
  label results "public tasks only; private set excluded."
- Treat the public-task lane as Khala's AgentCL Generalization Gain (GG) set:
  `generalizationSet=mirrorcode_public_tasks_no_rag` and
  `memoryPolicy=no_rag_public_tasks_only`. Khala memory, training, homework,
  retrieval, prompt-optimization, and RAG loops must not ingest MirrorCode task
  text, docs, tests, logs, or traces. The runner and ingest route may store only
  the public-safe score envelope.
- **Cost/wall-clock is the dominant risk.** A real sample burns ≥1B tokens
  (S/M) or 10B (L); the longest paper sample ran **19 days**. This runner sets
  hard token + wall-clock caps FAR below those limits — it is a **smoke**, not a
  paper run. **Never** run the L bucket (`ruff`/`pkl`/`cprepro`) or `--language
  all6` here.

## Requirements

- **Docker** running (MirrorCode builds images and runs **four containers per
  sample**: `workspace`, `reference-scoring`, `agent-scoring-visible`,
  `agent-scoring-hidden`). Images are built locally from the clone by default
  (no GHCR pull/auth needed).
- `uv`, `python3.13`, `jq`, `curl`.

## Usage

```sh
# Phase-0 S-target smoke (cal_python): mints a free Khala key, builds images, runs end-to-end.
./run.sh

# Pick a different public task (sample id = "<target>_<language>"):
MC_TASK_ID=uuidparse_python ./run.sh
OPENAI_API_KEY=oa_agent_xxx ./run.sh --task numfmt_python
```

### Choosing a task

`<target>_<language>` where language ∈ `python,c,go,rust,ocaml,ada`.

- **Phase-0 issue proof:** `cal_python` — a small public S-bucket target, used
  as the default because #6377 requires a real S-target smoke.
- **Endpoint-only validation:** `false_c` — the lightest target (reimplement
  `/usr/bin/false`). It is a *trivial, benchmark-excluded* target and requires
  `--allow-trivial-smoke`; do not use it as the #6377 proof.
- **Other small real S-bucket targets:** `uuidparse_python`, `numfmt_python`,
  `choose_python`.
- S bucket: `qsv_select, jq_simple, gron, bitwise, hexyl, uuidparse, numfmt,
  cal, choose`. M bucket: `giac, tex, gotree, mailauth, brotli, wren_cli,
  nonogrid, sed, tssql, bib2json`. **L bucket is off-limits in this runner.**

### Env knobs

| Var | Default | Meaning |
|---|---|---|
| `MC_CLONE` | `~/work/projects/repos/MirrorCode` | Read-only MirrorCode clone |
| `OPENAI_API_KEY` | minted free key | Khala key (bearer) |
| `MC_TASK_ID` | `cal_python` | Sample id `<target>_<language>` |
| `MC_TOKEN_LIMIT` | `20000000` | Hard per-sample token cap |
| `MC_TIME_LIMIT` | `7200` | Hard per-sample wall-clock cap (s) |
| `MC_MESSAGE_LIMIT` | `250` | Hard per-sample message cap |
| `MC_OUT` | `./mirrorcode-phase0-result.json` | Result JSON path |
| `MC_LOG_DIR` | Inspect default (`./logs`) | Inspect eval log dir |
| `MC_VENV` / `MC_KEEP_VENV` | fresh mktemp / removed | Reuse/keep the throwaway venv |
| `MC_ALLOW_TRIVIAL_SMOKE` | `0` | Set to `1` only for benchmark-excluded endpoint checks such as `false_c` |

The venv is **throwaway** (a `mktemp` dir, auto-removed) and is never committed.

## Result JSON contract (consumed by the service / gym-ingest lane)

```json
{
  "runId": "mc-phase0-<utc>-<rand>",
  "model": "openagents/khala",
  "taskId": "cal_python",
  "bucket": "S | M",
  "language": "python",
  "status": "passed | failed | error",
  "passRate": 0.0,
  "tokens": { "total": 0, "input": 0, "output": 0, "reasoning": 0,
              "cacheRead": 0, "cacheWrite": 0 },
  "startedAt": "<iso8601>",
  "finishedAt": "<iso8601>",
  "summary": "...",
  "grade": "smoke",
  "decisionGrade": false
}
```

Plus public-safe extras: `benchmark`, `scoreGroups` (MirrorCode per-group pass
rates: `all/visible/ablated/hidden/withheld`), `caps`, `demand`, and the derived
GG fields `generalizationSet=mirrorcode_public_tasks_no_rag` and
`memoryPolicy=no_rag_public_tasks_only`.

`passRate` is the **held-out** (`withheld` = hidden + ablated) reproduction rate
when present (the anti-contamination number), else `all`, else `visible`.
`status` is `passed` when that rate is ≥ 0.999 (target fully solved), `failed`
when the loop completed but did not fully reproduce the target, `error`
otherwise.

## Token accounting / honesty

- `tokens` is read from Inspect's per-sample `model_usage`. Reconcile against the
  exact `token_usage_events` rows tagged `demand_kind=internal`,
  `demand_source=gym_mirrorcode` for the real source of truth — counter movement
  alone is never proof.
- A fixture/illustrative run must never be published as a measurement; only a
  real, decision-grade run feeds the honesty-gated gym ladder (#6309).
