# Kev architecture

**Status:** describes external code in `projects/repos/kev/` (upstream
`github.com/jaredpalmer/kev`). Everything on this page is implemented and
measured there, and as of the port sequence (openagents#9337) it also runs
in this repository: `crates/kev/src/encode.rs` holds `encode()` and the
branch mask, `model.rs`/`lora.rs` the backbone and adapter, `head.rs` the
pointer readout, `decision.rs` the assembled model, `api.rs`/`render.rs`
the TypeSafe shapes, and `serve.rs` the HTTP surface.

This page describes the shared mechanism. Upstream's newer serving path
also uses optimized attention, shape bucketing, and a state-prefix KV
cache; the Rust port does not yet implement those optimizations. Both
implementations merge LoRA at load time, but their bf16 cast order differs.
See the [implementation review](2026-09-20-upstream-review.md#findings-in-this-repository)
before applying upstream's latency or numerical-parity claims to the port.

A kev checkpoint is two trained pieces on a frozen causal LM:

1. A **LoRA adapter** on the backbone (`kev-0.5b`: rank 16, alpha 32,
   dropout 0.05, on `q_proj k_proj v_proj o_proj gate_proj up_proj
   down_proj` across all 24 layers of Qwen2.5-0.5B).
2. A **pointer head**: two linear maps from the hidden size to a 256-wide
   pointer space. One maps the decision token's hidden state into a query;
   the other maps each option's closing-delimiter hidden state into a key.
   Scaled dot product plus softmax gives the option distribution.

Trainable parameters on `kev-0.5b` are 9.3M, about 1.9% of the 494M
backbone. The backbone's own vocabulary head is unused — the model never
generates.

## The packed sequence

`kev/model.py:encode()` renders one request into one token sequence:

```text
<|fim_prefix|> …state tokens…
<|fim_middle|> …instructions… <|box_start|> option 1 <|box_end|>
              <|box_start|> option 2 <|box_end|> … <|fim_suffix|>
<|fim_middle|> …question 2… <|fim_suffix|>
```

The five delimiters are pre-existing Qwen special tokens reused as
`<state>`, `<q>`, `<opt>`, `</opt>`, and `<decide>`, so no embedding rows
need to be added or trained; the LoRA adapter teaches them their new
meaning. Alongside the token ids, `encode()` produces three parallel
records:

- `seg` — segment id per token: `0` for the state, `k` for question `k`.
- `pos` — position ids. The state runs `0..S`; each question branch
  restarts at `S` and counts up over its own tokens, so every branch sees
  the geometry "state, then one question" and question order carries no
  signal.
- `opt` — per-token option index within its question, used to locate each
  `</opt>` token and the `<decide>` token for the readout.

## The block-causal mask

`branch_mask` allows token `i` to attend to token `j` when `j <= i` and
either `seg[j] == 0` (shared state) or `seg[j] == seg[i]` (same question).
The result is one prefill that computes the state once and runs every
question as an isolated branch off it:

- A question cannot see a sibling question — measured isolation puts a
  secret planted in one branch at `p = 0.03` (chance) while the same secret
  in the state reads `p = 0.99`.
- Packed and separate requests agree to `3.7e-6` maximum probability
  difference, and packing is about 2x faster at ~2.7 questions per request.
- All questions answer from one forward pass; there is no decode loop.

An `option_isolation` variant goes further: each option span becomes its
own sub-branch (it sees the state, the instruction, and itself only) with
shared position ids and `<decide>` at a fixed offset, making the readout
permutation-invariant by construction — at a measured accuracy cost at 4B.

## The readout

`PointerHead` maps the `<decide>` hidden state into a query vector and each
option's `</opt>` hidden state into a key vector, both `896 -> 256` on the
0.5B backbone. `logits = K(h_opts) @ q(h_decide) / sqrt(256)`; softmax over
the question's own options is the output distribution. `K` is whatever the
request sends — the head is a pointer over positions, not a fixed
classifier, so option sets are decided per request rather than baked into
the weights.

Because `<decide>` sits after every option, the decision reads the full
list before scoring — which is what makes "none of the above" options and
listwise effects work.

## Delimiter hardening

Caller text is tokenized through `user_tokens()`, which rewrites
`<|name|>` sequences into lookalike `<¦name¦>` text before tokenizing, so
no request body can forge a `<q>`, `<opt>`, `</opt>`, or `<decide>` token.
The forgery probe confirms option text containing fake delimiters leaves
the option count unchanged and gives the forged option at most `p = 0.09`.

## The wire contract

`kev/api.py` maps TypeSafe's `POST /v1/systemone` shapes onto the pointer
primitive:

| Question type | Rendered as | Answer derived from `p` |
| --- | --- | --- |
| `noul` | two options, `no` and `yes` (criteria text fills the descriptions) | `noul = p(yes)` |
| `choice` | one option per criteria entry, `name` or `name: description` | `choice = argmax`, `probabilities` by option key, `confidence = (p_max - 1/K) / (1 - 1/K)` |
| `score` | one option per ordered level description | `score = sum(k * p[k])`, `legend` maps indices to level text, `probabilities` by index |

`state`, `instructions`, and criteria values accept `string | object |
array`; `render()` flattens structured values into labelled text. Question
ids are caller-side — the model never sees them. Validation rejects
requests outside the bounds (for example `choice` over 255 options) with
`422`.

`kev/serve.py` serves the contract with FastAPI:

| Route | Purpose |
| --- | --- |
| `POST /v1/systemone` | The packed decision request → typed answers plus `usage` and `latency_ms` |
| `GET /v1/models` | Model, base, and run info |
| `POST /v1/systemone/permute` | One `choice` question under N option orders (order-sensitivity probe) |
| `POST /v1/systemone/separate` | Each question in its own pass (packed-vs-separate comparison) |
| `/api/*` | Playground routes (`predict`, `permute`, `eval`, `info`) |

The server is local-only — no authentication, one request at a time, no
cross-request KV cache, dense per-sample masks.

## Training

`kev/train.py` fine-tunes the LoRA adapter and head with cross-entropy over
each question's option distribution, averaged over the questions in a
record. The released `kev-0.5b` used AdamW at lr 2e-4 for 2 epochs on 9,000
records; the current preview recipe uses lr 5e-5 — the single largest
quality improvement the research log records, because the higher rate
erodes base-model knowledge the decision task relies on. Optional loss
terms add a permutation-consistency KL (`--perm_kl`) and a ranked
probability score for `score` questions (`--ord_w`).

Training and serving share one renderer: datasets convert to
TypeSafe-shaped requests and go through `api.to_record()`, so the model
never meets a format at inference it did not see in training. Augmentation
applies once per record before encoding: option order shuffles, a `none of
the above` replacement at p=0.10, an irrelevant distractor option at
p=0.15.

`kev-0.5b` trains in ~1h45m on an Apple M5 (32 GB) in fp32 on MPS. The
4B/8B previews train in 40–70 minutes on one H100 through `modal_app.py`,
which runs each trial in a container that verifies the shipped `kev/*.py`
hashes, the suite hash, and the git commit the launcher recorded before it
starts.

## Frozen suites and gates

Research rigor lives in `evals/`: frozen, checksummed suites with separate
training, calibration, development, and locked-test partitions, per-record
provenance, and pinned dataset and base-model revisions. Development
partitions select models; the locked test is read once per published
candidate (`--allow-test`, enforced single-read). Every trial checks the
mechanism — complete coverage, isolation, packing equivalence — and scores
a transfer suite the model never trained on. `kev/autoresearch.py` runs
bounded hill-climb rounds over a config allowlist and keeps
`runs/leaderboard.md`; roughly 90 trials have run this way at a few dollars
each.

## What this means for a Rust port

The mechanism is small and regular: one packed prefill, an additive
`[L, L]` mask, custom position ids, gather the `</opt>` and `<decide>`
hidden states, two tiny projections, softmax per branch. Everything a
serving stack needs that kev's Python does not provide — batching across
requests, a real KV-cache layout for the shared prefix, quantized or fused
kernels — is serving engineering on top of a fixed, testable semantic. The
packed-vs-separate equality bound (`~4e-6`) and the mechanism probes are a
ready-made conformance suite for any reimplementation; [`mesh-plan.md`](
mesh-plan.md) uses exactly that.
