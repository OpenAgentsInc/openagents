# 66. A complete CUDA decoder

Keeping packed weights resident is useful only if the whole step stays with
them. This chapter runs embedding, normalization, Q/K/V projection, YaRN,
sink/window attention, output projection, routing, selected experts, residuals
and the full vocabulary head on the GPU.

## A launch program, authored in Coax

[The planner](../lib/gptoss/device-plan.coax) builds an ordered program of
kernel IDs, work-item counts and bounded typed views. The
[C adapter](../platform/gptoss-device.c) translates those views into driver
arguments and executes them on one stream. It contains no tensor arithmetic,
router, cache indexing or token-selection implementation. The CUDA source is
emitted from [Coax kernels](../lib/gptoss/device-kernels.coax), compiled with
FMA contraction and flush-to-zero disabled, and loaded through NVRTC.
There is no served cuBLAS/cuDNN dependency.

Each layer normalizes the residual stream, projects Q/K/V, rotates split halves
using a startup YaRN table and evaluates the visible cache positions. A learned
sink participates in the denominator without adding a value. Even layers keep
their 128-position ring; odd layers reserve the selected full context. The
GPU router selects the same top four experts, resolving ties to the lowest ID,
and normalizes only those routes. Both expert products decode original MXFP4
coefficients directly. The clamped interleaved activation preserves chapter
62's gate, linear and plus-one rules.

GPU threads do not share a racing global error flag. Every output owns a
numeric flag; separate kernels reduce them. Pending keys/values, diagnostics
and logits are private until the complete step succeeds. Publication copies
the cache entries and public outputs; a later kernel advances the absolute
position. Failure preserves the public cache, logits, traces and routes.
An asynchronous driver failure poisons the session, which must be closed.

Callers still supply immutable authenticated inputs, matching sequential
history, truthful extents, valid aligned allocations and exclusive access.
The native session owns all device buffers and keeps model bytes live.

## Qualify before timing

The [owned native checks](../tests/gptoss_device.c) execute a complete tiny
model through the launch program. Its expected traces, routes and vocabulary
logits come from the independently authored dense fixture. A deliberately
nonfinite head coefficient tests failure after every layer has run.

For original 20b, [the comparison tool](../tests/gptoss_device_oracle.py) reads
chapter 64's recorded independent original CPU vectors. It never reruns the
original CPU model. Before reading a candidate, the GPU bound is fixed at
`abs(candidate-reference)/(1+abs(reference)) < 0.0005` for every logit,
intermediate residual and mixture coefficient. Expert IDs and top token must
match exactly. Both captured prefixes pass. This establishes those vectors;
it does not claim bitwise CPU/GPU equality or broad answer-quality evaluation.

The 4080 also completes a low-reasoning Harmony request, “Say hi.”, with the
original model. The native harness requires the return stop, parses the entire
turn and requires one displayable final message. Analysis bytes never pass
[the shared display gate](../lib/gptoss/tools.coax).

## Run an actual prompt

On a machine with the CUDA driver and NVRTC available:

```shell
COAX=coax/bin/coax psionic/run-gptoss.sh \
  "$HOME/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee" \
  "Say hi."
```

The small [command shell](../platform/gptoss-chat.c) uses the pinned original
Harmony table, Coax's low-reasoning template, GPU batched prefill and greedy
decode. It prints only complete permitted final bodies. The current profile
has a 2,048-position context and a 256-token generation limit. Truncated or
invalid turns refuse display. Timing and admission diagnostics go to stderr.
The first invocation authenticates/uploads the model and compiles its module;
warm per-token throughput excludes that startup. Use `--interactive` as the second argument to keep one session alive across
requests. Each input line starts a fresh request; EOF closes it. The weights
and compiled module survive those request resets. This mode has no implicit
conversation history.

Greedy generation is the qualified device policy here. Chapter 64's seeded
CPU sampler, chapter 75's speculation and chapter 77's tool masks are separate
device integration gates; this command does not silently claim those features.

Continue with [batched prefill](ch067-prefill-together.md) and the
[measurement record](../../docs/verification/psionic-gptoss-device-2026-10-29.md).

## Make the admitted address arithmetic cheap

The single-token BF16 and original-MXFP4 products now use the same unsigned
32-bit tensor-relative indexing as prefill. The memory plan already proves
those element offsets fit. The kernel retains view bounds checks, coefficient
bytes, lane assignment, multiplication order and the periodic f32 fold before
its ordered f64 reduction. It hoists the selected expert row and input-slot
bases out of the inner coefficient loop.

The [frozen pre-optimization program](../fixtures/gptoss/decode-products.coax)
checks bit-identical partials on varied signed coefficients, ragged lane work,
shared/slot inputs and fold boundaries. The
[owned decode-product benchmark](../tests/gptoss_decode_bench.c) alternates
before/after kernels and flushes more than the 4080 L2 before each timed launch.
The [full-model paired harness](../tests/gptoss_speed.c) authenticates once,
keeps one set of weights resident, alternates two Coax-generated CUDA modules,
and compares full caches, logits, traces, expert IDs and greedy continuations.
It selects and waits per token as the serving path does. Its measurements and
limits are in the [decode-speed record](../../docs/verification/psionic-decode-speed-2026-10-29.md).

Both numerical and backend profiles retain their identity: this change removes
address overhead rather than changing the model's arithmetic. Prefill, matrix
hardware, [graph replay](ch070-replay-the-device-program.md) and concurrent scheduling have separate measurements.

## Move the next bottleneck

The [performance notebook](../docs/serving/performance.md) records the process
and the broader optimization backlog. After packed products improved, a bounded
profile put sequential softmax normalization ahead of the remaining decode
attention address work.

We retain each head's sink, maximum, exponentials and ordered f64 denominator.
One kernel stores that denominator in norm scratch whose previous values are
already dead. The next ordered kernel distributes the original independent
f64 divisions across the device. It does not replace division with a rounded
reciprocal. The following attention kernels cannot read an unfinished row.
The single-token attention path also reuses prefill's checked 64-query/8-KV
address layout, including the wrapped window and pending current row.

Two entry points bring the generated module to 45 kernels, with the same
workspace size. Tiny native/Coax checks and full original-model comparisons
retain bit-identical state and greedy IDs. The
[second-pass record](../../docs/verification/psionic-decode-pass2-2026-10-29.md)
separates component profiling, paired core rates and actual Responses latency.
Prefill and all remaining numerical/scheduling experiments keep their own gates.
