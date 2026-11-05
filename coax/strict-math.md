# Fixed CPU scalar math

Issue [#12917](https://github.com/OpenAgentsInc/openagents/issues/12917)
replaces host-libm calls for CPU `sin`, `cos`, `exp` and `log` with
[one Coax implementation](lib/compiler/strict-math.coax). The C++ bootstrap
evaluator compiles its generated C seed; the emitter embeds that same seed
with private names into each C translation unit that uses these intrinsics.
Generated programs need no Coax installation or extra math object to link.
Verification C uses the same emission path. Metal and CUDA retain their own
math profiles; `tan` and `pow` remain host-libm operations.

The profile assumes IEEE binary64 round-to-nearest, gradual underflow and no
contraction or reassociation. f32 inputs widen exactly to binary64 and round
once on return. External C consumers must preserve these arithmetic rules
(`-ffp-contract=off`, no fast math); the normal compiler driver does so.

The sine/cosine polynomials and exp/log coefficients derive from Sun's
[fdlibm](https://netlib.org/fdlibm/), with its permission notice retained in
the Coax source. Small angles use the split reduction already qualified in
Psionic's bounded math library. Large angles multiply the 53-bit significand
by fdlibm's 1584-bit `2/pi` table with exact 24-bit integer limbs. The quadrant
and three fractional chunks produce a double-double remainder; they do not
first reduce modulo a rounded binary64 `2*pi`. Exp scales subnormal results
in two stages; log normalizes subnormal inputs before reducing its mantissa.
These are fixed approximations, not correctly rounded transcendentals.

Runtime sine preserves negative zero, cosine returns one at either zero,
exp underflows gradually and overflows to positive infinity, and log returns
negative infinity at either zero and NaN for negative inputs. NaN inputs
produce NaNs; sine/cosine of infinity produce NaNs. Compile-time nonfinite
results continue to fail with E0412. Psionic's older bounded `psm_*` functions
keep their explicit saturation and angle limits.

`tests/run.sh --only strict_math` runs the independent Decimal oracle, checks
the full exponent range by sampling, compares debug/release bits, replays
compile-time answers natively and checks generated C under ASan/UBSan. Its
two-ulp sample bound is evidence for those cases, not a universal accuracy
theorem. `seed-portability.sh --linux` also runs this oracle with Linux GCC
and compares the generated seeds. The book's exact training transcripts and
digests are separate end-to-end checks; there are no per-host math pins.

Normal bootstrap builds test the Coax source and compare its C, header and
embedded text with the committed seed; they never refresh a mismatch. To
refresh intentionally with a previously trusted compiler, from `coax/`:

```sh
bin/coax test lib/compiler/strict-math.coax --library
bin/coax c lib/compiler/strict-math.coax --library --header bootstrap/strict-math.h > bootstrap/strict-math.c
{ printf 'R"COAXSM('; cat bootstrap/strict-math.c; printf ')COAXSM"\n'; } > bootstrap/strict-math.inc
```

R15's full language profiles, jobs, SIMD rules and integrators remain planned.
