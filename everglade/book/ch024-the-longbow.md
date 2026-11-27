# 24. The longbow

> **Status: partly implemented.** Longbow geometry, audio cues, shared
> client/server arrow flight, launch-hand validation, and the Archery Range
> scoring rules live in [`everglade/longbow.coax`](../longbow.coax),
> [`everglade/shared/archery.coax`](../shared/archery.coax),
> [`everglade/client/arrows.coax`](../client/arrows.coax),
> [`everglade/server/arrows.coax`](../server/arrows.coax), and
> [`everglade/range/`](../range/score.coax). A standalone runnable book example
> remains to be extracted.

This chapter answers the player's question: "I aimed above the goblin's head
across the clearing—did my arrow drop into him?"

In Everglade, arrows are real ballistic projectiles rather than instant hitscan
lines. The client predicts the arrow's flight on the exact frame you release
right-click, while the room server flies the same arc over the same ground and
creature capsules and resolves the SRD 5.2.1 attack roll (`1d20 + DEX + PB`,
plus `+2` for Archery at level 2) when the arrow arrives.

## What this chapter teaches

- How the engine's bow physics (`metaverse-engine/action/archery.coax`,
  `bow.coax`) separate cleanly from Everglade's bow tuning (`everglade/longbow.coax`).
- How the room validates the archer's launch hand (`E2AR_HAND_SLACK = 3.0 m`,
  height `[0.4 m, 4.5 m]`) so a client cannot fire arrows from inside a wall or
  across the map.
- How client prediction (`everglade/client/arrows.coax`) and server authority
  (`everglade/server/arrows.coax`) share one capsule geometry (`eg_bow_body`)
  and one scoring module (`everglade/range/score.coax`).

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| Longbow geometry & cues | Bow hand offset (`eg_bow_hand`), look range (`EG_BOW_LOOK`), capsule (`eg_bow_body`), audio/dust cues | [`everglade/longbow.coax`](../longbow.coax) |
| Shared archery bounds | Launch slack (`E2AR_HAND_SLACK`), body count (`E2AR_BODIES = 13`), ground & target integration | [`everglade/shared/archery.coax`](../shared/archery.coax) |
| Client arrow prediction | Draws the nocked arrow, predicts flight on loose, and reconciles impacts | [`everglade/client/arrows.coax`](../client/arrows.coax) |
| Server arrow authority | Validates launch origin, steps arrows, rolls SRD attack/damage, lends range bows | [`everglade/server/arrows.coax`](../server/arrows.coax) |
| Archery Range scoring | Seven target faces, 10-ring scoring (`rg_points`), tallies on foot / horseback / dragonback | [`everglade/range/layout.coax`](../range/layout.coax), [`everglade/range/score.coax`](../range/score.coax) |

## The method

### Shared flight, zero client authority

When you release the bowstring, the client immediately draws the departing
arrow from `eg_bow_hand` (chest high, ahead of and beside the body) and sends
the launch vector to the room. The server checks two preconditions before
admitting the shot:

1. The claimed launch origin lies within `E2AR_HAND_SLACK` (`3.0 m`) of where
   the room stepped the archer—enough slack for unstepped movement ticks, not
   enough to shoot around walls—and between `E2AR_HAND_LOW` (`0.4 m`) and
   `E2AR_HAND_HIGH` (`4.5 m`) above the ground (covering a jump's peak).
2. The archer actually holds a ranged weapon (or is standing on the Archery
   Range where `e2a_range_lend` lends a practice Longbow).

Along the arc, `everglade/shared/archery.coax` tests terrain height, static
range faces (`rg_targets`), the 12 monster capsules (`E2AR_MONSTERS`), and the
path guard (`E2AR_GUARD`). On a creature strike, the server rolls the SRD
attack roll from [chapter 14](ch014-ability-scores-proficiency-and-the-d20.md);
on an Archery Range target face, `rg_points` awards `10` (gold bullseye) down
to `1` (outer white), counting line-cutters for the inner ring and tracking
streaks separately on foot (`RG_FOOT`), horseback (`RG_HORSE`), and dragonback
(`RG_DRAGON`).

## Worked calculation: SRD Longbow range in metres

```text
SRD normal range = 150 ft × 0.3048 m/ft = 45.72 m
SRD long range   = 600 ft × 0.3048 m/ft = 182.88 m (Disadvantage past 45.72 m)
Archery Range distance bands: <= 15 m, <= 25 m, <= 40 m, <= 60 m, > 60 m
```

## The checks

```shell
coax/bin/coax test --library everglade/shared/archery.coax
coax/bin/coax test --library everglade/range/score.coax
coax/bin/coax test coax/web/room/room.coax --only "arrow" --steps 4000000000
```

## What comes next

Archery and spellcasting both happen while you run. [Chapter 25](ch025-cast-while-you-run.md)
layers upper-body cast and draw animations over moving legs while keeping the
room's cast-pace rules exact.
