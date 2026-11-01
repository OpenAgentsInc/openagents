# A rest-to-rest quintic reference, not plant motion

This source-bound query retimes a straight seven-joint edge. Both endpoint
reference rates and accelerations are zero; concatenating edges stops at every
waypoint. It is not a moving-state retargeter or a drop-in 20 Hz VLA chunk
interpolator. No collision certificate, tracking behavior or motion execution
is implied. See the [timing design](../../../docs/robotics/panda-quintic-timing.md)
for the frozen formula, assumptions and reserve argument.

Public typed/raw timing and sample calls take q0/q1 in radians, velocity limits
in rad/s and acceleration limits in rad/s². Shapes precede q0 then q1 then vlim
then alim admission, per-index finite/range checks. Both endpoints use the
source joint boxes; vlim is .001..20 and alim .001..100 inclusively. Every input
is admitted before numeric-zero detection. Timing precedes tick admission;
duration refusal therefore wins over an invalid tick. Every ordinary refusal
preserves the complete old output. Raw adapters may copy scalars before their
delegated numerical admission.

Cv=1.875, Ca=5.773502691896258. The recipe rounds q1-q0, then calculates
Tv=(Cv*absdq)/v and Ta=sqrt((Ca*absdq)/a), with exactly that grouping. The maximum
of fourteen candidates is multiplied by 500. R>999999 refuses before any integer
cast. Otherwise a nonzero edge uses ceil(R)+1, implemented as floor plus a
strict fractional increment followed by one reserve tick. No epsilon is removed.
N<=1e6, nanoseconds=N*2e6 exactly, and duration=N/500. A zero edge has N0 and only
sample k0; q1 wins representation, including signed zero. A distinct subnormal
edge remains nonzero even when the time estimate underflows.

Sampling uses the nearest endpoint, half-interval Horner progress and analytic
first/second derivatives. Midpoint ties choose the first half. Exact endpoints
copy q0/q1 and +0 derivatives; no joint clamp is applied. Derivatives describe
the analytic curve, not rounded-value differentiation. Bounded admitted endpoints
and T>=.002 for nonzero edges keep arithmetic finite. No unreachable generic
output-admission recheck is claimed as separately observed product policy.

All helpers remain source-visible and unsealed. Quantize trusts the admitted-edge bound of finite,
nonnegative required seconds below 11250; the sampler requires admitted endpoints and their
actual computed timing with 0<=k<=N. A returned plan POD is not an admission
certificate, so public sample calls recompute it. The small owned analytic
coefficient fixture uses an explicitly supplied grid to observe derivative math;
it is not presented as a public speed-limit admission.

The 27 new frozen edges cover zero/signed zero/subnormal, both signs, dense limits,
whole source boxes, velocity/acceleration near-grid cases and duration-cap
neighbors. 25 timing records and 236 samples publish; two timing/two sample
refusals use unavailable transport only after atomicity checks. 47 endpoint samples
have exact angle copies and positive-zero derivatives. Independent exact-input
Fraction rounded operations give exact policy N; 80-digit Decimal real quintics
and extrema provide a separate analytic oracle. Observed estimate error is at
most 1.384e-10 tick and minimum real reserve 0.9999999999999937 tick. The declared
numerical assumptions and loose error allowance are documented separately.

First debug/release/fail-fast ASan+UBSan profiles each pass 26,756 assertions.
Complete files are byte-identical across modes; timing files also equal the
independent oracle exactly. Maximum angle/rate/acceleration errors are 4.441e-16
rad / 4.441e-16 rad/s / 1.421e-14 rad/s². 24 overlaps, 56 numeric endpoints, 140 IEEE cases,
10 tick refusals and 20 poisoned-reader controls pass. Position budget 2e-12 rad;
rate/acceleration budgets 2e-10+2e-12*abs(reference). N/ns/status/endpoint copies
and owned-mode bytes are exact criteria.

Thirteen new source groups, four meaningful E0317 unit refusals and 107/89 actual
emitted root sites are qualified with zero survivors. Initial mutation results
and precise endpoint/shared-half-policy closure are preserved, with no equivalence
waiver, corpus or budget tuning. Actual counts do not establish complete authored
quantity-token provenance. The example passed an independently prewritten output
in debug/release and its own display check.

A certified static geometric edge would retain its point set under monotone
time change, but the current API does not construct or bind such certification.
Collision, tracking error, dynamic obstacles, jerk, time optimality, R13 clocks,
compiler #bounded/R14 and hardware qualification remain separate. No SDK, neural,
old physics/IK/nullspace/rotor corpus or mutation gate was repeated.
