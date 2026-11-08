# Prone movement and attack edges

Source: SRD 5.2.1 printed p.179 Crawling and p.186 Prone; pinned PDF SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
The glossary explicitly rounds standing's half-Speed cost down and prohibits
standing at Speed zero. Crawling costs one extra foot per foot moved, two extra
in Difficult Terrain. Prone's attack edges depend on distance, not attack mode.

Previously turn start cleared Prone and halved the remaining cell budget.
Crawling paid an ordinary step, the attacker's Prone imposed no penalty, and
nearby ranged attacks against Prone gained the wrong edge. The new regression
first failed on turn-start clearing (`red.log`); implementation followed.

## Changed behavior

The strict shared turn start leaves Prone intact. `rs_stand_up` is an explicit
movement choice: half current tracked walking Speed, rounded down in feet,
never an Action or Bonus Action. Slow/halving/Spirit Guardians use the existing
tracked speed calculation shared with turn start. Movement blockers, anchored
turns, Speed zero, insufficient budget and another actor's turn refuse without
payment. Incapacitated alone does not prohibit movement. No new automatic
standing policy is installed in the world or NPC adapters.

`move_left` remains whole five-foot cells; `move_feet` retains the 0..4 residual
feet from payment. At Speed 25, crawling five feet spends ten, standing spends
twelve, and three feet remain. Whole-cell budget additions preserve the
residual. Crawling stacks additively with Difficult Terrain, including overlapping
terrain sources, and refusal changes neither position nor budget.

Prone gives its owner's attacks Disadvantage. Against a Prone target, distance
at most five feet grants Advantage and farther attacks have Disadvantage.
Ranged attacks' nearby capable-foe penalty remains an independent edge source:
it cancels the nearby Prone benefit, but an Incapacitated foe cannot impose it.

`fb_seat_stand` enters and restores the player's own saved turn. Its fixture
checks that the existing short beat cannot pay half full Speed, then explicitly
provides enough movement and verifies payment with no Action/Bonus Action.
This demonstrates the need for the still-open timing profile work rather than
pretending short beats already match a complete strict movement economy.

## Evidence

The cached 0.52.0 compiler and changed sources are SHA-256 recorded. All commands
are local, bounded below fifteen minutes. No passing unchanged gate was rerun
for confirmation. The terrain fixture initially named a nonexistent field and
then lacked zone bounds; those fixture errors were corrected before success.

- 155 checks pass through `prone-checks.coax`, including source-derived edge,
  odd-Speed/residual, terrain, owner, blocker and refusal fixtures.
- `prone-movement.coax`: 62 mutants killed, zero survivors, no exemptions.
- 170 shared hotbar and 184 casting-entry checks pass.
- Experimental: 17 selected checks pass; the room's Cult Priest fixture passes.
  Selected runs do not claim whole-entry authority coverage.
- Strict native witnesses: CONDITION-PRONE passes; 15 PASS, one parked Polymorph
  FAIL overall. Exit 1 remains unsuppressed. No full-SRD qualification is claimed.

Commands:

```sh
coax/bin/coax test aethyr/rules/srd/prone-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/prone-checks.coax --library --unit aethyr/rules/srd/prone-movement.coax --jobs 4
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test aethyr/rules/srd/cast-target-checks.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'
coax/bin/coax test apps/openagents-web/everglade5e-room-rules.coax --library --only 'Cult Priests'
coax/web/tool.sh aethyr/rules/srd/compliance-check.coax
coax/web/tool.sh tools-check
```

## Remaining acceptance

This does not close #13039 or #13040. Other conditions, movement modes, flying
falls, forced prone/held movement interactions, opportunity windows, exact
general Speed/Dash accounting and the rest of those issues remain unqualified.
Player input/buttons and NPC choice need to consume explicit standing through
#13046; no visible control is claimed. The current short-beat movement grant,
world movement outside the grid and unlimited-resource overrides still need
#13045/#13046. Turning Prone off automatically in core would hide these gaps.
Future turn snapshots/serializers must preserve residual feet; no published
wire contract changed in this slice. No owner action is required.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
