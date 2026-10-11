# Decision throughput: decisions per second per backend

How many decisions per second each decision backend sustains when it is
hammered with jobs back to back, and under which configurations. Issue
[#11270](https://github.com/OpenAgentsInc/openagents/issues/11270).

A **decision** is one answered question of a NIP-DEC `/v1/systemone`
request. A request with 13 `noul` questions that answers all 13 is 13
decisions. Every number here comes from
`scripts/bench/decision-throughput/harness.py`. The tables between the
`dt:` markers are regenerated from the results in Cloud Storage by
`report.py`, by hand or by the scheduled run.

## Which backend to use

First full run: 2026-10-11.

- **Bulk jobs (many decisions, nothing private): Jev, batched, many
  clients at once.**
  - **Speed.** Jev showed no knee up to 64 clients: 3.5 decisions/s from
    one client, 173/s from 64, with p99 under 0.8 s. Thirteen questions per
    request reach the same 172/s from only 4 clients.
  - **Cost.** $0.014 per 1,000 decisions batched 13 to a request, $0.066 one
    per request. Nothing else is close on both speed and cost.
  - **Through our gateway.** Keyless callers get the same thing at about
    half the rate (86/s at 32 clients, 134/s batched), for 60–100 ms more per
    call.
  - **Our own hardware.** For bulk work on our hardware, batch on the
    H100: Clef-Flash 39/s at 13 questions per request, $0.045 per 1,000.
- **Interactive calls (one person waiting): Jev direct, 0.27 s p50 and
  0.4–0.7 s p99 at any load.**
  - **Through the gateway.** p50 0.33–0.37 s.
  - **When Jev is down,** the fastest single answer is our own Clef-Flash:
    - CoderOS-4080: 0.24 s p50 at 1.5k tokens, 4.2 decisions/s for the
      whole box;
    - H100: 0.11 s p50, 9–10/s.

    Both run one decision at a time, so a second waiting caller doubles
    the latency. Through the gateway's Pylon-first path, the relay adds
    about 0.33 s.
  - **Gemini on Vertex** stays the last resort: 1.7–2.2 s p50, 5–11 s p99,
    $1.6 per 1,000 decisions, $7.9 at 16k tokens.
- **Private data: the sealed GPU (H100 in confidential-computing mode,
  over NIP-ATT).**
  - **Speed.** About 3 decisions/s, 0.65–0.7 s per verified round at 1.5k
    tokens, 1.2 s at 8k.
  - **Where the time goes.** About 0.3 s verifying the endpoint and about
    0.35 s on the relay; Clef itself is about 0.1 s. A client that keeps
    its verification would save most of the first part.
  - **Cost.** $0.61–1.3 per 1,000 decisions while it runs, plus a 4-minute
    wake.
  - **Limits.** The attested pylon's slots cap it ("no free slot" from 8
    clients up), and from 16 clients the relay rate-limits one IP.
  - **The sealed CPU** (TDX, always on) is for a trickle only: about 28 s
    per decision at 0.6k tokens, 0.036 decisions/s, $3.5 per 1,000. One
    slot, so anything past one client only waits.
- **Clef 27B** is a quality tier, not a throughput tier. On the H100 it
  does 3.8 decisions/s at 1.5k tokens and 13.5/s batched.

## Findings

- **Psionic serves one decision at a time.** Every Psionic Clef lane is
  flat from one client up: 4080, H100 Flash and 27B, sealed.
  - **Where extra clients go.** More clients only add queue time, which
    `psionic.timing` shows directly. For example, H100 Flash at 4 clients:
    queue 304 ms, prefill 97 ms, head 3 ms.
  - **Busy refusals.** Past 8 waiting decisions, Psionic answers `503 busy`
    (`max_queue`). With 16 or 32 clients on the 4080, retrying clients saw
    p99 latencies of 47 s and 77 s.
  - **What would raise it.** Batching prefills across requests (M4).
    Batching questions within one request already does: 13 questions per
    request lift the H100 from 9.9 to 39 decisions/s and the 4080 from 4.2
    to 16/s.
- **The head is cheap; prefill is the cost.** The head took 3–12 ms
  against 90–1,500 ms of prefill on the H100.
- **Warm means the exact logit cache, and it is not a throughput figure.**
  A repeated identical request is answered from Psionic's exact logit
  cache in about 2 ms: 2,463 decisions/s on the H100 and 3,064/s on the
  4080. Real traffic rarely repeats a request byte for byte. Jev and
  Vertex showed no warm speedup.
- **Jev billing.** The TypeSafe account ran out of credit mid-run (HTTP 402
  at about 03:41 UTC). The gateway failed over to the pylon and Vertex as
  designed. The owner added credits, and the affected points were run
  again.
  - The harness now stops a target on its first 402.
  - Jev has no rate limit up to 64 clients: no 429 and no rate-limit
    headers.
- **The gateway refuses the router's `cli_group` request.** It replies
  400 with "question `cli_group` serializes past 32768 bytes", the NIP-DEC
  per-question bound: the request is 56 KB, with one 33 KB question. A
  third of the router replay was refused on both gateway paths
  ([#11273](https://github.com/OpenAgentsInc/openagents/issues/11273)).
  Jev direct and Psionic direct accept it.
- **llama.cpp on the 4080** (b11538) is in the same range as Psionic. It
  peaks at about 3 decisions/s at 1.5k tokens, against Psionic's 4.2.
  - It ran with `-np 2 -c 10240`, because the planned `-np 4 -c 49152`
    didn't fit beside production.
  - Ollama was skipped: it needs a 10 GB Q8_0 pull and VRAM the 4080
    doesn't have spare. The numbers in
    [clef-jev-relevance-bench.md](clef-jev-relevance-bench.md) stand.


## Summary

The latest day with a full sweep for each backend.

- **Best, 1 question:** the best sustained decisions per second at the base
  shape: one question per request, a cold prompt of about 1.5k tokens (0.5k
  for the sealed CPU).
- **Best over all shapes:** usually batched questions (13 per request) or
  the router's 11-question main request.
- **Knee:** the concurrency after which more clients stop adding
  throughput (under 10 % more decisions/s) while p50 latency grows by more
  than half, or errors pass 5 %. "none seen" means throughput was still
  growing at the highest concurrency tried.
- **$ / 1,000:** for hosted backends, list price times the tokens they
  reported. For our hardware, the machine's hourly price divided by the
  measured throughput, so idle time is not counted.

<!-- dt:summary:start -->
| Backend | Best sustained decisions/s, 1 question | At concurrency | p99 | $ / 1,000 | Best over all shapes | Shape, concurrency | Knee | Day |
| --- | ---: | ---: | ---: | ---: | ---: | --- | ---: | --- |
| jev | 173 | 64 | 0.76 s | $0.0663 | 172 | 4k (4.2k measured) tok × 13 q, c=4 | none seen | 2026-10-11 |
| gateway-jev | 86.39 | 32 | 0.62 s | $0.0662 | 134 | 4k (4.2k measured) tok × 13 q, c=4 | none seen | 2026-10-11 |
| gateway-pylon | 4.14 | 4 | 1.24 s | $0.0081 | 16.56 | 4k (5.0k measured) tok × 13 q, c=4 | 4 | 2026-10-11 |
| vertex-gemini | 3.50 | 16 | 6.35 s | $1.722 | 15.40 | 4k (5.6k measured) tok × 13 q, c=4 | none seen | 2026-10-11 |
| coderos-clef | 4.21 | 1 | 0.27 s | $0.0079 | 16.06 | 4k (5.0k measured) tok × 13 q, c=2 | 1 | 2026-10-11 |
| coderos-llamacpp | 3.14 | 1 | 0.66 s | $0.0106 | 9.63 | 0.5k (0.6k measured) tok × 1 q, c=1 | 1 | 2026-10-11 |
| h100-clef-flash | 9.87 | 4 | 0.43 s | $0.180 | 39.45 | 4k (5.0k measured) tok × 13 q, c=4 | 1 | 2026-10-11 |
| h100-clef-27b | 3.83 | 1 | 0.30 s | $0.464 | 13.48 | 4k (5.0k measured) tok × 13 q, c=4 | 1 | 2026-10-11 |
| sealed-gpu | 2.94 | 2 | 0.78 s | $0.646 | 3.11 | 0.5k (0.6k measured) tok × 1 q, c=2 | 8 | 2026-10-11 |
| sealed-cpu | 0.04 | 1 | 31.28 s | $3.472 | 0.04 | 0.5k (0.6k measured) tok × 1 q, c=1 | 1 | 2026-10-11 |
<!-- dt:summary:end -->

## Method

**Requests.** The harness sends real NIP-DEC requests, built from two
corpora that live in
`gs://openagentsgemini-decision-throughput/corpus/v1/`. They are not kept
in git because they hold repository file contents.

- **File relevance** (`dataset.json`): Coder's triage question, "is this
  file relevant to solving this issue?", over the 12 issues and 72
  candidate files of [clef-jev-relevance-bench.md](clef-jev-relevance-bench.md).
  It was rebuilt with `scripts/bench/clef-relevance-dataset.py --cap 65536`
  so that 16k-token prompts can be filled.
  - A request holds one issue and Q candidate files, with one `noul` per
    file.
  - The file contents are cut so that the state is about 0.5k, 1.5k, 4k or
    16k tokens, at 3.3 bytes per token, measured on this corpus.
  - Q = 13 takes files from the next issues as well.
- **Router** (`router.jsonl`): the chat router's real split requests, 123
  turns of the web chat goldens, recorded with
  `CHAT_GOLDENS_REQUESTS=FILE chat-goldens router` (#11193's split):
  - `main`: 11 questions, about 38 KB;
  - `answer`: 1 question, about 13 KB;
  - `cli_group`: 1 question, about 57 KB;
  - `repository`: 3 turns.

  They are replayed in order.

**Cold and warm.**

- **Cold:** every request carries a fresh nonce at the start of its state,
  so no backend can answer it from a cache.
- **Warm:** a point cycles through 8 fixed requests under one nonce, so an
  exact cache (Psionic's logit cache) or a prefix cache can serve them.

**Points.** Each plan changes one factor at a time around the base shape.

- **Concurrency sweep.** 1, 2, 4, 8, 16, 32 and 64 closed-loop clients, each
  sending its next request as soon as the last one answers. The sweep stops
  once it is two doublings past the knee.
- **Shape points.** Prompt size (0.5k, 4k, 16k), questions per request
  (4 and 13, at 4k tokens), warm, and the router. They run at the best
  concurrency from the sweep, capped per target. For Jev the cap is 4, to
  stay inside the spend limit.
- **Warmup.** Each point first sends at least as many warmup requests as it
  has clients. Their results are discarded.
- **Duration.** A point runs for 120–150 s (`duration` in `targets.json`).
  The exceptions are listed under "First run" below.

**Recorded per point.** Each point is one JSON line in
`gs://openagentsgemini-decision-throughput/runs/YYYY-MM-DD.jsonl`.

- Decisions/s, sustained over the whole window and at the best whole 10 s
  window.
- Request latency at p50, p90 and p99.
- Psionic's server timing (`psionic.timing`: `queue_ms`, `prefill_ms`,
  `head_ms`, `total_ms`, `cached`), when the answer carries it. Transport
  time is the client latency minus `total_ms`.
- For sealed rounds, the verify, seal and relay time from `oa-att`.
- Every failure, by status and code.
- Psionic's `503 busy`, which it returns once more than 8 decisions are
  waiting (`ClefLimits::max_queue`). The harness retries a busy answer
  after 50 ms and counts each retry. The latency includes the waiting.
- The door that answered, for the gateway.
- Input and output tokens.
- GPU utilization and memory, sampled every 5 s with `nvidia-smi` where
  the harness can read them.
- $ per 1,000 decisions.

**Targets** (`scripts/bench/decision-throughput/targets.json`):

| Target | What it measures |
| --- | --- |
| `jev` | TypeSafe `api.typesafe.ai/v1/systemone`, `jev-latest`, our key |
| `gateway-jev` | `openagents.com/api/v1/systemone`, keyless: Jev first under the gateway's house key |
| `gateway-pylon` | the same, with `X-Decision-Order: pylon`: the production CoderOS-4080 Clef pylon over NIP-DEC on the relay first. Capped at 8 clients, the pylon's slots, so production decisions aren't starved |
| `vertex-gemini` | Gemini 3.8 Flash on Vertex with structured output: the prompt and schema of the gateway's Vertex door (`decision_dispatch.rs` `vertex_body`), thinking `low` |
| `coderos-clef` | Psionic Clef-Flash Q4_K_M on CUDA (RTX 4080), a separate bench server on port 18097, beside the untouched production `pylon-clef` |
| `coderos-llamacpp`, `coderos-ollama` | llama.cpp and Ollama on the same 4080, for reference |
| `h100-clef-flash`, `h100-clef-27b` | Psionic on one H100 80 GB, a plain spot `a3-highgpu-1g` |
| `sealed-gpu` | `oa-att round` per request against `clef-decisions-gpu` (H100 in CC mode + TDX). Each round verifies the endpoint, seals with NIP-44, crosses the relay, then opens the answer and checks its receipt. One question per request |
| `sealed-cpu` | the same against `clef-decisions` on `oa-att-tdx-1` (8 TDX vCPUs) |
| `mac-clef` | Psionic Clef on the Mac's Metal lane. Not run here: run it when the Mac is idle (below) |

**Prices used.** These are for $ per 1,000 decisions; change them in
`targets.json`.

- **Jev:** $0.042 per million input tokens, its list price
  (`crates/jev/src/nip_dec.rs`). Output tokens are not billed.
- **Gemini 3.8 Flash:** $0.75 per million input tokens and $3.75 per
  million output tokens, with thinking tokens counted as output
  (`crates/inference/src/upstream/vertex.rs`).
- **H100:** $6.40 an hour for a spot `a3-highgpu-1g` in us-east5. The
  sealed one is $6.83 an hour with the Confidential Computing surcharge.
- **Sealed CPU:** $0.45 an hour.
- **CoderOS-4080:** an assumed $0.12 an hour (the box amortized over 3
  years plus about 350 W). The Mac is an assumed $0.19 an hour. Both are
  assumptions, not bills.

## Per-backend tables

The newest day of each backend. "Shape" is the prompt size × questions per
request.

<!-- dt:backends:start -->
### jev

TypeSafe hosted. Model `jev-latest`, plan `full`, 2026-10-11, run on `ChristohersMBP2.lan`. Price: TypeSafe list price, crates/jev/src/nip_dec.rs.

| Point | Shape | Conc | OK / failed | Decisions/s | Peak 10 s | p50 s | p90 s | p99 s | busy retries | $ / 1,000 |
|---|---|---|---|---|---|---|---|---|---|---|
| c1 | 1.5k (1.6k measured) tok × 1 q | 1 | 423 / 0 | 3.52 | 3.80 | 0.27 | 0.35 | 0.62 | 0 | $0.0663 |
| c2 | 1.5k (1.6k measured) tok × 1 q | 2 | 855 / 0 | 7.12 | 7.60 | 0.27 | 0.33 | 0.53 | 0 | $0.0664 |
| c4 | 1.5k (1.6k measured) tok × 1 q | 4 | 1790 / 0 | 14.88 | 15.40 | 0.27 | 0.31 | 0.39 | 0 | $0.0664 |
| c8 | 1.5k (1.6k measured) tok × 1 q | 8 | 3544 / 0 | 29.49 | 30.30 | 0.26 | 0.31 | 0.52 | 0 | $0.0663 |
| c16 | 1.5k (1.6k measured) tok × 1 q | 16 | 6863 / 0 | 57.07 | 60.40 | 0.27 | 0.32 | 0.69 | 0 | $0.0663 |
| c32 | 1.5k (1.6k measured) tok × 1 q | 32 | 6350 / 0 | 105 | 113 | 0.29 | 0.37 | 0.51 | 0 | $0.0662 |
| c64 | 1.5k (1.6k measured) tok × 1 q | 64 | 10824 / 0 | 173 | 194 | 0.34 | 0.43 | 0.76 | 0 | $0.0663 |
| t500q1-cold | 0.5k (0.8k measured) tok × 1 q | 4 | 1331 / 0 | 14.76 | 15.60 | 0.27 | 0.32 | 0.39 | 0 | $0.0316 |
| t4000q1-cold | 4k (3.3k measured) tok × 1 q | 4 | 1287 / 0 | 14.28 | 14.80 | 0.27 | 0.33 | 0.51 | 0 | $0.138 |
| t16000q1-cold | 16k (7.9k measured) tok × 1 q | 4 | 1096 / 0 | 12.14 | 12.40 | 0.33 | 0.41 | 0.49 | 0 | $0.330 |
| t4000q4-cold | 4k (3.8k measured) tok × 4 q | 4 | 1086 / 0 | 48.10 | 56.00 | 0.29 | 0.45 | 0.82 | 0 | $0.0403 |
| t4000q13-cold | 4k (4.2k measured) tok × 13 q | 4 | 1191 / 0 | 172 | 181 | 0.29 | 0.36 | 0.61 | 0 | $0.0137 |
| t1500q1-warm | 1.5k (1.5k measured) tok × 1 q warm | 4 | 1349 / 0 | 14.95 | 15.30 | 0.26 | 0.31 | 0.47 | 0 | $0.0609 |
| router-cold | router (10,362 tok) | 4 | 995 / 0 | 47.60 | 50.60 | 0.36 | 0.45 | 0.79 | 0 | $0.101 |

### gateway-jev

openagents.com gateway, Jev first. Model `jev-latest`, plan `full`, 2026-10-11, run on `ChristohersMBP2.lan`. Price: Jev list price under the gateway's house key (keyless to the caller).

| Point | Shape | Conc | OK / failed | Decisions/s | Peak 10 s | p50 s | p90 s | p99 s | busy retries | $ / 1,000 | doors |
|---|---|---|---|---|---|---|---|---|---|---|---|
| c1 | 1.5k (1.6k measured) tok × 1 q | 1 | 320 / 0 | 2.67 | 2.90 | 0.37 | 0.43 | 0.51 | 0 | $0.0663 | jev 320 |
| c2 | 1.5k (1.6k measured) tok × 1 q | 2 | 718 / 0 | 5.97 | 6.30 | 0.33 | 0.39 | 0.54 | 0 | $0.0665 | jev 718 |
| c4 | 1.5k (1.6k measured) tok × 1 q | 4 | 1410 / 0 | 11.74 | 12.30 | 0.33 | 0.40 | 0.61 | 0 | $0.0665 | jev 1410 |
| c8 | 1.5k (1.6k measured) tok × 1 q | 8 | 2742 / 0 | 22.79 | 24.20 | 0.33 | 0.40 | 0.69 | 0 | $0.0662 | jev 2742 |
| c16 | 1.5k (1.6k measured) tok × 1 q | 16 | 5386 / 0 | 44.76 | 46.70 | 0.34 | 0.41 | 0.70 | 0 | $0.0662 | jev 5386 |
| c32 | 1.5k (1.6k measured) tok × 1 q | 32 | 5218 / 0 | 86.39 | 89.80 | 0.36 | 0.43 | 0.62 | 0 | $0.0662 | jev 5218 |
| t16000q1-cold | 16k (8.0k measured) tok × 1 q | 4 | 793 / 0 | 8.78 | 10.00 | 0.44 | 0.56 | 0.79 | 0 | $0.335 | jev 793 |
| t4000q13-cold | 4k (4.2k measured) tok × 13 q | 4 | 930 / 0 | 134 | 139 | 0.38 | 0.44 | 0.58 | 0 | $0.0137 | jev 930 |
| router-cold | router (7,295 tok) | 4 | 386 / 191 (400:invalid_request ×191) | 24.59 | 39.80 | 0.40 | 0.62 | 6.46 | 0 | $0.0513 | jev 357, vertex 29 |

### gateway-pylon

gateway, Pylon first: the production CoderOS-4080 Clef pylon over NIP-DEC. Model `jev-latest`, plan `full`, 2026-10-11, run on `coderos-4080`. Price: CoderOS-4080 amortized hardware + power (assumption, see doc).

| Point | Shape | Conc | OK / failed | Decisions/s | Peak 10 s | p50 s | p90 s | p99 s | busy retries | $ / 1,000 | doors |
|---|---|---|---|---|---|---|---|---|---|---|---|
| c1 | 1.5k (1.5k measured) tok × 1 q | 1 | 202 / 0 | 1.68 | 1.90 | 0.57 | 0.68 | 1.09 | 0 | $0.0198 | vertex 3, pylon:coderos-4080-clef 199 |
| c2 | 1.5k (1.5k measured) tok × 1 q | 2 | 369 / 0 | 3.06 | 3.30 | 0.64 | 0.78 | 1.06 | 0 | $0.0109 | pylon:coderos-4080-clef 369 |
| c4 | 1.5k (1.5k measured) tok × 1 q | 4 | 500 / 0 | 4.14 | 4.40 | 0.96 | 1.07 | 1.24 | 0 | $0.0081 | pylon:coderos-4080-clef 500 |
| c8 | 1.5k (1.5k measured) tok × 1 q | 8 | 511 / 0 | 4.19 | 4.40 | 1.92 | 2.03 | 2.16 | 0 | $0.0080 | pylon:coderos-4080-clef 511 |
| t500q1-cold | 0.5k (0.6k measured) tok × 1 q | 4 | 814 / 0 | 6.76 | 7.10 | 0.56 | 0.72 | 1.07 | 0 | $0.0049 | pylon:coderos-4080-clef 814 |
| t4000q1-cold | 4k (3.3k measured) tok × 1 q | 4 | 237 / 0 | 1.95 | 2.10 | 2.08 | 2.30 | 2.52 | 0 | $0.0171 | pylon:coderos-4080-clef 237 |
| t16000q1-cold | 16k (8.4k measured) tok × 1 q | 4 | 96 / 0 | 0.76 | 1.00 | 5.15 | 7.16 | 11.07 | 0 | $0.0440 | pylon:coderos-4080-clef 93, vertex 3 |
| t4000q4-cold | 4k (4.0k measured) tok × 4 q | 4 | 196 / 0 | 6.43 | 6.80 | 2.49 | 2.57 | 3.15 | 0 | $0.0052 | pylon:coderos-4080-clef 196 |
| t4000q13-cold | 4k (5.0k measured) tok × 13 q | 4 | 156 / 0 | 16.56 | 16.90 | 3.13 | 3.18 | 3.24 | 0 | $0.0020 | pylon:coderos-4080-clef 156 |
| t1500q1-warm | 1.5k (1.4k measured) tok × 1 q warm | 4 | 1145 / 0 | 9.52 | 10.80 | 0.39 | 0.56 | 0.96 | 0 | $0.0035 | pylon:coderos-4080-clef 1145 |
| router-cold | router (6,632 tok) | 4 | 120 / 59 (400:invalid_request ×59) | 5.81 | 6.00 | 3.98 | 4.15 | 4.42 | 0 | $0.0057 | pylon:coderos-4080-clef 120 |

### vertex-gemini

Vertex AI Gemini 3.8 Flash, structured output, thinking low. Model `gemini-3.8-flash`, plan `full`, 2026-10-11, run on `ChristohersMBP2.lan`. Price: crates/inference/src/upstream/vertex.rs list price; output includes thinking tokens.

| Point | Shape | Conc | OK / failed | Decisions/s | Peak 10 s | p50 s | p90 s | p99 s | busy retries | $ / 1,000 |
|---|---|---|---|---|---|---|---|---|---|---|
| c1 | 1.5k (1.7k measured) tok × 1 q | 1 | 37 / 0 | 0.41 | 0.60 | 1.75 | 4.10 | 4.86 | 0 | $1.585 |
| c4 | 1.5k (1.7k measured) tok × 1 q | 4 | 130 / 0 | 1.39 | 1.90 | 2.23 | 4.22 | 5.57 | 0 | $1.632 |
| c16 | 1.5k (1.7k measured) tok × 1 q | 16 | 569 / 0 | 3.50 | 6.80 | 2.19 | 4.51 | 6.35 | 0 | $1.722 |
| t16000q1-cold | 16k (9.6k measured) tok × 1 q | 4 | 89 / 0 | 1.43 | 1.50 | 1.90 | 4.70 | 10.91 | 0 | $7.851 |
| t4000q13-cold | 4k (5.6k measured) tok × 13 q | 4 | 40 / 0 | 15.40 | 18.20 | 2.67 | 4.93 | 10.25 | 0 | $0.416 |

### coderos-clef

RTX 4080 16 GB, Psionic Clef-Flash Q4_K_M CUDA (bench instance, port 18097). Model `clef-flash`, plan `full`, 2026-10-11, run on `coderos-4080`. Price: CoderOS-4080 amortized hardware + power (assumption, see doc).

| Point | Shape | Conc | OK / failed | Decisions/s | Peak 10 s | p50 s | p90 s | p99 s | queue / prefill / head ms (p50) | busy retries | $ / 1,000 | GPU util mean / max, mem max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| c1 | 1.5k (1.5k measured) tok × 1 q | 1 | 633 / 0 | 4.21 | 4.30 | 0.24 | 0.25 | 0.27 | 0 / 235 / 3 | 0 | $0.0079 | 98 % / 100 %, 14.9 GB |
| c2 | 1.5k (1.5k measured) tok × 1 q | 2 | 634 / 0 | 4.22 | 4.30 | 0.48 | 0.50 | 0.51 | 238 / 236 / 3 | 0 | $0.0079 | 98 % / 100 %, 14.9 GB |
| c4 | 1.5k (1.5k measured) tok × 1 q | 4 | 590 / 0 | 3.91 | 4.30 | 0.97 | 1.25 | 2.00 | 724 / 237 / 3 | 0 | $0.0085 | 98 % / 100 %, 15.4 GB |
| c8 | 1.5k (1.5k measured) tok × 1 q | 8 | 615 / 0 | 4.05 | 4.30 | 1.94 | 1.99 | 3.52 | 1,696 / 239 / 3 | 0 | $0.0082 | 98 % / 100 %, 15.4 GB |
| c16 | 1.5k (1.5k measured) tok × 1 q | 16 | 638 / 0 | 4.15 | 4.30 | 2.19 | 2.26 | 47.38 | 1,947 / 240 / 3 | 20424 | $0.0080 | 98 % / 100 %, 15.2 GB |
| c32 | 1.5k (1.5k measured) tok × 1 q | 32 | 654 / 0 | 4.15 | 4.30 | 2.21 | 19.79 | 76.76 | 1,948 / 240 / 3 | 67968 | $0.0080 | 97 % / 100 %, 15.2 GB |
| t500q1-cold | 0.5k (0.6k measured) tok × 1 q | 2 | 1176 / 0 | 7.83 | 8.00 | 0.26 | 0.26 | 0.27 | 124 / 122 / 2 | 0 | $0.0043 | 98 % / 100 %, 14.9 GB |
| t4000q1-cold | 4k (3.3k measured) tok × 1 q | 2 | 290 / 0 | 1.91 | 2.10 | 1.09 | 1.17 | 1.62 | 556 / 555 / 3 | 0 | $0.0174 | 98 % / 100 %, 15.2 GB |
| t16000q1-cold | 16k (8.0k measured) tok × 1 q | 2 | 109 / 28 (500:internal ×28) | 0.72 | 1.00 | 2.63 | 4.17 | 4.66 | 1,405 / 1,302 / 3 | 0 | $0.0464 | 100 % / 100 %, 15.5 GB |
| t4000q4-cold | 4k (4.0k measured) tok × 4 q | 2 | 245 / 0 | 6.49 | 6.80 | 1.25 | 1.29 | 1.32 | 609 / 606 / 5 | 0 | $0.0051 | 98 % / 100 %, 14.9 GB |
| t4000q13-cold | 4k (5.0k measured) tok × 13 q | 2 | 187 / 0 | 16.06 | 16.90 | 1.58 | 1.72 | 2.49 | 792 / 787 / 9 | 0 | $0.0021 | 98 % / 100 %, 15.2 GB |
| t1500q1-warm | 1.5k (1.4k measured) tok × 1 q warm | 16 | 459585 / 0 | 3,064 | 3,573 | 0.00 | 0.01 | 0.01 | 0 / 0 / 0, cached 100% | 534 | $0.0000 | 21 % / 100 %, 15.2 GB |
| router-cold | router (7,604 tok) | 16 | 95 / 42 (500:internal ×42) | 2.93 | 3.90 | 12.54 | 27.77 | 58.91 | 10,498 / 1,467 / 12 | 19742 | $0.0114 | 98 % / 100 %, 15.4 GB |

### coderos-llamacpp

RTX 4080 16 GB, llama.cpp b11538 Clef-Flash Q4_K_M. Model `clef-flash`, plan `full`, 2026-10-11, run on `coderos-4080`. Price: CoderOS-4080 amortized hardware + power (assumption, see doc).

| Point | Shape | Conc | OK / failed | Decisions/s | Peak 10 s | p50 s | p90 s | p99 s | busy retries | $ / 1,000 | GPU util mean / max, mem max |
|---|---|---|---|---|---|---|---|---|---|---|---|
| c1 | 1.5k (1.5k measured) tok × 1 q | 1 | 377 / 0 | 3.14 | 3.60 | 0.27 | 0.46 | 0.66 | 0 | $0.0106 | 98 % / 100 %, 15.4 GB |
| c2 | 1.5k (1.5k measured) tok × 1 q | 2 | 357 / 0 | 2.96 | 4.00 | 0.67 | 0.97 | 1.05 | 0 | $0.0113 | 99 % / 100 %, 15.5 GB |
| c4 | 1.5k (1.5k measured) tok × 1 q | 4 | 343 / 1 (0:TimeoutError ×1) | 2.84 | 4.10 | 1.02 | 1.48 | 1.54 | 0 | $0.0117 | 99 % / 100 %, 15.4 GB |
| t500q1-cold | 0.5k (0.6k measured) tok × 1 q | 1 | 1156 / 0 | 9.63 | 9.70 | 0.10 | 0.11 | 0.12 | 0 | $0.0035 | 96 % / 100 %, 15.3 GB |
| t4000q4-cold | 4k (4.0k measured) tok × 4 q | 1 | 166 / 0 | 5.53 | 6.00 | 0.70 | 0.72 | 1.75 | 0 | $0.0060 | 88 % / 100 %, 15.3 GB |

### h100-clef-flash

1x H100 80GB, Psionic Clef-Flash Q4_K_M CUDA. Model `clef-flash`, plan `full`, 2026-10-11, run on `oa-decision-h100.us-east5-a.c.openagentsgemini.internal`. Price: spot a3-highgpu-1g us-east5 (H100 $5.71 + vCPU/RAM).

| Point | Shape | Conc | OK / failed | Decisions/s | Peak 10 s | p50 s | p90 s | p99 s | queue / prefill / head ms (p50) | busy retries | $ / 1,000 | GPU util mean / max, mem max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| c1 | 1.5k (1.5k measured) tok × 1 q | 1 | 1098 / 0 | 9.15 | 9.90 | 0.11 | 0.12 | 0.13 | 0 / 105 / 3 | 0 | $0.194 | 65 % / 78 %, 24.3 GB |
| c4 | 1.5k (1.5k measured) tok × 1 q | 4 | 1188 / 0 | 9.87 | 10.00 | 0.41 | 0.42 | 0.43 | 304 / 97 / 3 | 0 | $0.180 | 74 % / 94 %, 24.3 GB |
| t500q1-cold | 0.5k (0.6k measured) tok × 1 q | 4 | 1288 / 0 | 10.70 | 10.80 | 0.38 | 0.38 | 0.39 | 281 / 90 / 3 | 0 | $0.166 | 86 % / 87 %, 24.3 GB |
| t4000q1-cold | 4k (3.3k measured) tok × 1 q | 4 | 616 / 0 | 5.10 | 5.20 | 0.80 | 0.85 | 0.90 | 606 / 206 / 3 | 0 | $0.349 | 82 % / 97 %, 24.5 GB |
| t16000q1-cold | 16k (8.4k measured) tok × 1 q | 4 | 258 / 0 | 2.12 | 2.40 | 1.90 | 2.48 | 2.92 | 1,407 / 480 / 4 | 0 | $0.841 | 85 % / 97 %, 24.8 GB |
| t4000q4-cold | 4k (4.0k measured) tok × 4 q | 4 | 513 / 0 | 17.00 | 17.20 | 0.94 | 0.98 | 1.02 | 708 / 220 / 6 | 0 | $0.105 | 81 % / 96 %, 24.5 GB |
| t4000q13-cold | 4k (5.0k measured) tok × 13 q | 4 | 368 / 0 | 39.45 | 40.30 | 1.32 | 1.33 | 1.33 | 987 / 321 / 10 | 0 | $0.0451 | 90 % / 99 %, 24.5 GB |
| t1500q1-warm | 1.5k (1.4k measured) tok × 1 q warm | 4 | 295577 / 0 | 2,463 | 2,558 | 0.00 | 0.00 | 0.00 | 0 / 0 / 0, cached 100% | 0 | $0.0007 | 4 % / 97 %, 24.4 GB |
| router-cold | router (8,697 tok) | 4 | 244 / 0 | 8.73 | 9.10 | 2.03 | 2.25 | 2.26 | 1,490 / 515 / 12 | 0 | $0.204 | 87 % / 97 %, 24.9 GB |

### h100-clef-27b

1x H100 80GB, Psionic Clef 27B Q4_K_M CUDA. Model `clef`, plan `full`, 2026-10-11, run on `oa-decision-h100.us-east5-a.c.openagentsgemini.internal`. Price: spot a3-highgpu-1g us-east5 (H100 $5.71 + vCPU/RAM).

| Point | Shape | Conc | OK / failed | Decisions/s | Peak 10 s | p50 s | p90 s | p99 s | queue / prefill / head ms (p50) | busy retries | $ / 1,000 | GPU util mean / max, mem max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| c1 | 1.5k (1.5k measured) tok × 1 q | 1 | 383 / 0 | 3.83 | 3.90 | 0.26 | 0.28 | 0.30 | 0 / 253 / 3 | 0 | $0.464 | 89 % / 100 %, 26.0 GB |
| c4 | 1.5k (1.5k measured) tok × 1 q | 4 | 388 / 0 | 3.84 | 3.90 | 1.04 | 1.08 | 1.13 | 778 / 254 / 3 | 0 | $0.463 | 95 % / 100 %, 26.0 GB |
| t500q1-cold | 0.5k (0.6k measured) tok × 1 q | 4 | 391 / 0 | 3.87 | 3.90 | 1.03 | 1.06 | 1.08 | 777 / 251 / 3 | 0 | $0.459 | 94 % / 100 %, 25.9 GB |
| t4000q1-cold | 4k (3.3k measured) tok × 1 q | 4 | 186 / 0 | 1.82 | 2.00 | 2.26 | 2.40 | 2.48 | 1,704 / 573 / 3 | 0 | $0.975 | 92 % / 100 %, 26.5 GB |
| t16000q1-cold | 16k (8.2k measured) tok × 1 q | 4 | 74 / 0 | 0.72 | 0.90 | 5.47 | 7.54 | 9.06 | 3,929 / 1,156 / 4 | 0 | $2.483 | 91 % / 100 %, 27.3 GB |
| t4000q13-cold | 4k (5.0k measured) tok × 13 q | 4 | 107 / 0 | 13.48 | 14.30 | 3.87 | 3.91 | 3.92 | 2,903 / 965 / 10 | 0 | $0.132 | 94 % / 100 %, 26.6 GB |
| router-cold | router (8,691 tok) | 4 | 72 / 0 | 2.98 | 3.80 | 5.92 | 6.60 | 6.60 | 4,367 / 1,531 / 12 | 0 | $0.598 | 99 % / 100 %, 27.3 GB |

### sealed-gpu

oa-att-h100-1: H100 in CC mode + TDX, Psionic Clef-Flash CUDA, NIP-ATT sealed. Model `clef-flash`, plan `full`, 2026-10-11, run on `oa-decision-h100.us-east5-a.c.openagentsgemini.internal`. Price: spot a3-highgpu-1g us-east5 + Confidential Computing surcharge.

| Point | Shape | Conc | OK / failed | Decisions/s | Peak 10 s | p50 s | p90 s | p99 s | verify / seal / relay ms (p50) | busy retries | $ / 1,000 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| c1 | 1.5k (1.5k measured) tok × 1 q | 1 | 130 / 0 | 1.44 | 1.50 | 0.70 | 0.73 | 0.83 | 328 / 0 / 366 | 0 | $1.321 |
| c2 | 1.5k (1.5k measured) tok × 1 q | 2 | 266 / 0 | 2.94 | 3.00 | 0.68 | 0.72 | 0.78 | 315 / 0 / 355 | 0 | $0.646 |
| c4 | 1.5k (1.5k measured) tok × 1 q | 4 | 277 / 0 | 2.38 | 3.20 | 0.66 | 0.71 | 0.76 | 308 / 0 / 347 | 2 | $0.795 |
| c8 | 1.5k (1.5k measured) tok × 1 q | 8 | 279 / 4 (503:['busy', 'no free slot'] ×4) | 1.21 | 3.20 | 0.65 | 0.70 | 0.78 | 303 / 0 / 337 | 6 | $1.569 |
| c16 | 1.5k (1.5k measured) tok × 1 q | 16 | 325 / 182 (0:oa-att: the relay closed the query: "rate-limited: REQ rate exceeded" ×173, 503:['busy', 'no free slot'] ×9) | 1.41 | 4.70 | 0.66 | 0.71 | 116 | 310 / 0 / 349 | 14 | $1.348 |
| t500q1-cold | 0.5k (0.6k measured) tok × 1 q | 2 | 282 / 0 | 3.11 | 3.20 | 0.64 | 0.69 | 0.73 | 299 / 0 / 331 | 0 | $0.610 |
| t4000q1-cold | 4k (3.3k measured) tok × 1 q | 2 | 239 / 0 | 2.63 | 2.80 | 0.76 | 0.82 | 0.93 | 302 / 0 / 450 | 0 | $0.720 |
| t16000q1-cold | 16k (8.3k measured) tok × 1 q | 2 | 154 / 0 | 1.69 | 1.90 | 1.24 | 1.56 | 1.86 | 293 / 0 / 940 | 0 | $1.126 |
| t1500q1-warm | 1.5k (1.4k measured) tok × 1 q warm | 2 | 351 / 0 | 3.88 | 4.00 | 0.51 | 0.55 | 0.63 | 281 / 0 / 214 | 0 | $0.489 |

### sealed-cpu

oa-att-tdx-1: 8 TDX vCPUs, Psionic Clef-Flash CPU, NIP-ATT sealed. Model `clef-flash`, plan `full`, 2026-10-11, run on `coderos-4080`. Price: c3-standard-8 + TDX surcharge, us-central1 (always on).

| Point | Shape | Conc | OK / failed | Decisions/s | Peak 10 s | p50 s | p90 s | p99 s | verify / seal / relay ms (p50) | busy retries | $ / 1,000 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| c1 | 0.5k (0.6k measured) tok × 1 q | 1 | 6 / 0 | 0.04 | 0.10 | 28.33 | 28.97 | 31.28 | 538 / 0 / 81 | 0 | $3.472 |
| c2 | 0.5k (0.6k measured) tok × 1 q | 2 | 6 / 1 (503:['busy', 'no free slot'] ×1) | 0.01 | 0.10 | 28.43 | 29.35 | 30.50 | 522 / 0 / 70 | 3 | $9.615 |
| c4 | 0.5k (0.6k measured) tok × 1 q | 4 | 8 / 1 (503:['busy', 'no free slot'] ×1) | 0.02 | 0.10 | 28.84 | 251 | 376 | 527 / 0 / 80 | 8 | $7.353 |
| t500q1-cold | 0.5k (0.6k measured) tok × 1 q | 1 | 6 / 0 | 0.04 | 0.10 | 27.81 | 28.88 | 31.70 | 545 / 0 / 85 | 0 | $3.472 |
| t500q1-warm | 0.5k (0.6k measured) tok × 1 q warm | 1 | 6 / 0 | 0.04 | 0.10 | 27.90 | 28.84 | 29.82 | 517 / 0 / 81 | 0 | $3.378 |

<!-- dt:backends:end -->

## Trend

Best sustained decisions/s at the base shape, per backend and day, from
every run in Cloud Storage (scheduled and by hand).

<!-- dt:trend:start -->
| Backend | Day | Plan | Best decisions/s, 1 question | Concurrency | p99 | $ / 1,000 |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| jev | 2026-10-11 | full | 173 | 64 | 0.76 s | $0.0663 |
| gateway-jev | 2026-10-11 | full | 86.39 | 32 | 0.62 s | $0.0662 |
| gateway-pylon | 2026-10-11 | full | 4.14 | 4 | 1.24 s | $0.0081 |
| vertex-gemini | 2026-10-11 | full | 3.50 | 16 | 6.35 s | $1.722 |
| coderos-clef | 2026-10-11 | full | 4.21 | 1 | 0.27 s | $0.0079 |
| coderos-llamacpp | 2026-10-11 | full | 3.14 | 1 | 0.66 s | $0.0106 |
| h100-clef-flash | 2026-10-11 | full | 9.87 | 4 | 0.43 s | $0.180 |
| h100-clef-27b | 2026-10-11 | full | 3.83 | 1 | 0.30 s | $0.464 |
| sealed-gpu | 2026-10-11 | full | 2.94 | 2 | 0.78 s | $0.646 |
| sealed-cpu | 2026-10-11 | full | 0.04 | 1 | 31.28 s | $3.472 |
<!-- dt:trend:end -->

## First run (2026-10-11): how it was taken

- **Jev and Vertex.** Run from the Mac.
  - Jev's sweep points ran 120 s, except 32 and 64 clients at 60 s; its
    shape points ran 90 s at 4 clients. The cut was to stay at a few
    dollars of Jev.
  - Vertex ran 60–90 s per point under a $2.5 cap, which stopped it before
    its router point.
- **The gateway.** Jev first ran from the Mac. Pylon first ran from
  CoderOS, at 120 s per point.
- **CoderOS-4080.** A second Psionic server on port 18097, built at
  `562aafb82c`, ran beside the production `pylon-clef`, which stayed
  untouched and kept answering, its slowest decision 2.0 s. The text pylon
  was paused for VRAM and restored. Only about 2 GB of VRAM was left, so
  its 16k and `cli_group` requests ran out of GPU memory (HTTP 500).
  Production never did. Its numbers include whatever production traffic
  shared the card.
- **H100.** A plain spot `a3-highgpu-1g` in us-east5-a ran Psionic from
  `42334c876b` (`att-provider-gpu` build, sm_90) with
  `--decision-chunk 2048 --decision-max-tokens 65536`: Flash on one port,
  27B on another, together 25 GB. The sealed GPU ran its current release
  (no `psionic.timing`).
- **H100 time.** 56 minutes plain and 29 minutes sealed, about $9.3. That
  is over the planned hour, because the VM's startup script waited on its
  own log pipe for 23 idle minutes after finishing. This is fixed.
- **Prompt sizes.** The 16k points measured about 8k tokens and the 4k
  points about 3.3k: short files left their share of the state unused.
  The tables show the measured size beside each nominal one. The harness
  now pads the state with unasked context files and assumes 3.8 bytes per
  token, so later runs fill 16k.
- **Total spend.** About $3.6 of Jev direct, $0.9 of Jev under the
  gateway's key, $2.2 of Vertex, and $9.3 of H100.


## Schedule and cost

Three schedules append to the same `runs/YYYY-MM-DD.jsonl`, and the doc's
tables are regenerated from all of them.

| What | Where | When (UTC) | Backends | Cost |
| --- | --- | --- | --- | --- |
| CoderOS daily | user timer `decision-throughput.timer` on coderos-4080 → `~/decision-bench/daily.sh` ([coderos-daily.sh](../../scripts/bench/decision-throughput/coderos-daily.sh)) | daily 09:30 | `coderos-clef` on the `daily` plan. A bench server on 18097 starts, runs and stops; the text pylon is paused and restored only if VRAM is short. It writes as `oa-decision-bench-writer@`, which holds only `storage.objectAdmin` on the bucket | electricity |
| Cloud daily | Cloud Scheduler `decision-throughput-daily` → Cloud Run job `decision-throughput-daily` ([cloud/](../../scripts/bench/decision-throughput/cloud/)), as `oa-decision-bench@` | daily 11:00 | `jev`, `gateway-jev`, `gateway-pylon`, `vertex-gemini`, `sealed-cpu`, on the `daily` plan: 1 and 4 clients (1 for `gateway-jev`), plus 13 questions and the router at one client, each hosted target under a daily spend cap. Afterwards it regenerates this doc's tables and pushes them to `main` | at most $0.60 of Jev direct, $0.40 of Jev under the gateway's key and $0.30 of Vertex, plus Cloud Run time: at most about $1.4 a day |
| H100 weekly | Cloud Scheduler `decision-throughput-h100-weekly` → Compute `instances.insert` of a spot `a3-highgpu-1g` ([h100/](../../scripts/bench/decision-throughput/h100/)) | Saturdays 10:00 | `h100-clef-flash`, `h100-clef-27b`, and the sealed GPU, which the VM wakes, measures and stops. The VM deletes itself when done, and after 75 minutes at most | about $6–7 a week: about 35 minutes of plain H100 plus about 25 of sealed |

- **Changing the harness.** The weekly run reads it from
  `gs://openagentsgemini-decision-throughput/harness/`, so run
  `h100/h100-run.sh upload` after a change. The Cloud Run job carries it in
  its image: `cloud/setup.sh image`, then `cloud/setup.sh job IMAGE`.
  CoderOS keeps a copy in `~/decision-bench/harness`.
- **Pausing.** `cloud/setup.sh pause` pauses the cloud daily run, and
  `gcloud scheduler jobs pause decision-throughput-h100-weekly` the weekly
  one. On CoderOS: `systemctl --user disable --now decision-throughput.timer`.
- **Not scheduled.** The Mac (below) and llama.cpp/Ollama, which are
  reference points only.
- **Where the client runs matters.** The first scheduled run, from Cloud
  Run in us-central1, saw Jev at 6.3 decisions/s from one client (p50
  0.16 s), against 3.5/s from the Mac. Compare a backend's trend only
  within one client location: the `host` field of each point.
- **The pylon path at 16k tokens falls over to Vertex.** On the
  Pylon-first gateway path, 34 of 36 16k-token requests in that run timed
  out at the pylon and were answered by Vertex.


## Run it

```sh
H=scripts/bench/decision-throughput
export CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config            # GCS + Vertex as the automation account
set -a; . ~/work/.secrets/typesafe.env; set +a                     # TYPESAFE_API_KEY, jev only
python3 $H/harness.py --target jev --plan full --corpus gs://openagentsgemini-decision-throughput/corpus/v1 \
    --out results.jsonl --gcs gs://openagentsgemini-decision-throughput/runs
python3 $H/harness.py --target coderos-clef --plan daily ... \
    --gpu-cmd 'nvidia-smi --query-gpu=utilization.gpu,memory.used,memory.total --format=csv,noheader,nounits'
python3 $H/report.py --gcs gs://openagentsgemini-decision-throughput/runs --doc docs/inference/decision-throughput.md
```

- **Plans.** `--plan full | daily | smoke`. `--only c1,c8,t16000q1-cold,router-cold`
  picks points. `--duration`, `--max-conc`, `--budget-usd` and
  `--usd-per-hour` override the target.
- **Spend.** Hosted targets stop once their estimated spend passes
  `budget_usd`.
- **The Mac.** When the Mac is idle (no builds, no other benches), start
  Psionic on Metal, then run the harness against it:

  ```sh
  psionic-openai-server -m Clef-Flash-Q4_K_M.gguf --host 127.0.0.1 --port 18096 --decision-device metal
  python3 -I $H/harness.py --target mac-clef --plan full --corpus … --out mac.jsonl --gcs …
  ```

  There is no GPU sampler for Metal.
