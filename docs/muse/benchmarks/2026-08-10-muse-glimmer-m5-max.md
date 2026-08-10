# Muse Glimmer text-runtime benchmark on M5 Max

Date: 2026-08-10

Status: reproducible baseline; text only

This report measures the official 17 GB Muse Glimmer target alone and with the
official quantized DFlash drafter. It covers two distinct paths:

- llama.cpp provides the OpenAI-compatible localhost baseline that Omega can
  consume.
- ExecuTorch with the MLX backend reproduces Meta's native Apple-silicon
  reference path.

The experiment did not load the vision projector, use tools, route requests
through Omega, or perform any network or trading action. The result is allowed
to be negative: neither measured DFlash path reached Meta's published M5 Max
headline on this run.

## Reproduction pins

### Machine

- MacBook Pro `Mac17,6`
- Apple M5 Max, 18 CPU cores (6 Super and 12 Performance)
- 128 GB unified memory
- macOS 26.4 (`25E246`)

Machine serial numbers and device identifiers are intentionally omitted.

### Models

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `muse-glimmer-30B-kquant-17gb.gguf` | 16,756,681,056 | `7e9b74b7c8875e9e265695df9613bf6290f2392e479ce740495a129019c488d8` |
| `dflash-kquant.gguf` | 1,631,205,312 | `27d9a805fa29b943cfb6ad4843367cd4eaaaf06bd452d8cc3e00a2cd18a677bc` |
| `tokenizer.json` | local metadata | `c9dbee66967b58f31a7c27f723c3760da3526ccd0427578e8905b0abb0031c4d` |

The GGUF repository revision is
`93769bc7ab5ad1e9cd22d857e3138cf5d977ae81`. The tokenizer came from the
official `meta-models/Muse-Glimmer-30B` repository.

### Runtimes

| Runtime | Revision |
| --- | --- |
| llama.cpp | `62bf73d25c53b8161f8a22894d4f90c4aebbd7d0` (`llama-server` build 10349) |
| ExecuTorch | `a3bf5568b81483ca1ec30198f846d10eaaab4b58` |
| ExecuTorch MLX submodule | `7a1d4f5c12ac82f4b4d0a6e71538d89ca0605247` |

The locally built runner hashes were:

- `solo_runner`: `f204a4eb206d73bac130b9264826cb9f7778f196bcecb4bc6e8792de57b8b418`
- `dflash_runner`: `79ed066162d84f7245420c237dda8e407632ab31d171a921f1cd97d8f0628773`

## llama.cpp HTTP baseline

Both servers bound only to `127.0.0.1`, used one slot, advertised the
`muse-glimmer` alias, and reported a 32,768-token context through
`GET /v1/models`. The target-only run used isolated port `18000`; the clean
history reruns used ports `18011` and `18012`. A pre-existing process on port
`8000` and all evidence from it were excluded.

The command shape was:

```sh
LLAMA_CPP="$HOME/work/llama.cpp"
MODEL_DIR="$HOME/models/muse-glimmer"

"$LLAMA_CPP/build/bin/llama-server" \
  --model "$MODEL_DIR/muse-glimmer-30B-kquant-17gb.gguf" \
  --alias muse-glimmer \
  --ctx-size 32768 \
  --parallel 1 \
  --seed 42 \
  --api-key local \
  --host 127.0.0.1 \
  --port 18000 \
  --metrics \
  --no-webui
```

The DFlash configuration added:

```sh
--spec-draft-model "$MODEL_DIR/dflash-kquant.gguf" \
--spec-type draft-dflash
```

The checked-in [harness](2026-08-10-muse-glimmer-m5-max/harness.py) records the
launch vector, `/v1/models`, machine-readable requests, raw streamed SSE,
Prometheus metrics, server timing log, peak RSS samples, and clean shutdown.
It uses temperature zero, seed 42, a 128-token cap, and three repetitions of
each fixed prompt:

1. `Reply with exactly: muse-ready`
2. `Return only the integer result of 37 * 19.`
3. `Return exactly these three words separated by single spaces: amber cedar quartz`

### Results

All time and throughput cells are median with the observed three-run range in
parentheses. TTFT is time to the first streamed content or reasoning token in
the dedicated history runs; the original fixed matrix recorded visible
content TTFT.

| Prompt | Configuration | TTFT seconds | End-to-end seconds | Prompt tok/s | Decode tok/s | Completion tokens |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | target only | 5.955 (4.986-8.022) | 6.127 (5.168-8.271) | 22.64 (20.16-103.22) | 12.98 (9.92-15.39) | 76 |
| 1 | DFlash | 13.375 (10.723-18.805) | 13.514 (10.917-18.941) | 25.94 (25.32-31.39) | 7.78 (4.59-8.01) | 86 |
| 2 | target only | 8.828 (8.316-9.287) | 8.955 (8.359-9.341) | 25.55 (24.58-25.63) | 12.86 (12.63-13.51) | 110 |
| 2 | DFlash | 14.611 (11.220-17.625) | 14.963 (11.573-18.233) | 21.31 (14.44-36.49) | 7.48 (6.23-9.81) | 110 |
| 3 | target only | 9.775 (9.741-9.820) | 9.829 (9.802-9.874) | 26.77 (14.80-36.68) | 13.54 (13.33-13.82) | 128 |
| 3 | DFlash | no visible content | 15.104 (13.936-20.161) | 11.35 (8.13-44.83) | 8.85 (6.51-9.50) | 128 |

Target-only startup to a successful `/v1/models` response was 6.735 seconds
with peak RSS 17,144,656 KiB. DFlash startup was 44.851 seconds with peak RSS
19,228,848 KiB. Both servers exited zero.

The first two prompts were visibly identical across configurations. Prompt 3
was not: the target emitted the truncated visible prefix `amber ced`, while
DFlash spent the full 128-token budget in reasoning and emitted no visible
content. This is recorded as a greedy-equivalence failure, not hidden as a
successful text response. Every request still completed with finite timing,
valid SSE framing, and usage metadata.

DFlash draft acceptance across the nine fixed requests ranged from 53.5% to
69.7%; the three per-prompt triplets were 53/99, 74/108, and 85/122 accepted
tokens. Despite that acceptance, DFlash was slower and materially more
variable than target-only in this llama.cpp revision.

### Supplied-history check

Fresh, isolated 512-token runs supplied the complete prior conversation on the
second request. Both target-only and DFlash returned `acknowledged` on turn one
and `COBALT-731` on turn two. There were zero malformed SSE events and both
servers exited zero. DFlash accepted 86/141 draft tokens on turn one and
271/477 on turn two.

## ExecuTorch MLX reference path

ExecuTorch was built from the pinned revision using its `mlx-release` and
`muse-glimmer-mlx` CMake presets. The text-only exports used float16 activation
storage and the default 131,072-token maximum sequence length.

```sh
python -m executorch.examples.models.muse_glimmer.export.export_solo \
  --gguf "$MODEL_DIR/muse-glimmer-30B-kquant-17gb.gguf" \
  --backend mlx \
  --output-dir "$EVIDENCE_DIR/solo"

python -m executorch.examples.models.muse_glimmer.export.export_dflash \
  --target-gguf "$MODEL_DIR/muse-glimmer-30B-kquant-17gb.gguf" \
  --draft-gguf "$MODEL_DIR/dflash-kquant.gguf" \
  --backend mlx \
  --output-dir "$EVIDENCE_DIR/dflash"
```

| Export | PTE bytes | PTE SHA-256 | Wall time | Maximum RSS | Peak footprint |
| --- | ---: | --- | ---: | ---: | ---: |
| solo | 17,945,894,528 | `0aa5a6081ebf75dc6b70f1e0afc76cdeddc31484be5b3b80573ab99c46ebf5eb` | 289.21 s | 55,697,326,080 B | 69,591,501,608 B |
| DFlash | 19,641,819,904 | `184a261980d66b71f722aa11799fda9540b7b2c3efd8ecde4117dc32cf6c0f70` | 199.30 s | 56,346,935,296 B | 74,633,468,288 B |

The first DFlash export attempt received external SIGTERM before lowering and
is retained as `dflash-export.log`. Its clean retry used a new output directory,
completed normally, and is `dflash-export-retry1.log`.

The benchmark prompt was `The capital of France is`. Each configuration ran
three separate processes at temperature zero for exactly 128 generated tokens.
The solo runner's pinned engine path rejects a nonzero seed for this export, so
solo used seed zero with greedy temperature; DFlash used seed 42 with greedy
temperature. Commands were:

```sh
./solo_runner \
  --model_path="$EVIDENCE_DIR/solo/model.pte" \
  --tokenizer_path="$EVIDENCE_DIR/tokenizer/tokenizer.json" \
  --prompt='The capital of France is' \
  --temperature=0 --seed=0 --max_new_tokens=128 --ignore_eos=true

./dflash_runner \
  --model_path="$EVIDENCE_DIR/dflash/model.pte" \
  --tokenizer_path="$EVIDENCE_DIR/tokenizer/tokenizer.json" \
  --prompt='The capital of France is' \
  --temperature=0 --seed=42 --max_new_tokens=128 --ignore_eos=true
```

Each command was wrapped with `/usr/bin/time -l`. Raw logs and exact visible
continuations are in the artifact directory.

| Configuration | Decode tok/s, three runs | Median (range) | Wall seconds | Maximum RSS |
| --- | --- | ---: | --- | ---: |
| solo | 14.5, 13.5, 13.6 | 13.6 (13.5-14.5) | 13.00, 12.22, 11.93 | 18,141,954,048 B max |
| DFlash | 15.7, 14.1, 10.8 | 14.1 (10.8-15.7) | 10.34, 11.52, 14.68 | 22,017,605,632 B max |

All six visible continuations have SHA-256
`0d5972f008e6a835f852c1efdb4987619ac1950d333d53f9a4f7464f9589527b`,
so the measured greedy output is byte-identical. The DFlash repetitions also
report the same proposal pattern: attempts `[66, 43, 16]` and accepts
`[43, 16, 3]` by speculative row.

The measured median DFlash speedup over solo was only 1.04x. Meta's published
M5 Max reference is 26.6 tok/s solo and 50.2 tok/s with DFlash (1.8x). These
local medians are 51.1% and 28.1% of those respective references. The DFlash
coefficient of variation was 15.1%, versus 3.2% for solo. This comparison is
diagnostic rather than a like-for-like claim: the public headline does not
fully specify prompt length, thermal state, runner revision, or export details.

## Disposition

The llama.cpp transport baseline passes: direct model discovery, streamed text,
three repetitions per configuration, supplied two-turn history, finite metrics,
valid SSE, and clean shutdown are all demonstrated. DFlash is not recommended
as a default for this pinned llama.cpp build because it regressed latency and
failed one visible-output equivalence case at the fixed token cap.

The pinned ExecuTorch/MLX path also runs cleanly and preserves greedy output,
but its measured DFlash gain is small and unstable relative to Meta's published
reference. Keep both results as baselines; do not present either as proof of
Omega chat, tool use, image input, or production readiness.

## Evidence layout

The [artifact directory](2026-08-10-muse-glimmer-m5-max/) contains:

- `harness.py`: the exact localhost HTTP harness;
- `raw/llama-target` and `raw/llama-dflash`: requests, SSE, model discovery,
  metrics, launch vectors, result JSON, and server logs;
- `raw/history-target` and `raw/history-dflash`: the clean two-turn history
  reruns;
- `raw/executorch/exports`: raw export and `/usr/bin/time -l` output; and
- `raw/executorch/runners`: all six raw runner logs and extracted continuation
  files.

Public artifacts replace the local home-directory prefix with `$HOME`; no model
weights, generated PTE files, credentials, serial numbers, or device identifiers
are committed.
