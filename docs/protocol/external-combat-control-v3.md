# External combat control v3

Identity: `starter-warlock-v3`, `openagents.engine.combat.control.v3`.
Observation v1, evidence v1, target admission and the v2 action envelope
remain unchanged. This revises visual acquisition and survey behavior.

The recorded v2 baseline spent 31 inputs on Tab and 13 on clearing an
unbound target during three verified kills. A Tab-selected target could
never authorize a pull without a fresh visible selection. Those inputs
added selection churn without establishing a visual binding.

V3 turns right for 160 ms on each empty-view survey step, then follows the
same bounded waypoint circuit. It does not Tab during that circuit. Every
observation still scans all visible candidate bars; seeing one interrupts
the circuit. A candidate within one eighth of image width from centre can
be clicked, instead of requiring the old fixed 120-pixel alignment. This
is a selection tolerance, not permission to cast or a claimed view angle.
The following public target sample and fresh pre-input admission still
govern every spell. Off-centre candidates retain bounded turn pulses.
A visually bound approach before a fight uses 300 ms instead of 200 ms,
then invalidates that binding and requires another observed selection.
Movement during an admitted encounter retains the existing policy pulse.

All v2 health/mana/target/rank/lifetime, movement, encounter, stop/drain and
survey-fence bounds remain in force. Real before/after results include
level transitions and different creatures; raw XP/minute alone is not a
controlled measurement of the targeting change.
