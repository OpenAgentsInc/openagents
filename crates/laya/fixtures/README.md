# Laya conformance fixtures

Golden input/output pairs generated from the Python reference
(`rl_agent_api.py` + `rl_common.py`, transformers 4.57.6, torch, fp32
CPU). The Rust port is accepted by reproducing these numbers per
checkpoint; see [`docs/laya/conformance.md`](../../../docs/laya/conformance.md).

## Layout

| File | Contents |
| --- | --- |
| `requests-<checkpoint>.json` | Request bodies beside the reference's full `system_one` answers |
| `sequences-<checkpoint>.json` | `build_sequence` output: input ids and marker positions per question |
| `manifest-<checkpoint>.json` | SHA-256 and byte size of every artifact file the port loads |
| `gen_fixtures.py` | The generator |

Checkpoints: `english` (ModernBERT-large, 512/192),
`multilingual` (mmBERT-base, 1024/256), `typed-decisions`
(ModernBERT-large, 1024/256).

## Regenerating

Requires the reference environment: `rl_agent_api.py` and `rl_common.py`
beside the checkpoint directories (the layout `~/work/laya-artifacts`
ships), with torch + transformers 4.57.x installed. Then:

```text
.venv/bin/python crates/laya/fixtures/gen_fixtures.py \
    --checkpoint english --model-dir ~/work/laya-artifacts/english \
    --out crates/laya/fixtures
```

once per checkpoint directory. The generator writes no weights; the
manifest records digests so a conformance run proves it loaded the same
bytes.

Caveat that matters for regeneration: the reference resolves RoPE thetas
from flat config fields under transformers 4.x and ignores the nested
`rope_parameters` block the checkpoints ship. Generating under
transformers 5.x would produce different goldens for `multilingual`;
`docs/laya/conformance.md` records why the port mirrors the 4.x
resolution.
