Six maintained diagnostic drivers now run in Coax:

| Mode | Authored jobs | Readback words | Preserved rule |
| --- | ---: | ---: | --- |
| `filter-format` | 40 | 160 | All finite; unchanged authored bytes, mip uploads and uniforms |
| `ring8` | 728 | 2,912 | Exact numeric f32 references; signed zeros compare numerically |
| `sans-filter` | 40 | 160 | Existing `abs(actual-expected) <= 0.000001` |
| `sampling` | 6 | 96 | Existing `0.00001` bound; four normal jobs have zero wrong lanes and each planted control has 16 |
| `clip-output` | 8 | 32 | Every raw finite lane and all original clip/ID metadata retained |
| `deadgears-vs` | 96 | 384 | Every raw finite lane, source/resource hash and selector retained |

```sh
COAX=/qualified/coax coax/web/tool.sh aethyr/tests/primitive-webgpu.coax \
  filter-format PROBE.wgsl RESULT.json CASES.json
# Substitute ring8 or sans-filter with that family's original probe and cases.
COAX=/qualified/coax coax/web/tool.sh aethyr/tests/primitive-webgpu.coax \
  sampling NORMAL.wgsl RESULT.json FAULT.wgsl ARRAY-FAULT.wgsl
```

The old executors are retained with observer-only word reads in each slice's
qualification archive. They were removed from the maintained tree only after
same-input parity. No production shader, numerical tolerance or backend profile
changed. `VERTEX_WEBGPU.md` explains the clip/VS source-epoch boundary and its
explicit historical-source option.

Coax owns every fixture, GPU descriptor, program pair, job order, source hash,
finite check, expected result and verdict. `primitive-webgpu-effects.js` runs
inside muted headless Chrome and only creates supplied GPU resources, uploads,
draws and returns raw words/values. `webgpu-session.coax` supplies the shared
CDP transport, bounded async polling and loopback origin, including streaming
the exact hash-bound texture files. Every child/profile/origin is cleaned up.
The six earlier simple probes use that same transport now; a fresh six-case
raster transport control matches its earlier 24 raw words and all metadata.

The retained ring/Sans references are CPU f32 lanes; their numerical JSON
decoding restores that exact f32 word before f64 error arithmetic. The Sans
corpus's 160 reference values were independently verified as exact f32 values.
The checks do not claim correct rounding of arbitrary JSON binary64 input.

`primitive-webgpu-parity.coax` compares the original observations, case fields,
resource hashes, device/browser metadata, complete discrepancies and verdicts.
It invokes `raw_float_oracle.py` as a separate independent IEEE754 codec: Python
`struct` verifies every reported float equals its exact raw word, and its JSON
codec returns exact binary64 metadata words. This keeps equality strict where
Coax's existing approximate JSON scalar helper can disagree on two decimal
spellings of the same binary64 statistic. The oracle makes no driver, fixture
count, tolerance, build or browser decision; Coax owns the comparison.

```sh
COAX=/qualified/coax coax/web/tool.sh aethyr/tests/primitive-webgpu-parity.coax \
  MODE LEGACY.json CURRENT.json PARITY.json
```

The Coax loss harness checks altered words, a one-ulp numeric transport loss,
altered binary64 error statistics, device/resource metadata and invalid texture
hash preflight. `--data` names the retained raw receipt directory:

```sh
COAX=/qualified/coax coax/web/tool.sh aethyr/tests/primitive-webgpu-loss-checks.coax \
  --data DIR --binary PRIMITIVE_EXECUTABLE --parity PARITY_EXECUTABLE \
  --sans-cases CASES.json --sans-wgsl PROBE.wgsl --out NEW_DIRECTORY
```

Exact source epochs, compiler identity, passing checks/mutation, controls and
archive readback are recorded in
`docs/verification/aethyr-primitive-webgpu-port-2026-11-02.json`. That record
covers this four-driver switch, not completion of all renderer tooling.
