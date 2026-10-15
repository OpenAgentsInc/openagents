# s1-llm-auto-router: Decision Models' open router model, and running one in Psionic (2026-10-15)

Decision Models (see [README.md](README.md)) released `s1-llm-auto-router`
as open weights. It is their first decision model built for one use case:
it reads a request to an LLM and says what kind of request it is, so a
router can pick the model. This note records what the model actually is,
how it compares with what we run today (Clef first, then Jev), and what it
would take to run it, or a model like it trained on our own routing
records, in Psionic (`crates/psionic`).

Sources: the announcement (owner-pasted, 2026-10-15), the model card and
files at <https://huggingface.co/decisionmodels/s1-llm-auto-router>, the
results image from the announcement, and our own inspection of the ONNX
graph and tokenizer. We also ran a small local test: the published ONNX on
CPU, plus our own `route-v1` fixture. The downloads were deleted afterwards.

## What they announced

- "A small decision model that reads a request and picks the right LLM for
  it." 26 ms on a CPU, 22 languages, MIT license.
- It is their first use-case-specific decision model, from decisionmodels.io
  and Bittensor Subnet 92 (REAX). They call it the best of the decision
  models they tested for this job. It is live on the decisionmodels.io API.
- It is a joint project with msg group, a German IT consultancy (about
  €1.5B revenue and 10,000+ employees), which will evaluate it internally
  to make its own LLM use more efficient. They thank msg's CTO, Dr. Ralf S.
  Engelschall.
- The pitch from the second post: "Small specialised models make routing
  decisions in milliseconds, at a fraction of the cost of asking a big LLM."

**One correction to the announcement's wording.** The model does not pick
an LLM. It classifies the request, and a separate policy picks the LLM.
That policy is their open-source
[auto-model-router](https://github.com/fstandhartinger/auto-model-router)
(MIT, Python). There, `TaskClassification` "never names a model", and
`policies.py` prices a model catalogue using capability, cost, cache and
quota. This is the same split we use: typed judgments in, code decides.

## The model

| | |
| --- | --- |
| Base | `jhu-clsp/mmBERT-small` (MIT): a multilingual ModernBERT encoder (pre-norm, rotary positions, GeGLU MLP, LayerNorm without bias, alternating local and global attention) |
| Layers | Pruned from 22 to 11: layers 0, 2, 4, …, 18 and 21 of mmBERT-small (`meta.json`). Hidden size 384, MLP 1,152 (GeGLU input projection 384×2,304) |
| Distillation | Teacher: mmBERT-base fine-tuned on soft labels. Student: the pruned small model, distilled with alpha 0.9 for 12 epochs at learning rate 1.5e-4, on one RTX 4090 for about an hour. `meta.json` records the checkpoint from epoch 8 |
| Parameters | 119.4M. Of these, 98.3M are the token embeddings (256,000 × 384) and 21.1M are the encoder and heads |
| File | `model.gq.onnx`, 183 MB, ONNX opset 17. Embeddings are uint8 with one scalar scale (0.00256) and zero point 128, dequantised after the gather. Everything else is f32 |
| Pooling | The first token (`<bos>`) after the final LayerNorm |
| Heads | Four linear heads over the 384-d pooled vector: `category` [9], `difficulty` [5], `stakes` [4], `flags` [4] |
| Calibration | Each head has a temperature in `meta.json`: category 0.5, difficulty 0.8, stakes 0.75, flags 0.6. Softmax for category, difficulty and stakes; a sigmoid for each flag |
| Tokenizer | `tokenizer/tokenizer.json` (34 MB). A Gemma-family SentencePiece-style BPE: 256,000 tokens, 580,604 merges, byte fallback, Metaspace pre-tokenizer with the `▁` prefix always added |
| Input | A pair, `<bos> context <eos> request <eos>`. The context is a short conversation summary (first 600 characters) or `(new conversation)`. Truncation is 512 tokens, cutting only the request. Inputs are `input_ids` and `attention_mask`, int64 [b, t] |
| Output | Raw logits per head. The API turns them into the Jev / System One answer shape |
| License | MIT, for both the weights and the base model |
| Training data | About 39k synthetic routing requests: 34,671 train, 2,275 dev and 2,175 test, split by generation group. Four open LLMs (DeepSeek-V4-Flash, Kimi-K2.6, Gemma-4-31B, Qwen3.8-27B) wrote them from a persona × stack × task × language grid: 22 languages and 29 stacks, including SAP ABAP, PL/I and COBOL. The data was labelled blind by five open LLMs (those four plus GLM-5.1), and a model never labelled its own requests. Training used the soft vote. There is no real user traffic. The data was deduplicated against JevBench |

The graph uses only standard ONNX ops: MatMul, Add, Mul, LayerNormalization,
Softmax, Erf (exact GELU), Sin and Cos (rotary), Gather, Where, Slice,
Reshape, Transpose and one DequantizeLinear. There are no custom ops, so
porting it is a question of the architecture, not of ONNX.

### What it answers (the "labels")

The model answers seven fixed questions. It does not read question text,
so it cannot be asked anything else.

| Question | Type | Options |
| --- | --- | --- |
| `category` | choice | coding, agentic, math, knowledge, long_context, tool_use, design, summarisation, general |
| `difficulty` | score, 5 levels | trivial, easy, moderate, hard, frontier |
| `stakes` | score, 4 levels | negligible, low, medium, high |
| `needs_tools`, `needs_vision`, `needs_long_context`, `follow_up` | yes/no probability (`noul`) | |

On the API it is model `s1-llm-auto-router` at
`https://api.decisionmodels.io/v1/systemone`. That is the same request
shape as Jev and as our Psionic Clef lane. The launch price is USD 0.001
per million input tokens; output is free.

### Their results

From the announcement image: a held-out routing test of n = 2,175 synthetic
requests in 22 languages. Gold labels are the plurality vote of five LLMs.
"Routing agreement" means the auto-model-router policy picks the same model
from the predicted labels as from the gold labels.

| System | Routing agreement with gold | Category accuracy |
| --- | --- | --- |
| s1-llm-auto-router (CPU, 26 ms) | **74.2 %** | **89.8 %** |
| Jev 1.13 (hosted, 247 ms) | 63.0 % | 84.5 % |
| Weiche-395M (local router, 499 ms) | 62.2 % | 78.4 % |
| Winnow-12B / their s1-pro model (GPU, 671 ms) | 57.7 % | 84.1 % |
| Laya (local router) | 18.5 % | 15.0 % |

The model card adds more columns:

- **Flags.** Mean flag accuracy is 0.974. Jev 1.13 scores 0.935, Winnow-12B 0.928 and Weiche 0.903.
- **All seven answers exact.** s1 gets all seven right 0.819 of the time; Jev 1.13 0.603, Winnow-12B 0.608, Weiche 0.553.
- **A blind 100-request set labelled by Claude Opus.** Category accuracy: s1 0.88, Jev 0.80, Winnow 0.75. Routing agreement: s1 0.60, Jev 0.57, Winnow 0.35. At this size the uncertainty is about ±8 points.
- **Calibration.** Expected calibration error is 0.018 for category and 0.008–0.028 for the flags.
- **Latency.**
  - About 14 ms p50 and 47 ms p99 on 4 pinned EPYC 9555 cores.
  - 26 ms p50 on 4 threads of a Xeon E5-2680 v4.
  - About 93 ms end to end through their API.

These are vendor numbers, and they say so. Most test gold comes from the
same LLM-vote process that labelled the training data. Jev is a general
decision model being scored on a task it was not built for. The weakest
languages are Greek, Korean and Italian (category 0.73–0.78 at n ≈ 20 each).

## Our local test (2026-10-15)

We ran the published ONNX with onnxruntime on CPU (4 threads, Apple M5 Max)
in a throwaway virtualenv, using the model card's tokenizer setup.

**Latency.** 2.3–4.3 ms per decision for requests of 9–29 tokens. Over the
32 rows of our `route-v1` fixture, p50 was 3.3 ms and p99 7.9 ms. Most
Coder prompts are short, so their 26 ms figure (an older Xeon, longer
inputs) is conservative for us.

**Readings on example requests:**

- "Summarise this thread in three bullets" gives summarisation 0.99, easy.
- "Fix the failing test in crates/pylon/src/verify.rs and open a PR" gives agentic 0.95, moderate, needs_tools 0.99.
- "Wie spät ist es in Tokio?" gives knowledge, trivial.
- "Refactor the gateway decision dispatch to support a TPU door, keep all tests green, and write the migration doc" gives general 0.39, moderate. That is unsure, and wrong: it is a large agentic change.

**Against our `route-v1` fixture.** The fixture is
`crates/coder-new/fixtures/router/route-v1.json`: 32 rows labelled by us,
with Clef's probabilities recorded when the fixture was made. We fixed one
mapping before looking at results:

- difficulty trivial or easy means small, moderate means medium, and hard or frontier means large;
- "cheap answer" means small, `needs_tools` under 0.5, and a category other than coding or agentic.

| Measure | s1-llm-auto-router | Clef (recorded) |
| --- | --- | --- |
| Size (small / medium / large) matches our label | 20 / 32 | 24 / 32 |
| Cheap-answer yes/no matches our label (9 gold yes) | 30 / 32 | 29 / 32 |

At n = 32 the difference is within noise. What the rows do show is where
s1 goes wrong. It reads both of our "large" repository changes and a
medium research task as small. Its `difficulty` measures how hard the
request is for an LLM to answer, not how much agentic work it will take.
It is good at the easy end, deciding that a request is a quick answer that
needs no tools. That is the decision our `Cheap` door and the router's
`CheapAnswer` step make.

## How it relates to OpenAgents routing

**What we run today.**

- Coder's System One hooks (`crates/coder-sdk/src/system_one.rs`) and the harness router (`crates/coder-new/src/router.rs`) are rule first. Code decides when it can, and asks the Decisions API only when its rule is unsure.
- The router asks typed questions: `Kind` (answer, repo_change, issue_to_pr, research), `Size` (small, medium, large), `Risk`, `needs_repo`, `own_tools` and `new_work`.
- `policy()` then walks fixed steps: OnlyOne, Continue, Pinned, OwnTools, Leaning, NearLimit, CheapAnswer, QuickTurn, PlanWork, StandingOrder.
- Questions go to `openagents/decide`. The gateway's `decision_dispatch.rs` uses `Order::ClefFirst`:
  1. the hosted Clef pool (Clef-Flash 9B on the H100 and A100);
  2. connected Pylons;
  3. Jev under the house key;
  4. Gemini on Vertex.
- One answer in twenty is shadowed to the next door.
- The Clef door answers in 20–300 ms (`message_route.rs`, 2026-10-13).
- Model choice is code: the doors in `crates/coder-new/src/route.rs`, with `google/gemini-4-argon` as the no-harness default (`crates/coder-sdk/src/auto_policy.rs`).

**The deterministic-first rule allows a model like this.** The rule bans
ad hoc keyword routing. It does not ban a typed classifier. s1 is a fixed
typed selector: four heads with closed option sets and calibrated
probabilities, read with fixed thresholds. It fits the "central typed
semantic selector" the rule asks for, and it never matches words. It
would sit where Clef sits today: after the rule, before the policy.

**Clef and s1 compared.**

| | Clef (today) | s1-llm-auto-router |
| --- | --- | --- |
| Model | Clef-Flash 9B (Qwen3.5) or Clef 27B, with a joint decision head | 120M-parameter encoder with fixed heads |
| Questions | Any typed questions (choice, score, noul) | Only its seven |
| Hardware | GPU pool (H100, A100), or about 14 GB on Metal | CPU, about 200 MB of RAM, a few ms |
| Latency (as seen by Coder) | 20–300 ms through the gateway | 3–8 ms in process (our test), about 93 ms through their API |
| Cost | Our GPU hours | Free on CPUs we already run, or USD 0.001 per million tokens through their API |
| Fit to our schema | Exact: it answers `route-v1` as written | Partial: difficulty is roughly our Size, stakes our Risk, needs_tools our needs_repo or own_tools. It has nothing for issue_to_pr, research or new_work |

**Should we use s1 as is?** Not as the main door. Its questions are not
ours, its training data is synthetic chat requests rather than coding-agent
turns, and on our fixture it reads big agentic work as small. It is
reasonable as a free, early "is this a quick answer?" signal in front of
the `CheapAnswer` step. Even there it must pass the experiment below
first.

**Should we build a model like it?** Yes, and this is the real lesson of
the release. A Clef-sized model answering a fixed question set is
overkill. The same recipe (mmBERT-small, pruned, distilled, a few heads,
temperatures fitted on dev) trained on our own questions would answer
`route-v1` and `message-route-v1` in a few milliseconds on the gateway's or
the user's CPU. Clef would stay the teacher and the fallback door. It
would also give each step's small-versus-big call a cost close to zero.
Today that call costs a GPU round trip, which is why the monitor hook
asks only once every few calls.

**Where the training data would come from.** Each source has a limit:

- **Clef's own answers.** Clef's probabilities on our question sets can serve as soft labels, the way Decision Models used the LLM vote. This is distillation from Clef, which is open-weight. We can generate as many as we want over a request corpus we own.
- **Our labelled fixtures.** `route-v1` has 32 rows, and the message-route and context fixtures add more. They are too few to train on, but they are the gold set for evaluation.
- **Routing records.** `crates/coder-new/src/routing_record.rs` writes `chose` and `ended` JSONL: the typed features, System One probabilities, and outcome (cost, time, checks). By design they hold no prompt text and stay on the user's computer. Gateway decision logs also hold no question text. So the records can fit thresholds and score outcomes, but they cannot supply training inputs.
- **Opt-in text.** Real request text for training would need an explicit opt-in collection path with its own privacy review. Until then, use synthetic requests in the style of our users' Coder traffic, labelled by Clef with Jev as a second vote.

## Running it in Psionic

**Short answer:** Psionic cannot run this model today, but it is a small
encoder, and every piece of the port is bounded.

**What Psionic does today.**

- It runs GGUF decoder LLMs (Llama, Gemma 4, Qwen 2/3/3.5/3.8, Mistral, gpt-oss) and the Clef decision lane.
- `GgufEmbeddingFamily { Bert, NomicBert }` and a WordPiece tokenizer exist only as metadata and tokenizer loading.
- `/v1/embeddings` on CPU is a `ByteProjectionEmbedder`, a single projection layer, not a BERT forward pass.
- It has no ONNX support, and no ModernBERT, DeBERTa or XLM-R code.
- It already has the parts the port would reuse: rotary embeddings, sliding-window masks (decoder side), a threaded AVX-512 GEMM (`qwen35/cpu_gemm.rs`), safetensors loading (the Clef head), and a `/v1/systemone` route that already speaks the answer shape (`psionic-serve/src/clef/`).

### The gaps

| # | Gap | What it takes | Size |
| --- | --- | --- | --- |
| 1 | **Encoder forward on CPU** | A ModernBERT family in `psionic-models` and `psionic-serve`. It needs: <br>• bidirectional attention with no KV cache; <br>• rotary positions with two bases (mmBERT uses a larger base on global layers than on local ones; read both from the base config); <br>• alternating global and sliding-window layers (a symmetric window of 128, every third layer of the original 22 global; check which of the kept layers are global against the ONNX masks); <br>• a GeGLU MLP with exact (erf) GELU; <br>• LayerNorm without bias, including the embedding norm (layer 0 has no attention norm); <br>• first-token pooling. <br>Batch size 1 is enough at first. Rotary and the GEMM exist; GeGLU, bias-free LayerNorm and the bidirectional local mask are new on CPU | M |
| 2 | **Weight loading** | Do not add an ONNX runtime to Psionic: it is a large dependency for one small model. Instead, a one-off converter turns the ONNX initializers into a safetensors file with named tensors, plus a JSON config. The work is mapping the anonymous `onnx::MatMul_*` names to layers by walking the graph, and transposing to our layout. Keep the uint8 embeddings with their scale and zero point, and dequantise per gathered row (98M bytes instead of 393 MB in f32). Pin the converted file by sha256, as the Clef GGUF is pinned. A GGUF path is possible later, but nothing needs it | S |
| 3 | **Tokenizer** | Psionic's SentencePiece tokenizer reads GGUF score tables. This model ships an HF `tokenizer.json` with 580k ranked merges. The work: <br>• a loader for merge-ranked SentencePiece-style BPE with byte fallback and a Metaspace pre-tokenizer that always adds `▁`; <br>• the pair template `<bos> A <eos> B <eos>`; <br>• truncation of the second sequence only, at 512 tokens. <br>Acceptance: token ids identical to HF `tokenizers` on at least 1,000 strings across the 22 languages, plus code and paths | S–M |
| 4 | **Heads and answer shaping** | Four linear heads, the per-head temperatures from `meta.json`, softmax or sigmoid, and the System One response: a probability per option, expected values for scores, and the `psionic` provenance block. This is a small subset of what the Clef head already does | S |
| 5 | **Serving** | Two ways, and we should build both. <br>• **Over HTTP**: add the model to `psionic-openai-server` behind the existing `/v1/systemone` route. Its question schema is fixed, so a request asking anything else is refused as `not_admitted` and the judge chain moves to its next door. Warm it up before `/health` (from [reax.md](reax.md)). The gateway's `decision_dispatch.rs` then gets a door (for example a `router` order or model id) in front of hosted Clef for the questions it can answer. <br>• **In process**: Coder links the encoder crate directly and asks with no network hop. A few ms is the whole point, and an HTTP round trip to the gateway would cost more than the model | S–M |
| 6 | **CPU speed** | `psionic-backend-cpu` has no threading. Reuse the rayon GEMM from the qwen35 CPU lane. Target: p99 under 20 ms for 512 tokens on 4 cores. Short prompts should already be a few ms with f32 weights | S |
| 7 | **Parity and calibration gate** | A fixture of about 100 requests with onnxruntime's logits checked in, like `crates/psionic/fixtures/clef/`. Psionic must match: every argmax equal, and centred-log distance (as in [calibration.md](calibration.md)) under 0.01 per answer | S |

**Overall size: M.** About a week of focused work for one agent, nearly
all of it in gap 1. If we train our own router (below), gaps 1–7 stay the
same: only the heads and the weight file change. Doing the port once
serves both models.

## Experiment plan

Each step has a gate. Do not start a step until the one before it passes.

1. **Baseline (done above).** onnxruntime on CPU: 3.3 ms p50 on `route-v1`. Size 20/32 against Clef's 24/32; cheap-answer 30/32 against 29/32.
2. **Evaluation set.** Build 300+ Coder-shaped requests:
   - the `route-v1` and `message-route` fixtures;
   - synthetic requests in our users' style, in the 22 languages the model covers, including code, paths and issue links.
   Label every request two ways:
   - **Our gold.** Clef and Jev vote on `route-v1`'s questions, and we break ties.
   - **An outcome label, where a trace exists.** Did the cheap door's answer stand? It did not stand if the turn was re-briefed, escalated, or its checks failed. Take this only from runs whose owners opted in, or from our own test runs.
3. **Offline comparison.** Run s1 (onnxruntime, with the fixed mapping above), hosted Clef-Flash and Jev 1.13 on the same set. Report:
   - accuracy on each question against gold;
   - cheap-answer precision. This one matters most: sending big work to a small model costs more than the reverse, as in the router's `HOST` costs;
   - p50 and p99 latency, and cost per million tokens.

   **Gate:** s1 earns a door only if cheap-answer precision is at least Clef's, and recall is no more than 5 points below.
4. **Our own router, trained.** Use their recipe on our questions:
   - the teacher is mmBERT-base, trained on Clef's soft labels over the synthetic set;
   - the student is mmBERT-small pruned to 11 layers;
   - fit temperatures on dev;
   - train on one rented GPU (about an hour, per their numbers) or on our TPU hold.

   **Gate:** agreement with Clef's argmax of at least 90 % on dev; Size and Kind accuracy against our gold no more than 2 points below Clef; cheap-answer precision at least Clef's; expected calibration error at most 0.03.
5. **Psionic port (gaps 1–7).** **Gate:** the parity fixture passes (identical argmaxes, distance under 0.01); tokenizer ids are identical; p99 is under 20 ms on 4 CPU cores at 512 tokens.
6. **Shadow, then promote.** Run the small router next to Clef on live `openagents/decide` traffic for the questions it covers, logging both answers, the same way the one-in-twenty Jev shadow works now. Make it the first door, with Clef as the fallback when it is unsure (top probability under the step's threshold), only after:
   - 24 h of shadow traffic;
   - cheap-answer disagreement with Clef under 5 %;
   - no regression in the routing records' `ended` outcomes: cost per turn down, checks passed not down.

## Decisions for later

- **Their hosted API.** Do not put it in a routing path as a door. It sends user text to a third party, and our own CPU is cheaper and faster. It is fine as a reference in the offline comparison.
- **Docs drift, found in passing.** `INVARIANTS.md` (around lines 216–217) still describes Jev as the first decision door, but the code is Clef first (`Order::ClefFirst`, owner, 2026-10-12). Fix that separately; it is not part of this note.
