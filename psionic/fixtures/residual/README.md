# Public residual CNN route

Prospective profile `psionic-mnist-residual-frozen-v1`. This consumer uses the
public same-shape `mdl_residual_conv1x1` block, module-training adapter, trainer,
AdamW, checkpoint and named-state interfaces. It adds no tensor or optimizer
engine code. The definition returns both its model and its explicit prediction
node, so inference does not guess an internal graph index.

The block computes `x + contract(relu(expand(x)))`. Both convolutions are 1×1;
the second restores the input channels. This preserves N/H/W and the complete
identity branch without padding, cropping, projection or implicit broadcasting.
The classifier has 1,977 parameters, 23 graph nodes and six saved tensors.

## Reproduce without training

Use a qualified Coax compiler and the host C compiler. The checks are CPU-only,
download nothing and use no private keys or network services:

```sh
export COAX=/absolute/path/to/qualified/coax
bash psionic/tests/check_residual.sh --out /tmp/my-residual-checks
bash psionic/platform/residual-build.sh /tmp/my-residual-build release 0
/tmp/my-residual-build/residual --synthetic
```

An existing authenticated MNIST cache can independently check the first image
and every parameter derivative, without optimizer updates:

```sh
/tmp/my-residual-build/residual --oracle /tmp/psionic-mnist-data psionic/fixtures/residual/experiment.json /tmp/my-residual-capture
python3 psionic/tests/residual_reference.py verify-capture /tmp/my-residual-capture --data-dir /tmp/psionic-mnist-data --out /tmp/my-residual-comparison.json
```

New output directories must be unused. Preserve failed attempts. The dataset
identities remain in `../mnist/dataset.json`; no dataset is committed here.
`experiment.json` freezes the new architecture, initial seed 11821, shuffle
seed 11822, splits, AdamW learning rate 0.003, twelve epochs and quality gate
before any optimizer update. A separate source/build freeze must precede an
explicit `--train` invocation. The completed result and its refusal are recorded below.

## Supported profile

| Surface | This route |
| --- | --- |
| Layout and type | NCHW images, OIHW 1×1 kernels; f32 compute |
| Training | CPU eager first-order formal f64 VJPs; parameter entry derivatives narrow to f32; f64 master parameters and AdamW moments |
| Operators | Built-in convolution, add, multiply, ReLU, reshape, matmul and sum; loss is summed one-hot squared error |
| State | Six named F32 tensors; optimizer/cursor identity checkpoint; explicit prediction node for inference |
| Storage | Caller-owned live, disjoint arenas and unchanged public-constructor plans; capacities are admitted before execution |
| Limits | Existing 32 graph nodes, 16 modules, 16 saved tensors and 1,048,576 aggregate scalar-work profile |
| Refusals | CUDA/Metal convolution, compiled backward and higher-order convolution transforms; registered operators in `mtr_plan` |
| Separate work | General padding/stride/dilation/groups, public pooling/BatchNorm, larger profiles, ResNet-18/CIFAR-10 and GPU training |

Low-level pooling operator procedures exist, but the module/IR/trainer route
has no pooling node. LayerNorm is available separately and is not BatchNorm.
The generic trainer offers plain SGD and AdamW; this frozen experiment selects
AdamW. Python authenticates data and audits equations/artifacts only; native
Coax performs model execution, gradients, optimization and state handling.

## Frozen attempt outcome

The complete-freeze attempt `run-v2` finished 3,072 AdamW updates in 55.224s
on a shared Apple M5 Max host. It reached 1,576/2,048 correct (76.953125%)
against 242/2,048 for the training-majority baseline. Initial/final test mean
squared error was 1.40086278905801/0.486577263684012. It **fails** the frozen 80%
accuracy requirement; this is not a passed residual-model quality milestone.
No settings, seeds, splits or thresholds changed after observing it.

The independent audit recomputes these metrics from saved score captures,
checks the model/dataset/optimizer identities, compares the complete 47,788-byte
next-update checkpoints and shuffle orders, and verifies all 1,977 exported
F32 words in six named tensors. Coax's full export/reload score comparison
passes. These persistence results do not turn the failed quality gate into a
pass. This is not a controlled architecture comparison or a ResNet/CIFAR result.

The first attempt `run-v1` was stopped after a freeze-writer error failed to
stop a chained shell launch. Its prospective experiment and source inventory
existed, but the complete generated-artifact freeze did not. It is explicitly
unqualified; its last logged update was 768 and it wrote no final artifacts.
`attempt1-unqualified.json` and retained logs preserve that failure. The second
attempt uses the identical architecture/settings/gates and a completed,
separately verified source/build/data freeze before launch. It is the sole
completed frozen attempt here. New fail-closed launch tooling tests missing or
changed source/compiler/CC/provenance/generated artifacts/data with a train spy;
those controls ran after the already-launched second attempt and invoke no
training themselves.
