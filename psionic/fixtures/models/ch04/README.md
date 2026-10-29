# Owned original weight fixtures

These tiny Safetensors artifacts are authored in this repository for
[chapter 4](../../../book/ch004-reading-original-weights.md). They contain no
external checkpoint or training data. `owned-v1` is a fixture revision label,
not an upstream Git revision or proof of provenance.

| Tensor | Original dtype / shape | Values |
| --- | --- | --- |
| `linear.weight` | BF16 `[2,3]` | `[1,2,3,4,5,6]`, row-major |
| `linear.bias` | F32 `[2]` | `[0.5,-0.5]` |
| `scale` | BF16 scalar `[]` | `2` |

`model.safetensors` contains all three tensors. The two numbered shards and
`model.safetensors.index.json` describe the same inventory. The single-file
bias begins at payload byte 14, deliberately unaligned for F32. JSON key order
differs from payload and manifest order. Payload bytes total 22 in either form.
Config and tokenizer files are owned identity sidecars; chapter 4 hashes them
as opaque bytes and makes no tokenizer compatibility claim.

[manifest.json](manifest.json) records original artifact sizes and SHA-256
hashes plus the expected numerical inventory. The library accepts typed
caller-provided pins; it does not parse this teaching manifest and adopt it
as trusted input. [fixtures.coax](fixtures.coax) embeds byte-exact originals
for the native-free Coax example and evaluator. Native tests read these files.

From the repository root:

```shell
python3 -I psionic/tests/weights_fixtures.py
```

The independent standard-library generator verifies every original, manifest
and embedded constant without rewriting them. `--header` emits native test
inputs; `--write` is an intentional fixture-authoring operation. Any intentional
change also requires reviewing the pins, expected values, book and transcripts.
No model downloads or external scripts are involved.
