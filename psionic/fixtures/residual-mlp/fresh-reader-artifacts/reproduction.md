# Public retained-artifact reader: second attempt

A new consumer restores the retained MLP, CNN and residual-MLP states through
public constructors and `mdl_load`, pools original test row 0 through public
`mn_admit`/`mn_pool`, and reproduces all **30 score words bitwise**. Its checked
C passes at host-compiler O0/O2 and under ASan+UBSan at O1; stdout is identical.
Each one-byte-truncated state refuses while preserving all ten score sentinels.
There were no optimizer updates, downloads, old model gates or runtime changes.

This is explicitly an **author-documentation-assisted second attempt**. The
[first reader attempt](../fresh-reader/reproduction.md) found that the public
recipe delegated artifact checks to the author's harness and omitted the exact
residual constructor/file-byte boundary. The source author then published
[`INFERENCE.md`](../INFERENCE.md), including the exact public constructor,
saved names/shapes, explicit prediction node, `#export_c`/ABI file bridge and
row-zero pooling recipe. The reader consumed that new public material; it did
not open classifier, module, tensor, autodiff, trainer, optimizer, experiment,
reference or author-native implementation files. No private implementation fix
or unseen API advice was provided. The original 63-line consumer and its
qualification files remain unchanged.

The new [53-line Coax consumer](../../../examples/residual-retained-reader.coax)
uses the new residual recipe and the already-public MLP/CNN constructor bodies
in chapter 118. A generic reader function keeps each definition's prediction
node, switches the model to evaluation, strictly loads the named state,
refreshes transients, selects that prediction as output and runs a disabled-
gradient eager tape. A [43-line C host](../../../tests/residual-retained-reader.c)
only reads files, calls the generated ABI, compares words and reports checks.
No model arithmetic or parameter-offset mapping is implemented in C.

| Restored state | Bytes | Saved tensors | New inference observation |
| --- | ---: | ---: | --- |
| MLP `mnist/run-v1/model.safetensors` | 13,632 | 4 | All 10 row-zero scores match |
| CNN `cnn/run-v1/model.safetensors` | 11,976 | 4 | All 10 row-zero scores match |
| Residual-MLP `residual-mlp/run-v3/model.safetensors` | 13,972 | 8 | All 10 row-zero scores match |

For every model, the comparison reference is the first ten words of its own
retained `test-scores.f32`. The existing test IDX image/label files match the
decompressed SHA-256 identities in the public MNIST dataset manifest. Structural
admission and 2×2-to-14×14 pooling execute in Coax. The independent reference
here is the already-recorded capture, not a newly written independent numerical
model and not a new accuracy measurement.

## Commands and observations

From the repository root, use an unused directory and the qualified compiler:

```shell
export COAX=/absolute/path/to/qualified/coax
mkdir /tmp/new-retained-reader
"$COAX" test psionic/examples/residual-retained-reader.coax --only 'retained reader public'
"$COAX" c psionic/examples/residual-retained-reader.coax --library --header /tmp/new-retained-reader/retained-reader.h > /tmp/new-retained-reader/retained-reader.c
clang -std=c11 -O2 -fno-fast-math -ffp-contract=off -I/tmp/new-retained-reader /tmp/new-retained-reader/retained-reader.c psionic/tests/residual-retained-reader.c -lm -o /tmp/new-retained-reader/reader
/tmp/new-retained-reader/reader /tmp/psionic-mnist-data/t10k-images-idx3-ubyte /tmp/psionic-mnist-data/t10k-labels-idx1-ubyte psionic/fixtures/mnist/run-v1/model.safetensors psionic/fixtures/mnist/run-v1/test-scores.f32 psionic/fixtures/cnn/run-v1/model.safetensors psionic/fixtures/cnn/run-v1/test-scores.f32 psionic/fixtures/residual-mlp/run-v3/model.safetensors psionic/fixtures/residual-mlp/run-v3/test-scores.f32
```

Actual native builds additionally used O0 and O1 with
`-g -fno-omit-frame-pointer -fsanitize=address,undefined`. The sanitizer run
set `ASAN_OPTIONS=detect_leaks=0 UBSAN_OPTIONS=halt_on_error=1`; leak checking
is unsupported on this recorded Apple host. Coax emission used checked default
arithmetic, not `--release`. See `reproduction.json` for exact commands, hashes,
elapsed times, data/artifact identities, and the earlier attempt's provenance.

The metadata check passed first, then passed after a small construction cleanup
replaced direct assignment to the public `m.node` field with a local input-node
variable. That changed code justified the second metadata run. Native emission,
builds and executions then passed on the first attempt; no passing unchanged
native execution was repeated.

```text
MLP: 13632 state bytes; row0 all10 scores match retained capture bitwise; truncated-state refusal preserved10 outputs
CNN: 11976 state bytes; row0 all10 scores match retained capture bitwise; truncated-state refusal preserved10 outputs
residual-MLP: 13972 state bytes; row0 all10 scores match retained capture bitwise; truncated-state refusal preserved10 outputs
new public-API retained-artifact consumer: 30 score words matched; optimizer updates0
```

This follow-up resolves the original documented loading boundary for these
three artifacts. It does not qualify fresh training, optimizer/checkpoint
resume, all held-out scores, full quality gates, crash recovery, GPU/compiled/
higher-order convolution, broader operators, full framework graduation or
ResNet/CIFAR. The previous learning results remain **RETAINED**, including the
residual trial's previously seen evaluation rows. Runtime implementation
changes are **0**; new model-specific code is **53 Coax / 43 host C lines**,
including comments and blank lines.
