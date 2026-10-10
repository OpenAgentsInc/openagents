# Self-hosting Cloudflare's Clef decision models (2026-10-09)

Cloudflare released two open-weight decision models that answer the same
`POST /v1/systemone` request Jev answers: **Clef-Flash** (9B, post-trained
from Qwen3.5-9B) and **Clef** (27B, post-trained from Qwen3.8-27B). This page
records what runs on our two machines, how to run it, what it measured against
hosted Jev on the web chat router's own requests, and what we should do with
it.

**Short answer.** Clef answers our `crates/jev` client unchanged, and on the
small product-note requests it agrees with Jev 94–97 % of the time. It is not
a drop-in failover for the router's main decision: that request is about
24,500 tokens with two 50+-option questions, which is past what Clef was
trained on (16,384 tokens), past Ollama's request limits, and too slow on our
hardware for the router's 6-second timeout. It also reads our `none` option
differently from Jev. Recommendation: add Clef as an own-capacity door for
decisions that fit its limits, behind an admission check, and slim or split
the router schema before pointing the router at it. Details below.

## What the primary sources say

Checked on 2026-10-09 against the
[Cloudflare blog post](https://blog.cloudflare.com/clef-decision-models),
the Ollama library pages ([clef-flash](https://ollama.com/library/clef-flash),
[clef](https://ollama.com/library/clef)), the Hugging Face model cards
([Cloudflare/clef-flash](https://huggingface.co/Cloudflare/clef-flash),
[Cloudflare/clef](https://huggingface.co/Cloudflare/clef),
[mlx-community/clef-flash-4bit](https://huggingface.co/mlx-community/clef-flash-4bit),
[mlx-community/clef-4bit](https://huggingface.co/mlx-community/clef-4bit),
[ggml-org/Clef-Flash-GGUF](https://huggingface.co/ggml-org/Clef-Flash-GGUF)),
the [SGLang cookbook](https://docs.sglang.io/cookbook/autoregressive/Cloudflare/clef)
and the Workers AI model pages.

| Claim we were given | What the sources say |
| --- | --- |
| Clef-flash 9B, Clef 27B, Jev competitors | Confirmed. Qwen3.5-9B and Qwen3.8-27B backbones plus a small "joint schema head" (about 240 MB) that scores every option of every question in one prefill. No text is generated. Both read images too. |
| Served at `/v1/systemone` | Confirmed for Ollama, SGLang, the MLX loader and llama.cpp. All four describe themselves as "fully Jev/System One compatible". |
| Ollama ≥ 0.35.1 | The library pages say 0.35.1. The model manifests say `requires 0.40.0`, and Ollama 0.34.0 cannot load them. **Use 0.40.0.** |
| `ollama pull clef-flash`, `ollama pull clef` | Confirmed. `clef-flash:latest` is 12 GB (mxfp8 on a Mac), and `clef:latest` is 18 GB (nvfp4 on a Mac). Ollama has no 4-bit Clef-Flash tag. |
| MLX `clef-flash-4bit` (16 GB Mac min), 27B 4-bit (32 GB) | Confirmed: `mlx-community/clef-flash-4bit` (6.2 GB, 16 GB minimum) and `mlx-community/clef-4bit` (16.3 GB, 32 GB minimum). These are not chat models. Only the bundled `clef_mlx.py` runs the head. |
| SGLang recipe for H200 | Confirmed: `lmsysorg/sglang:dev-clef` or nightly `sglang==0.5.22.dev20261009+g37ae292e6f`, BF16, TP=1 on one H200, B200 or B300. |
| 41 GB (flash) / 85 GB (27B) at 64k context, single request | **Not found** in any source above. Treat it as unverified. |
| 64k context | Varies by source. Workers AI lists 65,536 for Clef but **24,576 for Clef-Flash**. Ollama says "64K" in its text but 262,144 in its metadata. The reference encoder, SGLang and the MLX loader cap prompts at **16,384** tokens, the length the head was trained at, and truncate the state to fit. |
| License | **Apache-2.0** on the weights, head and code (HF `license: apache-2.0`, and a LICENSE file in each repo). Commercial use and self-hosting are allowed. The MLX and GGUF conversions are Apache-2.0 as well. |
| (extra) Price on Workers AI | Clef-Flash costs $0.038 per million input tokens and Clef $0.24 per million, with no charge for output. Hosted Jev through OpenRouter cost about $0.042 per million input tokens on our calls ($0.00121 for a 28.9k-token router call). |

## Hardware

| | This Mac | coderos-4080 |
| --- | --- | --- |
| Chip / GPU | Apple M5 Max, 128 GB unified memory | RTX 4080, 16 GB VRAM (CUDA 13.2, driver 595.71), 28 threads, 125 GB RAM |
| Already running | Ollama.app 0.40.0 (brew/app) | `pylon-psionic` (psionic-openai-server, 1.6 GB VRAM), `pylon-provider`, the desk VM, verse, the owner's host service and a user `ollama.service` (0.34.0). Together they use about 3.8 GB of VRAM, leaving **about 11.6 GB free**. |
| Free disk at start | 124 GB | 61 GB |
| Load during the test | Heavy: other agents were building, with load averages of 27–370. Mac latencies below are pessimistic. | Desktop in use. Another agent's `phonelink` cargo builds took the root disk from 52 GB free to 4 GB free during the test (not Clef). |

**What fits where.**

- **Mac:** everything fits, including Ollama `clef-flash` and `clef` together with the MLX 4-bit builds.
- **coderos-4080, Ollama:** won't work there. Its `clef-flash` is 11–12 GB of weights, which does not fit beside the Pylon services in 11.6 GB, and it would break the 50 GB disk floor. nixpkgs ships Ollama 0.32.3, which is too old. The user unit's 0.34.0 is also too old, and we did not touch it.
- **coderos-4080, llama.cpp:** runs. llama.cpp b11538 (Clef support merged in ggml-org/llama.cpp#29831, text only) serves `ggml-org/Clef-Flash-GGUF` Q4_K_M (6.5 GB). The 27B Q4_K_M (17.7–19.2 GB) does not fit fully on the GPU. It runs with 13 of 65 layers on the GPU (see the prefill benchmark).

## How to run it

### Mac, Ollama (simplest; what we'd ship first)

```sh
brew install ollama            # or Ollama.app; needs 0.40.0+
ollama pull clef-flash         # 12 GB
ollama pull clef               # 18 GB (optional)
curl -s localhost:11434/v1/systemone -d '{"model":"clef-flash","state":"Checkout is failing for everyone.",
  "questions":{"urgent":{"type":"noul","instructions":"Is this urgent?"}}}'
```

### Mac, MLX (no request-size limits; slower)

```sh
uv venv -p 3.12 mlxenv && VIRTUAL_ENV=$PWD/mlxenv uv pip install "mlx-vlm>=0.7.4,<0.8" huggingface_hub
mlxenv/bin/hf download mlx-community/clef-flash-4bit --local-dir models/clef-flash-4bit
# read models/clef-flash-4bit/clef_mlx.py first: it is custom code (we did; it only loads weights and serves on loopback)
mlxenv/bin/python -I models/clef-flash-4bit/clef_mlx.py serve --port 18092 \
    --name clef-flash-4bit --max-length 32768 --no-truncate
```

The default `--max-length 16384` truncates the state silently. Our router's
*schema* alone is more than 16k tokens, so at the default length every router
request is refused with 413.

### coderos-4080, llama.cpp CUDA (user-level, no system change)

```sh
mkdir -p ~/clef-test/llama && cd ~/clef-test/llama
for a in llama-b11538-bin-ubuntu-cuda-12.8-x64.tar.gz cudart-llama-b11538-bin-ubuntu-cuda-12.8-x64.tar.gz; do
  curl -sLO https://github.com/ggml-org/llama.cpp/releases/download/b11538/$a && tar xzf $a && rm $a; done
curl -sL -o ../clef-flash-q4.gguf https://huggingface.co/ggml-org/Clef-Flash-GGUF/resolve/main/Clef-Flash-Q4_K_M.gguf
cd llama-b11538
LD_LIBRARY_PATH=$PWD:../cudart-llama-b11538-bin-ubuntu-cuda-12.8-x64:/run/opengl-driver/lib \
  ./llama-server -m ../../clef-flash-q4.gguf --host 127.0.0.1 --port 18091 -ngl 99 -np 1 -fa on \
  --alias clef-flash -c 17408 -b 17408 -ub 17408
```

The prebuilt Ubuntu binary runs on NixOS as-is: nix-ld is present and the
driver libraries are in `/run/opengl-driver/lib`. A decision model must fit
its whole prompt in one physical batch. With `-ub 512`, every request over
512 tokens fails with "input is too large to process". A 17k batch takes about
10.2 GB of VRAM in total. A 32k batch asks for an 8.75 GB compute buffer on
top of the weights and does not fit beside the Pylon services. Don't use
Ollama's `hf.co/ggml-org/Clef-Flash-GGUF` pull: Ollama 0.40's bundled llama.cpp
refuses it with `unknown model architecture: 'clef'`.

### Pointing our client at it

`crates/jev` needs no change. Use the environment set from
`crates/jev/README.md` ("Local Kev and Lev servers"):

```sh
TYPESAFE_BASE_URL=http://127.0.0.1:11434 TYPESAFE_API_KEY=local TYPESAFE_DEFAULT_MODEL=clef-flash \
  cargo run -p jev --example quickstart     # prints "billing", against Ollama and against MLX
```

You can also call `jev::Config::local("http://127.0.0.1:11434", "clef-flash")`.

## API differences from TypeSafe's System One

These were measured, not taken from documentation.

| Area | TypeSafe Jev | Clef servers |
| --- | --- | --- |
| Choice options | ≤ 255 (`crates/jev` checks) | **Ollama refuses outside 2–26**: `question "answer": criteria must contain 2–26 candidates`. Our router's `answer` (52) and `cli_group` (55) questions are refused. llama.cpp and MLX accept them. |
| Request size | Jev took our 28.9k-token router request | **Ollama refuses bodies over 64 KiB** (`text and schema must not exceed 64 KiB`), and our router request is about 100 KB. The reference, SGLang and MLX cap the prompt at 16,384 tokens and truncate the *state*. llama.cpp fails when the prompt is larger than the physical batch. |
| Structured `instructions` / criteria objects | `{question, context, focus}`, `{what, not_for, examples}` | Accepted by all three. They are rendered as JSON text into the prompt. |
| `confidence` | Jev's own | Three different formulas: MLX reports the top probability, Ollama a normalized-entropy value, and llama.cpp something else again. Both the Ollama page and SGLang say not to read it as accuracy. **Compare probabilities, never `confidence`.** |
| `usage` | `input_tokens`, `output_tokens` (≈1.6k), `cost` | `input_tokens`, with `output_tokens` 0 (or 3 in the Ollama docs) and no `cost`. Ollama adds `prompt_eval_cached_count`, and MLX adds `latency_ms`. |
| `model` echo | `typesafe/jev-1.13-…` | llama.cpp echoes the file path unless you pass `--alias`. |
| Choice `probabilities` order | request order | llama.cpp sorts the keys alphabetically. Read them by key. |
| `GET /v1/models` | TypeSafe's `{"models": …}` | OpenAI's `{"object":"list","data":…}`. `client.models().list()` fails with `ResponseValidation`, but decisions work. |
| Prompt cache | none visible | Ollama reuses a cached prompt (a repeat takes 0.2–0.25 s), so benchmark with a unique state. llama.cpp does not cache Clef: a repeat costs the same as a new request. |
| Token counts | 28,905 (router) / 3,306 (note check) | 24,466 / 3,195 for the same requests (Qwen tokenizer). |

## Measurements

**Method.** We ran `chat-goldens router` (built from `main`) over 47 phrasings
covering one golden from each of the 15 flows in
`bench/web-chat/goldens-v1.json`. `TYPESAFE_BASE_URL` pointed at a loopback
recorder that forwarded each call to hosted Jev on OpenRouter
(`typesafe/jev-1.13`) and saved every request and answer. That gave 75 real
requests:

- **47 router decisions.** Each asks 13 questions (route 22 options, answer 52, cli_group 55, risk, action and others) and runs about 24.5k Clef tokens.
- **28 product-note checks.** Each asks `relevant_1..8` nouls plus an answer choice and runs about 3.2k tokens.

We replayed the same bodies unchanged against each Clef server. "Agreement" is
the share of questions where Clef's top answer matches Jev's: the argmax for a
choice or score, and the same side of 0.5 for a noul. "Split" means one
question per request, a workaround for the size limits. For latency, every
request starts with a fresh nonce so no prompt cache is hit. The results are
the median of 5 runs after a warmup, with three questions in each request.

### Latency (uncached) and memory

| Server | ~1k tokens | ~4k | ~16k | Memory |
| --- | --- | --- | --- | --- |
| coderos-4080, llama.cpp, Clef-Flash Q4_K_M | **0.19 s** | **0.65 s** | **3.25 s** | 6.0 GB VRAM at a 512 batch; ≈10.2 GB at a 17k batch |
| Mac, MLX `clef-flash-4bit` | 1.2 s | 3.0 s | 14.3 s | 13 GB footprint; 33 GB peak after 24k-token router requests |
| Mac, Ollama `clef-flash` (mxfp8) | 1.25 s | 4.6 s | 19.2 s | 13 GB at load; 21 GB after 16k |
| Mac, Ollama `clef` 27B (nvfp4) | 3.9 s | 14.3 s | 60.3 s | 26–27 GB |
| Hosted Jev (OpenRouter), for reference | — | 1.1 s median (note checks) | — | 1.8 s median for the 29k router call |

The MLX card itself reports 0.31 s at 1k and 7.0 s at 16k for `clef-flash-4bit`
on an idle M5 Max, so our Mac numbers are 2–4× slow because of the build load.
The 4080 numbers were taken with the desktop and Pylon running. The
note-check replays there (distinct requests, about 3.2k tokens) took a median
of 0.53 s.

### Agreement with hosted Jev on our requests

| Server, mode | Requests answered | Note checks: relevance nouls | Note checks: answer | Router: route | Router: answer | Router: risk | All questions |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 4080 llama.cpp flash Q4, whole | 28/75 (router too big) | 214/224 (96 %) | 25/28 | — | — | — | 95 % |
| Mac Ollama flash, whole | 28/75 (router 413) | 212/224 (95 %) | 25/28 | — | — | — | 94 % |
| Mac Ollama **clef 27B**, whole | 28/75 (router 413) | **219/224 (98 %)** | 25/28 | — | — | — | **97 %** |
| Mac MLX flash 4-bit, whole (max_length 32k) | 75/75 | 214/224 | 25/28 | **34/47 (72 %)** | 27/47 (57 %) | **0/47** | 80 % |
| 4080 llama.cpp flash Q4, split, first 17 router | 17/17 | — | — | 10/17 | 14/17 | 3/17 | 77 % |
| Mac Ollama flash, split, 12 router | 12/12 (answer and cli_group refused) | — | — | 8/12 | refused | 3/12 | 78 % |
| Mac Ollama **clef 27B**, split, same 12 router | 12/12 (answer and cli_group refused) | — | — | **12/12** | refused | 0/12 | 77 % |

Noul probabilities sit close to Jev's: a median absolute difference of
0.03–0.05 and a mean of 0.07–0.11. On the router request, Clef-Flash's
choices are diffuse: its top route probability is 0.15–0.34, where Jev is near
1.0. We read that as the 24k-token prompt running past the head's 16k
training length.

**The `none` problem.** Our `risk` question offers `ok`, four hazards and
`none` ("None of these describes the message"). Jev reads `none` as "the
question doesn't apply" and answers `ok` at about 0.98. Clef answers `none` on
almost every message: 47/47 whole requests and 12/12 split requests for the
27B. That breaks the safety gate silently, because the router would act as if
no hazard check had run. Any Clef door needs the schema to drop `none` from
questions that have an explicit "nothing wrong" option, or a re-calibrated
reading. The same pattern shows on `answer`, where Clef never picks `none` and
always serves some prepared answer.

**End to end.** `chat-goldens router` pointed straight at the MLX server
timed out on all 10 cases: the router gives a decision 6 s, and a 24k-token
request takes about 25 s on this loaded Mac. The crate itself parsed every
Clef answer it received (the quickstart and the replay bodies).

## Prefill benchmark by prompt size (second pass, 2026-10-09 evening)

A decision model generates no tokens, so the total latency of a call is the
time to its answer, and prefill speed is input tokens ÷ latency. Every call
below uses a fresh random state (a random number plus random words about a
failed checkout) and the same two questions: an `urgent` noul and a four-way
`area` choice. p50 and p90 are over 5 runs per size (8 for the 4080
Clef-Flash run), after one warmup call. Token counts are llama.cpp's
`usage.input_tokens`.

**Setup on coderos-4080.** llama.cpp **b11539**, the newest release on
2026-10-09. The only change since b11538 is a vendored JSON patch, so Clef
support is the same as in #29831. Open PRs touch Clef only for Hexagon (#30143) and a
`--decision-type` override for GGUFs that lack metadata (#30113); neither
matters for the official GGUFs. The only decision route is
**`POST /v1/systemone`**: `/systemone` and `/v1/decisions` return 404. The
response is `{"answers": {id: {"type", "noul" | "choice", "probabilities",
"confidence"}}, "usage": {"input_tokens", "output_tokens": 0}}`. The README
documents up to 255 options per choice for Clef. Pylon, the desk VM and the
host service were left running, so about 11.7 GB of VRAM was free.

### coderos-4080, Clef-Flash Q4_K_M, fully on the GPU

`ggml-org/Clef-Flash-GGUF` `Clef-Flash-Q4_K_M.gguf` (6.5 GB):
`-ngl 99 -np 1 -fa on -c 17408 -b 17408 -ub 17408`. The llama-server
process uses **10.3 GB of VRAM**, which puts the card at 14.3 of 16 GB.

| Input tokens | p50 | p90 | Prefill tok/s (p50) |
| ---: | ---: | ---: | ---: |
| 414 | 0.07 s | 0.08 s | 5,600 |
| 964 | 0.16 s | 0.16 s | 6,050 |
| 3,064 | 0.51 s | 0.52 s | 5,960 |
| 5,864 | 1.04 s | 1.04 s | 5,640 |
| 11,764 | 2.39 s | 2.40 s | 4,920 |
| 15,764 | 3.47 s | 3.52 s | 4,540 |

The 267-token sanity request takes 0.05 s. A first 5-run pass, taken while
the desktop was busy, had the same p50s but p90s up to 2.2× higher
(7.6 s at 15.8k tokens): other GPU work on the card shows up in the tail.

### coderos-4080, Clef 27B Q4_K_M, partial offload

`bartowski/Cloudflare_clef-GGUF` `Cloudflare_clef-Q4_K_M.gguf` (17.65 GB;
bartowski keeps the decision head at Q8_0). `ggml-org/Clef-GGUF` also ships a
Q4_K_M, at 19.2 GB. Both are too big for the ~11.7 GB free. The run used
`-fitt 2048 -c 17408 -b 17408 -ub 17408`, so llama.cpp's own fitter picks the
layer count and leaves a 2 GB margin.

**Offload split.** 13 of 65 layers are on the GPU: 12 repeating layers plus
the output layer.

- **GPU:** 3.97 GB of weights and a 5.6 GB compute buffer for the
  17k-token batch, about 10.0–10.3 GB in all. The card sits at 14.2 of 16 GB.
- **CPU:** 13.9 GB of mapped weights and a 1.9 GB pinned host compute buffer.
  The process RSS is about 31 GB, which counts the page cache of the mapped
  file. The box's RAM is 125 GB.

Most of the space on the card goes to the compute buffer, because the whole
prompt must fit in one batch. llama.cpp's op offload then streams the
CPU-resident weights to the GPU for each large batch. Prefill therefore costs
a fixed ~1 s of PCIe transfer and then runs at about 1,300 tok/s.

| Input tokens | p50 | p90 | Prefill tok/s (p50) |
| ---: | ---: | ---: | ---: |
| 414 | 1.20 s | 1.22 s | 345 |
| 964 | 1.48 s | 1.50 s | 650 |
| 3,064 | 2.59 s | 2.61 s | 1,180 |
| 5,864 | 4.27 s | 4.31 s | 1,370 |
| 11,764 | 8.73 s | 8.75 s | 1,350 |
| 15,764 | 11.92 s | 12.05 s | 1,320 |

An earlier 3-run pass taken under heavier load measured 1.25, 2.11, 3.73, 4.80,
12.50 and 12.00 s. Lower quants would fit more layers but leave the compute
buffer the same size: IQ3_M is 13.8 GB and Q3_K_M is 14.5 GB. Bartowski also
warns that sub-4-bit quants are static, without an imatrix, because a decision
model cannot be calibrated. So the 27B on this card is about 12× slower than
Clef-Flash at 3k tokens and about 3.4× slower at 16k, and only usable for
decisions that are off the hot path.

### No prompt cache for Clef in llama.cpp

Sending the same state again costs the same as a new one: 0.52 s at 3k and
2.39 s at 11.8k tokens for Clef-Flash, and 2.5–4.7 s and 8.4–9.7 s for the
27B, which is within its noise. The Clef head needs the hidden state of
every prompt token, and the prompt is evaluated in one batch, so there is no
prefix to reuse. An earlier version of this page reported 17–80 ms "cached" repeats; those were small
requests, which are that fast anyway. Ollama on the Mac does cache:
0.2–0.25 s for a repeat.

### Mac (M5 Max, Ollama 0.40.0), same script

| Input tokens | Clef-Flash (12 GB), load avg 60–90 | Clef-Flash, load avg 14–17 | Clef 27B (18 GB), load avg 14–17 |
| ---: | ---: | ---: | ---: |
| 414 | 0.49 s | 0.54 s (773 tok/s) | 1.33 s (310 tok/s) |
| 964 | 1.12 s | 1.01 s (954 tok/s) | 3.20 s (302 tok/s) |
| 3,064 | 3.61 s | 3.05 s (1,005 tok/s) | 9.80 s (313 tok/s) |
| 5,864 | 6.68 s (≈850 tok/s) | 5.60 s (1,047 tok/s) | 18.34 s (320 tok/s) |
| ~11k | refused: over Ollama's 64 KiB body limit | refused | refused |

On the Mac, a repeated identical state answers in 0.2–0.25 s from Ollama's
cache. The first call after a model load (cold) takes 2.7 s.

### Sanity check: the urgent/area example

| State | Model | `urgent` (P true) | `area` |
| --- | --- | --- | --- |
| "Checkout is failing for every customer since the 3pm deploy; card payments return a 500 error." | 4080 Clef-Flash Q4 | 0.969 | payments 0.969 (other 0.019, auth 0.006, search 0.005) |
| same | 4080 Clef 27B Q4 | 0.995 | payments 0.982 (other 0.010, auth 0.004, search 0.003) |
| "Could you update the footer copyright year on the search results page when you get a chance?" | 4080 Clef-Flash Q4 | 0.016 | search 0.950 (other 0.037) |
| same | 4080 Clef 27B Q4 | 0.006 | search 0.929 (other 0.060) |

Both models give sensible, confident answers. The 27B is a little sharper
on urgency.

### Summary

| | Clef-Flash 4080 | Clef 27B 4080 (13/65 layers on GPU) | Clef-Flash Mac | Clef 27B Mac |
| --- | --- | --- | --- | --- |
| ~3k tokens | 0.51 s | 2.6 s | 3.1 s | 9.8 s |
| ~16k tokens | 3.5 s | 11.9 s | refused (Ollama) | refused (Ollama) |
| Prefill | 4.5–6k tok/s | ~1.3k tok/s after ~1 s fixed | ~1k tok/s | ~310 tok/s |

For anything latency-bound, Clef-Flash on the 4080 is the door to use. The
27B on the 4080 beats the 27B on the Mac by 3.8× at 3k tokens and can take
16k-token prompts, which Ollama refuses. It still costs about 10 GB of VRAM
for a 17k-token batch, the same as Clef-Flash. Results and logs are on the
box in `~/clef-test/results-2026-10-09b/`. The model files were deleted.

## Recommendation

1. **Yes to Clef as an own-capacity door, scoped by admission.** Add a Clef door to the judge failover chain (`crates/jev/src/doors.rs`: gateway → OpenRouter → TypeSafe → **local Clef**). It should take a decision only when the request fits:
   - ≤ 26 options per choice (Ollama's limit)
   - ≤ 64 KiB of state plus schema
   - ≤ 16,384 Clef tokens
   - no `none`-style escape option on a gating question

   Everything else should fail over past it, the way a 400 or 413 already does.

   Today that admits the product-note checks: 94–98 % agreement, about 0.5 s on the 4080, and $0 per call. It is also a real last resort when every hosted door is down, which no current door gives us.
2. **Not yet for the router's main decision.** First slim the router schema or split it into smaller requests:
   - move `answer` (52) and `cli_group` (55) into a second, conditional request, or into the product-note embeddings stage
   - keep each request under 16k tokens and 26 options per choice
   - fix the `risk` / `none` wording

   Then re-measure. On the split route question, Clef 27B already agreed 12/12. Speed decides the rest. The router's 6 s budget needs the 4080 (0.2–3 s per request) or a bigger GPU, not the Mac.
3. **Machine choice.** coderos-4080 with llama.cpp Q4 is the fastest box we have: about 10× the loaded Mac. Running it next to `pylon-psionic` leaves only about 2 GB of VRAM spare at a 17k batch, so it should be a managed service that unloads when Pylon needs the GPU. The Mac (Ollama) is the right place for development and for the 27B. Clef 27B on the 4080 runs only with partial offload, at about 2.6 s for 3k tokens and 12 s for 16k (see the prefill benchmark).
4. **Pylon / psionic angle.** psionic already serves Qwen3.5 on CUDA on this box (`crates/psionic/crates/psionic-serve/src/qwen35.rs` is what `pylon-psionic` runs) and has Qwen3.8 GGUF conversion work (`psionic-models/src/qwen38_gguf_*`). Serving Clef in psionic means three additions:
   - a prefill-only path that returns the final hidden states
   - the joint head: a small transformer over the question and option spans, about 240 MB in bf16, with the same span marking llama.cpp added in #29831 (`llama_batch_ext_set_decision_order`)
   - a `/v1/systemone` route that keeps our admission rules

   That would let a Pylon provider sell decisions on the network and keep the door on our own stack instead of Ollama. Build it after the door has proved itself on Ollama and llama.cpp, using their outputs as parity fixtures. The MLX card's parity method (top answer agrees, max |Δp|) is the bar to meet.

## What was left running

All test servers are stopped: the llama-server on the 4080, the MLX server,
and the recorder. Ollama.app on the Mac stays running as it was before, with
no Clef model loaded. `clef-flash` (12 GB) stays pulled on the Mac; remove it
with `ollama rm clef-flash`. The 27B and the MLX copy were deleted for disk
space. On the 4080, everything under `~/clef-test` except about 6 MB of logs
was deleted. The user `ollama.service` (0.34.0), the Pylon services, the
desk VM and the host service were not touched.
