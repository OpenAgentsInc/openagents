# Kev port roadmap

**Status:** implemented. The kev mechanism is ported from the Python
reference (`~/work/projects/repos/kev`, `jaredpalmer/kev`) into this
repository as `crates/kev`, a Rust implementation on
[candle](https://crates.io/crates/candle-core) with the TypeSafe
`/v1/systemone` wire contract. Every issue in the sequence is closed; the
measurements below are the recorded evidence.

The deprecated `psionic` repository informed earlier drafts of this plan.
Nothing is ported from there — the port pulls from the kev reference only.

## Order and rationale

The sequence is dependency order: data before text, text before structure,
structure before weights, weights before serving. Every step produces
verifiable artifacts before the next consumes them.

| # | Issue | Pulls over | Lands in | Proved by |
| --- | --- | --- | --- | --- |
| 1 | [#9338](https://github.com/OpenAgentsInc/openagents/issues/9338) | Golden fixtures + converted artifacts from the Python reference | `crates/kev/fixtures/`, `~/work/kev-artifacts/` | `manifest.json` digests; rerunnable `gen_fixtures.py` |
| 2 | [#9339](https://github.com/OpenAgentsInc/openagents/issues/9339) | Request/answer models, `render()`, `to_record()`, `to_answers()`, `sanitize()` | `crates/kev` `api.rs`, `render.rs`, `error.rs` | Every corpus request renders byte-identical records and metadata |
| 3 | [#9340](https://github.com/OpenAgentsInc/openagents/issues/9340) | `encode()` packing: `seg`/`pos`/`opt`, `decide_idx`, `opt_idx`, block-causal mask | `crates/kev` `encode.rs` | Every `encodings/` fixture reproduced exactly |
| 4 | [#9341](https://github.com/OpenAgentsInc/openagents/issues/9341) | Qwen2.5-0.5B forward on candle + safetensors load + inference LoRA on all seven projections | `crates/kev` `model.rs`, `lora.rs` | Hidden states at decide/opt positions match reference within fp32 tolerance |
| 5 | [#9342](https://github.com/OpenAgentsInc/openagents/issues/9342) | `PointerHead` (two linear maps, scaled dot product, per-question softmax) + packed forward | `crates/kev` `head.rs`, `decision.rs` | All `golden/` probabilities within tolerance; packed-vs-separate within `2.21e-6` fixture bound |
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

## Verification record

Run from the workspace root. Tests that need weights read
`KEV_ARTIFACT_DIR`/`KEV_BASE_DIR`, defaulting to
`../../../kev-artifacts/kev-0.5b` and `qwen25-0.5b` beside the repository.

```text
cargo test -p kev --features serve --release
KEV_TEST_DEVICE=metal cargo test -p kev --features metal --test conformance --release
cargo build -p kev --features serve,metal --release --bin kev-serve
target/release/kev-serve --adapter-dir ~/work/kev-artifacts/kev-0.5b \
    --base-dir ~/work/kev-artifacts/qwen25-0.5b --port 8009 [--device metal]
```

Measured on this machine (Apple Silicon, fp32, candle 0.11):

| Check | CPU | Metal | Reference |
| --- | --- | --- | --- |
| Golden probabilities, max abs delta | 4.1e-6 | 3.6e-6 | – |
| Packed vs separate, max abs delta | 1.2e-6 | 7.2e-7 | 2.2e-6 |
| Isolation, secret in sibling / absent / state | 0.056 / 0.056 / 0.997 | 0.056 / 0.056 / 0.997 | 0.056 / 0.056 / 0.997 |
| Permutation argmax, four orders | `returns` ×4 | `returns` ×4 | `returns` ×4 |
| Boundary forgery, option count | unchanged | unchanged | unchanged |
| `/v1/systemone` latency, 79-token pack | 218 ms | 726 ms | – |

The Metal path is slower than CPU at this size — the fp32 matmuls are small
and Metal pays dispatch plus the contiguous copies its batched matmul
requires. Both backends answer identically within the fp32 band.

The reference's own held-out metrics for this artifact live in
`crates/kev/fixtures/eval.json` (0.799 accuracy, 0.065 ECE, 0.031 after
temperature scaling). They describe the weights, not this port — the port's
evidence is conformance to the reference, measured above.

## What this sequence does not cover

Fleet fan-out, earn admission, catalog rows, and the service door at
`openagents.com` are the separate program in [`mesh-plan.md`](mesh-plan.md).
This sequence ends at kev answering `/v1/systemone` on this machine from
openagents code, with the mechanism probes passing — which it now does.
Training, larger checkpoints, and the `option_isolation` serving mode are
follow-on work.
