# Kev

**Status:** ported, all four checkpoints. `kev` is Jared Palmer's
open-source reconstruction of a Jev-style decision model, tracked in this
workspace as `projects/repos/kev/` (manifest entry `jaredpalmer/kev` in
`~/work/projects/manifest.txt`). The Rust port lives in `crates/kev` and
serves every published variant — `kev-0.5b`, `kev-0.6b`, `kev-4b`,
`kev-8b` — through `kev-serve` on CPU and Metal, with per-variant
conformance fixtures pinning each checkpoint to the Python reference.
[`port-roadmap.md`](port-roadmap.md) holds the issue sequence and the
conformance measurements, [`jev-comparison.md`](jev-comparison.md) holds
the per-variant side-by-side against hosted Jev, and
[`mesh-plan.md`](mesh-plan.md) holds the fleet path that follows.

## What it is

A decision model: one document (the *state*) plus a map of typed questions
goes in, one probability distribution per question comes out. It generates
no text. The implementation is a LoRA adapter and a small pointer head on a
frozen Qwen base model, run prefill-only under a block-causal mask that
gives each question access to the state and to nothing else.

Two properties make the shape worth attention:

- **Shared state, isolated questions.** Every question branch reads the
  same encoded state but cannot see a sibling question. One packed prefill
  answers every question; packed and separate requests agree to `4e-6`.
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

Kev is not Jev. It is a laptop-scale research prototype that shows the
mechanism works; on out-of-domain suites it trails the hosted Jev by
8–26 points depending on checkpoint size. [`model-cards.md`](model-cards.md)
holds the numbers.

## Checkpoints

All checkpoints are on the Hugging Face Hub in the
[kev collection](https://huggingface.co/collections/jaredpalmer/kev-6aad9d0ea49f2589665e07cd).
Weights ship as a LoRA adapter (`adapter_model.safetensors`), a pointer head
(`head.pt`), tokenizer files, and evaluation/provenance records — the base
model downloads separately under its own license.

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

The three previews fail kev's own predeclared release screen (both siblings
of a held-out policy pair correct ≥ 70%; best is 0.67). `kev-0.5b`'s full
card is in the upstream `MODEL_CARD.md` and digested in
[`model-cards.md`](model-cards.md).

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
| [`architecture.md`](architecture.md) | The mechanism kev implements: packing, block-causal mask, branch positions, pointer readout, delimiter hardening, and the wire contract. |
| [`jev-unmasked.md`](jev-unmasked.md) | What Archer Hume's probes established about the real Jev, and what stays inferred. |
| [`model-cards.md`](model-cards.md) | `kev-0.5b` in detail — data, recipe, metrics, mechanism tests, limitations — plus the preview family and the research findings behind it. |
| [`port-roadmap.md`](port-roadmap.md) | The in-progress port of the mechanism into `crates/kev`: what gets pulled over in what order, and the issue that tracks each step. |
| [`mesh-plan.md`](mesh-plan.md) | The proposed path to serving and training decision models on the earn mesh: Pylon manifests, a psionic decision-model lane, the fleet `systemone` work shape, and the TypeSafe-compatible fan-out API. |
| [`jev-comparison.md`](jev-comparison.md) | Side-by-side answers from the local port and hosted Jev on identical requests, with the divergence analysis: where the port tracks Jev and where the weights' limits show. |

## Licensing

The adapter and head are Apache-2.0. The base models carry the Qwen license
(Apache-2.0 for the Qwen2.5/Qwen3 bases used). The training datasets carry
their own licenses; `MODEL_CARD.md` lists them. Jev itself is TypeSafe AI's
closed product; nothing from it ships here — only the published API
contract and Hume's published reconstruction inform the design.
