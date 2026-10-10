# File-relevance triage: Clef vs Jev decision throughput (2026-10-09)

The question measured here is the one Coder asks when it triages an issue:
**"given a GitHub issue and one file's contents, is this file relevant to
solving this issue?"** It is asked as a System One `noul` against every Clef
build we can run (Clef-Flash 9B and Clef 27B, on Ollama and llama.cpp, on this
Mac and on coderos-4080) and against hosted Jev at `api.typesafe.ai`.

Setup and API notes for Clef are in [clef-self-host.md](clef-self-host.md).
The native Psionic plan is in [clef-native.md](clef-native.md).

## Headline

Decisions per second, after warmup. One decision means one file judged.
Sequential means one client sending one request after another. Quality is F1
at threshold 0.5 against ground truth on 56 labeled files. Every row answered
all of its requests unless the row says otherwise.

| Backend | Sequential, one file per request | Sequential, one request per issue (batch) | Best concurrent, one file per request | p50 latency per file request | F1 per file / batch |
| --- | ---: | ---: | ---: | ---: | --- |
| **Jev** (hosted TypeSafe, `jev-1.13.0`) | 3.4–3.6 | **18.1** | **82** at 32 clients (no 429) | 0.27 s | 0.78 / **0.89** |
| coderos-4080 llama.cpp Clef-Flash Q4_K_M, all on GPU | 2.4 | 3.4 | 3.9 at 2–4 clients | 0.35 s | 0.65 / 0.44 |
| coderos-4080 Ollama Clef-Flash Q8_0, 24/33 layers on GPU | 0.65 | 1.8 | 1.16 | 1.38 s | 0.64 / 0.44 |
| coderos-4080 Ollama Clef 27B Q4_K_M, 16/65 layers on GPU | 0.28 | 0.62 | 0.35 | 3.6 s | 0.81 / 0.88 |
| Mac Ollama Clef-Flash (MLX, mxfp8) | 0.84–0.90 | 0.91 | 0.92 (serialized) | 1.1–1.2 s | 0.67 / 0.44 |
| Mac Ollama Clef 27B (MLX, nvfp4) | 0.28 | 0.28 | 0.27 (serialized) | 3.5 s | 0.83 / 0.85 |
| Mac llama.cpp Clef-Flash Q4_K_M (Metal) | 0.64–0.66 | 0.70 | 0.65 (serialized) | 1.5 s | 0.65 / 0.44 |
| Mac llama.cpp Clef 27B Q4_K_M (Metal) | 0.19 | 0.19 | 0.16 (2 of 72 timed out at 4 clients) | 5.2 s | 0.81 / 0.87 |

What the table says:

- **Jev is the fastest and the most accurate.** Batched, it is also the cheapest per decision: $0.050 per 1,000 decisions batched and $0.068 per file. That is TypeSafe's list price of $0.042 per million input tokens (`crates/jev/src/nip_dec.rs`); its responses report tokens, not cost.
- **Clef 27B matches Jev on quality** (batch F1 0.85–0.88, AUC 0.96 vs Jev's 0.94). It is 30–60× slower on our hardware.
- **Clef-Flash ranks well but is under-confident.** Its AUC is 0.85–0.91, but at threshold 0.5 its recall is only 0.29–0.50. At threshold 0.15–0.2 its F1 is 0.75–0.86. Those thresholds were tuned on the same 56 files, so treat them as a hint, not a calibrated setting.
- **The fastest own-capacity door is llama.cpp on the 4080.** It is about 4× Ollama on the same card, because Ollama ships a bigger quant that does not fit beside the Pylon services (see "Why the backends differ").

## Method

**Dataset.** The dataset is built by `scripts/bench/clef-relevance-dataset.py`
and kept in a scratch directory, because it holds file contents. It has 10
closed `OpenAgentsInc/openagents` issues, each fixed by a commit whose subject
carries `(#N)`: #11123, #11117, #10998, #10295, #10179, #10283, #10177,
#10206, #10201 and #10170.

- **State:** the issue title and body.
- **Relevant files:** the `.rs` files the fixing commit changed (2–4 per issue).
- **Not-relevant files:** the same number of `.rs` files from the same directory, or else the same crate, that no commit mentioning `#N` touched. They are chosen at random with seed 11 and must be larger than 400 bytes.

Contents are read at the fix commit's parent and **cut to 4,096 bytes**. 65 of
the 72 files were cut (median full size 25.9 KB), so the model mostly sees
imports, doc comments and the first items. That gives 28 relevant and 28
not-relevant files, 56 labeled decisions in all.

There are also two **open** issues with no ground truth, 8 candidate files
each: #11190 (door quota pooled per tenant; candidates from
`crates/gateway/src`) and #11159 (`/coder/*` and `security.txt` are not owned
routes; candidates from `crates/openagents-web/src`). That makes 72 decisions
per pass.

**Requests.** The bench is `scripts/bench/clef-relevance-bench.py`, Python
stdlib only. Its JSON is exactly `crates/jev`'s wire shape
(`crates/jev/tests/fixtures/systemone-*.json`).

- **Per file:** `state` = `"RUN <nonce>\n\nISSUE #N: title\n\nbody\n\nFILE: path\n```rust\n…```"`, with one question: `{"relevant": {"type": "noul", "instructions": "Is this file relevant to solving the issue?"}}`. That is about 1,526 Clef tokens (1,626 Jev tokens) per request.
- **Batch:** one request per issue. The state holds the issue and then every candidate as `FILE fK: path` plus its contents. There is one noul per file, keyed `f1…fK`: "Is file fK (path) relevant to solving the issue?". That is about 7,550 tokens per request (max 10,075 tokens, 36.5 KB).

The nonce at the start of the state is fresh every run, so no run reuses a
previous run's cache. Ollama reported no `prompt_eval_cached_count` on any of
these calls. llama.cpp keeps no prefix cache for Clef ([clef-self-host.md](clef-self-host.md)).

**Runs.**

| Run | Requests per backend | Warmup (discarded) |
| --- | --- | --- |
| Sequential, one file per request | 72 | 3 calls |
| Batch | 2 passes of 12 = 24 requests (144 decisions) | 2 calls |
| Concurrent, one file per request, at 2, 4 and 8 clients | 72 each | 2 calls |

Decisions/s is the number of decisions divided by the wall time of the
measured phase. A decision model generates no tokens. The response comes when
prefill and the head are done, so **time to first token is the total
latency**. Clef reports `output_tokens: 0`. Jev reports 22 output tokens per
noul request and 106 per batch, which it does not bill. "Prefill tok/s" is
input tokens ÷ request latency. For coderos it includes the ssh-tunnel round
trip of about 0.09–0.13 s, measured with a 146-token request.

Quality takes one probability per (issue, file): the mean over every run of
that prompt form. Repeat-to-repeat spread was at most 0.011. Generate the
tables with `scripts/bench/clef-relevance-report.py`.

**Machine load.** Other agents were building on both machines throughout. The
1-minute load average is recorded before and after every run in the full
table. The Mac ranged from 10 to 271. Runs that repeated at lower load
changed by 5–10 %, not more: flash sequential was 0.84/s at load 23 and
0.90/s at load 15. The prefill section of [clef-self-host.md](clef-self-host.md)
has idle-ish per-size numbers.

## Full throughput table

The "Load" column is the 1-minute load average before → after the run, on
this Mac. coderos loads are listed under "Setups". Jev $ is per 1,000
decisions at $0.042 per million input tokens.

| Backend | Mode | Load | OK | Decisions/s | p50 s | p90 s | p99 s | Input tok/req | Prefill tok/s | Jev $/1k |
|---|---|---|---|---|---|---|---|---|---|---|
| Mac ollama-flash | seq | 23→22 | 72/72 | 0.84 | 1.19 | 1.44 | 1.58 | 1526 | 1258 | |
| Mac ollama-flash | seq (repeat) | 15→10 | 72/72 | 0.90 | 1.09 | 1.30 | 1.49 | 1523 | 1324 | |
| Mac ollama-flash | seq (paired with llama.cpp) | 21→17 | 72/72 | 0.87 | 1.13 | 1.42 | 1.58 | 1524 | 1324 | |
| Mac ollama-flash | batch | 21→219 | 24/24 | 0.91 | 6.83 | 9.04 | 9.17 | 7550 | 1131 | |
| Mac ollama-flash | batch (repeat) | 10→22 | 12/12 | 0.92 | 6.95 | 8.53 | 8.56 | 7551 | 1155 | |
| Mac ollama-flash | conc 2 / 4 / 8 | 208–271 | 72/72 each | 0.86 / 0.90 / 0.92 | 2.36 / 4.35 / 8.50 | | | 1523 | | |
| Mac ollama-flash, `OLLAMA_NUM_PARALLEL=4` | seq | 23→103 | 72/72 | 1.11 | 0.88 | 1.10 | 1.29 | 1523 | 1655 | |
| Mac ollama-flash, `OLLAMA_NUM_PARALLEL=4` | conc 2 / 4 / 8 | 103–265 | 72/72 each | 1.16 / 1.22 / 1.19 | 1.74 / 3.30 / 6.56 | | | 1525 | | |
| Mac ollama-clef (27B) | seq | 177→32 | 72/72 | 0.28 | 3.53 | 4.32 | 5.01 | 1525 | 424 | |
| Mac ollama-clef (27B) | batch | 34→31 | 24/24 | 0.28 | 21.5 | 28.8 | 29.3 | 7551 | 354 | |
| Mac ollama-clef (27B) | conc 2 / 4 / 8 | 13–29 | 72/72 each | 0.26 / 0.26 / 0.27 | 7.6 / 15.0 / 29.3 | | | 1525 | | |
| Mac llama.cpp flash | seq | 108→99 | 72/72 | 0.66 | 1.48 | 1.76 | 1.87 | 1524 | 1004 | |
| Mac llama.cpp flash | seq (paired) | 19→20 | 72/72 | 0.64 | 1.56 | 1.80 | 2.00 | 1526 | 983 | |
| Mac llama.cpp flash | batch | 101→34 | 24/24 | 0.70 | 8.52 | 11.9 | 13.0 | 7549 | 898 | |
| Mac llama.cpp flash | conc 2 / 4 | 19–34 | 72/72 each | 0.65 / 0.65 | 3.09 / 3.08 | 3.53 / 3.60 | 3.68 / **109** | 1525 | | |
| Mac llama.cpp 27B | seq | 18→42 | 72/72 | 0.19 | 5.22 | 6.03 | 6.67 | 1524 | 292 | |
| Mac llama.cpp 27B | batch | 93→55 | 24/24 | 0.19 | 30.3 | 43.8 | 45.7 | 7549 | 260 | |
| Mac llama.cpp 27B | conc 2 / 4 | 35–53 | 72/72, **70/72** (2 timed out at 300 s) | 0.15 / 0.16 | 11.2 / 11.8 | 19.0 / 15.9 | 26.4 / 120 | 1522 | | |
| coderos Ollama flash (Q8_0, `num_gpu` 24) | seq | 70→30 | 72/72 | 0.65 | 1.38 | 2.18 | 2.40 | 1525 | 1079 | |
| coderos Ollama flash | batch | 28→36 | 24/24 | 1.80 | 3.35 | 4.37 | 5.41 | 7548 | 2318 | |
| coderos Ollama flash | conc 2 / 4 / 8 | 18–39 | 72/72 each | 1.14 / 1.16 / 1.04 | 1.85 / 3.60 / 7.19 | | | 1524 | | |
| coderos llama.cpp flash (Q4_K_M, `-np 4`) | seq | 19→17 | 72/72 | **2.37** | **0.35** | 0.48 | 1.29 | 1526 | 4407 | |
| coderos llama.cpp flash | batch | 17→13 | 24/24 | **3.43** | 1.52 | 2.12 | 6.86 | 7549 | 4921 | |
| coderos llama.cpp flash | conc 2 / 4 / 8 | 13 | 72/72 each | 3.88 / 3.91 / 3.83 | 0.51 / 0.52 / 1.32 | 0.59 / 0.84 / 2.62 | 0.71 / 5.49 / 10.5 | 1526 | | |
| coderos Ollama 27B (Q4_K_M, `num_gpu` 16) | seq | 11→24 | 72/72 | 0.28 | 3.61 | 3.86 | 4.05 | 1524 | 426 | |
| coderos Ollama 27B | batch | 22→255 | 24/24 | 0.62 | 9.70 | 12.4 | 12.9 | 7550 | 799 | |
| coderos Ollama 27B | conc 2 / 4 | 33–249 | 72/72 each | 0.34 / 0.35 | 5.75 / 11.2 | | | 1526 | | |
| Jev | seq | 23→23 | 72/72 | 3.41 | 0.27 | 0.34 | 0.40 | 1626 | 6040 | 0.068 |
| Jev | seq (repeat) | 55→61 | 72/72 | 3.58 | 0.28 | 0.31 | 0.35 | 1628 | 5896 | 0.068 |
| Jev | batch | 23→22 | 24/24 | **18.05** | 0.33 | 0.38 | 0.43 | 7135 | 21444 | **0.050** |
| Jev | conc 2 / 4 / 8 | 22–26 | 72/72 each | 6.6 / 12.3 / 28.1 | 0.29 / 0.27 / 0.27 | | 0.46 / 0.80 / 0.36 | 1626 | | 0.068 |
| Jev | conc 16 / 32 (216 requests each) | 52–55 | 216/216 each | 44.3 / **82.1** | 0.33 / 0.34 | 0.45 / 0.43 | 0.55 / 0.50 | 1626 | | 0.068 |

The coderos llama.cpp row was the owner's question. It has to be read against
the tunnel. Sequential p50 is 0.35 s for about 1,526 tokens. The
[earlier prefill benchmark](clef-self-host.md) on the same card, with no
tunnel, measured 0.16 s at 964 tokens and 0.51 s at 3,064. That predicts
about 0.26 s here, and the remaining ≈ 0.09 s is the measured round trip. So
the card prefills at the same 5.6–6k tok/s as before. The 4.4k tok/s in the
table is lower only because it includes the network.

**Jev rate limits.** No 429 at 32 concurrent clients (216 requests in 2.6 s),
and no rate-limit headers in the responses (only `x-typesafe-request-id`). We
did not push further.

## Quality

The tables below cover 56 labeled files, with one probability per file (the
mean over repeats). "Prompt" is per-file or batch. Precision, recall, F1 and
accuracy are at threshold 0.5.

| Backend | Prompt | Precision | Recall | F1 | Accuracy | Mean p, relevant | Mean p, not | Brier | AUC |
|---|---|---|---|---|---|---|---|---|---|
| Jev | per-file | 0.74 | 0.82 | 0.78 | 0.77 | 0.80 | 0.35 | 0.134 | 0.91 |
| Jev | batch | 0.92 | 0.86 | **0.89** | **0.89** | 0.75 | 0.24 | 0.106 | 0.94 |
| Mac Ollama Clef 27B | per-file | 0.80 | 0.86 | 0.83 | 0.82 | 0.77 | 0.27 | 0.128 | 0.91 |
| Mac Ollama Clef 27B | batch | 0.88 | 0.82 | 0.85 | 0.86 | 0.77 | 0.16 | 0.086 | 0.96 |
| Mac llama.cpp Clef 27B | per-file | 0.77 | 0.86 | 0.81 | 0.80 | 0.78 | 0.30 | 0.134 | 0.90 |
| Mac llama.cpp Clef 27B | batch | 0.89 | 0.86 | 0.87 | 0.88 | 0.78 | 0.17 | **0.084** | **0.96** |
| coderos Ollama Clef 27B | per-file | 0.77 | 0.86 | 0.81 | 0.80 | 0.78 | 0.30 | 0.136 | 0.89 |
| coderos Ollama Clef 27B | batch | 0.84 | 0.93 | 0.88 | 0.88 | 0.80 | 0.19 | 0.085 | 0.96 |
| Mac Ollama Clef-Flash | per-file | 1.00 | 0.50 | 0.67 | 0.75 | 0.50 | 0.17 | 0.182 | 0.85 |
| Mac Ollama Clef-Flash | batch | 1.00 | 0.29 | 0.44 | 0.64 | 0.39 | 0.07 | 0.230 | 0.91 |
| Mac llama.cpp Clef-Flash | per-file | 0.93 | 0.50 | 0.65 | 0.73 | 0.49 | 0.18 | 0.188 | 0.84 |
| Mac llama.cpp Clef-Flash | batch | 1.00 | 0.29 | 0.44 | 0.64 | 0.38 | 0.07 | 0.236 | 0.90 |
| coderos Ollama Clef-Flash | per-file | 0.88 | 0.50 | 0.64 | 0.71 | 0.51 | 0.18 | 0.183 | 0.85 |
| coderos Ollama Clef-Flash | batch | 1.00 | 0.29 | 0.44 | 0.64 | 0.38 | 0.07 | 0.236 | 0.91 |
| coderos llama.cpp Clef-Flash | per-file | 0.93 | 0.50 | 0.65 | 0.73 | 0.49 | 0.17 | 0.189 | 0.85 |
| coderos llama.cpp Clef-Flash | batch | 1.00 | 0.29 | 0.44 | 0.64 | 0.37 | 0.07 | 0.238 | 0.90 |

**Threshold sweep.** F1 at each threshold, with precision/recall in brackets.
The thresholds are tuned in-sample on 56 files.

| Backend | Prompt | 0.15 | 0.2 | 0.3 | 0.5 | 0.7 |
|---|---|---|---|---|---|---|
| Mac Ollama Clef-Flash | batch | **0.84** (0.85/0.82) | 0.78 | 0.65 | 0.44 | 0.25 |
| coderos llama.cpp Clef-Flash | batch | **0.86** (0.86/0.86) | 0.78 | 0.59 | 0.44 | 0.25 |
| Mac Ollama Clef-Flash | per-file | 0.75 | **0.77** (0.72/0.82) | 0.76 | 0.67 | 0.49 |
| Mac Ollama Clef 27B | batch | 0.86 | 0.89 | 0.89 | 0.85 | 0.79 |
| Jev | batch | 0.78 | 0.82 | 0.85 | **0.89** | 0.78 |
| Jev | per-file | 0.74 | 0.77 | 0.80 | 0.78 | **0.85** |

**Calibration** (share actually relevant in each probability bin, with the
number of files in brackets):

| Backend | Prompt | 0–0.2 | 0.2–0.4 | 0.4–0.6 | 0.6–0.8 | 0.8–1.0 |
|---|---|---|---|---|---|---|
| Jev | per-file | 0.00 (11) | 0.33 (9) | 0.25 (8) | 0.60 (5) | 0.87 (23) |
| Jev | batch | 0.00 (16) | 0.27 (11) | 0.67 (6) | 1.00 (5) | 0.89 (18) |
| Mac Ollama Clef 27B | batch | 0.00 (21) | 0.40 (5) | 0.67 (6) | 0.86 (7) | 0.94 (17) |
| Mac Ollama Clef-Flash | per-file | 0.21 (24) | 0.46 (13) | 0.67 (6) | 1.00 (8) | 1.00 (5) |
| Mac Ollama Clef-Flash | batch | 0.24 (33) | 0.83 (12) | 0.67 (3) | 1.00 (5) | 1.00 (3) |

Reading the probabilities:

- **Jev** is reasonably calibrated at both ends. Per file, it over-calls the middle: 0.4–0.6 is right only 25 % of the time.
- **Clef 27B** is the best calibrated, with Brier 0.084–0.086 batched.
- **Clef-Flash is systematically low.** Batched, files it scores 0.2–0.4 are relevant 80 % of the time, and a quarter of what it scores under 0.2 is relevant. Batching makes this worse: it spreads probability across the candidates, which lowers each file's mean p (0.38 for relevant files batched vs 0.50 per file). Its ranking (AUC) is about the same either way.
- **Batching helps every model's ranking** (AUC +0.03–0.07). For Jev and the 27B it also helps F1 at 0.5. Seeing the other candidates lets the model compare them.

**Open issues (demo, no ground truth).** These files scored p ≥ 0.5, in
per-file / batch form. The issue text names the files.

| Issue | Jev | Clef 27B (Mac Ollama) | Clef-Flash (any backend) |
| --- | --- | --- | --- |
| #11159 `/coder/*` not owned | `upstream.rs` 0.92/0.97, `lib.rs` 0.90/0.81, `wellknown.rs` 0.86/0.71, `coder_sync.rs` 0.77/0.74 | `upstream.rs` 0.97/0.97, `wellknown.rs` 0.95/(0.24), `coder_sync.rs` 0.94/0.76, `lib.rs` 0.90/(0.49) | `upstream.rs` 0.92/0.97 only. `wellknown.rs` scores 0.40–0.55 and `lib.rs` 0.11–0.16. |
| #11190 door quota per tenant | `serve.rs` 0.96/0.96, `playground.rs` 0.52 per-file, `relay_worker.rs` 0.56 batch | `serve.rs` 0.98/0.97, `playground.rs` 0.70 per-file | `serve.rs` 0.70–0.90 only |

Every backend puts the file the issue names (`upstream.rs`, `serve.rs`) on
top. Jev and the 27B also pick `lib.rs` and `coder_sync.rs`, which the
#11159 body also names. Clef-Flash at 0.5 does not.

## Why the backends differ

Every claim below was checked against `ollama show` / `ollama ps`, the server
logs, and `nvidia-smi` during the runs.

### Same model, different speed

1. **The CoderOS Ollama model is a different file, and part of it ran on the CPU.** This is the main cause of the gap the owner asked about (Ollama 1.38 s vs llama.cpp 0.35 s).
   - **The file.** On Linux, `ollama pull clef-flash` gives a **Q8_0** GGUF: 10 GB plus a 0.9 GB mmproj (`print_info: file type = Q8_0`, `CUDA0 model buffer size = 8045 MiB`). The llama.cpp row uses ggml-org's **Q4_K_M** (6.5 GB).
   - **The default load fails.** With Pylon (`psionic-openai-server`, 1.6 GB), the desk VM, verse and the desktop holding about 3.9 GB, Ollama's default load put 33/33 layers on the GPU. It then failed with `cudaMalloc failed: out of memory … CUDA0 buffer of size 3490250880` (the compute buffer for a 16k batch).
   - **The working variant.** It needed `PARAMETER num_gpu 24` (model `clef-flash-g24`): `offloaded 24/33 layers`, 6.07 GB of weights on CUDA0, **3.78 GB on the CPU**, and a 3.89 GB compute buffer. `ollama ps` showed `33%/67% CPU/GPU`, and the card sat at 14.6 of 16 GB.
   - **The cost.** With CPU-resident layers, llama.cpp streams those weights to the GPU on every large batch, a fixed cost per request ([clef-self-host.md](clef-self-host.md) measured about 1 s for the 27B). That is why CoderOS Ollama gains so much from batching (0.65 → 1.80 decisions/s), where llama.cpp on the GPU gains less (2.37 → 3.43).
   - **The 27B** on CoderOS Ollama is the same story. Ollama ships it as Q4_K_M, the default (35/65 layers) failed with a 5.4 GB compute-buffer out-of-memory error, and `num_gpu 16` ran at `59%/41% CPU/GPU`.
2. **Parallel slots.** Ollama starts its bundled llama-server with **`-np 1`** (from the logged `cmd=`), so concurrent requests queue: 1.14/s at 2 clients vs 0.65/s sequential, flat after that. Our llama-server ran **`-np 4 -c 49152`** (12,288 tokens per slot).
   - On CUDA, 2 clients lift llama.cpp from 2.37 to 3.88/s. The gain comes from filling the idle time between sequential requests (tunnel round trip and HTTP), not from batching prefills together. 4 and 8 clients add nothing: prefill is compute-bound.
3. **Batch size.** Both run each prompt in one physical batch, so this is **not** a cause of the gap.
   - Ollama's command line says `-b 512 -ub 512`, but the runner log shows `n_batch = n_ubatch = 16384` (Ollama raises the batch to `num_ctx` for decision models).
   - Ours is `-b 12288 -ub 12288`; the largest prompt was 10,075 tokens.
4. **Flash attention.** Ollama runs `--flash-attn auto`; we ran `-fa on`. Clef-Flash has only 8 full-attention layers out of 32, so this is a minor factor. We did not test it on its own.
5. **On the Mac, Ollama does not use llama.cpp at all.** `ollama ps` shows `RUNNER mlx`, and the log says "starting mlx runner subprocess". `ollama show` gives quantization **mxfp8** for clef-flash (12 GB on disk, 17 GB loaded) and **nvfp4** for clef (26 GB loaded).
   - MLX on mxfp8 prefilled faster than llama.cpp Metal on Q4_K_M: 1.13 s vs 1.56 s, measured back to back at load 17–21, i.e. ≈ 1,320 vs ≈ 980 tok/s. The 27B is 3.5 s vs 5.2 s.
   - Prefill is compute-bound, so fewer bits do not help. We have not profiled why the Metal Q4_K kernels are slower, so treat "the MLX kernels are faster at this shape" as the observation, not a diagnosis.
6. **Metal vs CUDA.** The same Q4_K_M file and the same flags give 1.5 s on the M5 Max (Metal) and 0.26 s on the RTX 4080 (CUDA, after removing the tunnel), about 6×. That is the 4080's dense compute advantage on a compute-bound prefill.
7. **Concurrency does nothing on the Mac.**
   - Ollama's MLX runner serializes decisions. Even a separate instance on port 11435 with `OLLAMA_NUM_PARALLEL=4` (the log shows `OLLAMA_NUM_PARALLEL:4`, one MLX runner) gave 1.16–1.22/s at 2–8 clients, with latency growing in step with the client count.
   - llama.cpp `-np 4` on Metal also stayed flat (0.65/s). It occasionally left one request waiting for 100+ s: flash p99 was 109 s at 4 clients, and two 27B requests hit the 300 s timeout.
   - The NUM_PARALLEL instance was about 20 % faster than Ollama.app on sequential (0.88 vs 1.09–1.13 s p50). Its config is otherwise identical (`OLLAMA_FLASH_ATTENTION:false`, `CONTEXT_LENGTH:0`). We did not verify the cause; a different process priority for the app-launched server is a guess.
8. **No caching on either side.** Each run's nonce sits at the start of the state, Ollama reported no cached prompt tokens, and llama.cpp keeps no prefix cache for Clef. The llama.cpp log shows slots chosen by "LCP similarity" (`f_keep` 0.14–0.33), but a decision prompt is evaluated whole.
9. **Request limits.** No request here came near Ollama's limits: the largest batch body was 36.5 KB against the 64 KiB cap, there were no choice questions (so the 26-option cap did not apply), and every prompt was under 16,384 tokens. **Nothing was refused.** A batch of more than about 13 files at 4 KB each would cross 64 KiB on Ollama, and more than about 40 files would pass the 16k tokens Clef was trained on.

### Same model, different answers

The quantizations agree closely.

- **Clef-Flash** (mxfp8, Q8_0 and Q4_K_M, across the Mac and CoderOS): median |Δp| 0.004–0.011 per file and 0.005–0.009 batched, max 0.17, and **96–100 %** on the same side of 0.5. The precision, recall and F1 above are identical to two decimals across all four flash backends.
- **Clef 27B** (Mac nvfp4, Mac Q4_K_M, CoderOS Ollama Q4_K_M): median |Δp| 0.010–0.020, max 0.27, and 92–99 % on the same side.
- **Clef-Flash vs Clef 27B** is a different model: median |Δp| 0.14–0.18, with only 65–76 % on the same side.
- **Clef 27B vs Jev**: median 0.08–0.09, with 86–90 % on the same side.
- **Repeats** of the same prompt on one backend vary by at most 0.011 (mean absolute deviation). Concurrency did not change answers: NUM_PARALLEL vs default had median |Δp| 0.002.

## Recommendation for Coder's file-relevance triage

1. **Use Jev, batched: one request per issue, with one `noul` per candidate file keyed by file.**
   - **Speed:** about 18 decisions/s from a single sequential client (0.33 s per 6-file request), and it scales with concurrency. Per-file requests reached 82/s at 32 clients with no rate limit, so batched requests should scale the same way (we did not measure batched concurrency).
   - **Quality and cost:** the best quality here (F1 0.89, AUC 0.94) at $0.05 per 1,000 files. Keep each request at or under about 8–13 files at 4 KB each. That stays inside Clef's limits too, so the same request can fail over.
   - **Threshold:** 0.5 is right for batched Jev. Per file, Jev does better at about 0.6–0.7.
2. **Own capacity / failover: Clef-Flash on the 4080 through llama.cpp, not Ollama.**
   - **Setup:** Q4_K_M, `-ngl 99 -fa on -np 2`, with `-c/-b/-ub` sized to the largest prompt.
   - **Speed:** about 2.4 files/s per file and 3.4 batched for one client, 3.9 with 2 clients.
   - **Threshold:** Flash's probabilities run low, so do not use 0.5. Use about 0.15–0.2 batched and 0.2–0.3 per file, or keep the top-k, and re-fit on a larger labeled set before relying on it.
   - **Why not Ollama:** Ollama on that card ships Q8_0, spills to the CPU beside the Pylon services, and runs one slot. It is 2–4× slower.
3. **Use Clef 27B when quality matters more than speed** (offline re-ranking, or a second opinion on borderline files). It matches Jev's quality but manages only 0.3–0.6 decisions/s on our machines.
4. **Batch vs per file.** Batch wherever the backend has spare compute per request: Jev (5× throughput, better F1) and partially offloaded GPUs (CoderOS Ollama 2.8×). On a compute-bound local GPU or the Mac, batching saves only the shared issue prefix, about 10–40 % throughput. It still improves ranking for every model, at the cost of compressing Clef-Flash's probabilities.
5. **The Mac is a development box for this task, not a server:** about 0.9 files/s with Clef-Flash and 0.28 with the 27B, and no gain from concurrency.

## Setups

| Backend | Where | Server | Model file | GPU placement | Slots |
| --- | --- | --- | --- | --- | --- |
| Mac Ollama | this Mac (M5 Max, 128 GB) | Ollama.app 0.40.0, port 11434, user's own instance | `clef-flash` mxfp8 / `clef` nvfp4 | 100 % GPU (MLX) | 1 (`OLLAMA_NUM_PARALLEL:1`) |
| Mac Ollama NUM_PARALLEL | this Mac | same binary, a separate `ollama serve` on 127.0.0.1:11435, `OLLAMA_NUM_PARALLEL=4`, same model store; stopped after | `clef-flash` mxfp8 | MLX | 4 configured, serialized in practice |
| Mac llama.cpp | this Mac | `llama-b11538-bin-macos-arm64` llama-server, ports 18093 and 18094; stopped after | ggml-org `Clef-Flash-Q4_K_M.gguf` (6.5 GB), `Clef-Q4_K_M.gguf` (13.7 GB) | `-ngl 99 -fa on -c 49152 -b 12288 -ub 12288` | `-np 4` |
| CoderOS Ollama | coderos-4080 (RTX 4080 16 GB, about 11.5 GB free beside Pylon) | **user-level** Ollama 0.40.0 from the release tarball in `~/clef-test`, on 127.0.0.1:11436 with its own `OLLAMA_MODELS`. The user `ollama.service` (0.34.0, port 11434, too old for Clef) was not touched. Reached over an ssh tunnel (tailnet). | `clef-flash` → Q8_0 GGUF via Ollama's llamacpp runner; `clef` → Q4_K_M | flash `num_gpu 24` (24/33 layers, 33 % CPU); 27B `num_gpu 16` (59 % CPU). The defaults fail with CUDA out-of-memory. | `-np 1` |
| CoderOS llama.cpp | coderos-4080 | llama.cpp b11538 CUDA 12.8 prebuilt, 127.0.0.1:18091, ssh tunnel | ggml-org `Clef-Flash-Q4_K_M.gguf` | `-ngl 99 -fa on -c 49152 -b 12288 -ub 12288`, about 8.7 GB VRAM (card 12.7/16 GB) | `-np 4` |
| Jev | TypeSafe | `https://api.typesafe.ai/v1/systemone`, model `jev-latest` (echoed `jev-1.13.0`), key from `~/work/.secrets/typesafe.env` | — | — | — |

The coderos 1-minute load was 12–43 during the runs; the desktop and other
agents' builds were active. CoderOS Ollama's 33 %/67 % split is from
`ollama ps` during the run, with the card at 14.6/16 GB. The 4080 llama.cpp
rows were taken at load 13–19.

**Cleanup.** On coderos, the user-level Ollama 0.40 and llama-server were
stopped, and the model store, binaries and GGUF were deleted (about 37 GB).
Two small logs remain in `~/clef-test`. Disk went back to 113 GB free and the
GPU to 3.9 GB used. The Pylon services and the user `ollama.service` were
untouched; another agent's llama-server in `~/clef-m1` was left running.
On the Mac, the two llama-servers and the port-11435 Ollama were stopped.
Ollama.app was never restarted, and the downloaded GGUFs (about 20 GB) are
in the session scratch directory only.

## Reproduce

```sh
python3 -I scripts/bench/clef-relevance-dataset.py --repo . --out $SCRATCH/dataset.json   # file contents: keep out of git
set -a; . ~/work/.secrets/typesafe.env; set +a                                            # TYPESAFE_API_KEY, Jev rows only
python3 -I scripts/bench/clef-relevance-bench.py --dataset $SCRATCH/dataset.json --out $SCRATCH/r.jsonl \
    --backend ollama-flash --mode seq --warmup 3          # or --mode batch --rounds 2, --mode conc --conc 4
python3 -I scripts/bench/clef-relevance-report.py $SCRATCH/r.jsonl
```

`--backend` names the base URL and model. Override them with `--base-url` and
`--model`, for example `--model clef-flash-g24` for a `num_gpu` variant.
