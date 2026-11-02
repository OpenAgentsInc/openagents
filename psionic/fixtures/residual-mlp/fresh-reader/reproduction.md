# Fresh public-API reader: residual composition and named state

This reader wrote a new 63-line Coax consumer from the public residual-MLP
recipe, chapters 117/118 and the residual-block example. It constructs a
different two-channel residual graph, checks hand-derived nonzero forward
values, saves and reloads named state into a new model, and checks a strict
missing-key refusal. This is fresh-reader API usability evidence. It is not
fresh training, independent reproduction of classifier quality, full framework
graduation, or ResNet/CIFAR qualification.

The reader did not open the module, tensor IR, autodiff, trainer, optimizer,
residual classifier, experiment, reference, or native implementation files.
The compiler necessarily consumed their transitive dependencies. Source text
quoted in the public chapters was read as documentation. The root agent
provided the worktree, task boundaries and qualified compiler path/digest; no
model author supplied an implementation fix or private API help for this run.

## New result

[`residual-fresh-reader.coax`](../../../examples/residual-fresh-reader.coax)
uses `mdl_data`, `mdl_residual_conv1x1`, `mdl_buffer`, `mdl_output`, `mdl_find`,
`mdl_init`, `ad_tape`/`ad_run`/`ad_value`, `mdl_save` and `mdl_load`. It keeps the
inference output explicit before registering a saved buffer.

The input is `[1,2,2,3]`, with hidden width 1. The expand filter is `[1,-1]`
with bias `0.5`; the contract filter is `[0.5,-1]` with biases `[0.25,0.5]`.
For each spatial coordinate, the independent scalar calculation is:

```jai
h = max(x0 - x1 + 0.5, 0)
y0 = x0 + 0.5*h + 0.25
y1 = x1 - h + 0.5
```

Channel 0 is `[-1,-0.5,0,0.5,1,1.5]`, and channel 1 is six copies of `0.5`.
All twelve exact dyadic expected values match, including negative outputs.
The new graph has 12 nodes, seven parameter floats, eight saved floats and
20 input floats. Its four named parameter tensors and one `calibration`
buffer occupy a 480-byte state file. Every saved word and all twelve output
words match bitwise after strict loading into a freshly constructed model.
Adding a required absent buffer makes loading return `missing`; all 21
sentinel values in the supplied input arena remain unchanged.

The two selected new compile-time checks passed. Native checked C passed at
host-compiler O0 and O2 and with ASan+UBSan at O1. All three stdout files are
identical. Coax release-profile emission was not tested. LeakSanitizer is
unsupported on this Apple host and was disabled for the successful sanitizer
run; there is no leak-check claim. There were no optimizer updates, downloads,
original-data reads or changes to runtime/library implementation.

```text
fresh reader: 12 graph nodes, 7 parameters, 5 saved tensors, 480 bytes
12 independent nonzero-branch outputs matched; named weights/buffer and inference reload matched bitwise
missing required key refused with all 21 caller values preserved; training updates 0
```

## Reproduction and attempts

Use an unused output directory and a qualified compiler:

```shell
COAX=/absolute/path/to/qualified/coax bash psionic/tests/check_residual_fresh_reader.sh /tmp/new-residual-reader
```

The new shell driver is syntax-checked. Its constituent commands were run
individually, as recorded in `reproduction.json`; the complete driver was not
rerun after those commands passed. It performs no canonical learning and runs
only the two new checks, not imported library suites. No mutation run or
whole-library authority coverage is claimed.

Actual corrections are retained rather than hidden:

1. The first check command failed because the reader guessed a two-argument
   `mdl_shape` helper. The compiler reported three required arguments. The
   consumer now uses the documented explicit `TvShape` value.
2. Metadata then passed, while the inference/state check refused compile-time
   `print`. Printing moved to `main`; only the failed check was rerun and
   passed. The already-passing metadata check was not repeated.
3. A native link was launched before asynchronous C emission had completed;
   the empty file had no `main`. After emission finished, normal compilation
   succeeded. This was reader command sequencing, not a framework failure.
4. The first sanitizer invocation set `detect_leaks=1`; Darwin aborted with
   “detect_leaks is not supported on this platform.” The ASan/UBSan run with
   `detect_leaks=0` passed. The failed shell batch continued to comparisons;
   the corrected batch and new driver use `set -e`.

## Retained classifiers and remaining boundary

The recipe's existing synthetic/original-image commands invoke the author's
native classifier harness. Those commands were not run: this task required
a new consumer and prohibited rerunning old model checks or training. This
reader did not load or evaluate the retained MLP, CNN or residual-MLP artifact.
Chapters 117/118 document named state and give full MLP/CNN constructors, but
the residual-MLP section does not supply a complete constructor with its exact
names and retained prediction node, or a stand-alone file-to-new-consumer
evaluation recipe. The reader reported that usability boundary to the root
instead of opening the implementation. A public constructor plus a small
documented file-byte bridge would make that next attempt reviewable.

The recipe's 1,789/2,048 residual-MLP result and its replay/export audit are
**RETAINED**, not newly reproduced here. The recipe also discloses that the
same held-out rows were seen across architecture attempts. This reader's
synthetic state/inference success adds no new accuracy or generalization
claim to those observations.

The documented profile still excludes GPU/compiled/higher-order convolution,
pooling, BatchNorm and general convolution/projection options. The graph cap
is 32 nodes; callers retain live, disjoint, correctly sized arenas. This
reader checks no derivatives, checkpoint/trainer resume, crash publication,
full dataset, unfamiliar fresh training or broad framework compatibility.

Authored model-specific code: **63 Coax lines, zero C/host lines** (physical
lines including comments/blanks). Reproduction tooling: 28 shell lines.
Runtime implementation changes: **0**. Exact hashes, commands, timing and
the source baseline are in `reproduction.json`.
