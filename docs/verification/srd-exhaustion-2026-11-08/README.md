# Executable Exhaustion levels

Partial #13039 progress on baseline `e1cc7a3650`, compiler 0.52.0. Pinned SRD
5.2.1 p.181 Exhaustion, p.8 D20 Tests and pp.17-18 death saves. PDF SHA-256:
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

## Behavior

`RsCreature.exhaustion` stores cumulative levels 0..6. Authoritative callers
apply one level through `rs_gain_exhaustion`, which preserves condition immunity
and rejects malformed logical actors/dead recipients. Scalar state is independent
of expiring effect slots and base Speed. Each level subtracts two from attack,
saving throw, initiative and existing Athletics-escape totals, and five feet from
the current walking-Speed calculation before its existing halving modifiers.
Stand/Dash/turn movement consume that calculation; a zero-Speed grid step refuses
even if old movement credit remains. Seeking Spell sees the penalized attack
threshold. Attack logs show the Exhaustion penalty.

Level six kills directly without damage, temporary-HP absorption, damage immunity
or a story HP floor. It clears HP/stabilization and uses the shared interruption
hook for concentration, Rage, Turn Undead and Dodge. Events report cumulative
levels and death, with no invented attacker. Other concentration owners remain.
Natural attack criticals and death-save twenties retain their source meaning.
A natural death-save twenty never spends an unnecessary rescue die even when
Exhaustion and Bane make its modified total less than ten. The standalone death
save primitive applies the same Exhaustion penalty.

`rs_remove_exhaustion` removes exactly one level, clears only its condition bit
at zero, and cannot revive a dead creature. It is a consequence API; it does not
itself authorize or complete a Long Rest. No clock expiration removes levels.
The scalar is internal engine state, not a new versioned protocol field.

## Verification

Checks were authored first; the original red records absent state/APIs. Source
fixtures cover six-level death, immune/no-op and invalid-recipient boundaries,
exact flags and event identity/text, base-Speed preservation, single-level
recovery, every level's attack/save penalty, initiative/Athletics, zero-Speed
movement, natural criticals/death saves, Seeking Spell and Dodge retirement.
A shared native-callable scenario witnesses levels, recovery, saves, Speed and
concentration teardown; it is not a second independent oracle.

Mutation commands use `coax/bin/coax mutate aethyr/rules/srd/exhaustion-checks.coax --library --unit UNIT --jobs 4`:

- `exhaustion.coax`: 26 killed, zero survivors.
- `exhaustion-lifecycle.coax`: 49 killed, zero survivors.
- `condition-rolls.coax`: 66 killed, zero survivors.
- `death-save-roll.coax`: 27 killed, zero survivors.
- `death-saves.coax`: 59 killed, zero survivors.

Initial lifecycle survivors prompted unrelated-immunity/exact-bit and stabilized
death fixtures; none were exempted. Unchanged passing policy procedures retain
their receipt as later checks are added. The hotbar entry passes 202 checks,
including Exhaustion-adjusted saved-seat Dash. Before rebase all 21 selected
Experimental checks pass. Every command is local and below fifteen minutes.
Final combined source/native/world results are recorded below.

## Remaining active acceptance

#13039 stays open. This does not complete the Long Rest admission/recovery system,
full recalculation of already credited mid-turn movement, or every condition and
world producer. Timed `RsEffect` insertion/raw condition bits are not a substitute
for a cumulative level gain: new Exhaustion producers must call this level API.
Current imported content has no executable Exhaustion producer requiring migration.
Source-authorized recovery callers, exploration/monster producers, complete
visibility/condition relationships and world state-writer migration remain work.
No all-SRD or whole-world qualification is claimed.

## Final combined tree

Rebased onto `4fab48af86`, retaining the independent Everglade class-bar/room
casting and crowd-check work. No core rule procedure conflicts occurred.
The final entry passes 225 core checks. The incoming world/class-kit composition
passes all 21 selected Experimental checks. The hotbar files are unaffected by
that rebase; their 202-check receipt is reused. The final native command
`coax/web/tool.sh aethyr/rules/srd/exhaustion-native.coax` reports
`PASS CONDITION-EXHAUSTION`, exit 0. Tools-check passes with tracked additions.
The final native build includes the additive natural-face/Seeking/standalone
fixtures and consolidated module load placement added after the initial build.
Source/compiler hashes bind the final files. The earlier five policy mutation
receipts cover unchanged executable procedures and are reused, not rerun.
