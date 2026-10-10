# Psionic in OpenAgents

Psionic is the Rust-native ML and inference stack that the OpenAgents Pylon
serves models with. This directory holds the serving path of Psionic as its
own Cargo workspace inside the monorepo.

Imported from OpenAgentsInc/psionic at
`02e0bc85fe6a3c87dd45927314e6950c74c642a9` (committed 2026-09-13, imported 2026-10-07), Apache-2.0
([LICENSE](LICENSE)). Changes after the import happen here.

## What is here

The dependency closure of `psionic-openai-server` (the OpenAI-compatible
server in `psionic-serve`), plus `psionic-provider` and
`psionic-backend-tests`, which its tests need: 30 crates under `crates/`.
`fixtures/` holds only the 23 files those crates embed with `include_str!`.
The rest of the upstream repository (training programs, research lanes,
reports, and its other crates) was not imported.

## Why its own workspace

The root workspace excludes `crates/psionic`, like `crates/openagents-mobile`:

- It is about a million lines, mostly training and evaluation code that
  `psionic-serve` links. Keeping it out of the root workspace keeps the
  default `cargo check` and the Mac, web, and phone builds unchanged.
- It keeps its own lints and dependency versions (for example `sha2` 0.10).
- The CUDA backend compiles its kernels with `nvcc` only when `nvcc` is
  found (`NVCC`, `/opt/cuda`, `/usr/local/cuda`, or `PATH`); otherwise it
  links a stub and loads `libcudart`, `libcublas`, and `libcuda` at run
  time. CUDA builds happen on a machine with the toolkit.

The Pylon provider (`crates/pylon`) talks to `psionic-openai-server` over
its local HTTP API instead of linking it, so a CUDA fault or out-of-memory
error stops the model server and not the provider that publishes beacons.

## Build and run

On a machine with the CUDA 13 toolkit (the RTX 4080 box uses Nix packages;
`scripts/pylon-psionic.sh` sets the paths):

```sh
cargo build --release --manifest-path crates/psionic/Cargo.toml \
  -p psionic-serve --bin psionic-openai-server
psionic-openai-server -m qwen3.5-0.8b-q8_0.gguf --backend cuda \
  --host 127.0.0.1 --port 18080 --mesh-coordination disabled
```

On Apple Silicon, use `--backend metal`. `--backend cpu` works anywhere.
The CUDA and Metal lanes serve `qwen35`, `qwen38`, and `gemma4` GGUF
decoders. [`docs/psionic/README.md`](../../docs/psionic/README.md) covers
what runs where.

A Clef decision GGUF (`general.architecture = clef`, Cloudflare's
Clef-Flash) given with `-m` is served at `POST /v1/systemone` on the CPU
(`--decision-max-tokens`, `--decision-chunk`, `--clef-head`); see
[`docs/inference/clef-native.md`](../../docs/inference/clef-native.md).
