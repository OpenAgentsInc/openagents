# Psionic

Psionic is the Rust-native ML and inference stack that OpenAgents serves
models with. It now lives in this monorepo at [`crates/psionic`](../../crates/psionic/README.md),
imported from OpenAgentsInc/psionic at `02e0bc85fe6a3c87dd45927314e6950c74c642a9`.

## What was imported

The serving path only: the dependency closure of `psionic-openai-server`
(30 crates, including `psionic-serve`, `psionic-models`, `psionic-runtime`,
and the CPU, CUDA, and Metal backends), plus the fixture files those crates
embed at compile time. The upstream repository's training programs,
research lanes, and reports stay upstream.

`crates/psionic` is its own Cargo workspace, excluded from the root, in the
same way as `crates/openagents-mobile`. Build it with
`--manifest-path crates/psionic/Cargo.toml`. The default `cargo check`, and
the Mac, web, and phone builds, do not compile it.

## What it serves

| Backend | Families | Where |
| --- | --- | --- |
| `cuda` | `qwen35`, `qwen38`, `gemma4` GGUF decoders | NVIDIA GPUs; kernels compile when `nvcc` is present |
| `metal` | `qwen35`, `qwen38`, `gemma4` | Apple Silicon |
| `cpu` | Every admitted GGUF decoder family | Anywhere |

`psionic-openai-server` serves `/v1/models`, `/v1/chat/completions`, and
`/v1/responses` on the address you give it. On the RTX 4080 box, Qwen3.5
0.8B Q8_0 loads in about 2 GB of video memory and answers a short prompt in
about 0.2 seconds.

## Who uses it

The Pylon provider ([`docs/compute/pylon.md`](../compute/pylon.md)) runs
jobs on a local `psionic-openai-server`. `scripts/pylon-psionic.sh` builds
and starts it.

## History

An earlier, narrower import (`crates/psionic-gguf`: GGUF parsing, a
tokenizer, Qwen3.5, and Metal dispatch) was removed on 2026-09-18 in
`dabc08102f`. It was not restored, because the imported serving path
already covers GGUF loading and Qwen3.5 on Metal for Mac pylons.
