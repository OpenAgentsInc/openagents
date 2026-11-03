# Psionic oracles: correctly rounded sin, cos, exp, log and pow (#12695)

2026-11-03, Linux (glibc, Python 3.13), no other machine.

## Cause

`decoder_oracle.py` and `muon_oracle.py` called Python's `math.sin`,
`math.cos`, `math.exp`, `math.log` and `**`, which are the C library's. glibc
and macOS's libm differ in the last place (#12689: glibc's `sin(4)` is
correctly rounded, macOS's is the next double away from zero). The committed
references were recorded on the Mac, so the oracles didn't reproduce them on
Linux. Neither library is correctly rounded everywhere: over 3,000 random
inputs (|x| up to 600 for sin and cos), glibc differed from the correctly
rounded result 11 times. Each disagreement was confirmed by summing the
unreduced Taylor series at 500 digits.

## Fix

New `psionic/tests/cr_math.py` (Firm reason 2) returns the double nearest
the exact value. It computes in Python's `decimal` and `fractions` (pure
software) with an error bound, rounds once with `float(Decimal)`, and raises
the precision while the bound straddles a rounding boundary (Ziv's method).
sin and cos reduce by a 2*pi computed to the needed digits. exp and log use
decimal's correctly rounded `exp` and `ln`. pow is exact for integer
exponents, `math.sqrt` for 0.5 (IEEE-exact), and exp(y ln x) otherwise. It
doesn't share code or algorithm with `psionic/lib/strict-math.coax` (fdlibm,
under 1 ulp). Both oracles now use it for every transcendental call and power.
`math.sqrt` stays because IEEE 754 defines it exactly. Both oracles require
Python 3.12 or later, because they rely on 3.12's compensated `sum()`.

## References: rerecorded

The old values came from macOS's libm, which isn't correctly rounded, so they
were rerecorded from the platform-independent oracle.

**Decoder** (`TD_ORACLE_*` in `psionic/lib/transformer/decoder.coax`): 4 of
21 numbers moved by about 1 ulp. The new values equal the old oracle's output
on Linux. Checks compare within 1e-12, so the Coax side is unchanged.
Previous (Mac) values:

    TD_ORACLE_LOSS :: 1.8059255342158516;
    TD_ORACLE_LOGITS :: [20]f64.[0.3326536694254402,-0.33690265960461363,0.3022207555948625,-0.2326156358658774,0.13613054220848106,0.1762938243862832,-0.26067992506404286,0.31494307393205667,-0.3328128755312528,0.312224379243382,-0.5811340956483357,0.4951734754043695,-0.3519929264167466,0.1681377226500198,0.03514668923674214,-0.44766145897115195,0.4479311370634924,-0.3964399898735277,0.29913809332308927,-0.16726919907455778];

`TP_ORACLE_LOGITS` (`--split`, chapter 183) didn't change.

**Muon** (`psionic/fixtures/muon/reference.{json,coax}`): 1,059 of 6,487
values changed, with a largest change of 2.5e-14 relative to 1+|old|. The
source is 69 gradient inputs and one Polar Express input that came from a sin
or cos 1 ulp off the correctly rounded value; for example, the Mac's
`sin(4)` = `-0x1.837b9dddc1eafp-1` against the correct `-0x1.837b9dddc1eaep-1`.
The rest follow from those inputs. The previous reference is the version at
7704ccd3f1 (`git show 7704ccd3f1:psionic/fixtures/muon/reference.json`). The
old oracle on Linux (glibc) would have differed from the new reference in 139
values (largest 7.2e-16), because glibc isn't correctly rounded either.

## Checks

Linux, coax0 0.48.0 (the cached release build 727901f6a2be):

- `psionic/tests/check_decoder.sh`: passes in 26 s. The oracle, `--coax`
  against `decoder.coax`, debug and release native plain/ASan/UBSan, and the
  example against `causal-decoder.linux.out`.
- `psionic/tests/check_muon.sh --mutate --jobs 8`: passes in 59 s. The
  reference reproduces, and native debug/release plain/ASan/UBSan stay within
  1e-12 of the reference (largest gap 1.63e-14). Mutation: muon.coax
  root-only, 743/743 killed; strict-math.coax, 189/189 killed.
- `psionic/tests/check_tensor_parallel.sh`: passes. The `--split` oracle
  output is unchanged.
- On the Mac: not run, see NEEDS_OWNER.md.
