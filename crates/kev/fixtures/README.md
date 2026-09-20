# Kev conformance fixtures

Golden input/output pairs generated from the Python reference at
`~/work/projects/repos/kev` (`jaredpalmer/kev`, revision `e07ef43`). Every
phase of the Rust port is accepted by reproducing these numbers inside
tolerance, per variant.

These fixtures pin historical checkpoint contents. On 2026-09-20, the Hub
`main` adapters for 0.6B, 4B, and 8B differ from these manifests. A matching
variant name is not a matching checkpoint. The
[release review](../../../docs/kev/2026-09-20-upstream-review.md#artifact-downloads-no-longer-reproduce-the-fixture-set)
records the observed hashes and historical revisions. Preserve these
fixtures when measuring a replacement adapter.

## Variants

`fixtures/` holds the `kev-0.5b` set (checkpoint `jaredpalmer/kev-0.5b`
v0.1.0, base `Qwen/Qwen2.5-0.5B`). Every other variant lives under
`fixtures/variants/<id>/` with the same inner layout:

| Variant dir | Checkpoint | Base |
| --- | --- | --- |
| `fixtures/` | `jaredpalmer/kev-0.5b` | `Qwen/Qwen2.5-0.5B` |
| `variants/kev-0.6b/` | `jaredpalmer/kev-0.6b` | `Qwen/Qwen3-0.6B-Base` |
| `variants/kev-4b/` | `jaredpalmer/kev-4b` | `Qwen/Qwen3-4B-Base` |
| `variants/kev-8b/` | `jaredpalmer/kev-8b` | `Qwen/Qwen3-8B-Base` |

The test harness iterates every committed variant directory and skips any
whose artifacts are absent. `KEV_VARIANT=<id>` selects one;
`KEV_ARTIFACT_DIR`/`KEV_BASE_DIR` override its resolved paths;
`KEV_TEST_DEVICE=metal` runs the battery on Metal.

## Regenerating

Requires the reference checkout's `uv` environment (`uv sync --extra
serve`) and the variant's run directory (`hf download jaredpalmer/<id>
--revision <immutable-hub-commit> --local-dir <run>`). Pin the reference
code revision as well. Generate replacement checkpoints into a separate
fixture directory until conformance and workload evaluation are complete.

```sh
cd ~/work/projects/repos/kev
PYTHONPATH=. uv run python \
  ~/work/openagents/crates/kev/fixtures/gen_fixtures.py \
  --run <run dir> \
  --out ~/work/openagents/crates/kev/fixtures[/variants/<id>] \
  --artifacts ~/work/kev-artifacts/<id>
```

The script writes `head.safetensors` (converted from `head.pt`),
`head_meta.json` (the checkpoint's `base`, `head_dim`,
`option_isolation`), and the tokenizer files into `--artifacts`. The base
checkpoint lives at `~/work/kev-artifacts/<base dir>/`, where the
directory name derives from `head_meta.base`
(`Qwen/Qwen3-0.6B-Base` → `qwen3-0.6b`). A symlink into the HF hub cache
works; the loader reads `config.json` plus every `model*.safetensors`
shard.

## Layout

| Path | Holds |
| --- | --- |
| `requests/` | TypeSafe-shaped request bodies plus the `to_record()` output and per-question metadata the renderer must reproduce |
| `encodings/` | `encode()` records with their source `record` embedded: `ids`, `seg`, `pos`, `opt`, `decide_idx`, `opt_idx`, `state_truncated` |
| `eval.json` | The reference's held-out evaluation of this artifact (accuracy, ECE, temperature scaling, mechanism results); describes the weights, not the port |
| `golden/` | fp32 CPU per-question probability vectors, shaped answers, token counts |
| `probes/` | `isolation`, `packed_vs_separate`, `permutation`, `forgery` results per variant |
| `tokenizer.json` | Delimiter token ids and sanitized `user_tokens()` cases |
| `manifest.json` | sha256 and byte size of every artifact file the Rust side loads |
| `gen_fixtures.py` | The generator; development-time tool, runs in the reference venv |

## Weights stay out of git

`~/work/kev-artifacts/<variant>/` holds `adapter_model.safetensors`,
`head.safetensors`, `head_meta.json`, tokenizer files,
`adapter_config.json`, `eval.json`; `~/work/kev-artifacts/<base>/` holds
the base checkpoint. Each variant's `manifest.json` pins their digests;
verify before use.
