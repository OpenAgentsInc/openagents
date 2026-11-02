# Public residual-model delivery — 2026-11-01

Issue [#12626](https://github.com/OpenAgentsInc/openagents/issues/12626), following
the bounded CNN delivery. New product arithmetic is Coax. The existing shared
IR, tape, optimizer and trainer were unchanged.

`mdl_residual_conv1x1` composes two named CPU 1×1 convolutions, ReLU and an
identity add. The [support matrix](../../../psionic/fixtures/residual/README.md#supported-profile)
records CPU f32/first-order limits and explicit unsupported paths. The new
[MLP-head residual consumer](../../../psionic/fixtures/residual-mlp/README.md)
uses 3,329 parameters, 28 nodes and eight saved tensors.

All attempts remain recorded: an interrupted first launcher, a complete
76.953125% linear-head quality failure, and a separately frozen MLP-head trial
at 87.353515625% (1,789/2,048), versus 242/2,048 majority baseline. The same
evaluation rows were consulted across architecture trials; this is not a
pristine unseen-final-test result. Seeds 11821/11822 are initializer/shuffle
roles within one trial. AdamW0.003, twelve epochs, 3,072 updates and gates were
fixed before each complete trial. No existing MLP/CNN training was rerun.

The three new authority roots kill 248/248 focused mutants (14+117+117).
Constructor checks cover debug/release. Independent native equations cover
all seventeen parameters and input VJP of the multichannel block; original
first-image checks cover all 3,329 derivatives with zero observed error.
Native debug/release/sanitizer profiles, exact next-update replay, eight-tensor
export and all20,480 reloaded score words pass. See the immutable
[qualification](../../../psionic/fixtures/residual-mlp/qualification.json),
[linear-head qualification](../../../psionic/fixtures/residual/qualification.json),
[freeze](../../../psionic/fixtures/residual-mlp/source-freeze.json) and
[independent audit](../../../psionic/fixtures/residual-mlp/run-v3/audit.json).

A genuinely separate reader wrote the
[63-line consumer](../../../psionic/fixtures/residual-mlp/fresh-reader/reproduction.md)
without implementation internals. Its hand-derived outputs, state and refusal
checks pass O0/O2/ASan+UBSan. That attempt exposed an incomplete artifact recipe;
the author supplied a public constructor/load/inference recipe. The explicitly
[documentation-assisted follow-up](../../../psionic/fixtures/residual-mlp/fresh-reader-artifacts/reproduction.md)
then matched all thirty original row-zero score words for the retained MLP,
CNN and residual models, and preserved outputs on truncated artifacts. It
used 53Coax+43host C lines, no runtime modifications, learning or downloads.
Its data/dependency identity checks are post-run, not a prospective freeze.

Focused repeatable checks:

```sh
COAX=/qualified/coax bash psionic/tests/check_residual.sh --out /tmp/new-residual-checks
COAX=/qualified/coax bash psionic/tests/check_residual_fresh_reader.sh /tmp/new-reader-checks
```

These commands start no optimizer experiment. Recorded unchanged checks were
retained rather than rerun. Integration metadata checks validate chapter
sources, examples, links and two explicitly reviewed example-inventory hash
bindings; GPU qualification claims and chapter status are unchanged.
The [inventory tooling record](tooling/book-inventory.json) retains the initial
coverage refusal, five check-first RED/GREEN groups, and the final metadata
pass: 262 runnable programs, one native-only library and 1,292 exact listings.
The native library stays outside evaluator transcript replay.

Remaining: reader-led fresh training/full held-out reproduction, broad model
usability, pooling/BatchNorm/general convolution and residual projections,
compiled/higher-order/GPU convolution, ResNet-18/CIFAR-10, larger graphs,
durable incremental publication and package/tutorial graduation. #12626 stays
open; this CPU slice does not satisfy #12627's useful-GPU-training acceptance.
