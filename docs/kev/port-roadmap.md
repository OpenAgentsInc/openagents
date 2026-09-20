# Kev port roadmap

**Status:** implemented for the four checkpoint contents pinned by the
committed fixtures. The Kev mechanism is
ported from the Python reference (`~/work/projects/repos/kev`,
`jaredpalmer/kev`) into this repository as `crates/kev`, a Rust
implementation on [candle](https://crates.io/crates/candle-core) with the
TypeSafe `/v1/systemone` wire contract. Every issue in both sequences is
closed; the measurements below are the recorded evidence.

Upstream's current Qwen3 adapters have different contents under the same
names. This completed port sequence does not establish conformance for
those replacements or include upstream's newer serving optimizations.
The [2026-09-20 integration review](2026-09-20-upstream-review.md) identifies
the next work: pin artifact revisions, evaluate the new 4B, align bf16
merging, and measure attention and state-cache improvements. Artifact
pinning, identity, and merging are complete; the
[new 4B evaluation](measurements/2026-09-20-candidate-4b.md) records the
current workload baseline and conformance separately from this original
port sequence.

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

## Multi-variant sequence

Tracked by [#9356](https://github.com/OpenAgentsInc/openagents/issues/9356).
Qwen3 support lands first — every variant after `kev-0.5b` sits on a
Qwen3 base — then one issue per checkpoint, then bundle serving.

| # | Issue | Pulls over | Proved by |
| --- | --- | --- | --- |
| 8 | [#9357](https://github.com/OpenAgentsInc/openagents/issues/9357) | Qwen3 layers: per-head q/k RMSNorm, no q/k/v bias, declared `head_dim`, sharded safetensors, `bf16` dtype select | `kev-0.5b` suite unchanged; `Qwen3-0.6B-Base` packed forward on CPU |
| 9 | [#9358](https://github.com/OpenAgentsInc/openagents/issues/9358) | `kev-0.6b` artifacts + `fixtures/variants/kev-0.6b/` | Full battery on CPU; max delta 3.7e-6 |
| 10 | [#9359](https://github.com/OpenAgentsInc/openagents/issues/9359) | `kev-4b` artifacts + fixtures | Full battery; max delta 2.6e-6 |
| 11 | [#9360](https://github.com/OpenAgentsInc/openagents/issues/9360) | `kev-8b` artifacts + fixtures | Full battery; max delta 1.1e-6 |
| 12 | [#9361](https://github.com/OpenAgentsInc/openagents/issues/9361) | `kev-serve --bundle-dir`: loads every variant under an artifacts root, routes on the request `model` field | Live: 4 variants in one process, `/v1/models` lists all, `kev-latest` → largest; current unknown-id refusal is `model_unavailable` at 503 |
| 13 | [#9362](https://github.com/OpenAgentsInc/openagents/issues/9362) | Docs + per-variant Jev side-by-side | `jev-comparison.md` re-run for all four checkpoints |

## Artifact layout

One directory per variant under `~/work/kev-artifacts/` holding
`adapter_model.safetensors`, `head.safetensors` (converted from
`head.pt`), `head_meta.json` (`base`, `head_dim`, `option_isolation`),
tokenizer files, `adapter_config.json`, `eval.json`; one directory per
base checkpoint (`qwen2.5-0.5b`, `qwen3-0.6b`, `qwen3-4b`, `qwen3-8b`)
holding `config.json` plus the `model*.safetensors` shards. Symlinks into
the HF hub cache work. Each variant's `fixtures/**/manifest.json` pins
the artifact digests.

## Verification record

Run from the workspace root. The harness iterates every committed
fixture set (`fixtures/` for `kev-0.5b`, `fixtures/variants/<id>/` for
the rest) and resolves artifacts by convention under
`~/work/kev-artifacts/`. `KEV_VARIANT=<id>` selects one variant;
`KEV_ARTIFACT_DIR`/`KEV_BASE_DIR` override its paths;
`KEV_TEST_DEVICE=metal` runs on Metal.

```text
cargo test -p kev --features serve --release
KEV_VARIANT=kev-8b cargo test -p kev --features serve --release
KEV_TEST_DEVICE=metal cargo test -p kev --features metal --test conformance --release
cargo build -p kev --features serve,metal --release --bin kev-serve
target/release/kev-serve --adapter-dir ~/work/kev-artifacts/kev-0.5b \
    --base-dir ~/work/kev-artifacts/qwen2.5-0.5b --port 8009 [--device metal] [--dtype bf16]
target/release/kev-serve --bundle-dir ~/work/kev-artifacts --port 8009
```

Measured on this machine (Apple Silicon, fp32, candle 0.11):

| Check | kev-0.5b | kev-0.6b | kev-4b | kev-8b |
| --- | --- | --- | --- | --- |
| Golden probabilities, max abs delta | 4.1e-6 | 3.7e-6 | 2.6e-6 | 1.1e-6 |
| Packed vs separate, max abs delta | 1.2e-6 | 1.0e-6 | 1.3e-6 | 4.8e-7 |
| Isolation, sibling / absent / state | 0.056 / 0.056 / 0.997 | 0.003 / 0.003 / 0.982 | 0.047 / 0.047 / 0.635 | 0.061 / 0.061 / 0.851 |
| Permutation argmax, four orders | `returns` ×4 | `returns` ×4 | `returns` ×4 | `returns` ×4 |
| Boundary forgery, option count | unchanged | unchanged | unchanged | unchanged |
| `/v1/systemone` latency, ~100-token pack, CPU | ~200 ms | ~150 ms | ~0.8 s | ~1.5 s |

Isolation values are the checkpoints' own numbers reproduced exactly —
the port matches the reference to four decimals everywhere; kev-4b's
weaker `state_in_state` (0.635) is a property of its weights. Metal was
verified for `kev-0.5b` (3.6e-6 golden delta); larger variants run CPU
fp32 by default and `bf16` via `--dtype` when memory or latency demands.
The Metal path is slower than CPU at 0.5B — the fp32 matmuls are small
and Metal pays dispatch plus contiguous copies.

The reference's own held-out metrics for `kev-0.5b` live in
`crates/kev/fixtures/eval.json` (0.799 accuracy, 0.065 ECE, 0.031 after
temperature scaling). They describe the weights, not this port — the
port's evidence is conformance to the reference, measured above.

## What this sequence does not cover

Fleet fan-out, earn admission, catalog rows, and the service door at
`openagents.com` are the separate program in [`mesh-plan.md`](mesh-plan.md).
This sequence ends at the four pinned checkpoints answering
`/v1/systemone` from OpenAgents code, with the mechanism probes passing.
Training and admission of replacement checkpoints are follow-on work.
The encoder already reads `option_isolation` from head metadata; the
published fixture checkpoints use `false`, so these measurements do not
establish conformance for weights trained with it enabled.
