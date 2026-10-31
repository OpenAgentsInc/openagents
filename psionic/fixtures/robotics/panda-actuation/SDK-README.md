# Eighteen new SDK actuation observations

The [capture profile](sdk-capture-profile.json) and the existing
[experiment](experiment.json) were frozen before acquisition. The authored C
collector links the installed, hash-pinned MuJoCo 3.3.1 SDK. It reads the pinned
original/rewritten `demo_000` XML and authenticates referenced asset bytes; it
runs no downloaded repository scripts or Python SDK code.

The first phase loads compiled metadata without an authored per-case forward.
Both C and the pure Python `validate_model(model, experiment)` guard require
the supported named joint transmissions, scalar unit gears, stateless fixed
gains, used bias coefficients, control/force limits, damping and relevant flags.
All eight callback fields must be present and null. Plugins, tendons, flexes,
joint springs, gravity compensation and unexpected enabled specializations are
refused before cases. The reader's positive case and 34 poisoned or incomplete
metadata cases passed. Unused parameter tails remain finite captured facts;
they do not add supported dynamics.

The separate capture phase admits the actual loaded model again and emits
byte-identical metadata before evaluating inputs. It performs exactly 18
separately reset, new `mj_forward` calls and zero integration steps, renderer
calls or neural forwards. Every actual sparse transmission row is checked
immediately after forward: one named DOF column with coefficient exactly one.
Input q, velocity and original control words remain unchanged. A flushed
before/after progress ledger preserves actual forward counts on failure.

[sdk-values.f64](sdk-values.f64) contains 18 little-endian rows of 162 F64 words:

| Word offset | Field | Count |
| ---: | --- | ---: |
| 0 | actuator force | 9 |
| 9 | generalized actuator force | 9 |
| 18 | joint spring force | 9 |
| 27 | joint damper force | 9 |
| 36 | gravity compensation | 9 |
| 45 | fluid force | 9 |
| 54 | full passive force | 9 |
| 63 | actuator length | 9 |
| 72 | actuator velocity | 9 |
| 81 | actual selected sparse moment coefficient | 9 |
| 90 | stored control after forward | 9 |
| 99 | full-system `qacc_smooth` | 21 |
| 120 | full-system `qacc` | 21 |
| 141 | full-system `qfrc_constraint` | 21 |

The first fail-fast ASan/UBSan collector attempt passed 23,101 host assertions,
checked all 162 sparse rows, retained all 2,916 finite words and emitted no
stderr. The SDK dylib itself is not sanitizer-instrumented. This is the named
Darwin-arm64 F64 profile with fast math and contraction disabled. The original
SDK/successful metadata or numerical captures were not repeated as routine
checks. Two model loads serve distinct purposes: support preflight and the
actual capture instance. Counts describe authored API calls, not SDK internal
model compilation work.

[sdk-model.json](sdk-model.json) separately retains all nine actuator parameter
arrays, activation/control/force flags and ranges, joint references, damping
and frictionloss, plus all 26 body/root/geometry/inertia mappings and 152 geom
fluid parameter arrays and local poses. The source options have density 1.2,
viscosity 0.00002 and zero wind; these settings were observed and preserved.
They were not edited to simplify a future owned-fluid implementation.

[sdk-reference.json](sdk-reference.json) binds every public artifact and the
private binary, progress and execution receipts by hash. The full passive,
fluid, acceleration and constraint arrays are raw diagnostics. This receipt
qualifies acquisition only: it makes no owned fluid/contact/frictionloss,
full-passive, complete forward-dynamics, integration, hardware or compiler
boundedness claim. The separately authored product/oracle proof owns any
actuation and joint-passive comparison.

The tooling has separate preparation and one-shot capture commands. Existing
or partial output is preserved; a capture cannot silently be repeated:

```shell
python3 psionic/tests/robotics/panda_actuation_sdk_reference.py \
  --output /tmp/NEW-panda-actuation-reference
python3 psionic/tests/robotics/panda_actuation_sdk_reference.py \
  --output /tmp/NEW-panda-actuation-reference --capture-prepared
```

These commands require the exact authenticated private scene/assets and the
installed approved SDK. The ordinary product oracle reads the committed small
facts and never invokes this acquisition path.
