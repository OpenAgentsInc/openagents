# The fit library: audit, reproductions and a pooled CNN

Evidence for GitHub issue #12626. The library is
[`psionic/lib/fit.coax`](../../lib/fit.coax) with its `#authority` decisions in
[`fit-policy.coax`](../../lib/fit-policy.coax); its documentation is chapter
118's section "Fit any classifier: the fit library". All runs below were on
one shared Linux host (Intel i7-14700K, 28 threads, gcc 15.2, load average 35
to 98 from other agents' work) on the CPU, eager tape, f32 module values and
binary64 master weights. Times are for that host under that load; they are
not speed comparisons.

## Audit: model-specific lines before fit

The shared engines (`module.coax`, `module-train.coax`, `autodiff.coax`,
`tape.coax`, `tensor-ir*.coax`, `tensor-ops*.coax`, `trainer.coax`,
`optim/adamw.coax`, `checkpoint/record.coax`, `interchange.coax`,
`module-cnn.coax`, `module-residual.coax`, `data/mnist.coax`) were searched for
lines naming a model (`mnist`, `cnn`, `residual`, `mc_`, `cn_`, `rml_`). None
decides behaviour: the hits are comments, the reusable layers' own names and
the MNIST data adapter, a dataset module. The model-specific code was in the
per-model routes, each a copy of the others with different constants:

| Route | Classifier | Policy | Experiment driver | C host | Build script | Total |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| MLP (`mnist-*`) | 148 | 158 | 65 | 101 | 21 | 493 |
| CNN (`cnn-*`) | 157 | 158 | 65 | 101 | 21 | 502 |
| Residual-MLP (`residual-mlp-*`) | 162 | 158 | 65 | 101 | 21 | 507 |

Each copied piece is now a named public feature:

| Copied per model | Public feature |
| --- | --- |
| Hand-sized arena structs (`McWork`, `CnWork`, `RmlWork`) | `fit_model(...).need`, `fit_bind`, `fit_arena`, `FitArena` |
| `mc_move`, `mc_inference`, `mc_fit` | `fit_move`, the prediction tape in `FitModel`, `fit_room` |
| `mc_batch` (prepare, run, accumulate, apply) | `fit_step`, `fit_example` |
| `mc_eval` | `fit_eval`, `fit_check`, `fit_predict`, `fit_majority`, `fit_argmax` |
| `mc_digest`, `mc_ids` | `fit_sha`, `fit_ids` |
| Per-model extents and gates (`mc_memory`, `mc_workspace`, `mc_profile`, `mc_accept`) | `fit_split`, `fit_apart`, `fit_settings`, `fit_save_at`, `fit_stop`, `fit_room`, `fit_gate`, `fit_accept`: data, not constants; 119 of 119 mutants killed |
| `mc_experiment` | `fit_run`, and `fit_continue` with sessions for runs split across commands |
| `mnist-host.c`, `mnist-build.sh` | none: a Coax tool run by `coax/web/tool.sh` |

Data preparation (`fit_mnist`, `fit_example`), optimizer selection (the
trainer's `TrConfig`: SGD or AdamW with its settings), model state
(`fit_init`, `fit_move`, sessions, `mdl_save`/`mdl_load`) and inference
(`fit_predict`, `fit_check`) need no model-specific engine patch. No engine
line was changed for any of the three models.

## The author's MLP through fit: byte-identical

[`fit-mnist.coax`](../../examples/fit-mnist.coax) with the frozen MLP settings
classified 1,755 of 2,048 test rows (baseline 242), test loss
1.276122591283638 to 0.28320420931240164, exactly the retained
[run-v1](../mnist/run-v1/result.json). Its `model.safetensors` has the same
SHA-256 as the retained export made by the old harness on an Apple M5 Max
([model-sha256.txt](author-mlp/model-sha256.txt)), and as a rerun of that
harness on this host. Debug C took 5 min 52 s of CPU time, release C 6 min
50 s, the old harness 5 min 25 s, under different loads
([logs](author-mlp/)). The old harness took 17 s on the M5 Max; on this host
both are dominated by the engine's struct copies, not by fit.

## Fresh-reader reproduction

A separate agent with no knowledge of Psionic's internals worked from the
public docs only, without opening `psionic/lib`, `psionic/tests`,
`psionic/platform` or `psionic/tools`, in a fresh scratch folder, with no
repository edits. Its record is [reader/NOTES.md](reader/NOTES.md) and its
programs are beside it.

The reader translated each frozen manifest (MNIST, CNN, residual-MLP) into a
`FitSpec` before any run (0.8 × 2,048 → at least 1,639 correct; 0.5 × 2,048
→ a 1,024-row margin; checkpoint after update 6 × 256 = 1,536) and changed
nothing after seeing a result. Programs are kept as `.coax.txt` because their
relative loads point at the scratch folder they were written in.

| Route | Attempt | Result | Matches retained | Time on this host |
| --- | --- | --- | --- | --- |
| MLP | 1 | 1,755 / 2,048, baseline 242, loss 1.2761 → 0.2832, replay and reload exact, **gate passed** | yes, and the state file is byte-identical | 9 min 9 s wall, 6 min 2 s CPU |
| CNN | 1 | no result: killed by the 15-minute command limit, nothing printed | | 15 min wall, 8 min 4 s CPU |
| Residual-MLP | 1 | no result: killed by the 15-minute command limit, nothing printed | | 15 min wall, 8 min 8 s CPU |
| CNN | 2 | 1,814 / 2,048, baseline 242, loss 1.1770 → 0.3471, replay and reload exact, **gate passed** | yes, and the state file is byte-identical | 3 parts (2, 4, 6 epochs): 18 min 54 s wall, 13 min 45 s CPU |
| Residual-MLP | 2 | 1,789 / 2,048, baseline 242, loss 1.1571 → 0.2440, replay and reload exact, **gate passed** | yes, and the state file is byte-identical | 3 parts: 16 min 23 s wall, 12 min 45 s CPU |

All six state files' hashes are in [reader/model-sha256.txt](reader/model-sha256.txt).
The CNN and residual files were trained in three separate processes through
sessions and still equal the retained single-process exports from an Apple
M5 Max byte for byte.

Authored code: 59 (MLP), 76 (CNN) and 77 (residual) lines of Coax, of which
18, 18 and 19 are model-specific (the constructor and the spec); the rest is
the example's main program copied unchanged. No C, no runtime or library
change, no unsupported operation. Two 11-line shell wrappers only add the
time limit and timing. Effort: about 49 minutes over both attempts, mostly
waiting on training: 7 compiles (2 failed) and 9 training calls.

Documentation gaps the reader found, and what changed:

| Gap | Where | Fix |
| --- | --- | --- |
| G1: the documented loads left `mdl_conv2d`, `mdl_flatten`, `mdl_residual_conv1x1` undeclared (E0315) | ch118 "What a model must provide", "A complete run" | `fit.coax` now loads every public layer; the text says so |
| G2: no timing, no progress, `fit_run` silent until done, no way to split a run | ch118 fit section, fixture READMEs | `fit_continue` and sessions; the example prints each epoch and takes `EPOCHS_NOW`; measured times stated |
| G3: units and rounding of the gate | ch118 "Settings, splits and the gate" | stated, with the 1,639 / 1,024 arithmetic |
| G4: whether `save_at` is the saved or the replayed update | same | stated |
| G5: `TrConfig` defaults the example relies on | same | listed |
| G6: whether `FitSpec.name` must match | same | stated: it is hashed into checkpoint identity, compared with nothing else |
| H1: CNN and residual timing given only as a guessed ratio | ch118 "A complete run" | replaced by the measured times |
| H2: a finished run left its session, so a rerun would resume from it | the example | a finished run now deletes its session; the text says so |
| H3: parts equal one call was asserted, not shown | ch118 | `fit-native.coax` checks it; the reader's three-part exports also match |

The record, including every attempt and its output, is
[reader/NOTES.md](reader/NOTES.md).

## A pooled CNN: the first training run through the new pooling node

Frozen before learning in [pooled-cnn/experiment.json](pooled-cnn/experiment.json):
a 3×3 convolution to four channels, ReLU, 2×2 max pooling with stride 2
(12×12 to 6×6), and a linear readout, 1,490 parameters, the MLP's data,
optimizer and gate, seeds 11831/11832. Version 1 (twelve epochs) never ran to
completion: one launch failed to build, one was stopped by the 15-minute
command limit after 516 s of CPU time without printing anything, and one was
started by mistake and stopped by hand; no v1 outcome was observed. Version
2 changes only the epoch count to four (1,024 updates) and the replayed
checkpoint to update 512.

| | Result |
| --- | --- |
| Test | **1,828 of 2,048 correct (89.26%)**, training-majority baseline 242 |
| Gate (at least 1,639, margin at least 1,024, loss falls) | **passed** |
| Test loss | 1.770439535670448 to 0.3566704823169857 (initial 209 correct) |
| Validation after epoch 4 | 933 of 1,024 |
| Replay of update 513, reload of the saved file | exact |
| Time | 9 min 28 s wall, 329 s CPU, 254 MB peak |

Records: [run-v2](pooled-cnn/run-v2/) (stdout, time, validation trace, test
scores, the 6,296-byte state file) and the [freeze hashes](pooled-cnn/run-v2/freeze.sha256)
of the manifest and program taken before the run, from an isolated copy of
the source tree (`scratch-12626/t3/`) whose pooling and IR files are the ones
committed. That copy's `fit.coax` predates `fit_continue`; the arithmetic of a
one-call run is unchanged. The same 2,048 test rows were observed by every
earlier MNIST trial, so this is not a pristine unseen-test result, and with
four epochs against the others' twelve it is not an architecture comparison.

## Boundaries

| Boundary | Covered | Not covered |
| --- | --- | --- |
| Device | CPU | GPU: no GPU module trainer; the IR refuses convolution and pooling on CUDA and Metal |
| Execution | Eager tape, first-order gradients | Compiled forward/backward with convolution or pooling (refused), higher-order derivatives |
| Data | MNIST IDX, mnist-pool14, 4,096 training rows per epoch, 2,048 test rows | Full 60,000-row epochs, other datasets, augmentation |
| Scale | Up to 4,096 parameters, 32 graph nodes | ResNet-18/CIFAR-10 (chapter 117, "Toward ResNet-18 on CIFAR-10") |
