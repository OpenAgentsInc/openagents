# External combat v1

Identity: `external-combat-v1`, `openagents.engine.combat.v1`. A local
observe/act/verify journal over desk-control-v2, alongside the unchanged
external-navigation-v1. Quest automation is outside this profile.

The Classic Era SDK adapter draws a second visible RGB strip underneath
the navigation strip. It samples public APIs only and never invokes game
actions. All targeting, movement and action-bar presses use ordinary keys
through the target window's bounded lease. Each observation binds the desk
session, window lifetime, fresh navigation/combat serial and image.

The sensor is 168 bytes in 56 cells, each eight nominal pixels wide,
at nominal Y=12. Bytes are big-endian. 0..2 are 86,67,1; 3..4 serial;
5 player flags (valid=1, alive=2, combat=4, UI blocked=8, casting=16,
moving=32); 6 level; 7 class; 8/10 health/current maximum; 12/14 mana/current
maximum; 16/20 XP/current maximum (four bytes each). 24 target flags
(exists=1, attackable=2, dead=4, player-controlled=8, tap denied=16,
trivial=32, normal classification=64, combat=128); 25 target level;
26/28 target health/current maximum; 30 target epoch; 32/36 NPC ID and
spawn GUID suffix (four bytes); 40 cast remaining milliseconds; 42/44
UI error sequence/type; 46..47 reserved zero. Twelve seven-byte action
slots start at 48: flags (exists=1, usable=2, no power=4, range known=8,
in range=16, current=32, attack=64), 24-bit ID, 16-bit remaining cooldown
milliseconds, count. 132/134/136 are remaining Demon Skin/Corruption/
Immolate milliseconds; 138/140 selected nameplate X/Y normalized to u16;
142 flags (visible plate=1, target attacking player=2). This client restricts
nameplate measurement, so its SDK adapter leaves X/Y and bit 1 unavailable.
143 is zero;
144 is a 16-bit player PARTY_KILL event counter, 146/150 are its NPC ID and
spawn suffix, 154 its client milliseconds (four bytes each). 158..160 are
zero through byte 163. Bytes 164..167 are CRC-32/ISO-HDLC of bytes 0..163,
big-endian, reusing the PNG transport CRC in Coax. The wider strip needs a
stronger check than a byte sum to reject wrong UI-scale pitches. Integers wider than their field
saturate at its maximum. Kill events are filtered to the player's GUID in
the SDK adapter; pets/group-member kills are not covered by this first profile.
Coordinates and action states are observations, not a permission grant.

Creature eligibility must be checked in Coax: ordinary non-player,
attackable, living, non-gray, untapped and within the profile's level bound.
Before a pull, require adequate health and mana. During a fight, retain
target identity; observe casts, target damage and player resources, with
bounded retry and fight budgets. A target death alone is insufficient:
record a new matching player kill event and XP increase to count an
XP-giving kill, even if target selection disappears between samples. Level transitions use the
previous XP maximum. Stop on stale/corrupt samples, UI focus, changed window,
death, unexpected target changes, danger or exhausted budgets. An external
stop command releases input; an output-directory stop file ends the loop.

Qualification, exact policy limits and real trial records belong in the
combat work journal. Model confidence never establishes combat quality.
