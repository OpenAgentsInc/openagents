# RB-00 freeze and RB-01a decoded experiment contract

The first slice of the [robotics roadmap](microduck-implementation-roadmap.md)
is an owned, synthetic CPU F64 query. It admits already decoded descriptions
and maps normalized actions to requested/limited joint angles. It is not a
Microduck calibration, robot importer, persistent format, motor command,
simulator or training result.

## Inventory and boundary

Reused owners are `coax/lib/quantity.coax` for angle quantities and scalar
scaling, `coax/lib/crypto/bytes.coax` for exact 32-byte identity comparison,
and `psionic/lib/robotics/locomotion/gaussian-density.coax` for the existing
IEEE F64 admission predicate. Native qualification shares
`coax/lib/sys/native-check.coax`, extracted from the construction gate and
consumed by both that gate and the existing robotics dispatcher.

Panda's fixed actuation coefficients and GR00T's LIBERO action normalizer
have different contracts. Neither is renamed into a generic robot model.
No existing decoded experiment/embodiment owner was found in the inventory.
New units are `experiment-types.coax` and `experiment-policy.coax` under
`psionic/lib/robotics/`; the policy owns all admission and action decisions.

An experiment carries caller-declared 32-byte model, assembly, calibration,
task, partition and evaluator identities. They must be present. Admission
compares model/assembly/calibration against the caller's expected embodiment,
then compares **every active joint mapping field**, including exact F64 bits.
Keeping an old identity while swapping slots, signs or scales therefore refuses.
This matches caller-provided expectations; it does not verify source files,
licences, signatures, or the origin of an expected embodiment.

The in-memory structs are not a persistent schema. The next RB-01 slice must
register the versioned inert shape and independent golden bytes before adding
its owning codec. No struct-memory dump is permitted as a shortcut.

## Frozen supported profile

| Field | Admitted domain |
| --- | --- |
| Joint/action count | 1–16, with complete unique joint and motor permutations of 0…count−1 |
| Observations | 1–32, unique (joint, signal) pairs |
| Signals and units | 1 = joint angle/radians; 2 = joint angular rate/radians per second |
| Frame / execution target | 1 = joint-local; 1 = CPU F64 query |
| Actuator mode / unit | 1 = absolute position target; 1 = radians |
| Joint sign | −1 or +1 |
| Home / low / high | finite, absolute value ≤100 radians; low < high and low ≤ home ≤ high |
| Action scale | finite, 0 < scale ≤100 radians |
| Observation offset / scale | finite, abs(offset) ≤1,000,000 and 0 < scale ≤1,000,000 |
| Physics / control period | integer nanoseconds, 1…1,000,000,000; control is 1–128 whole physics periods |
| Collection budget | 1–16 worlds, 1–128 steps, 1–4,096 cycles; these are declarations, not allocated simulation |
| Seed | integer 0…2,147,483,647 |
| Latent action | finite F64 in [−1,+1]; refused outside, never silently rescaled |

These caps bound admission and arithmetic. They establish neither a real-time
deadline nor stability, calibration accuracy or an executable rollout. In
particular, this angle/rate-only observation profile does not implement the
61-observation vendor interface. The action map retains radians through the
quantity operations:

`requested = home + scale × (sign × latent)`

`limited = min(high, max(low, requested))`

Saturation reports whether requested and limited differ. Results retain the
joint and motor identifiers in action-slot order. All inputs and metadata
are checked before staged targets are published. A refusal writes no output.
Inactive fixed-array slots are copied by decoded admission but are not active
bindings; the later codec must define canonical handling for those slots.

## Independent fixture, fixed before the product functions

The fixture's identity bytes are illustrative tokens: repeated bytes 1 through
6 for model, assembly, calibration, task, partition and evaluator. They are
not hashes of a real robot. The local frame is joint-local; there are four
angle/rate observations, four worlds, horizon eight, 256 cycles, seed seven,
2,000,000 ns physics and 20,000,000 ns control periods.

| Action slot | Joint | Motor | Sign | Home | Scale | Low | High |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 0 | 1 | +1 | 0.20 | 0.40 | −0.10 | 0.70 |
| 1 | 1 | 0 | −1 | −0.125 | 0.25 | −0.25 | 0.125 |

The table below is direct decimal/rational arithmetic, not output generated
by the implementation. For example, slot 1 at +1 requests
−1/8 − 1/4 = −3/8, which limits to −1/4. A 1e−15 absolute tolerance covers
the non-dyadic F64 values in slot 0; slot 1 is checked exactly.

| Latent | Slot 0 requested / limited | Slot 1 requested / limited |
| --- | --- | --- |
| −1 | −0.20 / −0.10 | 0.125 / 0.125 |
| −0.5 | 0 / 0 | 0 / 0 |
| +0 or −0 | 0.20 / 0.20 | −0.125 / −0.125 |
| +0.5 | 0.40 / 0.40 | −0.25 / −0.25 |
| +1 | 0.60 / 0.60 | −0.375 / −0.25 |

Tests also exercise maximum capacities, exact period ratios, every identity
byte, every map field, duplicate coordinates, shape/length refusals, integer
endpoints, adjacent IEEE values, NaN/infinities, negative zero and late-action
atomicity. Native runs use these independently specified table values through
the public API, with debug/release and ASan/UBSan variants.
