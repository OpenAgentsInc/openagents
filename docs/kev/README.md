# Kev

**Status:** all four model sizes are ported, with conformance measured for
the checkpoint contents pinned in this repository. `kev` is Jared Palmer's
open-source reconstruction of a Jev-style decision model, tracked in this
workspace as `projects/repos/kev/` (manifest entry `jaredpalmer/kev` in
`~/work/projects/manifest.txt`). The Rust port lives in `crates/kev` and
serves all four variants — `kev-0.5b`, `kev-0.6b`, `kev-4b`,
`kev-8b` — through `kev-serve` on CPU and Metal, with per-variant
conformance fixtures pinning each checkpoint to the Python reference.
Upstream has since replaced all three Qwen3 adapters under the same Hub
names. Those new contents are not covered by the old measurements. Start
with the [2026-09-20 release review](2026-09-20-upstream-review.md) for the
artifact changes and serving gaps. The
[replacement 4B evaluation](measurements/2026-09-20-candidate-4b.md) now
records real-weight conformance and 461 open workload items per door. Shell
outcomes improve; action and program selection do not support replacement.
[`port-roadmap.md`](port-roadmap.md) holds the issue sequence and the
conformance measurements, [`jev-comparison.md`](jev-comparison.md) holds
the per-variant side-by-side against hosted Jev,
[`measurements/`](measurements/) holds what each checkpoint scores on our own
suite, and [`mesh-plan.md`](mesh-plan.md) holds the fleet path that follows.

## What it is

A decision model: one document (the *state*) plus a map of typed questions
goes in, one probability distribution per question comes out. It generates
no text. The implementation is a LoRA adapter and a small pointer head on a
frozen Qwen base model, run prefill-only under a block-causal mask that
gives each question access to the state and to nothing else.

Two properties make the shape worth attention:

- **Shared state, isolated questions.** Every question branch reads the
  same encoded state but cannot see a sibling question. One packed prefill
  answers every question. Packed and separate requests agree within the
  recorded fp32 fixture bounds; bf16 shape changes have separately measured
  rounding differences.
- **Probabilities, not prose.** A pointer head scores each option's
  closing-delimiter hidden state against the question's decision token and
  applies softmax. The head trains with cross-entropy against labelled
  outcomes, so the numbers are a learned predictive distribution rather
  than tokens that claim a confidence.

The architecture follows Archer Hume's black-box reconstruction of
TypeSafe's Jev in
[Jev's Architecture Unmasked](https://archerhume.com/posts/jevs-architecture-unmasked)
(digested in [`jev-unmasked.md`](jev-unmasked.md)). The API follows
TypeSafe's System One contract: `POST /v1/systemone` with `noul`, `choice`,
and `score` questions, so the official `typesafe-sdk` — and this
repository's `crates/jev` — work against a local kev server with a
`base_url` change.

`GET /v1/models` also publishes a digest of the loaded base, adapter, head,
tokenizer, and configuration bytes, plus separate numerical execution
settings. Gym retains both when scoring the door and rejects stale
calibration matches after a checkpoint or execution change. See
[model identity](../gym/model-identity.md) for the record versions and
verification limits.

Eager attention and exact sequence lengths remain the default. Metal serving
also accepts the independent experimental controls `--attention sdpa` and
`--bucket-size 64`. The [attention measurements](measurements/2026-09-20-metal-attention.md)
record their performance, memory, and numerical differences. In bf16, SDPA
changes one fixture's winning answer, so it requires its own workload
validation and calibration identity before adoption.

When the server declines a request it answers with the refusal envelope the
System One doors share: `{"detail": …, "error": {"code", "message",
"question"}}`, at 422 for contract violations (`invalid_request`,
`too_many_options`), 413 for a branch over the token budget
(`branch_too_long`) or an HTTP body over the byte limit
(`payload_too_large`), 503 for a `model` the server does not hold
(`model_unavailable`), and 500 when its own runtime fails
(`inference_failure`). `detail` keeps the FastAPI reference's shape;
`error.code` is the stable label `gym::eval::classify` reads, so a declined
item stays in the denominator as a refusal rather than reading as a harness
failure.

The server admits a request in three stages before it spends a forward
pass on it, and each stage answers with a typed refusal rather than a
runtime failure or an exhausted process. Before anything is encoded, the
request's shape is bounded: at most 64 questions, 1,024 options summed
over them, and the delimiter floor
`1 + 2·questions + 2·options` against the 8,192-token budget. A floor
over the budget answers `branch_too_long` at 413. After encoding and
before the block-causal mask is built, the forward length (including any
configured padding) is bounded to 8,192 tokens, and its mask bytes `4 × tokens²` are bounded to 256 MiB;
either refusal answers `branch_too_long` at 413 and names the bytes.
The forward permit is the final stage, and it is three bounds taken at
once: a host slot, a slot of the variant the request resolved to, and the
variant's share of a working-memory budget. When any of the three is at
its limit, the server answers `busy` at 503 naming which — `the inference
slots on this host`, ``the inference slots for `kev-0.5b` ``, or `the
working-memory budget (counted in MiB)` — rather than queueing, so a client's cancellation leaves
nothing waiting behind it. The permit rides with the blocking forward and
is returned when the forward ends, not when the caller stops waiting.

The per-variant bound is not a number an operator picks. At startup
`kev-serve` measures the memory the host can lend after the weights are
resident (`MemAvailable` on Linux, free plus inactive pages from `vm_stat`
on macOS; `--memory-budget-mib` states it instead) and asks each variant
what one forward at the token bound costs: `Variant::forward_bytes` reads
the loaded backbone's head count, head width, hidden and intermediate
sizes, and compute dtype — the shape the model card states — and sums the
`tokens²` attention scores per head across the three copies the forward
holds, the `tokens²` mask and its `f32` build buffers, and the per-token
activations. A variant's slots are the budget divided by that cost,
capped at the host's slots, and a variant whose single forward would not
fit is refused at startup with the two numbers, so it never learns that on
a request. `/api/info` reports the budget, what is in use, and each
variant's `forward_mib`, `concurrency`, and `in_flight`.
[`measurements/2026-09-20-per-variant-admission.md`](measurements/2026-09-20-per-variant-admission.md)
holds the estimate beside the resident-set growth a real `kev-0.5b`
forward showed on a Linux CPU box.

`kev::serve::Admission` holds the six bounds, and `ServeState::new` refuses
a state that could not honor them — no variants, a default index that
names none, an alias that is also a variant id, a zero bound, or a variant
that does not fit the budget — at construction rather than on the first
request.

`ServeState::select` resolves an exact variant id, `kev-latest`, an absent
model, or any alias the listing advertises — `jev-latest` included — so an
unmodified Jev client reaches the default variant. A name outside that set
answers `model_unavailable` at 503.

Kev serves on Linux and macOS, on CPU by default. Metal (`--device metal`)
requires macOS and the `metal` feature; CUDA is not built. Tests that need
the real `kev-0.5b` artifacts skip when `KEV_ARTIFACT_DIR` and
`KEV_BASE_DIR` are absent; the refusal and admission tests run everywhere
against a synthetic variant.

One caveat travels with `score`. On `kev-0.5b` the weighted mean it returns
is not a usable position on the rubric, and a caller should read the level
with the greatest probability instead: [Do not read this checkpoint's `score`
as a position](model-cards.md#do-not-read-this-checkpoints-score-as-a-position).
The convention that level is read under — argmax over `probabilities`, ties
to the last level listed — is stated for every door in
[`../decision-models/2026-09-20-score-contract.md`](../decision-models/measurements/2026-09-20-score-contract.md).
Two wire details follow from that. The numbers kev reports — `noul`,
`score`, `confidence`, and every `probabilities` entry — are rounded to two
decimals before they leave the door, so a reader should not compare them
past that precision. And kev serves no calibration map, so its answers carry
no `selected`: on this door the pick is always the argmax of what it
reported. An absent `selected` elsewhere is not proof a map never ran — only
that none was reported.

Kev is an independently trained family of open decision models. Upstream's
current 4B/8B transfer development results are about 6–7 percentage points
behind its hosted Jev reference; the gap varies by task and checkpoint.
[`model-cards.md`](model-cards.md) distinguishes those published results
from the weights measured here.

Which checkpoint you mean decides most of that sentence. On our own
`support-v2-three-way` suite the four checkpoints run from 0.675 to 0.879
accuracy, a spread three times the difference between most doors this
repository argues about, and the best of them wins the whole panel. Name the
checkpoint in any comparison;
[`measurements/2026-09-19-variant-scores.md`](measurements/2026-09-19-variant-scores.md)
has the scores.

## Checkpoints

All checkpoints are on the Hugging Face Hub in the
[Kev collection](https://huggingface.co/collections/jaredpalmer/kev).
Weights ship as a LoRA adapter (`adapter_model.safetensors`), a pointer head
(`head.pt`), tokenizer files, and evaluation/provenance records — the base
model downloads separately under its own license.

`./scripts/fetch-kev-artifacts.sh [<variant>...]` downloads a pinned adapter and
its base, converts verified `head.pt` to the `head.safetensors` and `head_meta.json`
the port reads, and checks source, converted-head, and base files against
the fixture set's `artifact-lock.json`. It writes to `KEV_ARTIFACTS`, by
default `../kev-artifacts` beside the checkout, and needs no token for
these public checkpoints. Run
`KEV_VARIANT=<variant> cargo test -p kev --features serve --release` for
the default artifact layout; the [fixture instructions](../../crates/kev/fixtures/README.md)
describe overrides for other roots.

The downloader now uses immutable revisions, including the historical
Qwen3 adapters that Hub `main` replaced. Fresh and populated directories
must pass the same checks. See [artifact acquisition](artifacts.md) for
recovery instructions and separate candidate locks.

The following table describes the **historical fixture checkpoints**.
For current Hub scores, use the
[current upstream table](model-cards.md#current-upstream-checkpoints-reviewed-2026-09-20).

| Checkpoint | Base | In-domain dev / locked test | Out-of-domain dev / locked test | Port status |
| --- | --- | --- | --- | --- |
| `jaredpalmer/kev-0.5b` | Qwen2.5-0.5B | 0.712 / – | 0.575 / – | served, conformant (4.1e-6) |
| `jaredpalmer/kev-0.6b` | Qwen3-0.6B-Base | 0.805 / 0.819 | 0.598 / 0.631 | served, conformant (3.7e-6) |
| `jaredpalmer/kev-4b` | Qwen3-4B-Base | 0.843 / 0.852 | 0.759 / 0.794 | served, conformant (2.6e-6) |
| `jaredpalmer/kev-8b` | Qwen3-8B-Base | 0.869 / 0.869 | 0.774 / 0.799 | served, conformant (1.1e-6) |
| Jev (hosted reference) | – | 0.845 / – | 0.857 / – | closed weight |

Conformance numbers are the max absolute probability delta between this
Rust port and the Python reference on the committed golden fixtures; the
dev/test columns are upstream's suite scores and describe the weights.

These earlier Qwen3 previews did not clear upstream's held-out policy-pair
screen. The current 4B candidate passes its individual research checks,
but the recipe does not clear the pair-correctness threshold on every
seed. [`model-cards.md`](model-cards.md) records the distinction and the
0.5B reference card.

## Reference checkout

`projects/repos/kev/` — synced by `~/work/projects/sync.sh
jaredpalmer/kev`. The map below is what the checkout holds at the e07ef43
revision.

| Path | Holds |
| --- | --- |
| `kev/model.py` | `encode()` packing, `branch_mask`, `PointerHead`, `DecisionModel`; the whole mechanism in ~175 lines |
| `kev/api.py` | TypeSafe request/response shapes; `Noul`/`Choice`/`Score` → pointer options; confidence formulas |
| `kev/serve.py` | FastAPI server: `POST /v1/systemone`, `GET /v1/models`, plus `/v1/systemone/permute`, `/v1/systemone/separate`, and `/api/*` playground routes |
| `kev/train.py` | LoRA fine-tune, one record per step with gradient accumulation, optional `--perm_kl` and `--ord_w` loss terms |
| `kev/data.py` | Public datasets → typed records; permutation, none-of-the-above, and distractor augmentation |
| `kev/evaluate.py` | Accuracy/ECE/NLL, temperature scaling, permutation stability, IIA, isolation probe, packed-vs-separate |
| `kev/suite.py`, `evals/` | Frozen checksummed research suites: train/calibration/development/locked-test partitions, pinned dataset and base revisions |
| `kev/benchmark.py`, `kev/experiment.py`, `kev/autoresearch.py` | Suite scoring, bounded config-only trials with provenance and coverage/isolation gates, hill-climb rounds |
| `kev/jev.py`, `kev/compare.py` | Scoring the real Jev through Vercel AI Gateway on the same items; paired bootstrap comparison |
| `modal_app.py` | Modal harness: one H100 container per trial, fp32-exact eval, results pulled into `runs/` |
| `playground/` | Next.js demo: request editor, packed-vs-separate, permute, isolation and forgery probes, and a chess game where every legal move is a `choice` option |
| `runs/` | Trial outputs, `leaderboard.md` (~90 autoresearch trials), Jev comparisons |
| `docs/model-cards/` | Cards for `kev-0.6b`, `kev-4b`, `kev-8b` |
| `MODEL_CARD.md`, `PLAN.md`, `AGENTS.md` | The released checkpoint's card, the running research log, the repo's own agent contract |
| `tests/` | `test_unit.py` and `test_research.py` run without weights in CI; `test_api.py` runs the TypeSafe docs' example requests plus the official SDK against a live server |

## Documents here

| Document | Holds |
| --- | --- |
| [`2026-09-20-upstream-review.md`](2026-09-20-upstream-review.md) | Current upstream revisions, artifact drift, bf16 and caching gaps, and the next integration milestones. |
| [`architecture.md`](architecture.md) | The mechanism kev implements: packing, block-causal mask, branch positions, pointer readout, delimiter hardening, and the wire contract. |
| [`jev-unmasked.md`](jev-unmasked.md) | What Archer Hume's probes established about the real Jev, and what stays inferred. |
| [`model-cards.md`](model-cards.md) | `kev-0.5b` in detail — data, recipe, metrics, mechanism tests, limitations — plus the preview family and the research findings behind it. |
| [`port-roadmap.md`](port-roadmap.md) | The in-progress port of the mechanism into `crates/kev`: what gets pulled over in what order, and the issue that tracks each step. |
| [`mesh-plan.md`](mesh-plan.md) | The proposed path to serving and training decision models on the earn mesh: Pylon manifests, a psionic decision-model lane, the fleet `systemone` work shape, and the TypeSafe-compatible fan-out API. |
| [`jev-comparison.md`](jev-comparison.md) | Side-by-side answers from the local port and hosted Jev on identical requests, with the divergence analysis: where the port tracks Jev and where the weights' limits show. |
| [`measurements/2026-09-19-variant-scores.md`](measurements/2026-09-19-variant-scores.md) | All four checkpoints scored on `support-v2-three-way` through the Gym: the panel, what clears the suite's noise floor, why no calibration map is admitted, and why the latency column settles nothing yet. |
| [`measurements/2026-09-20-program-selection-latency.md`](measurements/2026-09-20-program-selection-latency.md) | `kev-0.5b`, `kev-0.6b`, and `kev-4b` answering the program-selection question on a quiet CPU, eight blocks each: per-block p50 and p95 and the conditions behind the latency column in the cross-door record. |
| [`measurements/2026-09-20-real-weights-and-domain-gap.md`](measurements/2026-09-20-real-weights-and-domain-gap.md) | `kev-0.5b` and `kev-0.6b` with weights present, CPU only: the gated suite at 33 passes and no skips each, and both doors on `external-v1`'s public BoolQ and MultiNLI labels beside their `support-v2` scores, where one holds within the floor and the other rises two floors. |

## Licensing

The adapter and head are Apache-2.0. The base models carry the Qwen license
(Apache-2.0 for the Qwen2.5/Qwen3 bases used). The training datasets carry
their own licenses; `MODEL_CARD.md` lists them. Jev itself is TypeSafe AI's
closed product; nothing from it ships here — only the published API
contract and Hume's published reconstruction inform the design.

The [merge precision and loading-memory record](measurements/2026-09-20-merge-precision.md)
compares fp32 and bf16 on pinned historical weights and states the measured
Metal memory limits.
