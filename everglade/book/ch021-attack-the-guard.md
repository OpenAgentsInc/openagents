# 21. Attack the guard

> **Status: partly implemented.** The `#authority` room combat engine
> (`EgBattle`, `egb_patrol_turn`, threat routing, and SRD turn execution) is
> implemented in [`everglade/server/battle.coax`](../server/battle.coax),
> [`everglade/server/guard.coax`](../server/guard.coax), and
> [`everglade/server/aid.coax`](../server/aid.coax), with Experimental zone
> combat in [`everglade/zone-combat.coax`](../zone-combat.coax) and
> [`everglade/zone-combat-actions.coax`](../zone-combat-actions.coax). A
> standalone runnable book example remains to be extracted.

This chapter answers the player's question: "Why did the guard draw his sword
and hit me twice?"

The path guard is neutral while you leave him alone. The instant a player deals
damage or lands a hostile spell, the guard enters the room's SRD battle
(`everglade/server/battle.coax`), adds the attacker to his threat list, and
fights back using the exact same SRD 5.2.1 Fighter rules a player obeys.

## What this chapter teaches

- How the room maps world coordinates onto the 5-foot SRD battle grid
  (`EGB_SQ = 1.524 m`, `EGB_X0 = -110.0 m`, `EGB_Z_SHIFT = 352`).
- How the level-10 Fighter guard decides between closing in (`Dash`), swinging
  two Longsword attacks (`+8`, `1d8 + 4`), using `Second Wind` / `Action Surge`,
  or firing a Longbow past `EGB_SHOOT_FROM_FT` (`35 ft`).
- How each attack in a Multiattack turn is paced on the wire (`E2G_BLOW_S`) so
  clients see and hear distinct swings rather than one instant health drop.

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| `#authority` room battle | Maps seats (`1000 + s`) and creatures (`2000 + id`) onto `RsBattle`, executes `egb_patrol_turn` | [`everglade/server/battle.coax`](../server/battle.coax) |
| Guard combat cues | Reads fresh battle log entries (`e2_guard_blows`) and emits `e guard` hits/misses (`AMOUNT 0` = miss) | [`everglade/server/guard.coax`](../server/guard.coax) |
| Guard stat block | Level-10 SRD Fighter (`rs_path_guard`: AC 19, HP 84, Longsword `+8` / `1d8+4`, Longbow) | `rules/srd/npcs.coax` |
| Friendly aid & healing | Server-checked healing and help actions during fights | [`everglade/server/aid.coax`](../server/aid.coax) |
| Zone combat harness | Experimental combat action loop and HUD feedback | [`everglade/zone-combat.coax`](../zone-combat.coax), [`everglade/zone-combat-actions.coax`](../zone-combat-actions.coax) |

## The method

### One `#authority` combat resolver

`everglade/server/battle.coax` carries `#authority` and is mutation-checked
separately from the room host (`coax/web/room/battle-check.coax`). Every seat
occupies `EGB_SEAT + seat` and every guard or monster occupies `EGB_BEAST0 + q`.
When the guard's turn opens (`egb_patrol_turn`), the resolver checks distance
to his threat target:

- Within melee reach (`EGB_REACH = 1.8 m`, ~5 ft): takes Multiattack (two
  Longsword attacks at `+8` for `1d8 + 4` slashing) and triggers `Second Wind`
  when wounded.
- Between 5 ft and `EGB_SHOOT_FROM_FT` (35 ft): Dashes to close distance.
- Beyond 35 ft: draws his Longbow instead of chasing indefinitely.

`e2_guard_after` scans `b[0].book[0].seq` for new attacks since the previous
tick and schedules each blow `E2G_BLOW_S` apart on the wire (`e guard`),
sending `AMOUNT = 0` for misses so clients play a parry/whiff cue instead of
silence.

## Worked calculation: guard melee damage vs a level-1 Ranger

A level-1 Ranger wears AC 14 (`11 + DEX +3`) with 12 HP. The level-10 guard
swings at `+8` for `1d8 + 4` slashing (average `8.5` per hit):

```text
Hit roll needed on d20 = 14 - 8 = 6 or higher
P(hit per swing)       = 15 / 20 = 75%
Expected damage / swing = 0.75 × 8.5 + 0.05 × 4.5 (crit die) = 6.6 HP
Expected damage / turn (2 swings) = 13.2 HP > 12 HP (down in one turn!)
```

Attacking a level-10 guard solo at level 1 is deliberately lethal; groups or
higher-level heroes survive by kiting, healing, and controlling conditions.

## The checks

```shell
coax/bin/coax test --library coax/web/room/battle-check.coax
coax/bin/coax test --library everglade/server/room-checks.coax --only "guard"
```

## What comes next

When the guard drops you—or when a party finally drops the guard—HP hits zero.
[Chapter 22](ch022-death-and-respawn.md) covers death saves, recovery, rests,
and returning to the glade.
