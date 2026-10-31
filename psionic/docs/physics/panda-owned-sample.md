# Replace the fresh controller sample with owned queries

Status: planned integration. The genuine GR00T/LIBERO evaluation remains on
MuJoCo. This component replaces the fresh robot queries consumed by the Coax
controller while keeping SDK integration, contacts and observations available.
It does not claim a new policy episode result or an owned simulator.

The [FK](../../fixtures/robotics/panda-fk/README.md),
[finger branches](../../fixtures/robotics/panda-fingers/README.md),
[grip Jacobian](../../fixtures/robotics/panda-jacobian/README.md) and
[mass/zero-rate gravity](../../fixtures/robotics/panda-inertia/README.md) are
qualified separately. Velocity-dependent inverse dynamics is the next
prerequisite. A mass solve supplies a separate inertial response to an already
assembled net force; it cannot supply the missing bias by itself.

## Preserve the existing boundary

`PcSample` is 144 F64 words. The current host constructs it after an explicit
forward, then the Coax controller validates it. Its order and meanings are:

| Field | Words | Owned producer |
| --- | ---: | --- |
| position, rotation | 3, 9 | Grip SITE world position and world-from-site matrix, not hand BODY attitude |
| q, qd | 7, 7 | The same admitted arm coordinates and rates used by every query |
| mass | 49 | The arm block of the nine-coordinate mass matrix, including moving finger bodies and source armatures |
| jp, jr | 21, 21 | Fresh grip-site linear and world-angular Jacobian columns for the seven hinges |
| velocity | 6 | World linear then angular site velocity from those columns and the same arm rates |
| bias | 7 | Arm generalized force at the same nine coordinates/rates and zero accelerations, including gravity |
| lower, upper | 7, 7 | The existing fixed-source torque limits, retained by the controller's own admission |

The typed boundary accepts seven radian coordinates, two metre coordinates,
seven radian-per-second rates and two metre-per-second rates. Quantity and
frame values become raw coordinates only at the existing `PcSample` ABI.
All input shapes and numerical bounds precede publication. Refusal leaves the
caller's output unchanged. Trusted low-level profile/pose helpers are not
provenance certificates for caller-written PODs.

The producer must use one state throughout. Finger rates can affect the arm bias
even though the grip site's seven-column Jacobian does not depend on slide
motion. `bias` is not zero-rate gravity when any rate is nonzero. The new
adapter must not change the existing observation-before-forward order or
reinterpret cached observation geometry as a fresh query.

## Qualify in small stages

1. Before acquisition, freeze a separately versioned complete-sample record
   alongside the inverse-dynamics reference. Save its additional Jacobian,
   site velocity and actuator limits from the same new SDK forwards. Record
   each SDK call and exact q/v pairing. A repeated old M/gravity row retains
   its previous proof identity; newly measured sample fields give that capture
   a distinct purpose. No stepping, rendering or inference is required.
2. Add a checks-first Coax producer over the owned queries. Compare every
   `PcSample` field, including units, packing and all nonfinite/refusal paths,
   against the frozen complete samples. Declare tolerances before comparison.
   Keep exact-byte ABI/canary checks separate from numerical closeness.
3. Feed paired SDK and owned samples into fresh Coax controller states with
   identical home, action and goal-update inputs. Freeze that corpus and its
   output/state budgets before running it. Preserve singular/refused cases;
   near-identical sample values are not themselves controller parity.
4. Add an opt-in owned-query path to the existing SDK driver. The host reads
   coordinates/rates and performs effects; Coax selects and assembles the
   sample. A bounded recorded-command prefix qualifies the changed adapter
   and its ordering. It is not a new learned-policy success score.

Only then use the mass solve and velocity-dependent bias to qualify owned
forward dynamics, followed by the declared integrator/passive-force and
contact contracts. Observation rendering, full snapshots, typed clocks and
compiler-enforced bounded control keep their separate gates in the
[remaining sequence](owned-control-loop.md).
