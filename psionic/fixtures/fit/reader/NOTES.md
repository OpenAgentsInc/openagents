# Fresh-reader reproduction of three MNIST classifiers with the fit library (issue #12626)

Reader: a Claude agent with no prior knowledge of Psionic internals, 2026-11-02,
22:24 to 22:52 America/Chicago. Machine: shared Linux host, 28 cores, load
average 35 to 82 throughout (other agents' work). No `/usr/bin/time`, so wall
and user time are bash `time`.

## What I read and what I did not

Read: `psionic/book/ch118-one-trainer-for-every-model.md` (sections "The
interface", "The same trainer for a CNN", "Fit any classifier: the fit
library"), `psionic/book/ch117-models-are-compositions-of-modules.md`
("A convolutional module", "A same-shape residual block", "Public API"),
`psionic/examples/fit-mnist.coax`, the three `experiment.json` manifests,
`psionic/fixtures/{mnist,cnn,residual-mlp}/README.md`,
`psionic/fixtures/residual-mlp/INFERENCE.md`, the header of `coax/web/tool.sh`,
and a grep of `coax/spec.md` for struct defaults.
Not opened: anything under `psionic/lib/`, `psionic/tests/`,
`psionic/platform/`, `psionic/tools/`. I did not read the other files in
`scratch-12626/` outside `reader/` (earlier work by someone else).
Ran (not read): `psionic/tests/mnist_reference.py verify-data`,
`coax/web/tool.sh` (which builds into `coax/web/build/tools/`, git-ignored).
Nothing in the repository was edited; `git status --short` shows only
`?? scratch-12626/`.

## Data

`python3 -I psionic/tests/mnist_reference.py verify-data --data-dir scratch-12626/mnist`
returned `"verified": true, "objects": 4, "splits": 3, "payload_bytes": 54950048`
(1.4 s). Output: `verify-data.out`.

## Translating the manifests into FitSpec

All three manifests share splits, optimizer settings and acceptance; only the
model and seeds differ.

| Manifest field | FitSpec / TrConfig | How |
| --- | --- | --- |
| splits train 0+4096, validation 4096+1024 (official train), test 0+2048 (official test) | `train`, `valid`, `test` `FitSplit`s | direct |
| `final_test_accuracy_min: 0.8` | `gate.correct=1639`, `gate.count=2048` | 0.8 * 2048 = 1638.4; "at least" means the smallest integer count >= 1638.4, which is 1639 |
| `final_test_accuracy_above_majority_baseline_min: 0.5` | `gate.margin=1024` | 0.5 * 2048 = 1024 rows above the baseline's correct count (unit inferred, see gap G3) |
| test SSE must fall | built into the gate (ch118) | nothing to set |
| checkpoint "end of epoch6", replay the next update | `save_at=1536` | 4096 rows / 16 accumulate = 256 updates per epoch; 6 * 256 = 1536 (see gap G4) |
| AdamW, rate 0.003, beta1 0.9, beta2 0.999, eps 1e-8, clip 1, bound 16, binary64 | TrConfig fields of the same names | direct |
| microbatch 1, accumulate 16, epochs 12, shuffle, examples 4096 | `batch=1, accumulate=16, epochs=12, shuffle=true, examples=4096` | direct |
| decay 0.0, schedule constant, every 0, early stopping off | not set | relies on zero/first-member defaults (see gap G5) |
| initializer seed / shuffle seed | `FitSpec.seed` / `TrConfig.seed` | MLP 11801/11802, CNN 11811/11812, residual 11821/11822 |

These were written before any run and were not changed after any result.
The test split is only scored by `fit_run` itself; nothing I chose read it.

## Programs

Each program is `psionic/examples/fit-mnist.coax` with (a) `#load` paths
changed for a file in `scratch-12626/reader/`, (b) `fx_model`/`fx_spec`
replaced, (c) for CNN/residual one extra `#load`. The `fit-main` block
(`fx_read`, `tool_main`) is byte-identical to the example (checked with diff).

| Route | File | Total lines | Model-specific (fx_model + fx_spec, non-blank) | Generic (loads, main, comments, blank) | C | Shell |
| --- | --- | --- | --- | --- | --- | --- |
| MLP | `reader-mlp.coax` | 59 | 18 | 41 (6 comment, 2 blank) | 0 | 0 |
| CNN | `reader-cnn.coax` | 60 | 18 | 42 (6 comment, 2 blank) | 0 | 0 |
| Residual-MLP | `reader-residual.coax` | 61 | 19 | 42 (6 comment, 2 blank) | 0 | 0 |

Shared helper: `run.sh` (11 lines of bash, reader tooling only: wraps
`timeout 900` and `time` around `coax/web/tool.sh` and writes `runN-ROUTE.out`).
The model constructors are the documented ones: MLP from the example, CNN
from ch118 "The same trainer for a CNN" (`cn_model`), residual from
`fixtures/residual-mlp/INFERENCE.md` (`rml_definition`), each ending in
`fit_model(m,prediction)`. Unsupported operations hit: none. Runtime/library
changes needed: none (none were possible).

## Attempts, in order

| # | What | Result |
| --- | --- | --- |
| 1 | verify-data | verified (see above) |
| 2 | compile MLP (`tool.sh reader-mlp.coax` with no args) | built; printed usage, exit 2 as expected. 41.6 s wall, 1m39 user. `compile1-mlp.out` |
| 3 | compile CNN, attempt 1 (`attempt1-reader-cnn.coax`, only the three loads ch118 lists) | `error[E0315]: procedure 'mdl_conv2d' is not declared` and the same for `mdl_flatten`. Cause: `fit.coax` does not load `module-cnn.coax`. Gap G1. `compile1-cnn.out` |
| 4 | added `#load "../../psionic/lib/module-cnn.coax";` (from ch117's Public API table); compile CNN | built, usage, exit 2. 18.3 s wall. `compile2-cnn.out` |
| 5 | added `#load "../../psionic/lib/module-residual.coax";` to residual (same reasoning, before compiling it); compile residual | built, usage, exit 2. 31.4 s wall. `compile1-residual.out` (`module-residual.coax` alone also supplies `mdl_flatten`, as INFERENCE.md implies) |
| 6 | for the record, compiled the unfixed residual (`attempt1-reader-residual.coax`) | same E0315 for `mdl_residual_conv1x1` and `mdl_flatten`. `compile0-residual-attempt1.out` |
| 7 | MLP training run 1 | **passed**, see below. `run1-mlp.out` |
| 8 | CNN training run 1 (concurrently with 7, then with 9) | **killed by `timeout 900` at 15:00 wall, 8m04s user, no output, no model file**. `run1-cnn.out` |
| 9 | Residual training run 1 (concurrently with 8) | **killed by `timeout 900` at 15:00 wall, 8m08s user, no output, no model file**. `run1-residual.out` |

Never more than two trainings at once. Per the instructions, neither
timed-out run was retried with a longer limit, and no setting was changed.

## Results

### MLP (psionic/fixtures/mnist/experiment.json): PASS

```
status 0 passed true: 1755 of 2048 correct (training-majority baseline 242), loss 1.276122591283638 -> 0.28320420931240164
3322 parameters, 3072 updates; replay equal true, reload equal true; 4 tensors, 13632 bytes
reloaded from 13632 bytes: 1755 of 2048 correct
real 9m8.881s  user 6m1.584s  sys 0m0.988s  exit=0
```

- Gate: 1755 >= 1639; 1755 - 242 = 1513 >= 1024; loss fell; replay and
  reload equal. All conditions pass; `fit_run` reports `passed true`.
- Retained (fixtures/mnist/README.md, run-v1): 1,755/2,048, baseline 242,
  test mean squared error 1.2761225913 -> 0.2832042093, 3,072 updates,
  3,322 parameters in four tensors. **My run matches every one of these
  to all digits the README prints.** The README does not state the export's
  byte size, so 13,632 bytes is not compared.
- Time: retained 17.04 s "on the recorded CPU context" (the MLP README
  does not name the host; the CNN and residual READMEs say Apple M5 Max) versus 9m09 wall / 6m02 user here. The host was
  heavily loaded and the program differs from the author's harness, so this
  is not a speed comparison; I cannot tell how much of the gap is the
  machine and how much is the fit library.
- Output kept: `out-mlp/model.safetensors`.

### CNN (psionic/fixtures/cnn/experiment.json): NOT COMPLETED (timeout)

- Compiled after gap G1 was worked around. Training ran 15 minutes (8m04
  user) and was killed; it printed nothing (`fit_run` reports only at the
  end), so I cannot say how many of the 3,072 updates ran, whether it was
  progressing, or what it would have scored. No pass/fail against the gate.
- Retained (fixtures/cnn/README.md): 1,814/2,048 (88.5742%), baseline 242,
  loss 1.1769798681 -> 0.3471311970, 3,072 updates, 13.9 s for the author's
  native workflow, 11,976-byte export. Not compared: no result.
- Observation, not a conclusion: the retained CNN run was faster than the
  retained MLP run (13.9 s vs 17.0 s), but under fit on this host the CNN
  did not finish in 15:00 while the MLP finished in 9:09 under similar
  load. The docs give no fit-library timing for a CNN to tell whether this
  is expected.

### Residual-MLP (psionic/fixtures/residual-mlp/experiment.json): NOT COMPLETED (timeout)

- Compiled after the same load fix. Killed at 15:00 wall, 8m08 user, no
  output, no model file. No result, no pass/fail.
- Retained (fixtures/residual-mlp/README.md, run-v3): 1,789/2,048
  (87.353515625%), baseline 242, loss 1.1571417636 -> 0.2439513498, 3,072
  updates, 76.2 s on an Apple M5 Max host (about 4.5 times the retained MLP
  time), 13,972-byte eight-tensor export. Scaling my MLP's 9:09 by that
  ratio would predict roughly 40 minutes here, so a timeout is consistent
  with the retained timings, but that is an extrapolation, not a measurement.

## Documentation gaps and errors

- **G1 (blocking until worked around). ch118, "Fit any classifier: the fit
  library", subsections "What a model must provide" and "A complete run".**
  The first says any ch117 constructor may be used, naming `mdl_conv2d`,
  `mdl_flatten` and `mdl_residual_conv1x1`; the second says a file outside
  `psionic/examples` needs only `tool.coax`, `fs.coax` and `fit.coax`.
  With those three loads, a CNN or residual model fails with E0315
  (`mdl_conv2d`/`mdl_flatten`/`mdl_residual_conv1x1` not declared).
  The answer is in ch117's Public API table ("load `module-cnn.coax`",
  "load `module-residual.coax`"). Fix: add those two optional `#load` lines
  to the "A complete run" paragraph, or have `fit.coax` load them.
- **G2 (blocked two routes). ch118 fit section, and fixtures/cnn and
  fixtures/residual-mlp READMEs: no run-time expectation or progress for
  `fit_run`.** The only fit-library example is the MLP, and no time is
  given for it either; the fixture READMEs' timings are for the authors'
  separate harnesses on an Apple M5 Max. `fit_run` prints nothing until it
  returns, and the docs describe no progress output, per-epoch log or
  resumable checkpoint for a `fit_run` user (the "Callbacks" bullet says
  logging belongs in the caller's loop, which `fit_run` does not expose).
  So a reader cannot budget the run or tell a slow run from a stuck one.
  Fix: state measured `fit_run` wall/user times for all three models and
  their hosts, and give `fit_run` (or the example) a progress line per epoch,
  or document how to write the loop from `fit_step`/`fit_eval` with
  checkpoints so a run can be split under a time limit.
- **G3 (minor, inferred). ch118 "Settings, splits and the gate".**
  `FitGate.margin` is "the training split's majority class is beaten by at
  least margin": the unit (test rows, not percentage points) and whether it
  compares correct counts is not stated. Also how to round a fractional
  accuracy (0.8 * 2048 = 1638.4) into `correct`. I inferred both from the
  example matching the MLP manifest (1639, 1024). Fix: one sentence giving
  the formula from the manifest's fractions.
- **G4 (minor, inferred). ch118 "Settings, splits and the gate".**
  `save_at` is "the update whose checkpoint is replayed"; the manifests say
  "checkpoint end of epoch6, replay the next update". Whether `save_at`
  is the checkpointed update (1536) or the replayed one (1537) is not said;
  I used 1536 from the example. Fix: say "`save_at` = updates per epoch x
  checkpoint epoch; the replayed update is the one after it".
- **G5 (minor). ch118 "The interface" and the fit example.** The manifests'
  `decay: 0.0`, `schedule: constant`, `every: 0`, `early_stopping: false`
  are not set in the example's `TrConfig`; they rely on Coax's zero
  defaults and on `constant` being the first member of `TrSchedule`, whose
  declaration order the docs do not show. The fit section says `every` and
  `patience` "are 0" but not that the others default correctly. Fix: list
  which manifest fields map to defaults, or set them explicitly in the
  example.
- **G6 (minor). ch118 "Settings, splits and the gate".** `FitSpec.name` is
  "part of the checkpoint identity"; whether a reproduction must use the
  author's name for any identity comparison is not said. I used my own
  names; nothing in the run compared identities with retained artifacts.
- **Not a gap but worth stating:** the MLP route needed nothing beyond the
  docs and reproduced the retained numbers exactly.

## Effort

About 28 minutes wall clock (22:24 to 22:52), most of it waiting on the
three training runs. Attempts: 1 data verification, 5 compiles (2 failed
with E0315, 3 succeeded), 3 training runs (1 pass, 2 killed at the
15-minute limit). Authored: 59 + 60 + 61 lines of Coax (18/18/19 of them
model-specific; the rest copied from the example), 0 lines of C, 11 lines
of shell tooling.

## Files in this folder

`reader-{mlp,cnn,residual}.coax` (final programs),
`attempt1-reader-{cnn,residual}.coax` (failed first versions),
`run.sh`, `verify-data.out`, `compile1-mlp.out`, `compile1-cnn.out`,
`compile2-cnn.out`, `compile1-residual.out`,
`compile0-residual-attempt1.out`, `run1-{mlp,cnn,residual}.out`,
`out-mlp/model.safetensors` (out-cnn and out-residual are empty).

## Second attempt (CNN and residual-MLP), 2026-11-02 23:00 to 23:21

The coordinating agent asked for this after the documentation was updated.
The first attempt above is left as it was.

### What changed in what I read

Re-read ch118 "Fit any classifier: the fit library" and
`psionic/examples/fit-mnist.coax`. Provenance: at this point `git status`
shows those files and several `psionic/lib/` files (among them `fit.coax`,
`fit-policy.coax`, `autodiff.coax`, `tensor-ir.coax`) as uncommitted
working-tree changes on top of `c6594c09a2`. I used them as they stood and
did not open the library files. Because the library changed between my
attempts, first-attempt and second-attempt times are not a like-for-like
speed comparison.

How the updated docs address my first-attempt gaps:

- G1: the section now says `fit.coax` loads every public layer. Confirmed:
  the second-attempt files load only `tool.coax`, `fs.coax` and `fit.coax`,
  and both compiled on the first try.
- G3 to G6: the gate's row-count units and rounding (0.8 x 2048 -> 1639,
  margin 0.5 -> 1024 rows), `save_at` (the saved update; the next one is
  replayed; 6 x 4096/16 = 1536), the `TrConfig` defaults (decay 0, schedule
  constant, source rows, every/patience 0) and `FitSpec.name` (hashed into
  the model identity, compared with nothing else) are now stated. My
  first-attempt translation agrees with all of them, so no setting changed.
- G2: the example now trains one epoch at a time with `fit_continue`, prints
  one line per epoch, and with a third argument `EPOCHS_NOW` saves
  `OUT_DIR/session.psfs` and stops; the next call resumes. The section gives
  MLP timings on this host and says CNN and residual cost more per row (a
  residual run "roughly 4.5 times an MLP run"), so run a few epochs per call.

### Programs

`reader2-cnn.coax` (76 lines) and `reader2-residual.coax` (77 lines): the new
example with its `fit-main` block byte-identical (checked with diff), the
load paths changed, and the same `fx_model`/`fx_spec` as the first attempt
(checked with diff: same seeds, splits, gate, `save_at`). Model-specific
lines: 18 (CNN) and 19 (residual); the other 58 lines are generic (8
comments, 2 blank). C: 0. Shell: `run2.sh`, 11 lines of reader tooling
(`timeout 900`, `time`, one output file per part).

### Attempts

| # | Command | Result |
| --- | --- | --- |
| 1 | compile `reader2-cnn.coax` (no args) | built, usage, exit 2; 56.2 s wall, 1m25 user. `compile2a-cnn.out` |
| 2 | compile `reader2-residual.coax` | built, usage, exit 2; 66.4 s wall, 1m25 user. `compile2a-residual.out` |
| 3-8 | three parts per route, `EPOCHS_NOW` = 2, then 4, then 6 | all six parts exit 0. At most two trainings at once (the CNN and residual parts ran in parallel). |

I chose the part sizes after measuring part 1, to stay under 15 minutes.
This changes nothing in the experiment: the docs say the parts' result is
byte for byte the one-call result. I did not run a one-call run to check that.

### CNN: PASS

| Part | Epochs | Wall | User | File |
| --- | --- | --- | --- | --- |
| 1 | 1-2 | 3m24.7s | 2m12.0s | `run2-cnn-part1.out` |
| 2 | 3-6 | 4m35.3s | 4m03.7s | `run2-cnn-part2.out` |
| 3 | 7-12, then final test, export, reload | 10m54.0s | 7m29.7s | `run2-cnn-part3.out` |
| Total | 12 | 18m54s | 13m45s | |

Per-epoch validation (of 1024): 901, 921, 938, 934, 936, 934, 928, 931, 936,
933, 932, 938; validation loss 0.3684 -> 0.3106.

```
status 0 passed true: 1814 of 2048 correct (training-majority baseline 242), loss 1.1769798680907115 -> 0.3471311970060924
2910 parameters, 3072 updates; replay equal true, reload equal true; 4 tensors, 11976 bytes
reloaded from 11976 bytes: 1814 of 2048 correct
```

Gate: 1814 >= 1639; 1814 - 242 = 1572 >= 1024; loss fell; replay and reload
equal. Retained (fixtures/cnn/README.md): 1,814/2,048, baseline 242, loss
1.1769798681 -> 0.3471311970, 3,072 updates, 2,910 parameters, 11,976-byte
four-tensor export. **Every figure matches** to the digits the README prints.

### Residual-MLP: PASS

| Part | Epochs | Wall | User | File |
| --- | --- | --- | --- | --- |
| 1 | 1-2 | 2m12.3s | 1m48.3s | `run2-residual-part1.out` |
| 2 | 3-6 | 3m48.6s | 3m35.6s | `run2-residual-part2.out` |
| 3 | 7-12, then final test, export, reload | 10m21.8s | 7m21.5s | `run2-residual-part3.out` |
| Total | 12 | 16m23s | 12m45s | |

Per-epoch validation (of 1024): 874, 910, 920, 917, 940, 919, 936, 938, 931,
929, 929, 929; validation loss 0.3568 -> 0.2011.

```
status 0 passed true: 1789 of 2048 correct (training-majority baseline 242), loss 1.1571417636296246 -> 0.24395134977035582
3329 parameters, 3072 updates; replay equal true, reload equal true; 8 tensors, 13972 bytes
reloaded from 13972 bytes: 1789 of 2048 correct
```

Gate: 1789 >= 1639; 1789 - 242 = 1547 >= 1024; loss fell; replay and reload
equal. Retained (fixtures/residual-mlp/README.md, run-v3): 1,789/2,048,
baseline 242, loss 1.1571417636296246 -> 0.24395134977035582, 3,072 updates,
3,329 parameters, 13,972-byte eight-tensor export. **Every figure matches**,
including all printed digits of both losses.

### Remaining documentation gaps (second attempt)

- **H1. ch118 "A complete run", timing paragraph.** CNN and residual times
  are given only relative to the MLP ("cost more per row", residual "roughly
  4.5 times an MLP run"). On this host, one observation each: residual 12m45
  user against the MLP's 6m02 (first attempt, older library), about 2.1x;
  CNN 13m45 user, about 2.3x. Neither run would fit one 15-minute command
  here (18m54 and 16m23 wall in total), which agrees with the advice to
  split. Fix: state measured per-epoch or per-run times for the CNN and
  residual on a named host, instead of the 4.5x ratio (which comes from the
  authors' own programs on an Apple M5 Max).
- **H2. ch118 "A complete run" and the example's header comment.** After the
  final part, the `session.psfs` from the last saved part stays in OUT_DIR
  (observed: both `out2-*` directories still hold it next to
  `model.safetensors`). The example reads that file whenever it exists, so a
  new run in the same OUT_DIR would resume from it instead of starting at
  epoch 0. The docs do not say to use a fresh OUT_DIR per run or to delete
  the file. I did not test a rerun. Fix: one sentence ("use an empty OUT_DIR
  for each new run; a finished run leaves its last session file"), or have
  the program remove the file when done.
- **H3 (minor). Parts versus one call.** The section says the parts' result
  is "byte for byte the one-call result". I could not check that under the
  15-minute limit. My results match the retained runs exactly, which
  supports it but does not prove it.

### Second-attempt effort

About 21 minutes wall clock (23:00 to 23:21), most of it waiting on
training. 2 compiles (both succeeded first time), 6 training parts (all
exit 0), no failed attempts. Both attempts together: about 49 minutes, 7
compiles (2 failed with E0315), 9 training invocations (first attempt: MLP
pass plus 2 timeouts; second attempt: 3 parts each for CNN and residual).

Second-attempt files: `reader2-cnn.coax`, `reader2-residual.coax`,
`run2.sh`, `compile2a-{cnn,residual}.out`,
`run2-{cnn,residual}-part{1,2,3}.out`, `out2-cnn/model.safetensors`,
`out2-residual/model.safetensors` (and their leftover `session.psfs`).
