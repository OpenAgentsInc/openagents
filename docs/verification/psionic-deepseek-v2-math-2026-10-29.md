# DeepSeek V2-Lite attention components — 2026-10-29

This is progress toward chapter 90, not its completion record. The full
original V2-Lite decoder, tensor admission, dense/routed/shared FFNs and original
request comparison remain outstanding. Chapter 90 stays planned in the book.

`mla.coax` implements both compressed and expanded latent-attention CPU forms,
with BF16 up-projection coefficients, F32 stored activations/cache rows, and
compensated binary64 products/reductions. It consumes already-rotated query/key
inputs. Compressed query absorption and latent-context value projection move
linear products across attention and therefore have different F32 rounding
boundaries from the expanded form; no general bit-equality claim is made.

An independent stdlib Python oracle reconstructs the full prefix and evaluates
both forms using `math.fsum`, explicitly rounded F32 stages and BF16 parameters.
Its owned case has two heads, two plain/two rotary dimensions, a three-wide
latent and two values per head over four tokens. Every output, pending cache
row and stabilized attention exponential is compared. The observed maximum
compressed/expanded output difference is 3.7252903e-9. The fixture’s acceptance
bound is 2e-7 for output and 1e-13 for exponentials; pending rows match exactly.
This is evidence for that fixture, not a uniform model-error bound.

With original V2 dimensions, a compressed row is 576 F32 values versus 4,160
in expanded form. The core returns a pending row and never mutates history.
Output and pending row publish only after every head/value projection succeeds.
That allows the forthcoming full model to delay all cache commits until after
the final vocabulary head. Failed calls preserve public buffers; private work
and score scratch may change. Success preserves spare output tails.

`v2-rope.coax` implements the pinned original V2 adjacent-pair-to-split-half
permutation, fixed YaRN factor 40, theta 10,000, original context 4,096, beta
fast/slow 32/1, and 0.707 mscale/attention-scale rules. Frequencies and trig are
evaluated in binary64 before F32 coefficient storage; rotations use wide
products and F32 outputs. This is an explicit CPU reference, not upstream
Torch F32/BF16 or GPU bit parity. The fixed-width domain guarantees
0 <= low < high <= rope-1, so the upstream singular-range fallback is
unreachable. The ramp’s absolute-value expression clamps it to [0,1].

Independent rotary DATA covers widths 2, 8, 64 and 128 at positions 0, 1,
4,096 and 163,839, plus a nonuniform two-head adjacent-pair permutation.
Coefficient tolerance is 2e-7, rotated values 5e-7, and attention scaling 1e-15.
Finite overflow in either rotated component refuses both public outputs.

Commands:

```sh
psionic/tests/check_deepseek_v2_math.sh --mutate --jobs 8
psionic/check.sh --only deepseek/mla --jobs 8
```

The combined gate passes 55 Coax checks in debug and release, plus four native
plain/ASan+UBSan builds. Native checks include exact replay against an immutable
prefix, untouched history and public tails, first/final nonfinite query,
latent, rotary-key and history values, NaN/Inf scales, buffer aliases and
atomic refusals. Coax checks cover every shape/capacity boundary and late
finite overflow after earlier heads have succeeded. Mutation reports 782/782
for MLA and 701/701 for rotary, with zero survivors and no equivalent markings.
Those roots share unchanged numerical dependencies; counts are not additive
claims about unique new mutants.

Checked compiler/source/data identities:
[v2-math-validation.json](../../psionic/fixtures/deepseek/v2-math-validation.json).
