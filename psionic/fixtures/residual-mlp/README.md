# Frozen residual CNN with a public MLP head

Profile `psionic-mnist-residual-mlp-frozen-v1` is the third recorded attempt in
this residual work. It uses the same public 1×1 residual block as
[`../residual/`](../residual/README.md), then a standard 196→16→10 MLP head.
The model definition returns both `model` and `prediction`; callers pass the
retained output node to inference. No tensor, autodiff, trainer or optimizer
engine was modified.

The model has 3,329 parameters, 28 graph nodes and eight saved tensors. CPU f32
forward values and formal f64 first derivatives use the existing eager tape;
parameter derivatives publish as f32, while master weights and AdamW moments
are f64. The initializer seed 11821 and shuffle seed 11822 are two RNG roles in
one trial, not two different trials. All optimizer settings, data partitions
and the 80% accuracy/50 percentage-point majority-margin gates remain those
of the preceding linear-head trial.

| Attempt | Outcome |
| --- | --- |
| First linear-head launch | Interrupted after a freeze-writer error failed to stop a shell chain; explicitly unqualified, no final artifacts. |
| Completed linear-head trial | 1,576/2,048 correct (76.953125%); fails the frozen accuracy gate, retained with exact replay/export evidence. |
| This new MLP-head architecture trial | 1,789/2,048 correct (87.353515625%); passes its frozen gates, with the limits below. |

The same 2,048 official-test rows were observed across the architecture
trials. The new architecture was chosen after the earlier quality failure;
this is **not an independent pristine unseen-final-test evaluation** or a
controlled architecture comparison. The manifest's no-tuning rule applies
within this newly frozen trial. No settings, seeds, split, stopping point or
threshold changed during it. All outcomes remain visible.

## Reproduce the public APIs without learning

The [standalone constructor, named-state load and inference recipe](INFERENCE.md)
shows the complete public constructor and how to restore the retained artifact
without using the author's native experiment harness.

A qualified Coax compiler and a host C compiler are required. No GPU, network,
new dataset or Python runtime is needed by the native program. Python below
is authentication and numerical/artifact tooling only.

```sh
export COAX=/absolute/path/to/qualified/coax
bash psionic/tests/check_residual.sh --out /tmp/my-residual-checks
bash psionic/platform/residual-mlp-build.sh /tmp/my-residual-mlp release 0
/tmp/my-residual-mlp/residual --synthetic
```

`--synthetic` checks every zero-network parameter derivative, real IEEE
refusal, all eight saved tensors and explicit-output inference reload. An
existing authenticated MNIST cache adds an independent first-image oracle:

```sh
/tmp/my-residual-mlp/residual --oracle /tmp/psionic-mnist-data psionic/fixtures/residual-mlp/experiment.json /tmp/my-residual-mlp-capture
python3 psionic/tests/residual_mlp_reference.py verify-capture /tmp/my-residual-mlp-capture --data-dir /tmp/psionic-mnist-data --out /tmp/my-residual-mlp-comparison.json
```

Every new output directory must be unused. Routine checks and these commands
perform no optimizer updates. The separate explicit training launcher
`psionic/tests/residual_launch.py` requires a completed source/build/data
freeze and its exact SHA-256, and rehashes source, compiler, CC, provenance,
generated C/header/native binary and all dataset objects before and after the
native child. A new build requires a new prospective artifact freeze; do not
reuse the recorded absolute build paths as a claim about a different binary.
The tool-only negative controls in `residual_launch_test.py` prove missing or
changed required inputs never invoke its training spy.

## Recorded outcome and persistence

The completed native run performs 3,072 AdamW updates over 49,152 training-row
visits in 76.15598 seconds on a shared Apple M5 Max host. It reaches 1,789/2,048
correct against 242/2,048 for the training-majority baseline. Initial/final
mean summed squared error per image is 1.1571417636296246/0.24395134977035582. This is a bounded
slice result, not full MNIST, a timing comparison, a probability calibration
or ResNet-18/CIFAR-10.

Before learning, the independent original first-image oracle matches every
one of 3,329 parameter derivatives, all parameters, 196 pooled features, ten
scores and the loss with zero observed error. A dataset-free independent
multichannel block oracle separately checks every 17 parameter derivatives
and the full input VJP, including identity and learned branches.

The independent [audit](run-v3/audit.json) recomputes metrics from scheduled
score captures, checks checkpoint model/data/optimizer identities, compares
all 80,236 bytes of the next-update checkpoint and the 4,096-row shuffle,
and verifies every exported F32 word in the eight-tensor 13,972-byte file.
Coax reloads the named state into a fresh model and compares all 20,480 score
words. Replay is one optimizer update after epoch six, not the whole suffix.
Artifacts are published after the native run returns; incremental durable
publication and crash recovery are separate work.

See [experiment.json](experiment.json), [source-freeze.json](source-freeze.json),
[initial-comparison.json](initial-comparison.json) and [qualification.json](qualification.json).
The [shared support table](../residual/README.md#supported-profile) retains the
same CPU/f32/first-order, live-disjoint-arena and graph/work limits. Pooling,
BatchNorm, general convolution options, GPU or compiled/higher-order
convolution, ResNet/CIFAR and unfamiliar-consumer graduation remain separate.
