# Verse combat model: MMO play, dice behind the scenes

Status: decision, October 4, 2026, from the owner. It applies to every zone,
game, demo, and agent in Verse, now and later.

Verse plays like an MMO, not a tabletop game: real time, mana, cooldowns, cast
bars, buffs and debuffs. The dice stay. The simulation rolls SRD-style dice
behind the scenes for hits, damage, saves, and critical hits, and the player
sees the outcome (a hit or a miss, a damage number, a resist, a crit), never a
turn-based procedure. When Verse borrows an ability from the System Reference
Document (SRD), it keeps its dice and its saving throw and translates its
timing and resources into the model below.

The chamber and Everglade already work this way: their spells cost mana and
have cooldowns (`SpellDef::cost` and `cooldown`: "MMO tuning, not tabletop
rules"), and `verse-world`'s seeded `dice::Dice` rolls their damage and saves.

## Translation table

| Tabletop mechanic | Verse model |
| --- | --- |
| Rounds and turns | Continuous real time on the world's fixed clock. One tabletop round is **6 seconds** wherever a duration or tick needs converting. No initiative and no turn order. |
| Action, Bonus Action, Reaction | **Global cooldown** (GCD) of 1.0 s for actions. Bonus actions get their own 1.0 s off-GCD lane. Reactions become instant abilities with cooldowns, or procs. |
| Casting time | Instant (an action), or a **cast bar** for longer casts: 1.0 to 2.5 s by spell tier. Moving or being stunned interrupts a cast bar. |
| Attack roll against AC | **Rolled behind the scenes.** A d20 plus the attack bonus against the target's AC decides hit or miss; a natural 20 crits. The player sees "Miss" or a damage number. |
| Damage dice | **Rolled behind the scenes** with the SRD dice, scaled by the caster's level where the SRD scales them. Tooltips show the range (for example, 8 to 48 for 8d6). |
| Saving throws | **Rolled behind the scenes** against the caster's save DC. A success shows as "Resisted" or half damage, as the spell says. Repeated hard control on one target also gets **diminishing returns** (each repeat within 15 s halves the duration; the third is immune), so it can't chain-lock. |
| Spell slots and levels | **Mana** plus **cooldowns**. The spell level sets the tier, which sets cost, cooldown, and cast time (table below). Cantrips cost no mana. |
| Limited uses per rest (Wild Shape, Channel Divinity) | **Charges** that recharge over time (for example, one per 30 s), with a maximum. |
| Short and Long Rest | 5 s out of combat regenerates mana and health quickly. A demo can add an explicit "rest" control that refills everything. |
| Concentration | One **maintained effect** per caster. Starting another ends the first. Damage may break it on a behind-the-scenes Constitution save, as in the SRD. |
| Conditions | Timed **debuffs** with icons and durations in seconds: Restrained becomes rooted, Prone becomes knocked down for 1.5 s, Blinded misses ranged targeting, Poisoned takes damage over time, Charmed or Frightened stop attacking or run, Incapacitated or Stunned is a stun. A debuff that the SRD lets the target save against each round rolls that save every 6 s. |
| Advantage and Disadvantage | Kept as rolls: roll two d20s and take the higher or lower, behind the scenes. |
| Resistance, Vulnerability, Immunity | Damage multipliers 0.5, 2, and 0, as in the SRD. |
| Hit points and temporary hit points | Health and **shields** (absorbs). |
| Range and area in feet | Meters, at **5 ft = 1.5 m**. A 60-foot range is 18 m, and a 20-foot radius is 6 m. |
| Ability scores and proficiency | Kept: they set attack bonuses, save DCs, and saves. Players see them on the character sheet, not on every cast. |
| Skill checks and contests | Rolled behind the scenes where an interaction needs one; the player sees success or failure. |

## Spell tiers

Defaults that a zone may tune. Damage stays the spell's dice.

| Spell level | Mana | Cooldown | Cast |
| --- | --- | --- | --- |
| Cantrip | 0 | GCD only | Instant |
| 1 to 2 | 10 to 15 | 6 to 10 s | Instant |
| 3 to 4 | 20 to 25 | 12 to 20 s | Instant or 1.0 s |
| 5 to 6 | 30 to 40 | 20 to 30 s | 1.5 s |
| 7 to 8 | 45 to 60 | 45 to 60 s | 2.0 s |
| 9 | 75+ | 90 to 180 s | 2.5 s |

## Rules for authors and agents

- Keep the dice in the simulation, seeded so a fight replays exactly, and
  out of the player's way: no roll prompts, no turn order, no waiting.
- Show outcomes the MMO way: floating numbers, "Miss", "Resisted", "Crit",
  and tooltips with damage ranges, mana, cooldown, and range in meters.
- SRD material keeps its CC-BY attribution where an ability or text is taken
  from it, such as SRD 5.2.1 in the [druid demo](druid-demo.md).
- Authority stays with the world's simulation. Clients send intents, never
  results or rolls.

[Zone rules](zone-rules.md) describes each zone's current gameplay under this
model.
