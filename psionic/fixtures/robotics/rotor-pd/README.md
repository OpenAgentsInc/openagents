# Isolated constant-rotor PD analysis

This component analyzes and steps one constant rotor with a fixed target. It
imports only the shared Coax quantity declarations. It is not a Panda model,
nonlinear stability proof, motor admission, contact simulation, or hardware test.

`RpdParams` carries M in J*s²/rad², physical B and controller Kd in J*s/rad²,
Kp in J/rad², and h in seconds. State is unwrapped error in radians and rate in
radians/second. Physical B acts at the new rate, while controller Kd acts at the
old rate: D=M+hB, a=(-Kp*e-(B+Kd)*v)/D, v+=h*a, then e+=h*v(new).
Only a complete admitted error/rate/effective-acceleration result is published.

`rpd_analyze` reports dimensionless alpha/beta, transition matrix on `[e,h*v]`,
trace/algebraic determinant, continuous natural coefficient in **1/second**,
dimensionless zeta, physically typed D and Jury terms, strict flag and class.
The one-radian state basis is explicit before coefficient extraction. A natural
coefficient cannot implicitly pass as a joint rate in radian/second.

For the constant real-arithmetic model, strict Jury stability is B+Kd>0 and
4M+2hB-2hKd-h²Kp>0; alpha>0 is guaranteed by the admitted positive M/Kp/h.
The implementation reports a computed F64 classification: stable 0, boundary 1,
unstable 2. A computed boundary is not a generic boundedness or marginal-stability
certificate: a defective unit pole can grow polynomially. An unstable diagnostic
does not itself reject a numerically admitted step.

All shapes precede numeric admission. Parameters are checked in M/B/Kp/Kd/h
order, finite and within [.01,10]/[0,100]/[1e-6,500]/[0,100]/[1e-6,.5]. Then
error/rate are checked at inclusive absolute caps 1e6 rad/1e8 rad/s. Computed results
are checked in acceleration/rate/error order at inclusive caps 1e12 rad/s²,
1e8 rad/s and 1e6 rad. No refusal changes caller output. Raw bridges may stage input
scalar copies before delegated numeric admission. All low-level owned helpers
are trusted and unsealed; arbitrary helper PODs are not admission certificates.

The compact parameter domain guarantees finite analysis arithmetic: D>=.01,
alpha<=12500, |beta|<=6000, sqrt(M*Kp)>=1e-4, zeta<=1e6 and wn<=sqrt50000.
The analysis does not need a generic arbitrary-coefficient finite-operator claim.
Typed squared quantities precede explicit scalar square roots; no sqrt-quantity
language extension or implicit angle erasure is used.

The eleven profiles/two initial states are frozen before reference generation.
Exact binary-input Fractions and 80-digit Decimal root closed forms supply all
expected responses; the product recurrence is not the oracle. Distinct-real,
complex-conjugate and repeated-root branches are covered. The dyadic repeated
pole 63/64 fixture differs from continuous critical damping (zeta 1): its own
continuous zeta is 127/128. Exact Kd3±oneULP Jury fixtures have margins 0/±2^-51.
High explicit Kd can be discretely unstable despite a large continuous zeta.
For identical M=.01, B=.02, Kp=100 and h=.002, the existing reference table gives:

| Kd | Continuous zeta | Computed flip margin | Strict class |
| --- | --- | --- | --- |
| .2 | .11 | .03888 | Stable |
| 4 | 2.01 | .02368 | Stable |
| 12 | 6.01 | -.00832 | Unstable |

First debug/release/fail-fast ASan+UBSan profiles each pass 22,225 assertions.
Eleven analyses, 2032 applied updates and 22 unapplied input-transport records
match; complete files are byte-identical across modes. Initial acceleration is
unavailable, with a zero serialization placeholder, not a product-applied value.
Fourteen overlap cases, numeric endpoints, isolated late endcaps, IEEE cases and
sixteen poisoned-reader guards pass. Maximum analysis discrepancy is 2.23e-16. Worst response budget fraction is 1.125%;
maximum absolute acceleration error is 5.22e-8 rad/s² on large values.

Frozen response budgets are 2e-10+2e-12*abs(reference); analysis budgets are
2e-12+2e-12*abs(reference). Flags/classes/statuses/mode bytes have exact criteria.
Eight new source groups, four meaningful unit refusals and 136/136 actual emitted
mutation sites qualify the new root. One initial survivor is preserved and closed
by an independent dense-profile observation, with unchanged production arithmetic
and budgets. Current quantity lowering loses some authored-token provenance;
these counts do not claim full quantity-token mutation coverage.

The independently prewritten example passes its own display check and both
execution modes. No old Panda, pose, kinematic, physics, SDK, neural or renderer
gate was repeated. This is an isolated scalar-libm/F64 numerical profile, not
compiler `#bounded`, R14 or cross-machine reproducibility qualification.
