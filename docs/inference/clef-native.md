# Clef served natively by Psionic (plan, 2026-10-09)

This page covers serving Cloudflare's Clef decision models in our own engine,
**with no Ollama**. The full plan lives with the engine, in
[psionic `docs/CLEF_NATIVE_PLAN.md`](https://github.com/OpenAgentsInc/psionic/blob/main/docs/CLEF_NATIVE_PLAN.md).
It holds the architecture, the exact head math, the route spec, the
correctness plan and the speed targets. The measurements it starts from are
in [clef-self-host.md](clef-self-host.md).

## In short

- **What Clef is.** Clef is a Qwen3.5-9B backbone (Qwen3.8-27B for Clef 27B)
  plus a joint schema head of about 122M parameters. The backbone reads the
  whole prompt in a single pass and generates no tokens. The head then reads
  every hidden row and scores every option of every question at once. The
  26-option and 64 KiB limits are Ollama's, not the model's. The 16,384-token
  limit is the head's trained length.
- **Main gap in Psionic.** Psionic reads prompts one token at a time on every
  backend, so it needs batched, chunked prompt processing on CUDA and Metal.
  Psionic does not yet have the encoder, the head or the route either.
  Chunked prompt processing also removes llama.cpp's rule that the whole
  prompt must fit in one batch.
- **Route.** The route is `POST /v1/systemone` on `psionic-openai-server`, with
  the Jev request and response shape.
  - It accepts up to 255 options.
  - A request over the token budget is refused with
    `{"error":{"code":"not_admitted"}}`. The judge chain fails over on that
    code, and it does not fail over on a 400 or 413.
  - It truncates the state only when the request asks for it.
- **Speed targets.**
  - RTX 4080: at least as fast as llama.cpp b11538 (0.19 / 0.65 / 3.25 s at
    1k / 4k / 16k tokens), in 7.5 GB of VRAM or less.
  - M5 Max: faster than Ollama, and about level with the MLX build.
- **Where the code goes.** Engine code lands in `crates/psionic`, the copy
  Pylon builds. The psionic repo tracks the engine issues.

## Issues

| | Issue |
| --- | --- |
| M1: correct CPU lane, encoder, head, route (first) | [psionic#1159](https://github.com/OpenAgentsInc/psionic/issues/1159) |
| M2: CUDA batched prompt processing, head on the GPU (RTX 4080) | [psionic#1160](https://github.com/OpenAgentsInc/psionic/issues/1160) |
| M3: Metal batched prompt processing, head on the GPU (M5 Max) | [psionic#1161](https://github.com/OpenAgentsInc/psionic/issues/1161) |
| M4: Clef 27B, images, prefix reuse, batching | [psionic#1162](https://github.com/OpenAgentsInc/psionic/issues/1162) |
| Judge door on Psionic `/v1/systemone` (follows #11189) | [#11191](https://github.com/OpenAgentsInc/openagents/issues/11191) |
| Pylon sells decisions (`pylon/decision`) | [#11192](https://github.com/OpenAgentsInc/openagents/issues/11192) |
| Split the router decision so Clef can take it | [#11193](https://github.com/OpenAgentsInc/openagents/issues/11193) |
