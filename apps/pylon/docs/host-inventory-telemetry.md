# Pylon Host Inventory Telemetry

Status: implemented for `0.3.0-rc1` public projection and dashboard display.

`src/inventory.ts` projects host inventory into
`openagents.pylon.host_inventory.v0.3`. The projection is intentionally
public-safe and ref-oriented. It does not expose raw interface names, cache
paths, environment variables, provider tokens, or private topology.

Public fields that can contribute to `assignment-ready` claims:

- supported platform: macOS or Linux;
- CPU core count and sanitized CPU model ref;
- memory totals and free memory in rounded GB;
- home-disk free space in rounded GB, without the path;
- network interface counts only, without interface names or addresses;
- accelerator kind, such as `apple_silicon` or `gpu_unknown`;
- backend health refs for OpenCode, Codex future adapter boundary, Apple FM,
  Gemini, and local model inventory;
- model cache state and public model refs only;
- explicit blocker refs.

Fields that must not drive public `assignment-ready` by themselves:

- raw wallet balance or receive readiness;
- provider auth presence without backend capability checks;
- raw local model cache paths;
- private network topology;
- stale inventory;
- mocked or unavailable telemetry.

`pylon status --json` includes the inventory projection. `pylon inventory --json`
emits the inventory projection directly. The OpenTUI dashboard uses the same
discovery path and displays unavailable inventory separately from fresh
inventory.

Next target: split the generic local-model row into an optional Psionic Qwen3.5
backend projection. The first planned admitted model refs are
`model.psionic.qwen35.0_8b.q8_0` and `model.psionic.qwen35.2b.q8_0`, with raw
GGUF paths and cache paths kept out of public inventory. The full audit and
roadmap live in
`docs/2026-06-09-pylon-qwen35-local-inference-roadmap.md`.
