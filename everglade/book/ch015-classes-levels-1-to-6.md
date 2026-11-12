# 15. Classes, levels 1 to 6

> **Status: partly implemented.** The twelve class presets are the
> constructors `rs_barbarian` to `rs_wizard` in
> [`rules/srd/engine.coax`](../../rules/srd/engine.coax). Their
> features through level 6 are listed in
> [`rules/srd/class-features.coax`](../../rules/srd/class-features.coax)
> and checked one class at a time in `rules/srd/class-<class>-checks.coax`
> (#13044). Each class's bar in Everglade II is built by `xk_entry_at` in
> [`everglade/shared/class-kit.coax`](../shared/class-kit.coax).
> Other agents are extending these files now, so this chapter names functions
> and has no source listings.

Everglade aims to follow the SRD 5.2.1's classes faithfully while adapting them
for action combat. This chapter answers the player's question: "What can my
class do at this level?"

You pick one of the twelve SRD classes and a level from 1 to 6 in Character
Create. The world gives you that class's features at that level, and no
others. Your bar holds what you can press.

## What this chapter teaches

- How each class preset is built at a level.
- How a feature table ties each feature to its level and printed page.
- How a class's bar comes from its features.
- What changes about limited uses in real time.

## What exists today

Each class has one preset, with one subclass from the SRD:

| Class | Preset |
| --- | --- |
| Barbarian | Path of the Berserker |
| Bard | College of Lore |
| Cleric | Life Domain, Divine Order Protector |
| Druid | Circle of the Land, a Warden |
| Fighter | Champion, in Chain Mail |
| Monk | Warrior of the Open Hand |
| Paladin | Oath of Devotion |
| Ranger | Hunter, with a Shortsword and a Longbow |
| Rogue | Thief |
| Sorcerer | Draconic (Fire), with Quickened and Seeking Spell |
| Warlock | Fiend, with Eldritch Blast |
| Wizard | Evoker |

The pieces:

- **The preset.** `rs_fighter(name, team, level)` and the others build a
  creature at a level: its scores, Hit Points, training, slots, uses and
  features.
- **The feature table.** `rs_cf_table` lists every feature that the SRD's class
  tables and subclass descriptions give the twelve presets at levels 1 to 6,
  with its level and printed page. `rs_cf_held` says whether a built preset
  shows it.
- **The class checks.** One file per class. Each test is named for a feature
  and its page, and checks what the feature does against the SRD's text.
- **The bar.** `xk_entry_at(class, level, slot)` gives the ability on one slot
  of a class's bar at a level. `xc_url` names its picture.

## What follows the SRD exactly

- Each feature arrives at its level and not before.
- Each feature does what its text says, with its own numbers: dice, uses,
  ranges and the action it costs.
- Ability Score Improvements raise the scores the SRD's tables allow.
- Spell slots follow each class's table
  ([chapter 16](ch016-spells-and-slots.md)).

## What changes for action combat, and why

The class rules don't change. Two declared deviations in the world profile
([chapter 18](ch018-turns-become-time.md)) change how often you can use them:

- **Cooldown recovery** (`RS_DEV_RECHARGE`). In the action timing, a spent slot,
  Rage or other limited use comes back one at a time after 9 seconds. Focus
  Points, Sorcery Points and Bardic Inspiration come back one at a time after
  4.5 seconds. The SRD restores them on rests, which a fight that never stops
  doesn't have.
- **Unlimited presses** (`RS_DEV_FREE_PRESS`). In Everglade's world profile,
  each press refills slots, uses, Focus, Rage and the turn's Action budget
  first (`rs_press_prepare` in
  [`rules/srd/press-budget.coax`](../../rules/srd/press-budget.coax)).
  The bar's own cooldown paces how often you press. A press doesn't refresh
  your Reaction or movement, and a refused press rolls everything back.

Everglade II adds one more rule of its own, its social profile
([`everglade/shared/cast-rules.coax`](../shared/cast-rules.coax)).
In Everglade II today, no creature is selected for harm. A self or
ally feature affects you. An offensive spell pays its costs and harms no one.
An empty swing spends an Action. This is a named targeting rule, not a change
to any class.

## A worked example: a Fighter from level 1 to 5

Second Wind is a Bonus Action that heals 1d10 + your Fighter level. Action
Surge gives one more action this turn, from level 2.

| Level | Second Wind | Uses | Action Surge | Extra Attack |
| --- | --- | --- | --- | --- |
| 1 | 1d10 + 1 (6.5 on average) | 2 | No | No |
| 2 | 1d10 + 2 (7.5) | 2 | Yes, once until a rest | No |
| 4 | 1d10 + 4 (9.5) | 3 | Yes | No |
| 5 | 1d10 + 5 (10.5) | 3 | Yes | Yes, two attacks with the Attack action |

In a strict SRD battle, a level 4 Fighter who spends all three uses gets one
back on a Short Rest and all of them on a Long Rest. Under the action
world profile (`RS_ACTION_WORLD`, which 5E MMO uses), one use comes back 9
seconds after it is spent. In Everglade II, a
level 5 Fighter's Action Surge has its own slot on the bar and its own slot on
the wire, so the room knows exactly which feature you pressed.

## The checks

- `rules/srd/class-features.coax`, `#test "the slice's feature table:
  each done class's features are on its preset from their level, not before"`.
  It builds each of the twelve presets at levels 1 to 6 and checks every row.
- `rules/srd/class-fighter-checks.coax`, `#test "Second Wind p.48: a
  Bonus Action, 1d10 + Fighter level; two uses, three at 4; one back on a Short
  Rest, all on a Long Rest"`, `#test "Action Surge p.48: from level 2, one more
  action this turn, once until a rest"` and `#test "Extra Attack p.48: from
  level 5, two attacks with the Attack action"`.
- `rules/srd/timing-checks.coax`, `#test "only the declared action
  profile recharges spent resources on the existing cooldown"`.
- `everglade/shared/class-kit.coax`, `#test "class kits: a slot names one
  ability at every level; a level's bar holds only its own"`.
- `everglade/client/client.coax`, `#test "Everglade II's bar is your
  class's at your level: a level 5 Fighter's Action Surge goes by its own wire
  slot (#13046)"`.
- `everglade/shared/cast-rules-checks.coax`, `#test "Everglade II takes
  a slot only from the seat's own level's bar"`.

There are 127 class checks across the twelve `class-<class>-checks.coax` files.

```sh
coax/bin/coax test --library rules/srd/class-features.coax
coax/bin/coax test --library rules/srd/class-fighter-checks.coax
```

## Mistakes

- **A feature before its level.** A preset built from a table can carry a
  feature from a later row. The feature-table test builds every preset at
  every level and asserts that each feature appears from its level and not
  before.
- **A bar from another level.** The room must not take a press for a slot that
  your level's bar doesn't hold. The room-side test asserts that a seat's slot
  comes only from its own level's bar.

## What comes next

[Chapter 16](ch016-spells-and-slots.md) casts spells.
