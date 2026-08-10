# Muse Glimmer runtime benchmark on M5 Max

Date: 2026-08-10

Status: complete

Issue: [OpenAgentsInc/omega#307](https://github.com/OpenAgentsInc/omega/issues/307)

## Test system

| Property | Value |
| --- | --- |
| Computer | MacBook Pro, Apple M5 Max (`Mac17,6`) |
| Memory | 128 GiB unified memory |
| OS | macOS 26.4, arm64 |
| llama.cpp | `10349 (62bf73d25)` |
| ExecuTorch | `a3bf5568b81483ca1ec30198f846d10eaaab4b58` |
| Context | 32,768 tokens |
| Parallel slots | 1 |
| Sampling | Temperature 0, seed 42 |
| Target | `muse-glimmer-30B-kquant-17gb.gguf` |
| Drafter | `dflash-kquant.gguf`, block size 16, `n-max=15` |

The llama.cpp revision is the Muse Glimmer merge commit. Homebrew llama.cpp
10330 recognized DFlash flags but rejected the model's `muse-glimmer`
architecture, so these results use the pinned upstream revision.

The downloaded artifact sizes and SHA-256 digests match the pinned Hugging
Face release:

| Artifact | Exact bytes | SHA-256 |
| --- | ---: | --- |
| `muse-glimmer-30B-kquant-17gb.gguf` | 16,756,681,056 | `7e9b74b7c8875e9e265695df9613bf6290f2392e479ce740495a129019c488d8` |
| `dflash-kquant.gguf` | 1,631,205,312 | `27d9a805fa29b943cfb6ad4843367cd4eaaaf06bd452d8cc3e00a2cd18a677bc` |
| `mmproj-kquant.gguf` | 1,400,328,928 | `f48b452316f9b213758e8659444029b961a24a07f99a1abb2a9f88b06f7c00c6` |

The projector was downloaded and verified but was not loaded for this
text-only baseline.

## llama.cpp configurations

Target only:

```sh
llama-server \
  --model muse-glimmer-30B-kquant-17gb.gguf \
  --alias muse-glimmer \
  --ctx-size 32768 \
  --parallel 1 \
  --seed 42 \
  --api-key local \
  --host 127.0.0.1 \
  --port 8000 \
  --metrics \
  --log-timestamps
```

Target plus DFlash:

```sh
llama-server \
  --model muse-glimmer-30B-kquant-17gb.gguf \
  --spec-draft-model dflash-kquant.gguf \
  --spec-type draft-dflash \
  --spec-draft-n-max 15 \
  --alias muse-glimmer \
  --ctx-size 32768 \
  --parallel 1 \
  --seed 42 \
  --api-key local \
  --host 127.0.0.1 \
  --port 8000 \
  --metrics \
  --log-timestamps
```

The DFlash block contains 16 tokens. llama.cpp's documented setting is
`--spec-draft-n-max 15`, which is the largest proposal accepted by this
server build.

## Scenario matrix

The harness sent streamed OpenAI Chat Completions requests and recorded all
SSE content, private reasoning, token counts, llama.cpp timings, first-token
latency, end-to-end latency, and server RSS. Each configuration ran the same
six scenarios three times:

1. Exact instruction following at low reasoning strength.
2. Typed Python code generation at medium reasoning strength.
3. A constrained river-crossing problem at high reasoning strength.
4. Retrieval from an 18,024-token prompt at low reasoning strength.
5. Multi-turn conversation-history recall at low reasoning strength.
6. Compact structured JSON at medium reasoning strength.

Small output budgets were intentional in the first matrix because they expose
how the host handles Muse's separate reasoning and answer streams. The
reported time to first token includes either stream; visible-answer latency is
also preserved in the JSON.

## llama.cpp results

Values below are medians across three runs. Decode speed is llama.cpp's
`predicted_per_second`. A result passes only when the expected marker appears
in the visible answer.

| Scenario | Output budget | Target tok/s | DFlash tok/s | Target TTFT | DFlash TTFT | Target pass | DFlash pass |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Exact instruction | 128 | 13.81 | 8.36 | 0.575 s | 0.704 s | 3/3 | 3/3 |
| Code generation | 256 | 15.19 | 6.97 | 0.440 s | 1.094 s | 0/3 | 0/3 |
| Reasoning | 384 | 12.88 | 6.93 | 0.501 s | 1.012 s | 0/3 | 0/3 |
| 18K retrieval | 128 | 14.07 | 9.51 | 0.305 s hot | 1.897 s hot | 3/3 | 3/3 |
| Multi-turn memory | 128 | 13.81 | 12.12 | 0.410 s | 1.081 s | 3/3 | 3/3 |
| Structured JSON | 192 | 14.84 | 12.06 | 0.419 s | 1.240 s | 0/3 | 0/3 |

Decode-speed ranges show the variance hidden by those medians:

| Scenario | Target range | DFlash range |
| --- | ---: | ---: |
| Exact instruction | 11.96–22.88 tok/s | 5.93–8.87 tok/s |
| Code generation | 13.08–15.55 tok/s | 6.65–9.71 tok/s |
| Reasoning | 11.72–16.67 tok/s | 6.68–10.40 tok/s |
| 18K retrieval | 13.79–14.63 tok/s | 9.48–9.54 tok/s |
| Multi-turn memory | 13.67–14.43 tok/s | 12.03–12.44 tok/s |
| Structured JSON | 13.88–15.20 tok/s | 10.96–18.09 tok/s |

The first exact-instruction target run and third structured-JSON DFlash run
were the largest positive outliers. Three repetitions are enough to expose
them but not enough to estimate a stable tail distribution, so neither outlier
is used as a headline speed.

The code, reasoning, and JSON requests ended with `finish_reason=length` after
spending their entire budgets in `reasoning_content`; the server returned
well-formed SSE but no visible answer. This is an output-budget failure, not a
transport failure. The exact, long-context, and multi-turn scenarios emitted
the expected visible answers on every run.

Visible output was identical between target-only and DFlash for all 18 paired
requests, including the empty visible outputs from budget exhaustion. Private
reasoning was identical for four pairs and different for 14. Completion-token
counts also differed for the exact-instruction and multi-turn scenarios. Two
target-only scenarios produced two distinct private reasoning traces across
their three nominally deterministic repetitions; DFlash repetitions were
internally stable. The benchmark therefore does not establish byte-identical
greedy equivalence for the full reasoning stream. It establishes identical
visible answers for this prompt set and records the variance instead of
assuming equivalence from speculative-decoding theory.

The first 18K retrieval request was the cold-prompt measurement. Target-only
evaluated 18,024 prompt tokens at 264.06 tokens/s with 68.47 seconds to the
first generated token and 74.59 seconds end to end. DFlash evaluated the same
prompt at 216.90 tokens/s with 84.80 seconds to first token and 93.28 seconds
end to end. Later repetitions reused llama.cpp's prefix cache, so their TTFTs
are not cold-context numbers.

| Resource | Target only | Target plus DFlash |
| --- | ---: | ---: |
| Model load time | 11.68 s | 13.42 s |
| Peak server RSS | 16.75 GiB | 19.83 GiB |
| Additional peak RSS | — | 3.08 GiB |

### Conversation-history transport smoke

A separate two-request smoke test used the first streamed answer as the
assistant message in the second request. Both configurations returned
`acknowledged` on turn one and recovered `COBALT-731` on turn two. All four
responses completed with zero malformed SSE events.

| Configuration | Turn 1 TTFT | Turn 1 total | Turn 2 TTFT | Turn 2 total | Peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Target only | 1.794 s | 13.986 s | 1.375 s | 33.587 s | 16.28 GiB |
| Target plus DFlash | 1.450 s | 12.884 s | 1.195 s | 31.242 s | 18.26 GiB |

The second target-only request reused 63 prompt tokens and evaluated 27 new
tokens; the DFlash run exercised the same supplied-history request shape. The
smoke test demonstrates OpenAI-compatible history transport. It does not imply
server-side conversational state: the client must resend prior messages.

### DFlash finding

llama.cpp DFlash was slower than target-only Metal for every scenario in this
matrix. Acceptance depended on the output:

- Exact instruction: 18.3%, mean accepted length 3.74.
- Code generation: about 22%, mean accepted length about 4.25.
- Multi-turn memory: 44.6%, mean accepted length 7.69.
- Structured JSON: 40.5%, mean accepted length 7.07.

The five-layer drafter's overhead outweighed accepted-block savings on this
llama.cpp Metal implementation. This does not contradict Meta's M5 Max result:
Meta measured 26.6 tokens/s solo and 50.2 tokens/s with DFlash using
ExecuTorch's MLX backend, while its published llama.cpp DFlash result is for an
RTX 5090.

The practical llama.cpp recommendation for this M5 Max is target-only until a
newer llama.cpp revision demonstrates a speedup on this workload. Keep DFlash
available as an explicit A/B option instead of enabling it by default.

## ExecuTorch MLX results

The Apple-silicon reference path used ExecuTorch's MLX delegate and the official
prebuilt Metal PTEs at revision
`b0376783689fb024c95b43a063552f938c678ec2`. The canonical tokenizer and chat
template came from the BF16 repository at revision
`f84ecc3a0ea984a4c04542a84269e3d065350a6e`.

| Artifact | Exact bytes | SHA-256 |
| --- | ---: | --- |
| Solo text Metal PTE | 17,945,894,528 | `aa94d2e3e101c3ad9a49f048e452cac1a6c6b64337a1bf6a85349394ed1029ef` |
| DFlash text Metal PTE | 19,641,819,904 | `6fbec8fc06f50e84c1a0e1fb1588bb825b1b5a3bbc314558faec2541a12f42e4` |

The model-specific `muse-glimmer-mlx` workflow produced the arm64
`solo_runner`, `dflash_runner`, and `muse_glimmer_worker` binaries. Direct
runner measurements used a six-token raw continuation, 64 generated tokens,
greedy decoding, and `ignore_eos=true`:

| Direct runner | Block / drafts | Decode | Peak RSS | Output |
| --- | ---: | ---: | ---: | --- |
| Solo | — | 23.57 tok/s | 16.89 GiB | Coherent Paris continuation |
| DFlash default | 4 / 3 | 29.45 tok/s | 20.51 GiB | Byte-identical continuation |
| DFlash midpoint | 8 / 7 | 19.29 tok/s | 20.51 GiB | Byte-identical continuation |
| DFlash maximum | 16 / 15 | 6.24 tok/s | 20.54 GiB | Byte-identical continuation |

The shipped `4/3` DFlash setting was the best of this block sweep. The longer
blocks paid more verification cost than their shallow accepted prefixes could
recover. The direct default improved this prompt by 1.25x. Meta reports 26.6
tok/s solo and 50.2 tok/s DFlash on an averaged M5 Max prompt set; this direct
prompt measured below both published values and did not reproduce the reported
1.8x speedup.

The first attempted solo command included `--seed=42`. The prebuilt PTE has no
sampling method and rejected any seed, top-k, or top-p control. The successful
greedy commands therefore use `temperature=0` and omit `seed`. This is also
required in HTTP request bodies. The worker logs a harmless RE2 parse failure
before selecting its PCRE2 fallback for the tokenizer regex.

### Worker-backed HTTP matrix

The official Python server and `muse_glimmer_worker` exposed
`/v1/models` and `/v1/chat/completions` on localhost. It used the canonical
chat template, template reasoning controls, 32,768 maximum context, one worker,
one physical session, text only, and no tool parser. DFlash used the validated
`block_length=4`, `n_draft=3` defaults.

The server currently buffers generation and then emits a small sequence of SSE
events. Client-observed first-event latency is therefore nearly equal to total
latency and cannot be used to derive decode throughput. The values below use
the worker's `llm_turn_stats` records, which separate prompt evaluation and
decode. Values are medians across three requests.

| Scenario | Solo total | DFlash total | Solo decode | DFlash decode | Solo pass | DFlash pass |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Exact instruction | 3.545 s | 3.458 s | 20.4 tok/s | 19.9 tok/s | 3/3 | 3/3 |
| Code generation | 17.940 s | 14.142 s | 15.1 tok/s | 19.6 tok/s | 0/3 | 0/3 |
| Reasoning | 26.147 s | 16.526 s | 15.4 tok/s | 24.8 tok/s | 0/3 | 0/3 |
| 18K retrieval | 158.540 s | 135.324 s | 11.8 tok/s | 9.7 tok/s | 3/3 | 3/3 |
| Multi-turn memory | 5.091 s | 3.601 s | 16.3 tok/s | 25.5 tok/s | 3/3 | 3/3 |
| Structured JSON | 10.528 s | 6.080 s | 14.0 tok/s | 25.7 tok/s | 3/3 | 3/3 |

The code and reasoning cases used their full 256- and 384-token budgets in
private reasoning, so the absent visible answers are output-budget failures.
All other scenarios emitted the expected visible marker. Every request returned
HTTP 200 with valid SSE. Solo and DFlash produced identical visible output,
private reasoning, finish reason, and completion-token count for all 18 paired
requests.

The long-context medians require care. Solo prompt evaluation was 118.4 tok/s;
DFlash was 140.9 tok/s. Each request began with a fresh anonymous worker
session, so all three repeated the 18,073-token server-rendered prompt instead
of reusing a prefix. Totals ranged from 145.240 to 166.117 seconds solo and
129.808 to 156.963 seconds with DFlash. llama.cpp's cold total was 74.59 seconds
and later repetitions reused its prefix cache. For an interactive agent with
large repeatedly-sent histories, the current ExecuTorch server's lack of an
addressable warm session on this artifact is a material cost.

| Resource | ExecuTorch solo | ExecuTorch DFlash |
| --- | ---: | ---: |
| Worker artifact detection/load | 2.59 s | 5.96 s |
| Peak server plus worker RSS | 17.68 GiB | 22.55 GiB |
| Additional peak RSS | — | 4.87 GiB |

The strict two-request history smoke also passed in both configurations. Solo
returned `remembered` in 2.534 seconds and `cobalt-orchid` in 7.919 seconds;
DFlash took 1.474 and 4.789 seconds. The second request contained the actual
first assistant answer and reasoning, proving client-supplied history through
the canonical template.

### Runtime decision

ExecuTorch MLX is the better DFlash implementation on this Mac: it accelerated
generation-heavy scenarios by roughly 1.3x to 1.8x and preserved the full
greedy output exactly. Its current Python server buffers SSE output and its
anonymous request path does not reuse long prefixes, while llama.cpp streams
token deltas and evaluated the cold 18K prompt substantially faster.

For the first Omega chat integration, use target-only llama.cpp. It has the
simplest OpenAI-compatible streaming path, lower memory use, and the strongest
measured long-prompt latency. Keep ExecuTorch MLX as the performance lane to
improve: native incremental streaming and addressable warm-session reuse are
the two gaps exposed by this benchmark. Hydralisk remains the later NVIDIA
vLLM/SGLang serving lane; Psionic is the owner for native Metal/runtime work.

## Reproduction and evidence

- [Benchmark harness](run_llama_cpp_benchmark.py)
- [Target-only machine-readable results](evidence/2026-08-10-m5-max-target-only.json)
- [Target-only server log](evidence/2026-08-10-m5-max-target-only-server.log)
- [DFlash `n-max=15` machine-readable results](evidence/2026-08-10-m5-max-dflash-n15.json)
- [DFlash `n-max=15` server log](evidence/2026-08-10-m5-max-dflash-n15-server.log)
- [llama.cpp target-only history result](evidence/2026-08-10-m5-max-llama-target-history.json)
- [llama.cpp target-only history server log](evidence/2026-08-10-m5-max-llama-target-history-server.log)
- [llama.cpp DFlash history result](evidence/2026-08-10-m5-max-llama-dflash-history.json)
- [llama.cpp DFlash history server log](evidence/2026-08-10-m5-max-llama-dflash-history-server.log)
- [ExecuTorch MLX solo results](evidence/2026-08-10-m5-max-executorch-mlx-solo.json)
- [ExecuTorch MLX solo history result](evidence/2026-08-10-m5-max-executorch-mlx-solo-history.json)
- [ExecuTorch MLX solo server log](evidence/2026-08-10-m5-max-executorch-mlx-solo-server.log)
- [ExecuTorch MLX DFlash results](evidence/2026-08-10-m5-max-executorch-mlx-dflash-default.json)
- [ExecuTorch MLX DFlash history result](evidence/2026-08-10-m5-max-executorch-mlx-dflash-default-history.json)
- [ExecuTorch MLX DFlash server log](evidence/2026-08-10-m5-max-executorch-mlx-dflash-default-server.log)
- [ExecuTorch direct-runner logs](evidence/direct/)

The JSON files retain the complete prompts, request bodies, streamed visible
and reasoning output, usage, and per-run timing values. Server logs retain
model metadata, cache behavior, and DFlash acceptance statistics.
