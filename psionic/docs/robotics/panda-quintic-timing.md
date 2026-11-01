# A timed joint reference, before tracking

This profile assigns time to one straight seven-joint edge. It returns reference
angles, rates and accelerations; it does not move the plant. Both ends have zero
reference rate and acceleration. Joining edges stops at each waypoint. Retargeting
a moving reference requires a different boundary-value problem.

For u=t/T, progress is s=10u³−15u⁴+6u⁵ and q=q0+(q1−q0)s. Progress is monotone,
so the real curve stays in each endpoint interval. Its maximum rate is 15/8 at
u=1/2. Its maximum absolute acceleration is 10/sqrt(3), at u=(3±sqrt(3))/6.
Each joint requires T at least the larger of (15/8)*abs(dq)/velocity_limit and
sqrt((10/sqrt(3))*abs(dq)/acceleration_limit). Select the largest requirement
across all joints. No jerk constraint is imposed.

## The binary64 policy

Source joint boxes admit q0 and q1. Velocity limits are .001 through 20 rad/s;
acceleration limits are .001 through 100 rad/s². The numerical recipe fixes
Cv=1.875 and Ca=5.773502691896258. First round dq=q1−q0, then form
Tv=(Cv*abs(dq))/v and Ta=sqrt((Ca*abs(dq))/a) with the written grouping.
Select the largest of fourteen candidates and multiply by 500 to obtain R_float.

Admit every input before testing numeric endpoint equality. A zero edge has N=0.
Otherwise N=ceil(R_float)+1. Refuse R_float>999999 before conversion, giving
N<=1,000,000. This reserve tick is deliberate; it is neither the exact-real ceil
nor the fastest duration on the clock grid. A tiny nonzero edge remains nonzero
even if its estimate underflows to zero.

One tick is exactly 2,000,000 logical nanoseconds. Total nanoseconds=N*2,000,000;
the floating duration is N/500. Sample tick k uses k/N, not accumulated .002
additions. The sampler re-admits all inputs and recomputes timing. A returned
POD is not an admission certificate. A zero edge permits only tick 0 and copies
the target representation, including signed zero. Other endpoints are copied
exactly, with positive-zero rates and accelerations.

Inside the interval, use the nearer endpoint: v=min(k,N−k)/N and
b=v³*(10+v*(-15+6v)). Anchor position at q0 in the first half and q1 in the
second. Derivative magnitudes use 30v²(1−v)² and 60v(1−v)(1−2v), reversing
acceleration sign in the second half. No joint clamp changes the curve. The
returned derivatives describe the analytic reference, not differentiation of
floating-point rounding.

## The reserve under the declared numerical assumptions

Assume binary64 round-to-nearest, gradual underflow, correctly rounded sqrt,
no reassociation/fast-math and no contraction. Exact input values are the stored
binary64 angles/limits. Compare the estimate with the real quintic requirement
using exact endpoint subtraction, Cv=15/8 and Ca=10/sqrt(3).

For normal operations, a relative bound 2^-45 is deliberately loose: each
joint candidate has one subtraction, one multiplication, one division and,
for acceleration, one sqrt. The fixed Ca literal 5.773502691896258 has relative
error below two unit roundoffs. Selection of a maximum introduces no rounding
and preserves a common relative bound. Multiplication by 500 adds one rounding.
A standard gamma bound for 32 roundings is well below 2^-45. The fourteen
candidates (seven per family) are selected, not summed. Source |dq|<6 and v>=.001 imply R_real<5,625,000;
the acceleration requirement is smaller (sqrt((10/sqrt3)*6/.001)*500<100,000).
Thus normal absolute tick error is below 2^-22, hence below 2^-20.

A subnormal endpoint difference is exact: it is an integer multiple of the
minimum binary64 subnormal. Subsequent underflow needs an absolute bound,
not that relative argument. A multiplication/division underflow error is at
most half a minimum subnormal; propagation through v,a>=.001 and
|sqrt(x)-sqrt(y)|<=sqrt(|x-y|) leaves an absolute tick error far below 1e-150.
Tiny nonzero edges must retain nonzero-edge identity even if R_float becomes 0.
Max selection between tiny and normal candidates does not enlarge these bounds.

For any nonzero accepted edge, N=ceil(R_float)+1 gives
N-R_real>=1-epsilon, epsilon=2^-20. N<=1e6 therefore leaves at least
(1-epsilon)/1e6 relative velocity reserve. More precisely, with delta=(1-epsilon)/N,
velocity utilization is at most 1-delta and acceleration utilization is at most
(1-delta)^2, so acceleration reserve is at least 2*delta-delta^2.
N/500 is a rounded representation of exact integer-nanosecond time, not an
accumulated clock. Endpoint copies and stable half-interval sampling address
representation effects separately. Scalar sampling errors and sampled limit
observations are qualified against independent analytic values. These are
declared numerical-profile assumptions, not compiler bounded-control or
cross-target rounding certificates. No continuous collision/tracking/jerk claim.

## What this can be connected to

A monotone timing change preserves the point set of a static geometric edge.
It supplies no collision evidence. Collision reserves, dynamic obstacles,
actuator limits, tracking error and runtime supervision remain separate.
It is not a general 20 Hz VLA chunk interpolator: it requires rest at both ends.

The [frozen experiment](../../fixtures/robotics/panda-quintic/experiment.json)
and [independent reference](../../fixtures/robotics/panda-quintic/reference.json)
keep an exact rounded-operation policy oracle separate from 80-digit analytic
polynomials and continuous derivative extrema. Sparse samples are comparison
points, not the argument for continuous bounds. No simulator, neural model,
renderer or physical robot is executed by this reference query.
