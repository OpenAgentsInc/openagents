# 14. Ability scores, proficiency and the d20

> **Status: partly implemented.** The core roll is in Aethyr's shared SRD rules,
> [`rules/srd/engine.coax`](../../rules/srd/engine.coax)
> (`rs_roll`, `rs_d20`, `rs_edge`, `rs_mod`, `rs_proficiency`), with checks in
> [`rules/srd/ability-check-math.coax`](../../rules/srd/ability-check-math.coax)
> and saving throws in
> [`rules/srd/condition-rolls.coax`](../../rules/srd/condition-rolls.coax).
> Other agents are extending these files now, so this chapter names functions
> and has no source listings. The SRD's coverage is tracked in the
> [compliance ledger](../../docs/dnd/srd-compliance-ledger.md).

Everglade aims to follow the SRD 5.2.1's rules faithfully while adapting them
for action combat. The d20 is where that starts. This chapter answers the
player's question: "Why did my arrow miss?"

Your aim decides what an arrow can hit. The SRD's attack roll decides whether
it hits. The roll is the SRD's roll, with no change for real time.

## What this chapter teaches

- The SRD's core roll: a d20, plus a modifier, against a target number.
- How ability modifiers and the proficiency bonus are computed.
- How Advantage and Disadvantage combine and cancel.
- How the dice stay fair and testable.

## What exists today

| Rule | Function | SRD |
| --- | --- | --- |
| An ability modifier | `rs_mod(score)`: `score / 2 − 5`, the same as (score − 10) / 2 rounded down | Ability Scores and Modifiers |
| The proficiency bonus | `rs_proficiency(level)`: `2 + (level − 1) / 4`, so +2 at levels 1 to 4 and +3 at 5 to 8 | Proficiency Bonus |
| A d20 with Advantage or Disadvantage | `rs_d20(d, edge)`: two dice, the higher or the lower | Advantage and Disadvantage |
| Combining edges | `rs_edge(advantages, disadvantages)`: any of each cancel to neither | Advantage and Disadvantage |
| An ability check | `rs_check_plan(c, q)`: ability, skill, tool, Expertise and Jack of All Trades | Ability Checks |
| A saving throw | `rs_saving_throw` and its variants | Saving Throws |
| A spell's numbers | `rs_spell_attack(c)` and `rs_spell_dc(c)`: proficiency plus the spellcasting modifier, and 8 more for the DC | Spellcasting |
| A weapon's attack bonus | `rs_weapon_attack(c, w)` | Weapons |

## What follows the SRD exactly

Everything in this chapter. The world profile ([chapter 18](ch018-turns-become-time.md))
names every way Everglade departs from the SRD, and none of its five
deviations touches the d20, modifiers, proficiency, Advantage, checks or saving
throws.

In particular:

- Several sources of Advantage still give one extra die, and any Advantage
  with any Disadvantage cancels to a plain roll (`rs_edge`).
- A saving throw passes on an exact DC and fails one below it.
- Exhaustion lowers d20 totals by twice its level
  (`rs_exhaustion_penalty`), and a natural 20 stays a natural 20.
- A check that needs sight fails automatically for a Blinded creature, and one
  that needs hearing fails for a Deafened one.

## What changes for action combat, and why

The roll doesn't change. What changes is how a roll starts:

- **Aim picks the target.** In the SRD, you choose a target that you can see.
  In Everglade, your crosshair and the shot's path choose it. The server checks
  range and line of sight before it rolls
  ([chapter 16](ch016-spells-and-slots.md)).
- **A shot can miss without a roll.** A shot that flies, such as a Goblin's
  arrow, resolves when it arrives. If you step out of its path, no attack roll
  happens at all ([chapter 2](ch002-what-the-prototypes-taught.md)).
- **The server rolls.** The room owns the dice, so every player sees the same
  result.

## The method: fair dice you can test

`rs_roll(d, sides)` draws from a seeded generator. It throws away any draw past
the largest whole multiple of `sides`, so every face is equally likely. A
battle can also set its dice to fixed mode, where every roll returns half the
die's size. A test uses fixed dice to know the result in advance, and counts
the dice rolled (`d[0].rolled`) to prove that a rule rolled exactly as often as
the SRD says.

## A worked example: a Ranger's longbow

The Ranger preset has Dexterity 16 and a Longbow (1d8 piercing, 150 feet,
`W_LONGBOW`):

```text
Dexterity modifier   rs_mod(16)          = 16 / 2 − 5 = +3
proficiency, level 1 rs_proficiency(1)   = 2 + 0 / 4  = +2
attack bonus, level 1                    = +3 + 2     = +5
attack bonus, level 2 with Archery (+2)  = +5 + 2     = +7
```

Against a Goblin Warrior's Armor Class of 15 (`rules/srd/monsters.coax`, SRD p.290), a level 1 Ranger hits on a d20 roll of 10
or more:

```text
P(hit)                 = 11 / 20 = 55%
P(hit), Advantage      = 1 − (9/20)²  = 1 − 0.2025 ≈ 80%
P(hit), Disadvantage   = (11/20)²     = 0.3025      ≈ 30%
```

A hit deals 1d8 + 3 piercing damage, 7.5 on average. The 150-foot range is
150 × 0.3048 = 45.72 m in the world (`RS_FOOT_M`).

## The checks

- `rules/srd/class-ranger-checks.coax`, `#test "Fighting Style p.59:
  from level 2, Archery's +2 on ranged weapon attack rolls, not melee"`: the
  +5 and +7 above.
- `rules/srd/condition-roll-checks.coax`, `#test "normal saving throws
  pass on an exact DC and fail one below it without inventing a rescue"`.
- `rules/srd/exhaustion-checks.coax`, `#test "each Exhaustion level
  penalizes attack and saving throw totals without changing natural
  criticals"`.
- `rules/srd/ability-check-tests.coax`, `#test "sight and hearing
  dependent checks automatically fail only for their corresponding missing
  sense"` and `#test "Entangle escape doubles the proficiency bonus for the
  preset Rogues chosen Athletics Expertise"`.
- `rules/srd/condition-permission-checks.coax`, `#test "initiative
  chooses the source-required lower or higher face and rolls only once after
  cancellation"`.

```sh
coax/bin/coax test --library rules/srd/class-ranger-checks.coax
coax/bin/coax test --library rules/srd/condition-roll-checks.coax
```

## Mistakes

- **Advantage that stacks.** Two sources of Advantage are still one extra die.
  A rule that adds a die per source rolls three dice and picks the best. Every
  rule passes its sources through `rs_edge`, and the initiative test asserts
  that the roll happens once, after cancellation.
- **A rule that rolls twice.** If a rule rolls a save, then rolls it again for a
  rescue such as Inspiration that wasn't needed, the dice drift from what the
  SRD describes. The saving-throw checks count the dice and assert that rescue
  dice roll only while needed.

## What comes next

[Chapter 15](ch015-classes-levels-1-to-6.md) builds the twelve classes on
these numbers.
