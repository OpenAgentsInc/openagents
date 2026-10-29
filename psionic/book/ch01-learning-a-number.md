# 1. Learning a number

By the end of this chapter, a program will learn the multiplier in `y = 2x`
from two examples. It will start with a bad guess, measure how wrong it is,
and improve the guess sixteen times. Then it will predict an answer for an
input it did not train on.

The trainer lives in [lib/learn.coax](../lib/learn.coax) and the runnable
program in [examples/learn-number.coax](../examples/learn-number.coax). Later
chapters will extend this library. We begin with one parameter so we can
check every multiplication before building larger machinery.

## Run it

From the repository root, build Coax and run the example:

```sh
coax/build.sh
coax/bin/coax run psionic/examples/learn-number.coax
```

The actual current output is:

```text
model: prediction = weight * x
before: weight = 0, loss = 10.0
one update: weight = 1.25, loss = 1.40625
16 updates: weight = 1.9999996, loss = 3.1974423e-13
held-out: x = 3, prediction = 5.999999, target = 6
```

To check the library, the book, and the native C boundary together:

```sh
psionic/check.sh
```

There are no model files to download. The example owns a few small arrays;
the computation runs on the CPU. The checker uses Python only for book
verification and the C compiler for native tests and sanitizers.

## A model is a rule with a knob

Our rule is:

```text
prediction = weight * x
```

The single adjustable number `weight` is the model's parameter. If it is
zero, every prediction is zero. If it is two, the rule doubles any input.
The training function receives the examples below; it does not contain a
special case that assigns the answer two.

| Input x | Target y | Prediction with weight 0 |
| --- | --- | --- |
| 1 | 2 | 0 |
| 2 | 4 | 0 |

There is no added constant, usually called a bias. Our model can learn a
relation through the origin. It cannot express every possible relation.

## Measure the error

For one example, subtract the target from the prediction. Square that error
so positive and negative mistakes both count. Average the squared errors
across the examples. That average is the **mean squared error**, our loss:

```text
error[i] = weight * x[i] - y[i]
loss = sum(error[i] * error[i]) / sample_count
```

At weight zero the errors are `-2` and `-4`. Their squares are `4` and `16`.
The loss is `(4 + 16) / 2 = 10`. Smaller is better for this objective. A loss
of zero means every training example matches exactly.

## Which direction should the knob turn?

The derivative tells us how the loss changes when the weight changes a
little. For `error = weight*x - y`, the derivative of `error²` with respect
to weight is `2*x*error`. Average those terms to get the gradient:

```text
gradient = sum(2 * x[i] * error[i]) / sample_count
```

At weight zero:

```text
gradient = (2*1*(-2) + 2*2*(-4)) / 2
         = (-4 - 16) / 2
         = -10
```

A negative gradient says increasing the weight reduces the loss locally.
We subtract a small multiple of the gradient. That operation is called
**gradient descent**. Our learning rate is fixed at `1/8`:

```text
next_weight = weight - (1/8) * gradient
            = 0 - (1/8) * (-10)
            = 1.25
```

Now the errors are `-0.75` and `-1.5`. The new loss is
`(0.5625 + 2.25) / 2 = 1.40625`. These first-step values all have exact
binary representations. They are an independent oracle for the program.

## Implement the equation once

Here is the actual shared-library procedure. Its caller has already checked
that the two input spans have equal nonzero length and bounded values.
`f32` names the 32-bit floating-point type; `[] f32` is a borrowed view of
caller-owned storage. Read [Coax's memory chapter](../../coax/book/ch06-memory.md)
for the details of views.

<!-- source: psionic/lib/learn.coax moments -->
```jai
psionic_moments :: (xs: [] f32, ys: [] f32, weight: f32) -> PsionicStats {
    loss: f32 = 0; gradient: f32 = 0;
    for i: 0..<xs.count {
        error := weight * xs[i] - ys[i];
        loss += error * error;
        gradient += 2.0f32 * xs[i] * error;
    }
    return PsionicStats.{loss = loss / f32(xs.count), gradient = gradient / f32(xs.count)};
}
```

Each operation and sum uses `f32`, in sample order. The native test builds
disable fused multiply-add and fast-math transformations. This chapter does
not claim bit-identical math on different devices or under other arithmetic
options. The loop allocates no memory.

## Write the expectations before the implementation

The file begins with `#test` and finite `#forall` checks. The first check
calls this procedure and asserts the independently derived loss `10` and
gradient `-10`, then checks the first trained weight and loss. Other checks
exercise sample/step boundaries, both trace capacities, signs and numeric
limits, unchanged storage on refusal, and failure during later updates.

Run those authored checks directly:

```sh
coax/bin/coax test psionic/lib/learn.coax
coax/bin/coax mutate psionic/lib/learn.coax
```

The file is `#authority` because it decides whether work and storage are
admitted. Builds execute its authored checks; the check script also runs
mutation testing and requires zero survivors. The tests enumerate concrete
cases. They do not prove correctness for every input or native address.
[Coax's verification chapter](../../coax/book/ch25-verification-boundaries.md)
explains the difference.

## Keep the work and storage bounded

This first API deliberately has a small contract:

| Item | Contract |
| --- | --- |
| Samples | Equal-length x/y spans, between 1 and 16 values each. |
| Updates | Between 1 and 64; learning rate is always `1/8`. |
| Numbers | Finite inputs, targets, and initial/candidate weights in `[-16, 16]`; signed zero and subnormals are accepted. |
| Loss trace | Caller supplies at least `updates + 1` floats: initial loss, then one loss after each update. |
| Scratch | A second caller buffer of at least the same length, used before publishing the trace. |
| Aliasing | Writable buffers are disjoint from each other and from inputs. Read-only x/y may share storage. |
| Lifetime | All storage remains valid for this synchronous call. No borrowed view is retained or returned. |

At these magnitudes, a prediction is at most `256` in absolute value and a
training error at most `272`. Sixteen squared errors total at most
`1,183,744`; the absolute gradient sum is at most `139,264`. A candidate
weight before admission has magnitude at most `1,104`. These small bounds
keep intermediate arithmetic far below `f32` overflow even when an unstable
update is about to be refused.

The numeric classifier examines the sign-cleared IEEE binary32 bits and
compares them with the encoding of `16`. Larger magnitudes, infinities,
and every NaN encoding are outside the admitted range. The evaluator checks
finite values and raw integer bit patterns; native tests pass real NaNs,
infinities, signed zero, and subnormals through the generated ABI.

This domain does not guarantee decreasing loss for arbitrary datasets.
Some data need a smaller learning rate. For now an update outside the weight
domain returns `update`; there is no clamping or silent retry.

## Publish a result only after the work succeeds

The shared trainer is:

<!-- source: psionic/lib/learn.coax trainer -->
```jai
psionic_train_checked :: (xs: [] f32, ys: [] f32, initial_weight: f32, steps: int,
        trace: [] f32, scratch: [] f32, overlap: bool) -> PsionicTraining #export_c {
    status := psionic_admit(xs.count, ys.count, steps, trace.count, scratch.count, overlap);
    if status != PsionicStatus.ok { return PsionicTraining.{status = status}; }
    if !psionic_number(initial_weight) { return PsionicTraining.{status = PsionicStatus.number}; }
    for i: 0..<xs.count {
        if !psionic_number(xs[i]) || !psionic_number(ys[i]) {
            return PsionicTraining.{status = PsionicStatus.number};
        }
    }
    weight := initial_weight;
    stats := psionic_moments(xs, ys, weight);
    scratch[0] = stats.loss;
    for step: 0..<steps {
        next := weight - 0.125f32 * stats.gradient;
        if !psionic_number(next) { return PsionicTraining.{status = PsionicStatus.update}; }
        weight = next;
        stats = psionic_moments(xs, ys, weight);
        scratch[step + 1] = stats.loss;
    }
    for i: 0..<steps + 1 { trace[i] = scratch[i]; }
    return PsionicTraining.{status = PsionicStatus.ok, weight = weight, loss = stats.loss, written = steps + 1};
}
```

Shape, step, capacity, overlap, and numeric admission happen before any write.
The trainer then writes losses into scratch. A candidate outside the weight
domain returns an error while the published trace stays unchanged. On success,
it copies just the required prefix into the trace. Unused tail elements stay
untouched. Scratch can contain partial work after an update failure.

The result reports `status`, `weight`, `loss`, and `written`. On success,
`written` is `updates + 1`; the other fields describe the final model. On
failure the numerical result fields are zero and must not be installed as a
new model. The statuses, in admission order, are `shape`, `steps`, `capacity`,
`alias`, `number`, then `update` during execution. `ok` means success.

The pure Coax core receives an overlap observation. Our example owns four
separate local arrays, so it can supply `false`. Native callers use
[platform/memory.h](../platform/memory.h): `psionic_train` observes real spans
in [memory.c](../platform/memory.c) and forwards the observation to Coax.
The adapter contains no training equations. It checks whole declared spans,
including unused capacity, conservatively. As with a normal C view API,
callers must supply valid pointers, extents, and lifetimes; the adapter cannot
prove that an arbitrary pointer really owns memory.

## Train, then predict

This is the complete successful example, calling the same library used by
the native tests:

<!-- source: psionic/examples/learn-number.coax example -->
```jai
#load "../lib/learn.coax";
#import "Basic";

main :: () {
    xs := [2] f32.[1, 2]; ys := [2] f32.[2, 4];
    trace: [17] f32; scratch: [17] f32;
    // These four local arrays own disjoint storage, so overlap is false.
    first := psionic_train_checked(xs[..], ys[..], 0, 1, trace[..], scratch[..], false);
    #assert first.status == PsionicStatus.ok;
    print("model: prediction = weight * x\n");
    print("before: weight = 0, loss = %\n", trace[0]);
    print("one update: weight = %, loss = %\n", first.weight, first.loss);
    trained := psionic_train_checked(xs[..], ys[..], 0, 16, trace[..], scratch[..], false);
    #assert trained.status == PsionicStatus.ok;
    prediction := psionic_predict(trained.weight, 3);
    #assert prediction.status == PsionicStatus.ok;
    print("16 updates: weight = %, loss = %\n", trained.weight, trained.loss);
    print("held-out: x = 3, prediction = %, target = 6\n", prediction.value);
}
```

The first call performs one update. The second starts again at zero and
performs sixteen. It does not continue from the first call. Prediction is a
separate call with the learned weight and input `3`, which was held out of
training. The result is about `6`, as expected from the underlying relation.
This synthetic held-out example checks our small rule; it says nothing about
generalization on real-world data.

## Why stop near two instead of exactly at two?

Substituting our two samples into the gradient gives `5*weight - 10`.
In exact arithmetic, one update is:

```text
next_weight = (3/8) * weight + 5/4
next_error_from_2 = (3/8) * current_error_from_2
weight_after_k = 2 - 2*(3/8)^k
loss_after_k = 10*(9/64)^k
```

These closed forms are independent native test oracles. The test checks
every step count from 1 through 64, and compares additional small datasets
with a separately derived quadratic loss.

After sixteen steps the exact weight error is about `3.06e-7`. For this
fixture and operation order, a conservative absolute rounding allowance of
`2^-22` per update accumulates to at most `2^-22 / (1 - 3/8)`. Adding the
exact error gives less than `7e-7`. We use a weight tolerance `1e-6`, a
held-out prediction tolerance `3e-6` (including its multiply rounding), and
a final training loss bound `1e-11`. These are fixed expectations, not
values copied from the observed output. They are specific to this workload.

The native and Coax printers show different numbers of significant digits.
The stable example transcript is checked exactly in both build modes;
native numerical correctness is checked against the independent values
and tolerances rather than their printed text.

## Make a bad request

[refuse-capacity.coax](../examples/refuse-capacity.coax) supplies only one
trace slot for one update. It receives `capacity`, confirms both output and
scratch are unchanged, and prints:

```text
refused: one update needs two trace values
trace and scratch unchanged
```

Run it with:

```sh
coax/bin/coax run psionic/examples/refuse-capacity.coax
```

The book's [example manifest](examples.json) covers every runnable program,
its expected exit status, transcript, and both debug/release modes. The book
checker compares the listings above directly with their source markers and
checks local links. Native tests exercise actual short buffers and aliases
under ASan/UBSan as well as ordinary compiled execution.

## Try a change

Change the targets to `[-2, -4]`. Derive the first gradient, update, and loss
before running the program. Then add an authored check for them. The held-out
prediction should approach `-6` with the same step count and error budget.

Next try `[3] → [16]` for five updates in a separate test. The candidate
weights initially stay inside the domain, then leave it. Check that no
published trace changes even though scratch contains completed work. The
library already retains this regression so publication remains atomic as
later chapters add more operations.

We now have a shared numerical implementation, a learning rule, prediction,
bounded admission, and a reproducible verification path. The next chapter
will give arrays shapes and storage contracts so the same ideas can grow
beyond one parameter.
