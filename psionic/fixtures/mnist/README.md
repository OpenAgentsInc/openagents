# Frozen MNIST classifier experiment

`experiment.json` freezes the model, fixed original-index splits, preprocessing,
optimizer, seeds, evaluation schedule and success conditions before learning.
The completed first run is retained in [run-v1](run-v1/result.json) and its
[independent audit](run-v1/audit.json). The prospective manifest remains unchanged,
so its digest still names the experiment used by the checkpoints.
[source-freeze.json](source-freeze.json) binds the actual source closure, compiler
and compiled artifacts before the first optimizer step.

`dataset.json` pins the four original gzip objects and their decompressed IDX
bytes. The [CVDF primary mirror](https://github.com/cvdfoundation/mnist) links
those Google objects and acknowledges Yann LeCun's permission to mirror them.
`acquisition.json` records the HTTPS object generation, Google MD5 header and
independently computed hashes from this acquisition. No downloaded script ran.
`artifacts.coax` contains only the four decompressed SHA256 byte arrays, for
the Coax driver's streaming admission. It contains no dataset or executable
product logic.

Full data remains outside the repository, by default in
`/tmp/psionic-mnist-data`. The tool authenticates the pinned gzip lengths,
MD5 and SHA256, bounds decompression, checks exact IDX headers and extents,
and checks the three fixed split-payload hashes. It refuses changed bytes.

```shell
python3 psionic/tests/mnist_reference.py acquire
python3 psionic/tests/mnist_reference.py verify-data
python3 -m unittest discover -s psionic/tests/mnist -p 'test_*.py'
python3 psionic/tests/mnist_reference.py oracle --out /tmp/mnist-reference.json --header /tmp/mnist-reference.h
python3 psionic/tests/mnist_reference.py verify-capture /tmp/mnist-native-oracle
```

The oracle has no optimizer and performs no Python training. It computes a
tiny dyadic model's loss and gradients twice, including exact rational chain
rules, and computes fixed initial forward/gradient references for the real
196→16→10 model. Native qualification can compare every gradient against the
temporary header for original official training row0. It does not need to
commit thousands of floating-point constants. `reference.json` records compact
hashes and probes; the regenerated temporary header contains the full arrays.

`verify-capture` reads the native Coax oracle's `params.f64`, `gradient.f64`,
`scores.f32`, `features.f32` and `oracle.json`. It requires original training
row0, the frozen initializer and exact file counts, and refuses non-finite
values. Parameter and feature bits must match exactly. Loss, scores and all
3,322 published gradients must satisfy the predeclared bound
`abs(error) <= 2e-6 + 2e-6 * abs(reference)`. These tolerances were declared
before observing a native capture and are recorded in `reference.json`.
The observed workspace counts are reported, not described as independent
workspace-planner qualification. No tolerance is widened after a difference.

Float32 arithmetic is explicit through Python `struct`, rounding each dense
product and each sequential addition to match the public tensor IR profile.
The separate eager tensor operators' compensated binary64 dots are not this
profile. Reverse gradients accumulate in binary64 and round
once when published as float32, matching the declared public autodiff profile.
The reference is a numerical cross-check, not proof of general gradients or
evidence of classifier training quality. NumPy is not required.

## First run: passed

The sole canonical run completed 3,072 AdamW updates over 49,152 training-row
visits. It classified 1,755 of 2,048 official held-out images correctly
(85.6934%), compared with the training-majority class's 242 (11.8164%). Mean
held-out squared error fell from 1.2761225913 to 0.2832042093. Every frozen
quality criterion passed; no settings, seed, split or stopping point changed.

The measured native workflow took 17.036464 seconds on the recorded CPU
context, including admission, hashing, fixed evaluations, learning, checkpoint
and export/reload work. The process's maximum resident size was 58,425,344
bytes. This is one bounded CPU run, not a throughput comparison or a full
60,000-image MNIST result.

All 80,068 bytes of the canonical next-update checkpoint and the replayed one
match, as do all 4,096 shuffle entries. The four named exported tensors contain
3,322 F32 words; the independent audit verifies every word against the final
master parameters' F32 copies. Coax additionally checks all 20,480 held-out
score words after loading that file into fresh model storage. This last
prediction comparison is producer-recorded, not a second Python inference run.

`run-v1` preserves both initial/final test-score captures, the epoch-six and
final checkpoints/pointers, canonical/replayed next states and shuffle arrays,
the named-tensor export, all thirteen validation observations, process logs,
execution timing and the independent audit. The host writes these files after
the run; incremental durable publication and crash/restart recovery were not
exercised. One next update was replayed; the six-epoch prefix was not rerun.
