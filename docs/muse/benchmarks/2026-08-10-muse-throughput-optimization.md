# Muse Glimmer throughput optimization on M5 Max

Date: 2026-08-10

Status: complete

Implementation: [Omega `1115dc54df`](https://github.com/OpenAgentsInc/omega/commit/1115dc54df98c9c57606fd7addfda9ea21f45004)

This follow-up optimized the text-chat workload exposed by the first Omega
acceptance. That run sent 20,561 prompt tokens and decoded at 12.87 tokens per
second. The changes address both sides of that result:

- Omega now gives only `Muse Glimmer (Local)` a compact text-chat system
  prompt. It excludes project context, instruction files, skills, executors,
  and tool guidance while preserving conversation history. The existing
  `Omega Agent` prompt path is unchanged.
- llama.cpp uses Q8 KV cache, Flash Attention, and DFlash with a three-token
  proposal limit. The prior matrix used the maximum 15-token proposal and
  therefore measured the wrong operating point for this Mac.

## Test system and method

The machine, model artifacts, llama.cpp revision, 32,768-token context, single
parallel slot, temperature zero, and seed 42 match the
[HTTP runtime matrix](2026-08-10-m5-max-http-runtime-matrix.md). Each request
used streamed OpenAI Chat Completions. Decode speed is llama.cpp's server-side
`predicted_per_second`; correctness requires the expected visible marker.

Two workloads were used:

1. The 18,073-token retrieval case detects prefill and long-context
   regressions.
2. The 62-token exact-instruction case approximates the small request envelope
   that the new Omega text-chat mode is intended to produce. The Omega unit
   test enforces a system-prompt ceiling of 512 bytes, but the desktop UI run
   was not used to claim an exact post-change token count because GPUI's
   executor popover is absent from macOS accessibility automation.

All recorded requests returned HTTP 200 and the expected visible answer.

## Long-context screening

| Configuration | Prompt evaluation | Decode | Total | Notes |
| --- | ---: | ---: | ---: | --- |
| Target, F16 KV, Flash Attention auto | 379.59 tok/s | 19.65 tok/s | 52.091 s | Clean-machine control |
| Target, Q8 KV, Flash Attention on | 392.19 tok/s | 19.67 tok/s | 50.556 s | Best conservative target-only setting |
| Target, Q4 KV, Flash Attention on | 418.41 tok/s | 19.52 tok/s | 49.343 s | Faster prefill; completion length changed |
| DFlash `n_max=3`, F16 KV, cold | 359.67 tok/s | 23.07 tok/s | 54.065 s | 60/81 draft tokens accepted |
| DFlash `n_max=3`, F16 KV, cached repeat | Cached | 16.93 tok/s | 5.198 s | Shows long-run variance |

Q8 KV improved long-context prefill by 3.3% without changing decode speed. Q4
improved prefill further but slightly reduced decode speed and changed the
completion length, so Q8 is the default.

DFlash `n_max=3` improved the cold long-context decode rate by 17.4%, but its
slower prefill made the cold request 1.974 seconds slower overall. Its cached
repeat also fell below target-only decode speed. DFlash is therefore justified
for compact chat and generation-heavy turns, not as a universal long-context
win.

## Compact-chat sweep

The local DFlash neighborhood was screened before the matched three-run test:

| DFlash setting | Decode | Draft acceptance | Result |
| --- | ---: | ---: | --- |
| `n_max=2`, Q8 KV | 19.01 tok/s | 41/68, 60.3% | Regression |
| `n_max=3`, F16 KV | 34.42 tok/s | 53/99, 53.5% | Improvement |
| `n_max=4`, F16 KV | 20.52 tok/s | 55/124, 44.4% | Regression |

The three-token proposal was then compared with target-only using Q8 KV and
three identical exact-instruction requests:

| Repetition | Target only | DFlash `n_max=3` |
| ---: | ---: | ---: |
| 1 | 28.89 tok/s | 37.87 tok/s |
| 2 | 28.95 tok/s | 38.34 tok/s |
| 3 | 28.95 tok/s | 35.74 tok/s |
| **Median** | **28.95 tok/s** | **37.87 tok/s** |

The optimized DFlash median is **30.8% faster** than the matched target-only
median and **2.94x** the first accepted Omega turn's 12.87 tok/s. Visible output
was `muse-ready` in every run. Target-only generated 76 completion tokens and
DFlash generated 86, so this establishes a throughput and visible-correctness
win, not byte-identical private reasoning.

The measured rate also remains below Meta's published 50.2 tok/s M5 Max result
for ExecuTorch MLX. The existing direct MLX test reached 29.45 tok/s with its
shipped 4/3 DFlash configuration; the HTTP matrix records where MLX wins and
where its buffered streaming and cold anonymous sessions remain costly.

## Recommended llama.cpp command

```sh
llama-server \
  --model muse-glimmer-30B-kquant-17gb.gguf \
  --spec-draft-model dflash-kquant.gguf \
  --spec-type draft-dflash \
  --spec-draft-n-max 3 \
  --spec-draft-n-min 0 \
  --spec-draft-p-min 0.00 \
  --spec-draft-backend-sampling \
  --flash-attn on \
  --cache-type-k q8_0 \
  --cache-type-v q8_0 \
  --alias muse-glimmer \
  --ctx-size 32768 \
  --parallel 1 \
  --seed 42 \
  --api-key local \
  --host 127.0.0.1 \
  --port 8000 \
  --metrics \
  --log-timestamps \
  --no-webui
```

For a workload dominated by cold prompts near the context limit, remove the
draft model and retain Q8 KV plus Flash Attention. For ordinary compact chat,
the three-token DFlash command is the measured default.

## Omega validation

Focused validation for the compact mode passed:

- the request-construction test first creates an agentic prompt-cache layout,
  then proves Muse bypasses it;
- the compact prompt stays below 512 bytes and excludes project context,
  `AGENTS.md`, skills, tools, executor catalog, and Omega coding-agent text;
- conversation history remains in the request and tools remain absent;
- new and restored Muse threads receive the compact policy;
- switching back to `Omega Agent` retains its existing prompt path;
- `cargo test -p agent
  text_only_chat_uses_a_compact_prompt_and_bypasses_agent_prompt_cache` passed;
- `cargo test -p agent_ui
  muse_glimmer_selection_is_isolated_from_omega_agent` passed;
- `cargo build --profile release-fast -p omega` passed.
- `./script/clippy -p agent -p agent_ui` passed on the committed change.

Raw result files are indexed in [the benchmark evidence README](README.md).
