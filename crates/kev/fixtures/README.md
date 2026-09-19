# Kev conformance fixtures

Golden input/output pairs generated from the Python reference at
`~/work/projects/repos/kev` (`jaredpalmer/kev`, revision `e07ef43`,
checkpoint `jaredpalmer/kev-0.5b` v0.1.0). Every phase of the Rust port is
accepted by reproducing these numbers inside tolerance.

## Regenerating

Requires the reference checkout's `uv` environment (`uv sync --extra serve`)
and the `kev-0.5b` artifacts at `runs/kev` (`hf download
jaredpalmer/kev-0.5b --local-dir runs/kev`).

```sh
cd ~/work/projects/repos/kev
PYTHONPATH=. uv run python \
  ~/work/openagents/crates/kev/fixtures/gen_fixtures.py \
  --run runs/kev \
  --out ~/work/openagents/crates/kev/fixtures \
  --artifacts ~/work/kev-artifacts/kev-0.5b
```

The script also writes `head.safetensors` (converted from `head.pt`) and the
tokenizer files into `--artifacts`. The Qwen2.5-0.5B base lives at
`~/work/kev-artifacts/qwen25-0.5b/` (`hf download Qwen/Qwen2.5-0.5B`).

## Layout

| Path | Holds |
| --- | --- |
| `requests/` | TypeSafe-shaped request bodies plus the `to_record()` output and per-question metadata the renderer must reproduce |
| `encodings/` | `encode()` records with their source `record` embedded: `ids`, `seg`, `pos`, `opt`, `decide_idx`, `opt_idx`, `state_truncated` |
| `eval.json` | The reference's held-out evaluation of this artifact (accuracy, ECE, temperature scaling, mechanism results); describes the weights, not the port |
| `golden/` | fp32 CPU per-question probability vectors, shaped answers, token counts |
| `probes/` | `isolation` (secret in sibling `0.056` / absent `0.056` / in state `0.997`), `packed_vs_separate` (max delta `2.21e-6`), `permutation` (4 orders of `support.department`), `forgery` (option count under fake delimiters) |
| `tokenizer.json` | Delimiter token ids and sanitized `user_tokens()` cases |
| `manifest.json` | sha256 and byte size of every artifact file the Rust side loads |
| `gen_fixtures.py` | The generator; development-time tool, runs in the reference venv |

## Weights stay out of git

`~/work/kev-artifacts/kev-0.5b/` holds `adapter_model.safetensors`,
`head.safetensors`, tokenizer files, `adapter_config.json`, `eval.json`;
`~/work/kev-artifacts/qwen25-0.5b/` holds the base. `manifest.json` pins
their digests; verify before use.
