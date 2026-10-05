# Verse combat model: real time, no dice

Status: decision, October 4, 2026, from the owner. It applies to every zone,
game, demo, and agent in Verse, now and later.

Verse plays like an MMO, not a tabletop game. Nothing rolls dice. When Verse
borrows from the System Reference Document (SRD) or any tabletop source, it
keeps the idea of an ability, meaning what it does, its shape, its fantasy, and
its relative power, and translates the mechanics into the real-time model
below. There is no separate dice-based "fifth edition" ruleset to select, and
none is planned.

The chamber and Everglade already work this way. Their spells cost mana and
have cooldowns (`SpellDef::cost` and `cooldown`: "MMO tuning, not tabletop
rules"). This page makes that the rule everywhere and fixes how each tabletop
mechanic translates.

## Translation table

| Tabletop mechanic | Verse model |
| --- | --- |
| Rounds and turns | Continuous real time on the world's fixed clock. One tabletop round is **6 seconds** wherever a duration or tick needs converting. There is no initiative and no turn order. |
| Action, Bonus Action, Reaction | **Global cooldown** (GCD) of 1.0 s for actions. Bonus actions get their own 1.0 s off-GCD lane. Reactions become instant abilities with cooldowns, or procs. |
| Casting time | Instant (an action), or a **cast bar** for longer casts: 1.0 to 2.5 s by spell tier. Moving or being stunned interrupts a cast bar. |
| Attack roll against AC | **Always hits** within range and line of sight. Armor appears as **mitigation**, a percentage reduction of physical damage, never a miss chance. |
| Damage dice | A **fixed amount**, the dice's average rounded down, scaled by the caster's spell power. For example, 8d6 is 28. No variance, no criticals. |
| Saving throws | No saves. An effect "save for half" deals its full listed (average) damage. Control that a save could resist becomes a **diminishing-returns** duration: each repeat on the same target within 15 s halves the duration, and the third is immune. |
| Spell slots and levels | **Mana** plus **cooldowns**. The spell level sets the tier, which sets cost, cooldown, and power (table below). Cantrips cost no mana. |
| Limited uses per rest (Wild Shape, Channel Divinity) | **Charges** that recharge over time (for example, one charge per 30 s), with a maximum. |
| Short and Long Rest | Out of combat for 5 s regenerates mana and health quickly. A demo can add an explicit "rest" control that refills everything. |
| Concentration | One **maintained effect** per caster. Starting another ends the first. Taking damage does not break it; hard control (stun, incapacitate) does. |
| Conditions | Timed **debuffs** with icons and durations in seconds, mapped from the tabletop meaning: Restrained becomes rooted, Prone becomes knocked down for 1.5 s, Blinded misses ranged targeting, Poisoned takes damage over time, Charmed or Frightened stop attacking or run, Incapacitated or Stunned is a stun. |
| Advantage and Disadvantage | A damage or healing modifier (+20% / -20%) or a cast-speed change, never a reroll. |
| Resistance, Vulnerability, Immunity | Damage multipliers 0.5, 1.5, and 0. |
| Hit points and temporary hit points | Health and **shields** (absorbs) that decay after their duration. |
| Range and area in feet | Meters, at **5 ft = 1.5 m**. A 60-foot range is 18 m, and a 20-foot radius is 6 m. |
| Ability scores and proficiency | Character stats feed **spell power** (Wisdom for druids), health (Constitution), and mitigation (armor). Players never see a modifier added to a die. |
| Level-based scaling (cantrips at 5, 11, 17) | Scales with character level through spell power, smoothly rather than in steps. |
| Skill checks and contests | Not part of combat. Out of combat, an interaction succeeds or fails by stated, deterministic conditions, such as having a tool or a stat at or above a threshold. |

## Spell tiers

Defaults that a zone may tune, but never replace with dice:

| Spell level | Mana | Cooldown | Cast | Typical power (level 20 caster) |
| --- | --- | --- | --- | --- |
| Cantrip | 0 | GCD only | Instant | 20 to 30 damage |
| 1 to 2 | 10 to 15 | 6 to 10 s | Instant | 25 to 45 damage, or short control |
| 3 to 4 | 20 to 25 | 12 to 20 s | Instant or 1.0 s | 50 to 80 damage, or an area |
| 5 to 6 | 30 to 40 | 20 to 30 s | 1.5 s | 80 to 120 damage, walls, summons |
| 7 to 8 | 45 to 60 | 45 to 60 s | 2.0 s | 120 to 180 damage, large areas |
| 9 | 75+ | 90 to 180 s | 2.5 s | 200+ damage, or a transformation |

## Rules for authors and agents

- Never add dice, random hit or miss, random damage, or critical hits to
  combat. Randomness may appear only in presentation, such as particle
  scatter, and in deterministic seeded world events.
- The source's numbers inform tuning; Verse keeps no copy of the tabletop
  procedure. An ability's tooltip states its Verse numbers (damage, mana,
  cooldown, duration, range in meters), not dice.
- SRD material keeps its CC-BY attribution where an ability or text is taken
  from it, such as SRD 5.2.1 in the [druid demo](druid-demo.md).
- Authority stays with the world's simulation. Clients send intents, never
  results.

[Zone rules](zone-rules.md) describes each zone's current gameplay under this
model.
