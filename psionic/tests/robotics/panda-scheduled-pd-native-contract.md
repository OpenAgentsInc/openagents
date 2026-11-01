# Common scheduled-PD native proof

This is a new, fixed controller law. It does not change Part A's nine valid,
zero-feasible result or promote its profile8. All cases use Kp
[240,240,240,240,160,80,40] and ordinary Kd
[16,16,16,16,10,6,4]. Only joint7 damping is scheduled from the absolute
reference-minus-actual rate error. The sealed affine recipe returns4 at or
below0.1rad/s,20 at or above0.5rad/s, and the declared rounded interpolation
between them. It receives no case ID and never retains the old brake override.

The law, goals and separate future C/D validation descriptors were sealed
before the new brake result. C/D and the old unused A/B heldouts are not run
here. Positive requested relative damping power is not a whole-plant
passivity or discrete-stability certificate; the reference moves and the
combined P+D+gravity torque is clipped. The interior incremental damping can
reach40 although the gain itself is bounded20.

## Reuse and execution boundaries

The first stage builds the new interfaces in debug, release and fail-fast
ASan/UBSan. Each profile checks688 new ABI assertions and62 scalar probes with
360 assertions. Scalar status and diagnostic payloads match the independently
sealed table exactly. Invalid transport diagnostics are zero only after the
full poisoned output, spare object and canaries are observed unchanged.

Next,2,250 NEW controller calls use the authenticated old profile8 prestates
for cases0–2. Every712 command byte and every1,040 reconstructed fresh-record
byte must match the old qualified trajectory. Every schedule is low-branch
with exact gain4. Any mismatch stops the process; there is no plant fallback.
The successful carry has760,605 assertions and zero plant steps.

Only the brake scenario executes a new physical trajectory: one release run,
750 attempted and committed updates,202,535 assertions. All step outputs are
staged with input snapshots, spare objects and canaries. No other successful
step guard or old rollout is executed. The source authority checks are a
separate qualification scope.

The resulting four-case cache is scored in three native profiles,381,267
assertions each. Complete score bytes agree. The three carried360-byte metric
cards equal their old Coax cards after ONLY changing the profile identity
from8 to the new experiment's local0. Their old independent-fsum proofs carry
without recomputation. The new brake card and four-case combination have a
fresh independent audit; all3,000 schedule records are independently checked.
This is one hypothesis assessed on four cases, not a candidate selection grid.

## Interfaces and transport

The header is `panda-scheduled-pd.h`. A private identical alias under
`panda-joint-pd-tracking.h` supplies the unchanged shared transport helpers.
PsdSchedule is32 bytes: three F64 fields (signed error, absolute error, damping)
and an I64 regime. PsdGains is144 bytes; PsdForce320; PsdCommand744;
PsdStep976. The test harness also observes typed/raw scalar, gain and force
identity, force/command overlap, and score/combination overlap. Step refusal
observations execute no successful plant updates.

A PSR record is1,072 bytes: the existing1,040-byte fresh Pnt record followed by
the32-byte schedule. The public trace header is four U32 words:
PSR1,4,750,1072. Its separate index has four rows of five I64 words:
case, offset, committed count, failure stage, failure code. Complete750 rows
require status0/0; a shorter brake prefix requires a nonzero refusal. No failed
state is fabricated. Scalar result records contain I64 status plus32 diagnostic
bytes; unavailable diagnostics are transport markers, not published outputs.

The carry phase uses PSC1,3,750,1072. Its exact public identity is recovered
from the first2,250 public PSR records with the carry header. The raw brake
retains five I64 header words: case, committed count, attempted count, stage,
code. Interrupted outputs preserve actual complete-row/trailing-byte extents.
The carry ledger has2,250 exact-comparison events; the brake ledger has1,500
before/committed events. All serialized floating fields must be finite before
comparisons or maxima.

## Reproduction scope

`check-panda-scheduled-pd.sh` invokes only the additive cached-scorer adapter.
It uses committed trace/index/receipts, with no private raw trace or SDK
requirement. Its new wrapper is syntax/plan/stub-routing checked here; the
already-passing numerical results are carried, not rerun during packaging.

The explicit native driver has separate preparation, exact-carry and one-shot
brake phases. Markers prevent an accidental repeated carry or brake. The
measured driver, source, descriptor and receipts remain immutable. No SDK,
NN, renderer, contact, hardware, real-time or bounded-execution qualification
is claimed. Reference velocity/acceleration limits do not bound actual motion.
