# Kev model cards and measurements

**Status:** records external measurements from `projects/repos/kev/`
(`MODEL_CARD.md`, `docs/model-cards/`, `runs/leaderboard.md`, `PLAN.md`)
and the Hugging Face model pages. None of these numbers come from
OpenAgents infrastructure; they are quoted here so the integration plan in
[`mesh-plan.md`](mesh-plan.md) can reason about them without re-deriving
them.

## kev-0.5b (released v0.1.0)

The reference checkpoint. `jaredpalmer/kev-0.5b` on the Hub, also attached
to GitHub release `v0.1.0` as `kev-0.5b.tar.gz` (38 MB; SHA-256
`15639f79…6e12f8`, full digest in the sidecar `.sha256`). The archive and
the Hub repo hold `adapter_model.safetensors`, `head.pt`, tokenizer files,
`eval.json`, and the training log. Trained 2026-09-17; Apache-2.0 for the
adapter and head, Qwen license on the base, dataset licenses unchanged.

| | |
| --- | --- |
| Base | `Qwen/Qwen2.5-0.5B`, 494M parameters, frozen |
| Adapter | LoRA r=16, alpha 32, dropout 0.05, all seven projection targets on all 24 layers |
| Head | two linear maps `896 -> 256`, scaled dot product, softmax |
| Trainable | 9.3M parameters (8.8M LoRA + 0.46M head), 1.9% of the backbone |
| Precision | fp32, training and serving on Apple MPS |
| Context | trained at <= 384 state / <= 1,024 branch tokens; serves to 8,192 per branch (backbone supports 32k) |
| Question types | `noul`, `choice` (2–255 options), `score` (2–255 ordered levels) |
| Language | English |

### Training data

Six public datasets converted to TypeSafe-shaped requests through the same
renderer serving uses. 1,500 records per source from the standard train
splits: 9,000 records, 13,500 questions (4,500 Choice, 6,000 Noul, 3,000
Score).

| Source | Becomes | Notes |
| --- | --- | --- |
| Banking77 | Choice, K=77 | intent names as option keys; templated descriptions, 50% `null` |
| BoolQ | Noul | passage as state; 40% carry `true`/`false` criteria |
| AG News | Choice K=4 + 2 Noul | derived yes/no questions packed beside the topic question |
| MNLI | Choice K=3 | premise as state, hypothesis in instructions |
| SST-5 | Score, 5 levels | |
| Yelp Review Full | Score 5 levels + Noul | text truncated to 220 words; `recommend` = stars >= 4 |

Rendering variation at conversion: ~30% null option descriptions, ~10%
structured `{"what": …}` descriptions, ~15% structured
`{"question", "focus"}` instructions, ~32% states wrapped as objects or
arrays. Augmentation once per record: option-order shuffle, `none of the
above` replacement at p=0.10, distractor option at p=0.15. No LLM-generated
data, no annotation beyond the original datasets.

### Recipe

AdamW lr 2e-4, weight decay 0.01, OneCycle (10% warm-up), batch 1 with
gradient accumulation 8, clip 1.0, 2 epochs (2,250 steps), seed 0, ~1h45m
on an Apple M5 at fp32 on MPS (~0.06 kWh). Final training loss 0.27. This
checkpoint predates the `--perm_kl` and `--ord_w` loss terms; re-running
today's conversion does not reproduce it bit-identically.

### Held-out results

1,350 held-out questions (150 records per source, test/validation splits,
seed 1). Baselines are the raw and Instruct Qwen2.5-0.5B answering the same
rendered text via next-token logits over option letters.

| Source | K | Zero-shot base acc/ECE | Zero-shot Instruct acc/ECE | kev-0.5b acc / ECE / NLL |
| --- | --- | --- | --- | --- |
| banking77 | 77 | – | – | 0.860 / 0.057 / 0.56 |
| agnews | 4 | 0.813 / 0.069 | 0.787 / 0.160 | 0.940 / 0.028 / 0.22 |
| agnews yes/no | 2 | 0.780 / 0.103 | 0.853 / 0.062 | 0.960 / 0.017 / 0.10 |
| boolq | 2 | 0.427 / 0.274 | 0.607 / 0.084 | 0.753 / 0.136 / 0.63 |
| mnli | 3 | 0.460 / 0.225 | 0.433 / 0.390 | 0.747 / 0.100 / 0.63 |
| sst5 | 5 | 0.373 / 0.083 | 0.447 / 0.344 | 0.533 / 0.121 / 1.17 (MAE 0.59 levels) |
| yelp | 5 | 0.313 / 0.043 | 0.353 / 0.078 | 0.553 / 0.118 / 0.95 (MAE 0.54 levels) |
| yelp yes/no | 2 | 0.833 / 0.129 | 0.833 / 0.066 | 0.887 / 0.084 / 0.33 |
| **all** | | | | **0.799 / 0.065** |

Temperature scaling fitted on even-indexed records, tested on odd:
`T = 1.47`, NLL 0.505 → 0.481, ECE 0.057 → **0.031**. The model is mildly
over-confident before scaling.

### Mechanism tests

| Test | Result |
| --- | --- |
| Isolation — secret in sibling / absent / in state | p = 0.03 / 0.03 / **0.99** |
| Packed vs separate — max probability difference | **3.7e-6**; packed ~2.0x faster at ~2.7 questions per request |
| Permutation — 4 orders, Choice K >= 3 | argmax flips 7.4%; mean spread of p(correct) 0.065, p90 0.25 |
| IIA — append one irrelevant option | mean |Δ log-odds| top-2 = 0.13, p90 0.34 |
| Boundary forgery — fake delimiters in option text | option count unchanged; forged option p <= 0.09 |

### Stated limitations

In-distribution only — every number above is a held-out split of training
sources. 0.5B of knowledge: BoolQ 0.75 and MNLI 0.75 are far below the
state of the art, and on TypeSafe's own structured-criteria example it
picks `return_policy` where Jev picks `return_status`. About ten
instruction templates; code, tables, multi-turn, arithmetic, and multi-step
conditions untrained. Order sensitivity remains (7% argmax flips). `score`
confidence is a stand-in formula — TypeSafe's is unpublished. Calibration
in-domain says nothing about a new workflow. Inherits the datasets' biases
(US-centric news, English banking terms, review-site skew) and Qwen2.5's.

Not intended for production decisions affecting people — moderation, fraud,
credit, hiring, medical, legal. A `confidence: 0.92` from this model is a
statistic about its own distribution, not a verified probability of being
right.

## The preview family

The research track moved to Qwen3 bases and frozen suites
(`evals/decision-v4`/`v6` for in-distribution, `evals/transfer-v4` for
out-of-domain: QNLI, SciQ, TweetEval, PAWS, MMLU, Emotion, plus held-out
programmatic policy rules). Same items for every row; locked test read once
per candidate.

| Checkpoint | In-domain dev / locked | Out-of-domain dev / locked | Brier (OOD dev) | Confident errors (p>=0.9, wrong) | Held-out policy pairs | Order flips |
| --- | --- | --- | --- | --- | --- | --- |
| kev-0.6b | 0.805 / 0.819 | 0.598 / 0.631 | 0.521 | 5.2% | 0.11 | 0.08 |
| kev-4b | 0.843 / 0.852 | 0.759 / 0.794 | 0.346 | 5.5% | 0.62 | 0.08 |
| kev-8b | 0.869 / 0.869 | 0.774 / 0.799 | 0.339 | 8.2% | 0.61 | 0.03 |
| Jev | 0.845 / – | 0.857 / – | 0.211 | 3.7% | 0.86 | 0.00 |

The previews fail kev's own release screen (held-out policy pairs >= 0.70
both-siblings-correct; best 0.67). They ship as research previews with the
failure recorded on each card — which is the honest pattern worth copying.

## What the research log established

From `PLAN.md` and `runs/leaderboard.md` (~90 bounded trials, each ~$1–7 of
H100 time):

- **Capacity dominates out of domain.** Public data and synthetic budget
  held equal: 0.6B → 4B is +14–19 points; 4B → 8B is +1.5–2.
- **Learning rate controls knowledge erosion.** The 4B base scores 0.69
  zero-shot on the MMLU items; the default recipe trains it down to
  0.60–0.66. lr 5e-5 recovers most of it (+4.7 points, replicated at three
  seeds on 4B and 8B).
- **More public data lifts in-domain, not transfer.** Knowledge-MCQ sources
  raise MMLU a few points without moving the OOD total.
- **Programmatic contrastive policy pairs work.** Trained rule structures
  hit 0.85–1.0 and transfer partially to unseen compositions (0.5–0.67 at
  4B/8B). None-of-the-above minimal pairs fixed the "none" shortcut
  in-domain (0.75 → 0.93 at 4B).
- **The remaining gap to Jev is task coverage, not hyperparameters.**
  Fourteen one-knob mutations land within ±1 point; the gap concentrates in
  MMLU, PAWS, Emotion, and date arithmetic.
- **Jev's zero observed order flips is not proof of invariance** — its
  probabilities still move under permutation.

## Practical reading for the mesh plan

- `kev-0.5b` proves the mechanism and costs nothing to serve — it is the
  right conformance target for a Rust port.
- `kev-4b` is the serving sweet spot the upstream README recommends: best
  accuracy per byte, ~1 s in bf16 on a 32 GB Mac.
- None of these checkpoints is production-calibrated. Out-of-domain ECE is
  ~0.1, and temperature fitted in-domain does not transfer. Any deployment
  needs per-workflow measurement before a probability gates an action —
  the same rule this repository already applies to Jev thresholds.
