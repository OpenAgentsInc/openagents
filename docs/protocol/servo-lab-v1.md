# Illustrative servo workbench v1

`psionic.robotics.servo-lab.v1` is a bounded single-joint companion to RXE1,
whose published bytes remain unchanged. It composes the qualified
`scalar-servo-play-f64-v1` actuator with visual RBA1 articulation in the app.
The engine does not load Psionic. This is an authored teaching experiment,
without full-body, contact, balance, motor-identification or hardware authority.

The fixed base and all other joints retain their source stand. The chosen
joint uses its RBA1 positive axis and absolute stand; all simulated angles,
targets and limits below are **offsets from stand**, in radians. Motor and
observed output remain distinct. Requested/delivered/passive/net generalized
effort uses the existing one-radian torque basis, displayed as mNm.

The profile fixes a 2,000,000 ns semi-implicit step, zero initial angle/rate/
output and zero initial delay queue. A target is a held step input. Delay is
0–16 simulation steps, independent of rendering. Presets reset the experiment:
Baseline (zero delay, 150 mNm), Delayed (16 steps), Limited (30 mNm).
Authored defaults: inertia .01, kp 2, kd .08, viscous .02, Coulomb .001,
voltage 1, play .002 rad, target .1 rad. These are not measured coefficients.

Configuration admission first validates every binary64 word through the
shared actuator, then fixed step, source identities, joint index (0–31),
relative limits within ±100 rad and containing zero, stand within ±100 rad,
and target within those limits. Simulation refuses an output outside source
limits before publishing the snapshot, delay queue, graph ring or tick.
Configuration edits require the current revision, do not wrap, and replace
the complete candidate atomically. Refusal leaves accepted state unchanged.
Pause/background clears the frame clock's fractional carry. Resume discards
its first frame delta. Stalls over 64 ms advance no simulation; normal frames
use the engine's one FrameClock with at most 32 steps. No catch-up burst.
The 256-row chronological telemetry ring samples every 10 steps (20 ms), plus
the initial row. Eviction drops only oldest rows; stepping is unaffected.

The sole codec is `psionic/lib/robotics/servo-lab-wire.coax`. Its immutable
SLV1 packet is exactly 312 bytes, little endian, with no padding/trailing bytes:

| Offset | Meaning |
| --- | --- |
| 0 | ASCII `SLV1` |
| 4 | u32 length 312 |
| 8, 12 | u32 joint index, delay |
| 16, 48, 80 | 32-byte model, assembly-description, topology source SHA-256 |
| 112 | 13 binary64: inertia, kp, kd, viscous, Coulomb, effort, voltage, step, play, target, low, high, absolute stand |
| 216 | u32 result status (ok=0, shape=1, revision=2, config=3, step=4, limit=5, exhausted=6) |
| 220, 224, 228 | u32 accepted revision, tick, retained row count |
| 232 | 10 binary64: commanded target, applied target, motor position, motor rate, observed output, requested, delivered, passive, net, available effort |

The tagged SHA-256 of the entire packet, tag `psionic.robotics.servo-lab.v1`,
binds its exact finite words, including signed zero. It is an artifact identity,
not a signature or authorization. Readers require the expected digest and
exactly one output; writers require the exact output span. Both stage before
publication. Status packets describe the accepted experiment and latest real
output; a refused candidate never replaces that configuration. Row counts
are 1–256, ticks 0–2,147,483,647 and revision 1–2,147,483,647. Full snapshots,
command journals, persistence and replay require the next companion profile.

Native and browser producers expose optional `coaxial_servo_lab_len_v1` /
`coaxial_servo_lab_v1` exports for the same packet. SDK projections copy exactly
312 bytes. Reading it performs no command, restore or physical-device effect.
