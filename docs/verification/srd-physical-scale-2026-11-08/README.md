# Shared walking rate reaches Experimental physics

Pinned SRD 5.2.1 SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
Current walking Speed and additive Crawling/Difficult Terrain costs are the
shared rules (`rs_walk_speed`, `rs_step_feet`); this change connects those
values to the world's declared physical locomotion tuning.

## Result

`rs_locomotion_scale` normalizes current walking Speed against the action
world's Speed-30 baseline and applies the shared per-foot cost. Logical actor
bounds and movement permission are checked; Stunned/Blinded alone do not become
movement blockers. Rate can exceed one. Slow, tracked halving, exhaustion,
Prone and current projected difficult zones flow through the same core values.

The player passes this rate separately from environmental `pace`, which the
generic walker deliberately clamps to one. New opt-in
`eg_step_advance_scaled` scales the ordinary walking/running or current dash
locomotion profile without changing gravity/jump tuning or the collision shape.
Scale one delegates to the existing controller export. Invalid/unsupported
nontrivial boat/Ocarina profiles refuse instead of silently ignoring the rate.
The ordinary exported controller and network replay path remain unchanged.

A bounded subdivision policy selects 1/2/4/8/16 integrations per paid fixed
step. The opt-in walker preserves the original FrameClock, fixed-step history,
camera cadence and single queued jump, while every collision integration stays
within the original maximum travel. It never raises the collision radius/travel
limit or creates extra logical ticks. Any failed integration restores the fixed
step's physical/control input and interpolation history with refusal diagnostics.
The old direct `step` travel convention is preserved, including its existing
nonstandard-dt landing fixture. Internal `step_body` follows the admitted
wrappers; it is not a new C admission surface.

Monster AI uses the same proportional rate for pursuit and return-home walking.
Its bounded subdivisions use the shared travel policy and each candidate passes
the existing before-departure Reaction boundary before the next AI integration.
The visible AI clock receives exactly the original frame time. The policy lives
in an independent module, avoiding a duplicate `input_runs` native export when
the Lab and controller units both need it. No compiler or collision-profile
change was needed.

## Evidence

Missing-API regressions preceded implementation. A further regression caught
refused-step interpolation history; the corrected driver preserves legacy
history. Ordinary movement matches the original driver, and fast movement
stops at a thin wall without increasing the travel bound. Shared rate entry:
200 checks pass. World fixtures verify Monk level-six Speed 45 produces 1.5,
halving/crawling/blockers, and actual AI travel at half/base/double Speed.
The combined three-world fixture exceeded its 20-million-step budget; it was
split into independently bounded checks, not given a larger budget.

Core gates: subdivision policy 25 killed, driver/admission 25 killed, SRD rate
15 killed; zero survivors/no equivalent exemptions. The unchanged driver gate
is reused after extracting its numerically identical policy into an independent
module; the changed policy was re-gated. Complete checked Lab Wasm build passes
in 41.3 seconds, 2,536,771 bytes, including the actual C face's new scalar/driver
calls. Temporary output naming has no registered size budget; no shipped target
was added. Existing compiler-source and near-limit advisories remain. Final
native and walker-suite receipts are retained below.

```
coax/bin/coax test aethyr/tests/walk-scaled-checks.coax --library
coax/bin/coax mutate aethyr/tests/walk-scaled-checks.coax --library --unit aethyr/walk-substep-policy.coax --jobs 4
coax/bin/coax mutate aethyr/tests/walk-scaled-checks.coax --library --unit aethyr/walk-scaled.coax --jobs 4
coax/bin/coax test aethyr/rules/srd/locomotion-scale-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/locomotion-scale-checks.coax --library --unit aethyr/rules/srd/locomotion-scale.coax --jobs 4
coax/bin/coax test apps/openagents-web/everglade-step.coax --library --only 'combat walking rate'
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental walking scale'
coax/web/tool.sh apps/openagents-web/experimental-motion-native.coax
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-physical-scale-fivelab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
coax/web/tool.sh tools-check
```

## Limits

The existing free continuous action locomotion remains an explicit house
profile: player base walk/run 3.5/7.5 m/s, dash 9.2 m/s and AI pursuit/home
2.4/1.5 m/s represent Speed 30. This is proportional source-effect integration,
not strict feet-per-turn payment. Acceleration remains the physics profile's;
strict current-turn movement budget reconciliation remains active work.
Environmental bush pace still composes separately. Nontrivial boat/Ocarina
scaling is unsupported. Jumping, special speeds, all effect grants (including
Haste itself), source equipment/armor eligibility, moving world-zone coordinate
frames, exact physical strike distances and full timing qualification remain
open. No new movement refill, resource waiver, teleport cause or E2 combat
profile is introduced. No issue closes from this slice.

Final full walker entry passes all 316 checks after module extraction. Native
execution checks half/base/double controller motion with exactly 64 logical
ticks, positive Monk rate, actual half/double AI displacement, and fast motion
stopping at a thin wall with an accepted collision result. Tools-check/diff
check pass. This is native arithmetic/controller evidence, not a browser or
complete rendered world qualification.

The final controller refusal/zero-rate extension passes both focused checks.
The final artifact build after that test-only extension passes in 3.5 seconds
with cached unchanged units, retaining exactly 2,536,771 bytes. Native final
output is PASS PHYSICAL-MOTION, including the accepted thin-wall collision
status. Hashes describe the final sources and artifact.
