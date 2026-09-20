# The zero-label compile

What our 98 `routing` labels bought, against an adapter a hypernetwork wrote
from the question text alone. This directory holds the harness and the
records for openagents#9387; the reading lives in
[`docs/decision-models/2026-09-20-compiled-functions.md`](../../docs/decision-models/2026-09-20-compiled-functions.md).

`measure.py` sends a spec to the hosted ProgramAsWeights compiler
(`paw-4b-qwen3-0.6b`), loads the compiled adapter through the vendor SDK, asks
it every held-out `routing` item from `support-v2`, and writes one JSON record
with every raw answer beside its label. It compiles the spec twice: once from
prose alone, and once from the same prose with four example pairs drawn from
the calibration partition. It also prompts the interpreter's own base model,
`Qwen/Qwen3-0.6B`, with the same two specs and no adapter, so the record says
what the compiler added to the model it writes into.

`compare.py` pairs a run against every door in the Gym result store on the
40 `development` items and reports an exact McNemar p-value, which is what a
two-item difference on 40 items is worth.

## Why this is Python

The compiler is a hosted service and the compiled program is a GGUF LoRA the
vendor's runtime loads over a quantized base. The SDK is the only documented
way to load one. This is a measurement of an external service against our own
guide, not an integration; nothing in this directory is a dependency of any
crate, and no crate learns that it exists.

## Running it

```sh
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python -r requirements.lock
.venv/bin/python measure.py --spec prose --out runs/prose.json
.venv/bin/python measure.py --spec with-examples --out runs/with-examples.json
.venv/bin/python measure.py --spec base-prompt --out runs/base-prompt.json
.venv/bin/python measure.py --spec base-prompt-examples --out runs/base-prompt-examples.json
.venv/bin/python compare.py runs/prose.json runs/with-examples.json
```

The first run downloads the 594 MB `qwen3-0.6b-q6_k.gguf` base into
`~/.cache/programasweights/`. Everything runs on the CPU; the whole set of
seventy items takes about ten seconds per compiled program and about twenty
five seconds per base prompt on an eight-core box.

The compile is anonymous, so it runs as a public program: the free tier
answers a private anonymous compile with `401 auth_required`. The spec is the
question text and criteria from `crates/gym/suites/support-v2.json`, which is
already public, plus four states from the calibration partition in the
`with-examples` variant. No scored item leaves this machine before it is
asked, and no key is needed or read.

## What the records hold

| File | Holds |
| --- | --- |
| `runs/programs.json` | Program ID, compiler snapshot, spec digest, and compile time for each compiled spec. Compiling the same spec again returns the same ID. |
| `runs/prose.json` | Spec A: prose only, zero labels. |
| `runs/with-examples.json` | Spec B: the same prose plus four example pairs, and the IDs of the items they came from. |
| `runs/base-prompt.json` | The interpreter's base model, no adapter, spec A as the prompt, `Message:` and `Team:` fields. |
| `runs/base-prompt-examples.json` | The same with spec B as a few-shot prompt. |
| `runs/base-prompt-input-output.json`, `runs/base-prompt-examples-input-output.json` | The first prompt form tried, `Input:` and `Output:` fields, kept because the second form was chosen after seeing this one collapse to a single answer. |

Every record pins both suite digests, the SDK version, the machine, and the
spec text verbatim. Accuracy is the only metric: a compiled function returns a
bare string and no distribution, so ECE, Brier, and log loss are undefined and
the record says so rather than inventing a probability.

The scored set is the union of the `support-v2` `evaluation` split (50
`routing` items) and the `support-v2-three-way` `development` partition (40
items, 20 of them shared), so a run can be read against both the published
Lev numbers and the paired doors in the result store. The `locked` partition
is never read.
