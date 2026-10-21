# Base Labs' mammoth-1 and elephant-1: what they are and how OpenAgents could support them

Status: research note, 2026-10-21. Nothing here was run against Baseten's API,
and no weights were downloaded. The note reads Baseten's pages, the Hugging Face
model repositories and their small text files, the public Decision Index board,
and this repository.

Base Labs, Baseten's research team, released two open-weight decision models on
2026-10-19: **mammoth-1** (27B) and **elephant-1** (9B). This note says what
they are, how they compare with Clef and our other doors, and what supporting
them would cost. Every section keeps published facts, cited by URL, apart from
our inference.

## Contents

- [Verdict](#verdict)
- [Two corrections to the brief](#two-corrections-to-the-brief)
- [1. What the models are](#1-what-the-models-are)
- [2. How they compare with Clef and our doors](#2-how-they-compare-with-clef-and-our-doors)
- [3. Ways to support them](#3-ways-to-support-them)
- [4. Recommendation and sequence](#4-recommendation-and-sequence)
- [5. Licensing and attribution](#5-licensing-and-attribution)
- [6. Issue-ready steps](#6-issue-ready-steps)
- [Sources](#sources)

## Verdict

- **They are System One models, and they speak our wire format.** Baseten serves
  them at `POST https://inference.baseten.co/v1/systemone` with the same
  `{model, state, questions}` body that Jev, Clef, and our gateway take. Both are
  prefill-only full fine-tunes of Qwen3.5. They return a probability per option
  and generate no text.
- **On the public board, both beat Clef by a wide margin.** On the Decision
  Index board's current Full score, mammoth-1 scores 68.87 and Cloudflare's Clef
  27B scores 54.92. elephant-1 scores 60.26 and Clef-Flash scores 44.43, so the
  9B model also beats Clef 27B. mammoth-1 leads Clef in all five skill areas.
  The board's latency figures put mammoth-1 at about twice Clef's median
  latency, but each entrant was measured on its own serving stack.
- **We have no measurement of our own yet.** None of our Gym suites has been run
  against either model. Use the free window to run them before 2026-10-27,
  through Baseten's API. That needs no gateway change.
- **Self-hosting is a Psionic job, not a vLLM job.** The backbone is the same
  Qwen3.5 hybrid that Psionic already serves for Clef, with an identical tensor
  layout. The new work is the attention mask, the prompt, and the readout.
  Start with elephant-1 on CUDA, then a Pylon lane, and then mammoth-1 as a
  quality tier.
- **Settle the license before we redistribute anything.** Baseten's pages say
  MIT. The Hugging Face card says `license: other` with the name `mit`, and
  neither repository has a LICENSE file. The board describes the weights as
  under "a proprietary license". Commercial evaluation and serving look low
  risk. Shipping a converted GGUF to Pylon operators should wait for a LICENSE
  file or written confirmation from Baseten.

## Two corrections to the brief

1. **Clef is not our model.** It is Cloudflare's open-weight decision model
   (Apache-2.0), and we self-host it. Clef-Flash is post-trained from
   Qwen3.5-9B, and Clef from Qwen3.8-27B
   (`docs/inference/clef-self-host.md:1-5`; the `Cloudflare/clef` and
   `Cloudflare/clef-flash` cards). Our gateway asks it first
   (`INVARIANTS.md:208-219`), and Pylons serve Clef-Flash
   (`crates/pylon/src/release.rs:69-87`).
2. **The Jev Decision Index is not ours, and we don't run it.** It is a
   community board, the Hugging Face Space `multimodalart/jev-decision-index`.
   Its reproduction kit is `apolinario/decision-index` (MIT), whose README says
   it is not affiliated with TypeSafe AI. The repository has no Decision Index
   runner and no Clef score from one. Two existing documents cite it only as a
   third party's claim (`docs/research/decisionmodels/README.md:147`,
   `docs/inference/subq-snap.md:224`). The closest benchmark we hold is the
   vendored public JevBench suite (`crates/gym/suites/external-jevbench-v1.json`,
   231 items), and no door has been scored on it yet
   (`docs/decision-models/others/2026-09-20-jevbench.md`).

## 1. What the models are

### Published facts

**What a decision model is in their framing.** A decision model takes a JSON
`state` and a set of typed questions, and returns a probability distribution
over each question's options without generating text. Base Labs credits
TypeSafe's Jev and System One API with the format, and names three question
types: choice, yes/no, and score. Their argument is that this format is cheaper
than a generative model, that callers can set thresholds or escalate on it, and
that it can be trained to be calibrated
([Labs article](https://labs.baseten.co/articles/mammoth-1-and-elephant-1-frontier-decision-models),
[blog](https://www.baseten.co/blog/the-best-decision-models-in-the-world-mammoth-1-and-elephant-1/)).

**Architecture.**

| | mammoth-1 | elephant-1 |
| --- | --- | --- |
| Base (card `base_model`) | `Qwen/Qwen3.5-27B` | `Qwen/Qwen3.5-9B` |
| Parameters (HF safetensors metadata) | 27,356,728,560, BF16 | 9,409,813,744, BF16 |
| Layers | 64: 16 units of 3 Gated DeltaNet + 1 full attention | 32: 8 units of the same |
| Hidden size, FFN | 5,120, 17,408 | 4,096, 12,288 |
| Full-attention heads, KV heads | 24, 4 (head dim 256) | 16, 4 (head dim 256) |
| Vocabulary | 248,320 | 248,320 |
| Vision tower | Included, frozen in training | Included, frozen in training |

Sources: each repository's `config.json`
([mammoth-1](https://huggingface.co/baselabs/mammoth-1/blob/main/config.json),
[elephant-1](https://huggingface.co/baselabs/elephant-1/blob/main/config.json))
and the Labs article.

- **Only the full-attention layers change.** Their mask is block-bidirectional.
  Block 0 is the state and the shared prefix. Block 1 is one question's suffix.
  A token sees its own block and earlier blocks, so a question is encoded
  together with its options, while the prefix never sees a suffix. The Gated
  DeltaNet layers stay causal. The config records this as
  `modeling_overrides.attention = {full_attention: block_bidirectional,
  linear_attention: causal}`. In a single-run ablation on the 9B, the mask
  raised the public-sample score from 44.8 to 48.6.
- **The readout uses no extra head.** `readout.json` is
  `{"readout": "slot_lm", "head_dim": 256, "n_slots": 256}`. The answer is a
  softmax over the language-model logits of the option letters at the answer
  position, with up to 255 options. The prefix is encoded once and its cache is
  reused. Each question costs one batched forward pass over its suffix.
- **Context.** `max_position_embeddings` is 262,144, inherited from the base.
  Training used sequences of up to 16,384 tokens. The published engine refuses
  requests over the trained length limits
  ([`engine.py`](https://huggingface.co/baselabs/mammoth-1/blob/main/engine.py)).
- **Prefill-only.** The model card describes mammoth-1 as answering every
  question from one shared-prefix prefill, without generating text
  ([card](https://huggingface.co/baselabs/mammoth-1)).

**Output format.** The output is probabilities, which a caller turns into
typed answers.

- A choice question takes 1 to 255 options. The answer is the probability
  vector, its argmax, and a confidence of `(max p - 1/k) / (1 - 1/k)`.
- A yes/no question returns P(true). The kit and engine call this type `noul`,
  as our contract does.
- A score question takes 2 to 10 ordered levels. The answer is a probability
  per level, the expected level, and a confidence.
- Training teaches abstention. It sometimes adds an "I don't know" option and
  sometimes removes the correct answer
  ([Labs article](https://labs.baseten.co/articles/mammoth-1-and-elephant-1-frontier-decision-models)).
  Neither page shows how the hosted API exposes abstention.

**API shape.** It is a decision-specific endpoint, not an OpenAI-compatible
one. The library pages show `POST https://inference.baseten.co/v1/systemone`
with `Authorization: Api-Key $BASETEN_API_KEY`. The model names are
`decision-models/baseten-mammoth-27b-latest` and
`decision-models/baseten-elephant-9b-latest`. Each question in the body has
`type`, `instructions`, and `criteria`. Neither page shows a response body. The
library pages list text input only and "Structured Decision" output
([mammoth-1](https://www.baseten.co/library/mammoth-1/),
[elephant-1](https://www.baseten.co/library/elephant-1/)). Baseten's
OpenAI-compatible Model APIs accept `Authorization: Bearer`, and Baseten
recommends about 60 requests per minute per session
([Model APIs](https://docs.baseten.co/development/model-apis/overview)). Model
APIs keep no data by default
([blog](https://www.baseten.co/blog/the-best-decision-models-in-the-world-mammoth-1-and-elephant-1/)).

**Precision, formats, and files.** Both repositories hold BF16 safetensors
only, with no GGUF and no quantized variants. The top-level `config.json`
says `float32`, but the text and vision configs and the stored tensors are
BF16. Data as of the revisions below
([HF API](https://huggingface.co/api/models?author=baselabs)):

| | mammoth-1 | elephant-1 |
| --- | --- | --- |
| Revision (2026-10-21) | `9d1962f193e48a68cf0f3598dc7fb4462fd23ad9` | `5494c13e598c713127938cc7492d4deebf20b9a1` |
| Weights | `model-00001-of-00002.safetensors` 49,825,162,976 B; `model-00002-of-00002.safetensors` 4,888,445,168 B (54.7 GB in all) | `model.safetensors` 18,819,722,392 B (18.8 GB) |
| Other files | `readout.json`, `engine.py`, `serve.patch` (41 KB), `setup.sh`, `constraints.txt`, `chat_template.jinja`, tokenizer (20 MB), configs | Same, byte-identical |
| History | Weights committed 2026-10-19, then replaced 2026-10-20 (`8f9f86193e`) | Weights committed once, 2026-10-19 (`14537f08b0`) |

**Serving bundle.** `setup.sh` installs two things on a
`pytorch/pytorch:2.8.0-cuda12.8` image with one CUDA GPU. The first is
OmniJev's `OneJev` runtime (Apache-2.0) at commit `81ce62f`, with
`serve.patch` applied. The second is the Decision Index kit at `f5a6e4f`. The
patch changes OneJev's `qev` engine, prompt, and readout. It also changes
`train/sft.py` and `train/common.py`, so the bundle carries the training path
as well. `engine.py` serves `choice`, `noul`, and `score`. It falls back to
sequential prefix forks when a large request overflows 32-bit indexing.

**Benchmarks, as Base Labs reports them.**

- Jev Decision Index 0.4 Full score: mammoth-1 71.24, #1 of 208. elephant-1
  62.65, tied 7th in the Labs article and tied 8th in the blog, best under
  15B. The article reports mammoth-1 ahead of Perplexity Decider v1.2 (69.42)
  by 1.8 points, Jev 1.14 by 9.0, and Cloudflare Clef by 15.5.
- Artificial Analysis Decision Index, a weighted mean of AA-Omniscience-Decision,
  HLE-MultiChoice, When2Call, and WildGuardTest: elephant-1 65, tied for #1
  with Liquid AI's d1-ultra. Base Labs' own replica gives mammoth-1 65.9 and
  elephant-1 65.2. We haven't checked Artificial Analysis' board.
- **How the Decision Index is measured.** The public suite has 37
  benchmarks in five weighted areas. Each benchmark is chance-corrected and
  coverage-adjusted. The 0.4 Full score is 60% private tests of the same skills
  and 40% private tasks from new domains. The maintainers run both private parts
  on one RTX PRO 6000 with the author's serving code, and each part is put on
  the public scale using stock models as the yardstick. Scores less than 0.9
  apart share a rank. The board doesn't list a model whose median, mean, or p80
  latency exceeds 1,000 ms on that card
  ([kit README](https://github.com/apolinario/decision-index)).

**Training, in summary.** The method was one epoch of full-parameter supervised
fine-tuning on soft target distributions, with the vision encoder frozen. The
loss was cross-entropy plus a Brier term at the answer position. The final data
mix had 6.21M examples. Core decision skills were 58%. Safety and abstention
were 16%, and so were judging and expert domains. Public decision data, drawn
from 86 datasets, was 9.5%. Synthetic items came from models such as DeepSeek
V4-Pro and GLM-5.3. A panel of five judges labeled them, and a label was kept
only if four agreed. The data was decontaminated against both indices. The
9B ran 5,546 steps on 128 H100s, and the 27B ran 9,859 steps on 72 GB300s, both
with FSDP. Base Labs didn't report the cost
([Labs article](https://labs.baseten.co/articles/mammoth-1-and-elephant-1-frontier-decision-models)).

**Deploying and post-training on Baseten.** Both models are on Model APIs and
offer a dedicated deployment from their library pages. Neither page gives
post-training steps for these models. Baseten Training in general runs your own
training container on H100 or H200 nodes from a `config.py` and
`baseten train push`. It syncs checkpoints, and
`baseten train checkpoint deploy` turns one into an endpoint
([Training](https://docs.baseten.co/training/overview)).

**Free period.** The blog says the models are free to try "for a week" and
gives no end date. The owner's announcement posts give 2026-10-27 11:59 PM PT.
No Baseten page lists a price after that.

### Our inference

- **Same architecture as Clef, but not the same weights or base.** mammoth-1's
  and Clef 27B's weight indexes list the same 1,184 tensor names. The two
  models have exactly the same parameter count, and so do elephant-1 and
  Clef-Flash. This is the Qwen3.5 layout without the multi-token-prediction
  weights, and several other board entries share the count. The cards name
  different 27B bases, Qwen3.5-27B against Qwen3.8-27B, while both 9B models
  start from Qwen3.5-9B. Nothing suggests that one model was trained from the
  other.
- **The readout is closer to Jev's semantics than Clef's is.** Clef adds a
  121.8M joint head, and its questions decide together, so splitting a request
  changes the answers (`docs/inference/clef-native.md`). In mammoth-1, each
  question sees the shared prefix and its own suffix only. Questions don't
  interact, which matches how our callers treat Jev answers.
- **The 16k limit matters to us.** Our chat-router goldens run about 14k tokens
  per turn (`docs/inference/subq-snap.md`), and our Clef door accepts up to
  65,536 tokens with chunking (`docs/inference/clef-decisions-h100.md`).
  Requests above the trained limit would be refused, and a refusal falls through
  to the next door.

## 2. How they compare with Clef and our doors

### The public board, today

These rows are from the board's data file, `data/index.json` at Space revision
`f6dfe71d9f`, generated 2026-10-21 20:30 UTC with the label "Decision Index
0.4.1" and 220 entries
([board](https://huggingface.co/spaces/multimodalart/jev-decision-index)).
Full = 0.4 Full score. Public = public 0.3 index. Same = private same-skill
part. New = private new-domain part. Latency is the median in milliseconds on
one RTX PRO 6000 with each author's serving code.

| Model | Rank (tie) | Full | Public | Same | New | Median ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Jeffan-31b-1710 (CC BY-NC 4.0) | 1 | 69.75 | 73.13 | 66.15 | 59.06 | 121 |
| **mammoth-1** | 1 | **68.87** | 75.18 | 66.56 | 56.78 | 219 |
| Perplexity Decider v1.2 (27B) | 2 | 67.98 | 67.74 | 64.30 | 57.76 | 144 |
| **elephant-1** | 8 | **60.26** | 72.24 | 62.44 | 44.41 | 114 |
| Jev 1.14 (hosted) | 8 | 60.24 | 60.70 | 59.28 | 48.26 | 196, over HTTPS |
| Cloudflare Clef (27B) | 13 | 54.92 | 61.71 | 53.11 | 45.06 | 102 |
| Cloudflare Clef-Flash (9B) | 22 | 44.43 | 56.15 | 44.01 | 35.00 | 53 |
| Laya | 51 | 9.21 | 6.19 | 9.08 | 6.63 | 6 |

The area skill scores are on the public split. "Knowledge" is
Knowledge & Reasoning.

| Area | mammoth-1 | Clef | elephant-1 | Clef-Flash | Jev 1.14 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Knowledge | 68.1 | 52.8 | 60.6 | 53.0 | 57.1 |
| Language | 75.6 | 61.3 | 73.8 | 47.2 | 61.2 |
| Retrieval & classification | 77.3 | 62.8 | 76.2 | 52.2 | 63.5 |
| Tools & automation | 88.5 | 80.7 | 88.0 | 81.9 | 73.8 |
| Arts & human taste | 63.6 | 48.9 | 61.6 | 48.4 | 39.2 |

The board's notes on both Base Labs entries say the following:

- The public runs were adopted as submitted.
- The private tests ran on the author's serving code, with 99.8% decision
  agreement against the submitted rows.
- One private new-domain test was not counted for either model, because the
  board found signs that the training data overlapped it.
- elephant-1's training includes train splits of Decision Index benchmarks, as
  the author declared.

### Our reading

- **They beat Clef, and not narrowly.** mammoth-1 leads Clef 27B by 13.95
  Full points and leads in every area, by 8 to 15 points. elephant-1 leads Clef-Flash
  by 15.8 Full points and also beats Clef 27B, by 5.3. The smallest margins are
  in Tools & Automation, where all four models score above 80 and many of our
  router-style questions live.
- **The announcement's numbers are stale.** The posts' 71.24 and 62.65 don't
  match today's 68.87 and 60.26. The board was revised to 0.4.1 today and now
  excludes one private test for each model. mammoth-1 is still tied for #1,
  but second on raw score.
- **elephant-1 is weaker away from its training mix.** It scores 72.2 on the
  public suite and 44.4 on new-domain tasks, a gap of 28 points. mammoth-1's gap
  is 18 points, and Jev's is 12. Clef-Flash shows the same pattern as elephant-1,
  with a 21-point gap. Our product questions are a new domain to every model, so
  the "New" column predicts our results better than the "Public" column does.
  On that column, elephant-1 is below Jev (44.4 against 48.3), and mammoth-1 is
  above it (56.8).
- **The latency cost is probably a serving effect.** The board measured
  mammoth-1 at 219 ms median and Clef at 102 ms, on the same card but on
  different stacks: OneJev's PyTorch engine against Cloudflare's. The backbones
  are identical, and mammoth-1's readout is lighter than Clef's head. A native
  Psionic engine should land near Clef's per-token cost: H100 Q4 at 5.1k tok/s
  and $0.328/M tokens on spot for 27B, and 15.0k tok/s and $0.112/M for 9B
  (`docs/inference/gpu-options.md`). The multi-question fork adds suffix tokens
  per question, so a 13-question request costs somewhat more than Clef's single
  prompt. Measure it.
- **On our own questions, the answer is unknown.** Our evidence on Clef is
  mixed. It agrees with Jev 94-97% on product notes, but it isn't a drop-in for
  the router's main decision (`docs/inference/clef-self-host.md`). On file
  relevance, Clef 27B roughly ties Jev (F1 0.81/0.88 against 0.78/0.89), and
  Clef-Flash trails (0.65/0.44) (`docs/inference/clef-jev-relevance-bench.md`).
  No such numbers exist for mammoth-1 or elephant-1. Getting them is the first
  job.

## 3. Ways to support them

Effort: S is under two days, M is up to two weeks, and L is longer.

### (a) A door that calls Baseten's API this week, for evaluation

**What exists.** Three paths already speak `/v1/systemone` to an outside
host:

- The coder harness's `hosted_http` decision profile, configured with
  `CODER_DECISION_URL`, `CODER_DECISION_MODEL`, and `CODER_DECISION_KEY`. It
  drove the router eval and the chat goldens against SubQ Snap with no harness
  change (`docs/inference/subq-snap.md`, "Chat router").
- `gym eval --door name=url`. It sends a placeholder key
  (`crates/gym/src/bin/gym.rs:572-590`), so it can't authenticate to Baseten
  yet.
- The gateway's decision chain: Clef, then Pylons, then Jev, then Vertex
  (`crates/gateway/src/decision_dispatch.rs:488-530`). Jev is the precedent for
  an external door that speaks System One under a house key. The registry doors
  in `gateway.json` don't fit, because they require our `{"models":[...]}`
  identity card before forwarding (`crates/gateway/src/serve.rs`).

**Steps.**

1. Use `hosted_http` as it is for the router eval and the goldens.
2. Add a key option to `gym eval --door`, for example `--door-key-env NAME`.
3. Only if the numbers justify it, add a `Kind::Baseten` gateway door. Keep it
   out of every default order, reach it only through an explicit
   `X-Decision-Order` value, and don't shadow user traffic to it.

**Effort.** S. Path 1 is zero code. Step 2 is about 20 lines. Step 3 is about
a day with tests.

**Risk.**

- Baseten's example uses `Api-Key`, and our Jev client sends `Bearer`. A smoke
  test settles which one Baseten accepts. If only `Api-Key` works, we need a
  header option in `crates/jev`.
- The response body isn't documented. Check it against the strict answer
  validation that the gateway already applies (`INVARIANTS.md:219`).
- The free period ends on 2026-10-27, and post-window prices are unknown.
- Requests over 16k tokens are refused.
- Only public or synthetic suites may leave our machines. Locked items and
  Pylon check items stay home.

### (b) Serving the open weights ourselves on GPU and TPU

**Option 1: the OneJev reference stack.** Run the published bundle on one
scratch H100 or RTX PRO 6000. It is Python, so treat it as a reference oracle,
the way the HF f32 reference serves Clef parity. Don't treat it as production.
It is the only path that is faithful by construction, and it records the
answers that every later port must match.

- Effort: S.
- Risk: low.
- mammoth-1 needs 54.7 GB at BF16, which rules out the A100 40GB.

**Option 2: vLLM or SGLang.** Neither supports a block-bidirectional mask over
a per-question suffix fork, or the letter-slot readout. Both would need custom
model code and scheduler changes, and our own records show that SGL-JAX
already fails on Clef's extra keys (`docs/inference/clef-tpu-survey.md`).

- Effort: M to L.
- Not recommended.

**Option 3: Psionic native on CUDA and Metal (the production path).** These
parts already exist and can be reused:

- The Qwen3.5 hybrid backbone and Gated DeltaNet kernels
  (`psionic-serve/src/qwen35*`, `psionic-backend-cuda/src/clef_prefill.rs`).
- Exact prefix snapshot reuse (`psionic-serve/src/clef/prefix.rs`).
- The gather of option-token rows from `lm_head`, which Clef already does.

These parts are new:

- A renderer for Base Labs' prompt and chat template, ported from the patched
  `qev/prompt.py`.
- A block mask on the full-attention layers inside the suffix.
- A batched fork of the prefix state for each question.
- The `slot_lm` readout, and `calibration.json` if a revision ships one.
- A GGUF conversion with a quantization check against the reference, with a
  target like Clef's 104/104 at BF16 and 98/104 at Q4_K_M
  (`docs/inference/clef-tpu.md`).

Effort and risk:

- Effort: M for elephant-1 on CUDA, then about a week more for mammoth-1 and
  Metal.
- Risk: medium. Mask fidelity is the main risk, and quantization drift is the
  second.

**Option 4: TPU.** The JAX and Pallas worker (`crates/psionic/clef-tpu/`)
behind the Rust door has the same structure. Replace Clef's head with the slot
readout, give splash attention a block mask, and add the fork.

- elephant-1 at BF16 (18.8 GB) fits a v6e-1.
- mammoth-1 needs a v6e-4 or a v5p chip, or int8. Clef 27B ran at 5.1-5.4k
  tok/s per chip on v6e-4 and 9.8k tok/s at int8 on v5p
  (`docs/inference/clef-tpu-data.md`).
- Effort: M.
- Risk: medium.

Build any of these on a scratch VM that carries the run label. Don't touch the
`clef-decisions-*` machines.

### (c) Pylon serving

Pylons pin one decision lane today: Clef-Flash Q4_K_M, 6.49 GB, identified by
the hash of its tensor data, with floors of 16 GB on Apple and 8 GB on NVIDIA
(`crates/pylon/src/release.rs:69-90`). Beacons advertise `<key>:pylon/decision`
on `cj-decision` with that identity (`docs/compute/pylon.md`).

An elephant-1 lane would need these parts:

- A pinned GGUF that we convert and host.
- A tensor identity, plus `verify` limits and a calibration reference.
- A beacon identity such as `elephant-1@gguf-tensors:sha256:...`.
- The gateway's acceptance of that identity.
- The Psionic engine from (b).

A Q4_K_M 9B is about Clef-Flash's size, so the existing floors hold. mammoth-1
at Q4 is about 17 GB. It fits `oa-pylon-gpu-1`'s L4 (24 GB, with about 8.4 GB
in use today) only if it replaces Clef-Flash, and few contributor cards could
hold it.

- Effort: M, after (b).
- Risk: medium. Hosting a converted GGUF is redistribution, so the license
  question in section 5 must be settled first.

### (d) Evaluation in the Gym against Clef

These are the suites and their item counts in `crates/gym/suites/`:

| Suite | Items |
| --- | ---: |
| `chat-router-v5` | 1,076 |
| `external-jevbench-v1` | 231 (never scored by any door) |
| `external-v1` | 200 |
| `support-v2` | 196 |
| `program-selection-v3` | 95 |
| `coder-turns-v2` | 95 |
| `coder-risk-v1` | 40 |

Beyond the suites, add the harness router eval and the chat goldens, and the
file-relevance bench (`docs/inference/clef-jev-relevance-bench.md`).

**Doors to compare.**

- mammoth-1 and elephant-1, through Baseten.
- Clef and Clef-Flash, through our gateway's hosted-only order, signed in as
  the test account.
- Jev 1.14, as the reference.

**What to record.** Record each suite's gate, accuracy, Brier score, and ECE,
plus latency by prompt size, and the refusals over 16k tokens.

About 2,000 requests per door fit in under an hour at Baseten's suggested
rate. Check each suite's provenance before sending it, and send only items
that are public, synthetic, or ours to share.

- Effort: S.
- Risk: low.
- This step decides everything else.

### (e) Post-training, or distilling into Clef

- **Fine-tuning their weights.** The serving patch also patches OneJev's
  `train/sft.py`, and the constraints include `peft`, so LoRA and full
  fine-tuning both have a published path. A LoRA adapter on elephant-1 fits a
  single node. Base Labs' own full fine-tune of the 9B used 128 H100s. Our
  trained lane (`tenancy::training`, `crates/tenancy/src/admission.rs`) already
  gates a trained candidate on Gym evidence.
- **Doing it on Baseten.** Baseten Training can run the OneJev training code
  in a custom container. Nothing documents serving a resulting decision
  checkpoint as a `/v1/systemone` endpoint there, so expect to serve it
  ourselves.
- **Distilling into Clef.** It is possible, using mammoth-1's probabilities as
  soft labels for Clef-Flash's joint head and backbone. But elephant-1 already
  beats Clef 27B on the board. If our Gym agrees, the better move is to adopt
  elephant-1 as the base for our trained lane, not to keep pouring signal into
  Clef. Take distillation labels from self-hosted weights, not from Baseten's
  API, so that the API's terms of service never come into it. We haven't read
  those terms.
- Effort: L.
- Risk: medium. The license has to allow it (see section 5), the run needs
  compute, and a fine-tune can lose general skill.

## 4. Recommendation and sequence

1. **2026-10-21 to 2026-10-22: set up.** Get a Baseten API key from the owner
   and store it in `~/work/.secrets/baseten.env`. Run a smoke test of one
   request per model per question type. Record the response shape, the auth
   header that works, the 16k refusal, and the latency. Ask Baseten to add a
   LICENSE file to both repositories, with the copyright line and the Qwen
   notice.
2. **2026-10-22 to 2026-10-25: measure.** Run section (d) against all five
   doors. Record the results in `crates/gym/results/` with the model names and
   the Hugging Face revisions. Write one measurement note.
3. **2026-10-24 to 2026-10-27: capture a reference.** Run the OneJev reference
   for elephant-1 on a scratch GPU at the pinned revision. Record its answers on
   the same items, so that parity work no longer depends on Baseten after the
   window closes.
4. **After 2026-10-27: decide.** If elephant-1 meets or beats Clef 27B on our
   gates, follow these steps in order:
   1. Build the Psionic native engine for elephant-1 on CUDA.
   2. Add it to the hosted pool beside Clef, shadowed first.
   3. Add a Pylon lane once the license is settled.
   4. Add mammoth-1 as an H100 quality tier.

   If it doesn't, file the numbers and stop. Add a Baseten gateway door only if
   Baseten publishes a price below what our own Clef costs to serve, and only
   as a backup.

## 5. Licensing and attribution

**Facts.**

- Baseten's library pages state "License: MIT" for both models.
- Each Hugging Face card's front matter says `license: other` with
  `license_name: mit`. Neither repository has a LICENSE file, so no copyright
  line is published.
- The Decision Index board's notes call the weights public "under a
  proprietary license".
- Both bases, Qwen3.5-27B and Qwen3.5-9B, are Apache-2.0.
- The serving runtime OneJev is Apache-2.0, and the Decision Index kit is MIT.
- The training data included 25 community datasets and synthetic data from
  other vendors' models. The 9B also used train splits of Decision Index
  benchmarks.

**What we may do (our reading, not legal advice).**

- **Evaluate through the API:** yes, under Baseten's Model API terms.
- **Serve the weights commercially:** MIT allows it, and Apache-2.0 on the
  base allows it with its notices kept. The risk is low, but resolve the
  board's "proprietary" note first.
- **Redistribute to Pylon operators, as weights or a converted GGUF:** MIT
  allows it with the license text and copyright notice included. That notice
  doesn't exist yet. Wait for a LICENSE file or written confirmation, then ship
  it with every artifact.
- **Post-train or distill:** MIT allows modification. Use self-hosted weights
  for labels.

**Attribution.** Attribute the models in any door, Pylon lane, or model card
as "mammoth-1 and elephant-1 by Base Labs (Baseten), MIT; based on Qwen3.5 by
the Qwen team, Apache-2.0". Keep both license texts with converted weights. MIT
grants no trademark rights, so give a derived model our own name, not
"mammoth" or "elephant".

## 6. Issue-ready steps

These are drafts. None is filed.

1. **Smoke-test Baseten's System One endpoint for mammoth-1 and elephant-1.**
   Send one `noul`, one `choice`, and one `score` request per model, with
   `Bearer` and with `Api-Key`. Then send one request above 16k tokens.
   Acceptance: the response shape, auth, refusal, and latency are recorded in a
   short note, and the key is not in the note or the logs.
2. **Let `gym eval --door` authenticate to a remote door.** Add
   `--door-key-env NAME` so the bearer is read from the environment.
   Acceptance: the existing tests pass, a new test covers the key, and the key
   never appears in a record.
3. **Gym: mammoth-1 and elephant-1 against Clef, Clef-Flash, and Jev, before
   2026-10-27.** Run the suites in section (d), plus the router eval, the
   goldens, and file relevance. Acceptance: results are in
   `crates/gym/results/` with revisions, there is one measurement note, and
   only shareable items are sent.
4. **Record an elephant-1 reference with the OneJev bundle.** Use a scratch
   GPU VM with the run label, the pinned revision `5494c13e59`, and the items
   from step 3. Acceptance: a reference answers file exists, and the VM is
   deleted.
5. **Psionic: a native engine for Base Labs' `slot_lm` decision models.** Add
   the prompt renderer, the block-bidirectional full-attention mask, the batched
   prefix fork, the letter-slot readout, and the GGUF conversion. Acceptance:
   top-answer agreement with the reference at BF16 and Q4_K_M, recorded as for
   Clef.
6. **Gateway: an opt-in Baseten door.** Do this only if step 3 justifies it.
   Use `Kind::Baseten`, reached through an explicit order header only, with no
   default order and no shadowing. Acceptance: tests like the Jev door's, and
   the strict answer validation applies.
7. **Pylon: an elephant-1 decision lane.** This is blocked on step 5 and on
   the license. Pin the GGUF, the tensor identity, the floors, the
   verification limits, and the beacon identity. Acceptance: a Pylon serves
   and passes its self-check on an L4.
8. **License: ask Baseten for a LICENSE file in both repositories.** Track the
   answer, then update section 5 of this note.

## Sources

- Baseten library: <https://www.baseten.co/library/mammoth-1/>,
  <https://www.baseten.co/library/elephant-1/>
- Baseten blog:
  <https://www.baseten.co/blog/the-best-decision-models-in-the-world-mammoth-1-and-elephant-1/>
- Base Labs article:
  <https://labs.baseten.co/articles/mammoth-1-and-elephant-1-frontier-decision-models>
- Hugging Face: <https://huggingface.co/baselabs>,
  <https://huggingface.co/api/models?author=baselabs>,
  <https://huggingface.co/baselabs/mammoth-1>,
  <https://huggingface.co/baselabs/elephant-1>
- Decision Index board and kit:
  <https://huggingface.co/spaces/multimodalart/jev-decision-index>,
  <https://github.com/apolinario/decision-index>
- OneJev runtime: <https://github.com/OmniJev/OneJev>
- Clef cards: <https://huggingface.co/Cloudflare/clef>,
  <https://huggingface.co/Cloudflare/clef-flash>
- Baseten docs: <https://docs.baseten.co/development/model-apis/overview>,
  <https://docs.baseten.co/training/overview>
- This repository: `docs/inference/clef-self-host.md`,
  `docs/inference/clef-native.md`, `docs/inference/clef-tpu.md`,
  `docs/inference/clef-tpu-data.md`, `docs/inference/gpu-options.md`,
  `docs/inference/clef-jev-relevance-bench.md`, `docs/inference/subq-snap.md`,
  `docs/compute/pylon.md`, `crates/pylon/src/release.rs`,
  `crates/gateway/src/decision_dispatch.rs`, `crates/gym/src/bin/gym.rs`,
  `.agents/skills/decision-api/SKILL.md`.
