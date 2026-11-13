# 16. Spells and slots

> **Status: partly implemented.** The SRD's 339 spell descriptions are imported
> into [`rules/srd/data/`](../../rules/srd/data/README.md). The
> spells that Everglade's levels 1 to 6 use, cantrips through 3rd level, run
> as effect programs in the shared rules, and
> [`rules/srd/spell-coverage.coax`](../../rules/srd/spell-coverage.coax)
> counts which ones. Casting under latency shares its reach, sight and refusal
> words between the room and the page in
> [`everglade/shared/cast-limits.coax`](../../everglade/shared/cast-limits.coax);
> v1's bars, wait and Repair stay in
> [`apps/openagents-web/everglade-cast-rules.coax`](../../apps/openagents-web/everglade-cast-rules.coax)
> (#12790, #13346).
> Other agents are extending the spell files now, so this chapter's only listing
> is the cast rules, which already have markers.

Everglade aims to follow the SRD 5.2.1's spells faithfully while adapting them
for action combat. This chapter answers the player's question: "Why didn't my
spell go off?"

A spell can fail for an SRD reason: no slot, no free hand for a Somatic
component, a gag that stops a Verbal one, or a target out of range. It can also
fail for a real-time reason: the target walked out of range while you cast. The
game tells you which, with a reason icon over the bar.

## What this chapter teaches

- Where the spell text comes from, and how a spell becomes executable.
- How slots, components, concentration, areas and saving throws work.
- What changes for action combat: range slack, cast bars and areas.

## What exists today

| Rule | Where |
| --- | --- |
| The spell text, pinned to the SRD PDF | `rules/srd/data/spells.coax`, `rules/srd/data/class-lists.coax` |
| Which spells have an effect program | `rules/srd/spell-coverage.coax` |
| Spells prepared per class and level | `RS_PREP_*` in [`rules/srd/spell-limits.coax`](../../rules/srd/spell-limits.coax) |
| Verbal, Somatic and Material components | [`rules/srd/cast-components.coax`](../../rules/srd/cast-components.coax) |
| Concentration | [`rules/srd/concentration.coax`](../../rules/srd/concentration.coax); its damage DC in `rules/srd/hit-points.coax` |
| Rituals | [`rules/srd/ritual.coax`](../../rules/srd/ritual.coax) |
| Whether an area includes its caster | [`rules/srd/area-origin.coax`](../../rules/srd/area-origin.coax) |
| A spell's attack bonus and save DC | `rs_spell_attack` and `rs_spell_dc` in `rules/srd/engine.coax` |

The data folder holds the full text of every spell, with its page, school,
range, components and duration. An imported spell is text until an effect
program maps to it. The coverage check counts, for each class's table, which
cantrips and 1st- to 3rd-level spells have one.

## What follows the SRD exactly

- **Slots.** A leveled spell spends a slot of its level or higher. A cantrip
  spends none. A Ritual spends none and takes 10 more minutes.
- **Components.** A Verbal component needs speech. A Somatic one needs a free
  hand, or a hand holding a focus where the spell allows it. A priced Material
  component can't be replaced by a focus.
- **Concentration.** Damage forces a Constitution save against the larger of
  10 and half the damage, up to 30 (`conc_dc`). Incapacitation or death ends
  concentration at once.
- **Areas and saves.** An area spell rolls its damage once, and each creature
  in the area makes its own save.
- **Durations.** A timed duration runs on the world clock. One minute is 60
  seconds, ten rounds.

## What changes for action combat, and why

- **Range slack.** Between your screen and the server, your position differs a
  little. During a cast, the target keeps walking. So the server allows 3 m
  past a spell's range when the cast starts and 6 m at its release. A spell
  reaches at least 4.5 m, so a touch spell works at arm's length.
- **Line of sight from your eyes.** The line starts 1.8 m over your feet and
  ends 1 m short of your aim. Low pieces under 1.2 m, such as crates and
  fences, don't block it.
- **Cast bars.** Most casts release as they start. Only Everglade's own four
  house spells have a cast bar.
- **The caster is spared by its own area** (`RS_DEV_SPARE_CASTER`). In the SRD,
  a Fireball centred near you can catch you. In Everglade's world profile, a
  caster is never in its own area. Fights move fast, and you aim areas at a
  crowd you are running beside.
- **A refusal says why.** In the Everglade room, a refused cast's bar shows its
  reason icon (`CR_SHOW`). In Everglade II, a matching refusal shows over the
  hotbar for four seconds, and the page lifts any cooldown it predicted.

The cast rules for range, sight and time are one block of constants:

<!-- source: everglade/shared/cast-limits.coax cast-reach -->
```jai
// ---- range and sight
CR_SLACK :: 3.0;          // m past a spell's range at its start: where the page and the server have you differ a little
CR_SLACK_END :: 6.0;      // m past it at its release: the target walks during the cast bar
CR_REACH_LEAST :: 4.5;    // m a spell reaches at least (a touch, a 5-foot reach, with the slack)
CR_EYE :: 1.8;            // m over the caster's feet the line of sight starts
CR_SHORT :: 1.0;          // m short of the aim it ends: the wall or roof you aim at is not in the way
CR_AIM_UP :: 0.5;         // m over the aim it ends
CR_LOW :: 1.2;            // m: a piece lower than this (a crate, a fence, a bench) doesn't block the line
CR_NEAR :: 2.5;           // m: an aim this close needs no line of sight (a press aims 2 m ahead of you; the rest is the wire's cm and your pose)
```

<!-- source: apps/openagents-web/everglade-cast-rules.coax cast-rules -->
```jai
// ---- time
CR_WAIT :: 15;            // ticks (0.5 s, the room's EG_BANK) a cast waits for the inputs sent before it
CR_SHOW :: 0.9;           // s a refused cast's bar shows its reason icon
// Everglade's own four's cast bars (metaverse/rules/srd5.coax srd_spell bar: Meteor Swarm, Thunderbolt,
// Mega Thunderbolt, Water Orb). Every other cast has none: it is released as it starts.
CR_BAR :: [4] f32.[2.5, 1.2, 0.5, 0.6];
// Repair (R; everglade-repair.coax): a broken structure within reach stands again whole after its bar,
// unless you move or are hurt first; then it waits a while. Its slots: the druid's and the Warlock's R.
CR_REPAIR_SLOTS :: [2] int.[47, 70];
CR_REPAIR_BAR :: 1.5;      // s
CR_REPAIR_EVERY :: 8.0;    // s from one repair to the next
CR_REPAIR_REACH :: 18.3;   // m (60 feet) from you to what you repair
CR_REPAIR_MOVE :: 0.6;     // m you may shift on its bar
// What a Repair names: a group of the town's pieces (a building, a house, a 16 m cell of the palisade or
// the streets: everglade-spells.coax sp_group), or an admin's placed thing i as CR_REPAIR_PLACED0 + i.
CR_REPAIR_PLACED0 :: 100000;
CR_PLACED_MAX :: 64;       // an admin's placed things at most (everglade/world/edits.coax EG_EDIT_MAX)
```

## A worked example: a Fireball

A level 5 Evoker Wizard with Intelligence 16 casts Fireball, a 3rd-level spell:
a 20-foot-radius sphere at a point within 150 feet, 8d6 Fire damage, a
Dexterity save for half.

```text
save DC     rs_spell_dc = 8 + rs_proficiency(5) + rs_mod(16) = 8 + 3 + 3 = 14
damage      8d6, 28 on average; one roll for every creature in the area
range       150 ft × 0.3048 = 45.72 m; the server takes up to 48.72 m at the start
radius      20 ft × 0.3048 = 6.10 m
```

A Goblin Warrior has Dexterity 15, a +2 modifier. It saves on a d20 roll of 12
or more, 45% of the time, and takes 14 damage on average. On a failed save it
takes 28, which is more than its 10 Hit Points.

Under strict SRD areas, a Wizard who centres the sphere 5 m away is inside it
and makes the save too. Under Everglade's world profile, the Wizard is spared.

## The checks

- `rules/srd/kit-spells-high-checks.coax`, `#test "Fireball p.131: a
  20-foot-radius Sphere at a point within 150 feet, a Dex save for half of 8d6
  Fire, 1d6 more a slot level"`.
- `rules/srd/shared-damage-checks.coax`, `#test "Fireball shares one
  damage roll while each recipient makes its own save"`.
- `rules/srd/area-origin.coax`, `#test "strict areas leave out only a
  caster at an origin or named 'other than you'; the declared house rule spares
  it everywhere"`.
- `rules/srd/component-checks.coax`, `#test "a gagged caster cannot cast
  Healing Word and refusal preserves every resource"`.
- `rules/srd/concentration-checks.coax`, `#test "a retained Stunned
  effect ends the target's concentration and releases only its owned benefits
  immediately"`.
- `rules/srd/spell-coverage.coax`, `#test "a source entry has an
  executor only when a canonical effect program maps to it"`.
- `rules/srd/turn-order-checks.coax`, `#test "every executable spell's
  timed duration and Concentration match the pinned source"`.
- `everglade/client/client.coax`, `#test "Everglade II shows a matching
  cast refusal over the hotbar and retires it after four seconds"` and `#test
  "Everglade II lifts the cooldown it predicted for a cast the room refused; an
  older cast's refusal changes nothing (#13046)"`.

```sh
coax/bin/coax test --library rules/srd/kit-spells-high-checks.coax
coax/bin/coax test --library rules/srd/shared-damage-checks.coax
```

## Mistakes

- **A spell that worked on the screen and failed on the server.** Your page and
  the server see your position a little differently, and the target moves
  during the cast. Without slack, a strict range check refuses casts that look
  fine on your screen. The start and release slack in the cast rules covers
  that gap.
- **A cooldown with no cast.** The page starts a slot's cooldown when you
  press, before the room answers. If the room refuses, the bar would stay
  cooling for a spell that never went off. The page now lifts the predicted
  cooldown, and the test above asserts it.

## What comes next

[Chapter 17](ch017-conditions.md) covers what a spell can do to you.
