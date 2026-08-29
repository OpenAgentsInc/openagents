# Owner intent: Psionic local inference in the OpenAgents binary

- Class: owner intent
- Status: accepted 2026-08-29
- Source: current owner conversation
- Ledger: this file plus [PLAN.md](./PLAN.md)
- Feature issue: none. Repository policy keeps GitHub issues for reproducible
  bugs. This folder is the work-packet ledger.

## Outcome

Pull **only the parts of Psionic required for local inference** into this
repository. The first product target is the Qwen 3.8 lane Coder already runs
through Ollama (`qwen3.8:27b-mtp-q8_0` on the development machine).

Those parts live in this repo. The CLI uses them. The released `openagents`
binary ships them. Users should not need a separate Psionic checkout, a
Cargo invocation, or a second engine installer for that local lane.

## Owner decisions

1. **In-process in the `openagents` binary.** The 2026-08-26 sidecar analysis
   recommended a separately versioned `psionic-openai-server` process and
   rejected linking the full `psionic-serve` graph into `openagents`. That
   recommendation is superseded **for this slice**. The product command is
   still `openagents inference`. The execution library is compiled into
   `openagents`, not fetched as a second artifact.
2. **Narrow import, not the Psionic monorepo.** Do not copy training,
   cluster, mesh, Tassadar, gym, eval research, speech, MLX product lanes,
   or unrelated model families. Import the smallest crate graph that can
   admit a Qwen 3.8 GGUF, run it on an admitted backend, and speak to Coder.
3. **Qwen 3.8 first.** Bind one content-addressed GGUF (digest, family,
   tokenizer, chat template, quantization, license) before claiming
   equivalence with the current Ollama tag. The Ollama name is a discovery
   string, not the artifact contract.
4. **Two CLI namespaces.**
   - `openagents inference` — product lifecycle: register models, serve,
     status, stop, doctor, and the Coder local engine.
   - `openagents psionic` — harness and library work: inspect artifacts,
     list backends, run admission, and other commands that grow the ML
     library inside this repo.
5. **Ollama stays until replacement gates pass.** Do not delete `--local` /
   `ollama:` in the first landing. Do not fall back silently from Psionic
   to Ollama.
6. **Weights stay out of git and out of the binary.** The binary carries
   code and kernels. Model files live in a user-local content-addressed
   store after an explicit add/download with license disclosure.

## Why this is allowed here

`AGENTS.md` forbids new Rust surfaces outside the Cloud crates **unless the
owner directs it**. This document is that direction for a local-inference
crate family under `crates/`, consumed by `crates/openagents-cli`.

The first implementation commit that adds those crates must amend `AGENTS.md`
in the same change so the contract matches the tree.

## What this is not

- Not a revival of the retired Tassadar / distributed-training program.
- Not a live Cloud Psionic worker. `oa-node psionic attach` remains the
  file-backed Cloud adapter until a later packet.
- Not an instruction to vendor the entire `OpenAgentsInc/psionic` tree.
- Not permission to relicense imported crates as MIT. Psionic is Apache-2.0.
  Imported crates keep Apache-2.0. The CLI notices file must list them.
- Not a seven-platform promise on day one. The first useful ship is the
  platform where Ollama already serves this model (Apple Silicon). Other
  CLI targets may compile a CPU path or refuse with an explicit
  unsupported-backend message.

## Success

A machine that today runs `openagents coder --model ollama:qwen3.8:27b-mtp-q8_0`
can instead:

1. `openagents inference add` a pinned Qwen 3.8 GGUF.
2. `openagents coder --local` (or `--model psionic:<id>`) complete a
   multi-round tool turn **inside this process**.
3. See engine, backend, and artifact digest recorded in ATIF.

That is the product. Broader Psionic-in-the-harness work rides on
`openagents psionic` after this lane works.
