# Replace the fresh controller sample with owned queries

Status: owned construction, held-state paired controllers, and the
[explicit query selector with two recorded-action SDK prefixes](../../fixtures/robotics/panda-query-source/README.md)
are qualified. The full neural runner retains its original SDK query path and
needs separate provider wiring. The new prefixes recompute every control and
step the SDK 500 times in total; they run no NN or renderer and qualify no owned
integrator, contact backend, task-success score or hardware command.

The [FK](../../fixtures/robotics/panda-fk/README.md),
[finger branches](../../fixtures/robotics/panda-fingers/README.md),
[grip Jacobian](../../fixtures/robotics/panda-jacobian/README.md) and
[mass/zero-rate gravity](../../fixtures/robotics/panda-inertia/README.md) are
qualified separately. The [velocity-dependent inverse query](../../fixtures/robotics/panda-inverse-dynamics/README.md)
now supplies the remaining force prerequisite. A mass solve supplies a separate inertial response to an already
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

## Delivered evidence and remaining adapter work

[The constructor](../../fixtures/robotics/panda-owned-sample/README.md) qualifies
all 2,160 fields from fifteen matched capture contexts under frozen per-field
budgets, with 9,811 native assertions per mode. The initial 100/97/3 emitted-site
result is retained; only the changed constructor is requalified at 29/29, while
five exact bodies carry earlier evidence. Quantity-token provenance remains a
separate compiler gap, with typed literal and SDK-field math checks independent.

[The paired-controller proof](../../fixtures/robotics/panda-owned-controller/README.md)
qualifies 45 successful reset pairs and 1,125 successful held-call pairs, plus
150 atomic intentional refused calls. It uses fixed observations and source
home, not elapsed physics time. Exact status/readiness/clipping and bounded
logical state/output comparisons are separate from field closeness. The
portable gate checks historical C/header/sample byte aliases before reuse;
its historical receipt is not a newly generated execution record.

## Qualify in small stages

1. **Delivered:** before acquisition, freeze a separately versioned complete-sample record
   alongside the inverse-dynamics reference. Save its additional Jacobian,
   site velocity and actuator limits from the same new SDK forwards. Record
   each SDK call and exact q/v pairing. A repeated old M/gravity row retains
   its previous proof identity; newly measured sample fields give that capture
   a distinct purpose. No stepping, rendering or inference is required.
2. **Delivered:** add a checks-first Coax producer over the owned queries. Compare every
   `PcSample` field, including units, packing and all nonfinite/refusal paths,
   against the frozen complete samples. Declare tolerances before comparison.
   Keep exact-byte ABI/canary checks separate from numerical closeness.
3. **Delivered:** feed paired SDK and owned samples into fresh Coax controller states with
   identical home, action and goal-update inputs. Freeze that corpus and its
   output/state budgets before running it. Preserve singular/refused cases;
   near-identical sample values are not themselves controller parity.
4. **Delivered first shadow profile:** the additive `PandaRobotStateV1` effect
   reads nine named qpos and nine separately named DOF velocities. Coax selects
   the sample and refuses without fallback. Two 250-step recorded-action SDK
   prefixes qualify fresh control queries after forward and cached requests
   before forward, with all 1,000 selected/shadow controller calls successful.
   Telemetry stays unchanged; SDK geometry, integration and contact remain.
5. **Pending:** wire the explicit selector into the full neural runner, retaining
   its declared observation, clock and command contracts. This needs its own
   changed-context learned-policy qualification, not a repeated old episode.

The [unconstrained applied-force query](../../fixtures/robotics/panda-forward-dynamics/README.md)
separately composes the mass solve and velocity-dependent bias. It does not
convert controller finger-position targets to force. The declared integrator,
passive-force, actuator-mapping and contact contracts remain future work. Observation rendering, full snapshots, typed clocks and
compiler-enforced bounded control keep their separate gates in the
[remaining sequence](owned-control-loop.md).
