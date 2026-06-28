# GLM own-capacity continual-learning burn

A sustained, **$0 marginal-cost** loop that keeps our self-hosted
GLM-5.2-REAP-504B fleet at full tilt with **genuinely valuable** continual-
learning work, producing a real distillation dataset + improvement candidates.

Driver: `scripts/khala-glm-continual-learning-burn.mjs`.

## The $0 own-capacity guarantee (verified live 2026-06-28)

Every request the driver sends is `model: openagents/khala` with:

```
x-openagents-demand-kind:   internal_stress
x-openagents-demand-source: glm-saturation
```

The gateway (`apps/openagents.com/workers/api/src/inference/chat-completions-routes.ts`,
the `glmSaturationStressKhalaRequest` branch) routes that exact combination to
the GLM lane **only** — `[HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID]`, with **no**
Vertex Gemini / Fireworks / OpenRouter paid fallback. Consequences:

- A `200` is **always** served by our own GLM boxes (GCP G4 spot, billed hourly
  ⇒ **$0 marginal per token**).
- When GLM is unavailable the request **fails closed** (HTTP `000`/`5xx`); it
  **never spills to a paid provider**. The driver retries/skips; it never incurs
  external spend. (Confirmed live: a GLM-down tick returns `000`, not a paid
  serve.)
- `internal_stress` is **preemptible** — real external Khala users always win;
  this burn yields under load (reserved-headroom + preemption policy).

This is the same demand tag the GLM stress harness uses
(`inference/benchmark/stress-saturation-plan.ts`,
`GLM_STRESS_DEMAND_SOURCE = 'glm-saturation'`).

> The raw model id `openagents/glm-5.2-reap-504b` is **not** publicly servable
> (returns `model_unavailable`); the public Khala alias + the glm-saturation
> headers is the supported own-capacity entry point. Plain `openagents/khala`
> (no headers) is **latency-first** and prefers Vertex Gemini / Fireworks
> (external paid) — do **not** use it for $0 burn.

## The valuable workload (not toy work)

Two real continual-learning lanes, both distilled by GLM itself:

1. **Corpus lane** — distill a staged technical corpus (default the Inference
   Engineering book at `~/work/inference-engineering-fulltext.txt`) into
   structured SFT/eval records under rotating *lenses*: exam Q&A with gold
   step-by-step reasoning, worked quantitative derivations, common-misconception
   critiques, and first-principles concept explainers. Diverse, high-token,
   genuinely useful synthetic data for our own models' inference-engineering
   knowledge.
2. **Trace lane** (admin token) — pull the live operator trace-review report
   (`GET /api/operator/khala/trace-review`, real fleet failure modes) and have
   GLM produce a root-cause analysis + concrete remediation + a **mutalisk-
   shaped** optimization candidate `{signature, base_module, optimized_module,
   metric, trace_provenance}`. Candidate evidence only — the Effect product
   surface remains the acceptance/promotion authority (mutalisk contract).

## Output (owner-private)

Under `$CL_OUT_DIR` (default `~/work/.khala-continual-learning`, gitignored):

- `corpus-dataset-<date>.jsonl` — one record per chunk × lens, with provenance
  (corpus/chunk digest prefix, lens, observed usage).
- `remediation-candidates-<date>.jsonl` — mutalisk-shaped candidates from real
  failure modes.
- `receipt-<date>.json` — public-safe batch receipt (counts, tokens burned,
  route, digests; **no** raw corpus text, prompts, keys, or PII).
- `burn-<date>.jsonl` — per-cycle log.

The derived dataset is **never committed** (it is derived training data, and the
source corpus is third-party). Only the driver + this runbook are tracked.

## Run it

```sh
cd ~/work/openagents
# keys come from ~/work/.secrets/khala-heartbeat.env (KHALA_HEARTBEAT_KEYS);
# admin token (optional, enables the trace lane) from vortex-admin.env
. ~/work/.secrets/vortex-admin.env
export OPENAGENTS_ADMIN_API_TOKEN

# one cycle (smoke):
node scripts/khala-glm-continual-learning-burn.mjs --once

# sustained full-tilt loop (keeps GLM saturated until stopped):
nohup node scripts/khala-glm-continual-learning-burn.mjs \
  > ~/work/.khala-continual-learning/loop.out 2>&1 &
```

Tunables (env): `CL_CONCURRENCY` (default 4), `CL_MAX_TOKENS` (1024),
`CL_CHUNK_CHARS` (3200), `CL_TRACE_EVERY` (5 cycles), `CL_CORPUS`, `CL_OUT_DIR`.

## Keeping it alive

The loop runs until killed. To keep GLM at full tilt 24/7:

- Run under `nohup`/a process supervisor; on exit, relaunch.
- It self-heals around GLM flapping (spot preemption): GLM-down ticks back off
  and retry; they never fall back to paid.
- Raise `CL_CONCURRENCY` to push more GLM utilization (watch the canary —
  external users preempt this load by design, so it is safe to be aggressive).
- The corpus lane never "runs out": lenses rotate and the loop re-passes the
  corpus indefinitely, producing fresh diverse records each pass.

## Verification (2026-06-28 smoke)

9 corpus records + 2 real trace candidates in one cycle, **20,650 tokens**
burned (15,473 prompt + 5,177 completion), 0 empty, 0 GLM-down, served entirely
by own GLM. Sample corpus record: a medium-difficulty exam question with
multi-step gold reasoning. Sample trace candidate: a correct root-cause of the
real 14k empty-completion failure mode (probe traffic mis-flagged) with a
remediation + measurable metric.
