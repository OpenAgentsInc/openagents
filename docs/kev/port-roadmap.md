# Kev port roadmap

**Status:** in progress. The kev mechanism is being ported from the Python
reference (`~/work/projects/repos/kev`, `jaredpalmer/kev`) into this
repository as `crates/kev`, a Rust implementation on
[candle](https://crates.io/crates/candle-core) with the TypeSafe
`/v1/systemone` wire contract. The work sequence is tracked by
[openagents#9337](https://github.com/OpenAgentsInc/openagents/issues/9337)
and the issues below; each one names what gets pulled over, where it lands,
and the evidence that proves it.

The deprecated `psionic` repository informed earlier drafts of this plan.
Nothing is ported from there — the port pulls from the kev reference only.

## Order and rationale

The sequence is dependency order: data before text, text before structure,
structure before weights, weights before serving. Every step produces
verifiable artifacts before the next consumes them.

| # | Issue | Pulls over | Lands in | Proved by |
| --- | --- | --- | --- | --- |
| 1 | [#9338](https://github.com/OpenAgentsInc/openagents/issues/9338) | Golden fixtures + converted artifacts from the Python reference | `crates/kev/fixtures/`, `~/work/kev-artifacts/` | `manifest.json` digests; rerunnable `gen_fixtures.py` |
| 2 | [#9339](https://github.com/OpenAgentsInc/openagents/issues/9339) | Request/answer models, `render()`, `to_record()`, `user_tokens()` sanitize | `crates/kev` `api.rs`, `render.rs` | Every corpus request renders byte-identical records and metadata |
| 3 | [#9340](https://github.com/OpenAgentsInc/openagents/issues/9340) | `encode()` packing: `seg`/`pos`/`opt`, `decide_idx`, `opt_idx`, block-causal mask | `crates/kev` `encode.rs`, `mask.rs` | Every `encodings/` fixture reproduced exactly |
| 4 | [#9341](https://github.com/OpenAgentsInc/openagents/issues/9341) | Qwen2.5-0.5B forward on candle + safetensors load + inference LoRA on all seven projections | `crates/kev` `model.rs` (candle backbone), `lora.rs` | Hidden states at decide/opt positions match reference within fp32 tolerance |
| 5 | [#9342](https://github.com/OpenAgentsInc/openagents/issues/9342) | `PointerHead` (two linear maps, scaled dot product, per-question softmax) + packed forward | `crates/kev` `head.rs`, `model.rs` | All `golden/` probabilities within tolerance; packed-vs-separate within `2.21e-6` fixture bound |
| 6 | [#9343](https://github.com/OpenAgentsInc/openagents/issues/9343) | `kev-serve` binary: `POST /v1/systemone`, `GET /v1/models`, typed refusals | `crates/kev` `serve.rs`, `src/bin/kev_serve.rs` | `jev` client round-trips all three question types unmodified |
| 7 | [#9344](https://github.com/OpenAgentsInc/openagents/issues/9344) | Metal run, all four mechanism probes, docs | this directory + `fixtures/` | Probe outputs committed; status table flips to implemented |

Steps 2–3 carry no weights and are exact-match tests. Step 4 is the first
numerics-sensitive step; step 5 closes the correctness loop on CPU before
any serving or backend work starts.

## Artifact layout

- `~/work/kev-artifacts/kev-0.5b/` — `adapter_model.safetensors`,
  `head.safetensors` (converted from `head.pt`), tokenizer files,
  `adapter_config.json`, `eval.json`.
- `~/work/kev-artifacts/qwen25-0.5b/` — base `model.safetensors`,
  `config.json`, tokenizer files.
- `crates/kev/fixtures/manifest.json` — the sha256 of every file above.

## What this sequence does not cover

Fleet fan-out, earn admission, catalog rows, and the service door at
`openagents.com` are the separate program in [`mesh-plan.md`](mesh-plan.md).
This sequence ends at kev answering `/v1/systemone` on this machine from
openagents code, with the mechanism probes passing.
