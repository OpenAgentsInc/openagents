# A frozen public-API CNN consumer

Profile `psionic-mnist-cnn-frozen-v1`, recorded on 2026-11-01 in America/Chicago.
The UTC freeze and execution timestamps fall on 2026-11-02. This is a bounded
CPU experiment, separately frozen from the earlier MNIST MLP. Raw images remain
outside the repository and retain the original four-object identities in
[`../mnist/dataset.json`](../mnist/dataset.json).

The public model is NCHW `[1,1,14,14]` → valid 3×3 convolution with two filters
→ ReLU → flatten `[1,288]` → linear ten scores → summed one-hot squared error.
It has 2,910 trainable parameters. The fixed 2×2 MNIST pooling, 4,096 training,
1,024 validation and 2,048 official-test slices match the earlier consumer.
Initialization/shuffle seeds are 11811/11812; twelve epochs, 16-row accumulation
and AdamW settings were frozen before learning. Nothing selected a seed,
checkpoint or stopping point from evaluation results.

| Record | Scope |
| --- | --- |
| [experiment.json](experiment.json) | Prospective settings and quality thresholds; an opaque identity input, not executable configuration. |
| [profile.json](profile.json) | New CPU convolution layout, arithmetic and unsupported paths. Earlier IR operation tags and metadata stay unchanged. |
| [reference.json](reference.json) | Independent initial-image equations, fixed tolerances and selected diagnostic words; no optimizer steps. |
| [source-freeze.json](source-freeze.json) | 54 source/context inputs and exact compiler/generated native identities, saved before original-data CNN learning. |
| [initial-capture/](initial-capture/) and oracle mode receipts | All 2,910 parameters/gradients, 196 features, ten scores and loss. Three native modes agree with the independent oracle with zero observed error. |
| [../cnn-core/qualification.json](../cnn-core/qualification.json) | Focused IR/tape calculus, lifetime, flags, shape/work and unsupported-path checks; shared-engine full mutation omitted. |
| [../cnn-modules/qualification.json](../cnn-modules/qualification.json) | Constructors, sticky refusals, named state, frozen layers, four native modes and final 51/51 root mutants killed. Earlier attempts remain. |
| [policy-qualification.json](policy-qualification.json) | Eight groups in debug/release and 117/117 application-policy mutants killed. |
| [mlp-compatibility.json](mlp-compatibility.json) | Three fresh MLP captures match the earlier committed bytes; no repeat of MLP training or old-record edits. |
| [gate-routing.json](gate-routing.json) | Nonempty focused selectors, alias/jobs dispatch and corrected mutation flags; tooling-only checks. |
| [run-v1/](run-v1/) | The sole canonical learning run, scheduled evaluations, checkpoints, one next-update replay, export and independent audit. |

The run reaches 1,814/2,048 correct (**88.5742%**) against 242/2,048 for the
training-majority baseline. Initial/final held-out mean squared error is
1.1769798681/0.3471311970. It takes 3,072 updates and 13.895613 seconds for the
native workflow on a shared Apple M5 Max host. This is not an isolated speed
comparison, full-MNIST result or calibrated probability model. The MLP has a
lower final squared error; models and seeds differ, so these observations are
not a controlled architecture comparison.

Checkpoint replay compares the next optimizer update after epoch six, not the
whole remaining suffix: all 70,180 state bytes and 4,096 shuffle entries match.
The four-tensor 11,976-byte export contains 2,910 F32 words, checked independently
against final master weights. Coax reloads it and compares all 20,480 test-score
words. The independent audit recomputes metrics from captures rather than
running another inference model. Artifacts are written after completion;
incremental durable publication and crash/restart recovery remain open.

Routine checks download nothing and do not train:

```shell
psionic/check.sh --only cnn
psionic/tests/check_cnn.sh --data-dir /tmp/psionic-mnist-data
```

Explicit reproduction starts another experiment in a new directory:

```shell
python3 psionic/tests/mnist_reference.py acquire --data-dir /tmp/psionic-mnist-data
psionic/run-cnn.sh --train /tmp/psionic-mnist-data /tmp/my-cnn-run
python3 psionic/tests/cnn_reference.py verify-run /tmp/my-cnn-run/run --data-dir /tmp/psionic-mnist-data --out /tmp/my-cnn-run/audit.json
```

The launcher performs the independent preflight before learning. Python is
build/acquisition/verification tooling; the compiled executable uses Coax model,
preprocessing, gradient, optimizer, state and policy code, with C host effects.
Use retained results for unchanged checks. Do not overwrite `run-v1` or change
the frozen manifest to relabel an outcome. GPU convolution, compiled backward,+higher-order transforms, pooling/options, residual/CIFAR acceptance and larger
datasets retain their separate gates in [issue #12626](https://github.com/OpenAgentsInc/openagents/issues/12626).
