# SRD 5.2.1 rules: one shared system for every world

Status: specification, 2026-11-02. Owner direction: a fully compliant SRD 5.2.1
implementation that any world creator can add to a world. Everglade (a shared
MMO world) and Hold the Stair (a tower defense) play by the same underlying
system. Each world can tweak the rules and draw the abilities its own way, on
shared infrastructure.

This document covers what the system is, where it lives, how a world uses it,
what "compliant" means and how it is checked, and the work to get there from
what exists today.

## Legal and naming

The rules come only from the **System Reference Document 5.2.1** (Wizards of
the Coast, CC BY 4.0). Our copy is `~/Desktop/SRD_CC_v5.2.1.pdf`. The import
tool records the PDF's SHA-256 so a later SRD is a deliberate update.

- Every surface that shows SRD content (a world's page, its `index.md`, a
  spell card's long form, the rules pages) carries the SRD's attribution
  statement, word for word:

  > This work includes material from the System Reference Document 5.2.1
  > ("SRD 5.2.1") by Wizards of the Coast LLC, available at
  > https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the
  > Creative Commons Attribution 4.0 International License, available at
  > https://creativecommons.org/licenses/by/4.0/legalcode.

- No other attribution to Wizards. Product copy may say "5E compatible" or
  "compatible with fifth edition". It never says "Dungeons & Dragons" or
  "D&D", or uses its logos, settings or non-SRD names. This folder is
  `docs/dnd/` for us; what players see says "SRD 5e" or "5E".
- Nothing from outside the SRD: no non-SRD spells, monsters, subclasses,
  settings or art. A world's own inventions are allowed and are always
  marked as its own (see [World tweaks](#world-tweaks)).

## What "fully compliant" means

The system holds **all** of the SRD's game content as data, and executes the
rules the SRD defines:

| SRD chapter | What the system holds |
|---|---|
| Playing the Game, Rules Glossary | Ability checks, saving throws, attack rolls, Advantage and Disadvantage, proficiency, actions, Bonus Actions, Reactions, movement, cover, hiding, damage and healing, death saves, the 15 conditions (Exhaustion among them), resting, the glossary's defined terms |
| Character Creation, Character Origins | Ability scores, levels and XP, proficiency bonus, backgrounds, species, origin feats |
| Classes | The 12 classes, their features by level, spell slots, and the one SRD subclass each |
| Feats | Origin, general, fighting style and epic boon feats |
| Equipment | Weapons (with masteries), armor, tools, gear, mounts, services |
| Spells | Every spell in Spell Descriptions (about 330; the import tool's count is the authority), each class's spell list, spellcasting rules (slots, preparation, rituals, concentration, components, upcasting, cantrip scaling) |
| Gameplay Toolbox | Traps, poisons, diseases, environmental effects, curses |
| Magic Items A–Z | Every item: rarity, attunement, charges, what it does |
| Monsters, Animals | Every stat block |

"Compliant" means a rule the system executes gives the SRD's answer. A spell
cast at a level, against a creature with a save modifier, on given dice,
produces the damage, condition, duration and save outcome the SRD text
defines. Where a world changes a rule, the change is declared, not silent.

## Architecture

Five layers. Only the first three are shared code; the last two belong to
each world.

```
  SRD content (data)        aethyr/rules/srd/data/   generated from the PDF, checked in
        │
  Rules engine (pure)       aethyr/rules/srd/        dice, checks, combat, spells, conditions, rests
        │
  Time adapter              aethyr/rules/srd/time    turns ⇄ real time
        │
  ┌─────┴──────────────┐
  World tweaks          Presentation               per world: house rules, art, sound, UI
```

### 1. SRD content: data, not code

Every spell, class, monster and item is a **record** in a Coax table under
`aethyr/rules/srd/data/`, generated from the PDF by an import tool
(`aethyr/rules/srd/import.coax`, a Coax tool per `coax/tools-in-coax.md`)
and checked in. A world never hand-writes an SRD spell's numbers again, as
Everglade's 43 bar spells do today.

A spell record holds the SRD's fields and a typed **effect program**:

```
SrdSpellRecord
  id            stable key, "moonbeam"
  name          "Moonbeam"
  level         0–9 (0 is a cantrip)
  school        Abjuration … Transmutation
  classes       the class lists it is on (Druid, …)
  cast_time     action | bonus action | reaction (+ trigger) | minutes N | hours N
  ritual        bool
  range         self | touch | feet N | sight | unlimited (+ an area's origin)
  area          none | sphere | cube | cone | cylinder | line | emanation, with sizes in feet
  components    V, S, M (+ the material text, its cost, consumed or not)
  duration      instantaneous | rounds/minutes/hours/days N | until dispelled | special
  concentration bool
  text          the SRD's description, verbatim, and "Using a Higher-Level Spell Slot"
  effects       the effect program (below)
  srd_page      the PDF page, for review
```

The **effect program** is a short list of typed steps the engine runs. The
steps are a closed set, so any world can run any spell without spell-specific
code:

| Step | Meaning |
|---|---|
| `target` | Who or what: creatures, objects or points in range or area, with filters (willing, you can see, a type) |
| `attack` | A melee or ranged spell attack against AC |
| `save` | An ability saving throw against the caster's DC, with what a success does (half, none, a lesser effect) |
| `damage` | Dice and a damage type, scaling by slot level or caster level |
| `heal` | Dice plus a modifier, or temporary hit points |
| `condition` | Apply or end one of the 15 conditions, with its duration and repeat-save rule |
| `modify` | A bonus or penalty to a roll, AC, a speed or a sense, for a duration |
| `zone` | A lasting area with its own triggers: on enter, on start of turn, on end of turn (Moonbeam, Spike Growth, Wall of Fire) |
| `summon` | Creatures from the SRD's stat blocks, under the caster's control |
| `move` | Push, pull, teleport, lift, or a speed (Gust of Wind, Misty Step, Levitate) |
| `terrain` | Difficult terrain, a wall, an object made or reshaped (Wall of Stone, Plant Growth) |
| `light` | Bright and dim light radii (Daylight, Produce Flame) |
| `sense` | Detection and information (Detect Magic, Locate Object) |
| `transform` | A creature's statistics replaced (Polymorph, Wild Shape) |
| `narrative` | A part with no mechanical rule the engine can run (Speak with Animals' talk, Commune with Nature's answers). The world decides how to present it; the engine still tracks duration, concentration and slots |

A spell whose program includes `narrative` is still fully implemented. Its
mechanics run, and its free-form part goes to the world, for example to an NPC
voice host for Speak with Animals.

### 2. The rules engine: pure and deterministic

`aethyr/rules/srd/` executes the rules. It draws nothing and knows no world.
It is deterministic: the same seed and the same inputs give the same rolls
and results. That's what lets a shared world replay, an agent predict, and a
test pin an answer.

Its parts:

- **Dice:** today's `verse/rules/srd5.coax` SplitMix64 stream, rejection
  sampled, moved and kept.
- **Checks, saves, attacks:** ability modifiers, proficiency, Advantage and
  Disadvantage, critical hits, cover.
- **Creatures:** a stat block or a character sheet, both the same shape:
  abilities, AC, hit points, speeds, senses, resistances, immunities,
  conditions, actions.
- **Characters:** class levels, spell slots, prepared spells, features with
  uses and recharge (short or long rest), and multiclassing.
- **Spellcasting:** slot spending, upcasting, cantrip scaling, rituals,
  components (with a world's option to ignore material costs, see tweaks),
  concentration (one spell; a Constitution save on damage, DC 10 or half the
  damage, at most 30).
- **Effects and conditions:** durations in rounds, repeat saves, stacking
  rules (the same effect doesn't stack).
- **Objects:** today's armor class by material and hit points by size,
  moved and kept.

The engine's API is events in, results out. A world sends "this caster casts
Moonbeam at slot 3 at this point" and gets back the rolls, the saves, the
damage per target, the conditions applied, and the zone created. The world
draws those and moves things.

### 3. The time adapter: one rule set, two clocks

The SRD runs in turns: a round is 6 seconds, and each creature has one action,
one Bonus Action, one Reaction and its movement. Worlds run on one of two
clocks:

- **Turn-based:** initiative order and one creature's turn at a time. For
  tabletop-style worlds and a future table mode.
- **Real time:** Everglade and Hold the Stair. The previous engine's combat
  model (`0b401561a0^:docs/verse/combat-model.md`) is the base:
  - A round is 6 s of the world's clock.
  - Each creature has an action budget that refills every 6 s: one action,
    one Bonus Action, one Reaction.
  - A cast time is a cast bar.
  - Durations count in 6 s rounds.
  - "Start of turn" and "end of turn" triggers fire on each creature's own
    6 s beat, staggered by its initiative so they don't all fire at once.

The engine counts rounds either way; only the adapter changes. Hold the
Stair's per-class "turn T seconds" is a real-time adapter setting, not a
separate rule set.

### 4. World tweaks

A world can change the rules, but only by declaring the change in its
**ruleset profile**, a small table beside its `realms.json` entry, or in a
made world's spec. The engine applies tweaks; worlds never fork engine code.

```
ruleset srd5
classes druid ranger wizard          # which classes a player can be (default: all 12)
spells  druid                        # which spell lists the bar draws from, or "all"
grant   druid: meteor-swarm telekinesis levitate   # extra spells for a character, outside its list
cost    slots | none | gold | cooldown N            # how casting is paid for
material ignore-cost                 # material components with a cost are not needed
damage  scale objects 1.0            # objects take full damage (Everglade's destruction)
custom  thunderbolt                  # a world's own spell, defined in its folder, marked srd: false
```

Rules:

- A tweak names exactly what it changes. Anything not named is the SRD's.
- A world's own spell (Everglade's Thunderbolt, Mega Thunderbolt, Water Orb)
  is a record in the same shape with `srd: false`. Its card says it is the
  world's own, and it is never mixed into the SRD's lists.
- Granting a class spells outside its list is allowed. Everglade's druid
  keeping Meteor Swarm, Telekinesis, Levitate, Feather Fall and Black
  Tentacles is a declared `grant`, not a different rule set.
- Everglade's "no mana, no cooldown" destruction bar is `cost none`; Hold the
  Stair's gold price is `cost gold`.

### 5. Presentation: each world draws its own

The engine says what happened; the world decides how it looks, sounds and
reads. A world registers a **look** per spell id:

- the effect it draws (particles, a mesh, a zone's outline), from the shared
  effect library or its own;
- its sounds;
- its icon and card art;
- its bar slot and key.

A shared **default look** exists for every SRD spell. A world that adds the
ruleset gets every spell playable at once and replaces only the looks it
cares about. Hold the Stair's Wall of Stone and Everglade's Wall of Stone are
the same record and the same engine call, drawn differently.

### Formats: the same spell everywhere

One record feeds every surface, so a spell reads the same wherever it shows
up:

| Surface | What it shows |
|---|---|
| The bar and its card (Coax, every world) | Name, level, school, cast time, range, duration, concentration, the short effect line, the world's look |
| A spellbook page (Coax, shared component) | The full record: the SRD's text verbatim, at-higher-levels, classes |
| `index.md` and the hidden `<main>` text | The world's spells and rules, as text for agents, search engines and screen readers, with the attribution |
| Voice | The voice host reads the same record ("Moonbeam, level 2, a 5-foot cylinder of light…") and casts by name |
| Agents (CLI, MCP `realm_*` tools) | List, describe and cast by `id`; results come back as the engine's events |
| Aethyrforge | The world's ruleset profile and looks, edited as data |

## How a world adds it

- **A catalog world** (`realms.json`): `"ruleset": "srd5"` (already read by
  `coax/web/realms.ts` and passed to the page as `data-ruleset`), plus its
  ruleset profile at `apps/openagents-web/realms/<slug>/ruleset.txt`.
- **A made world** (Aethyrforge, `openagents realm …`): the ruleset and its
  profile are fields of the world's spec. A maker picks "SRD 5e" and the
  classes and spells to allow. Every SRD spell works with its default look,
  and an agent can set looks and tweaks through the same edit model.
- A world without a ruleset loads none of this and casts nothing, as today.

## Checking compliance

- **Import check:** every spell, class, monster and item in the PDF is in the
  data, and the counts match the PDF's own lists. The PDF hash is pinned.
- **Golden rules tests:** a test per spell (and per rule) with a fixed seed:
  cast at its base level and one level higher, against a fixed target, and compare
  rolls, damage, save outcome, conditions and duration with the SRD text.
  The tests are written from the SRD text, not from the implementation.
- **Effect program coverage:** a spell whose program can't express part of
  its text marks that part `narrative`. The coverage report lists every such
  part, so the gap is visible.
- **Tweak isolation:** a world's tweaks are applied in one place and tested
  per world. Removing a world's profile gives pure SRD results.
- **Per-world coverage report:** for each world, which SRD spells its classes
  can cast, which have a custom look, and which use the default.

## What exists today

| Where | What | Keep, move, replace |
|---|---|---|
| `verse/rules/srd5.coax` (305 lines) | Dice, saving throws, spell save DC, 14 damage types, the Prone condition, objects by material and size, 7 spell records (4 SRD, 3 Everglade's own) | Move into `aethyr/rules/srd/`; its spell table becomes generated data plus Everglade's `srd: false` records |
| `verse/rules/srd5-combat.coax` (103 lines) | 8 creature stat blocks (Guard, Commoner, Veteran, Mage, Knight, Priest, Archmage, Gladiator) and a Ranger 5; the attack roll | Stat blocks become generated data; the attack roll moves into the engine |
| Everglade (`apps/openagents-web/everglade-*.coax`, about 10 files) | 47 bar spells, each hand-coded with its numbers in the file, real-time, with its own art | Numbers come from the records; the files keep only the look and the world's physics (debris, water, the watch) |
| Hold the Stair (`stair.coax`, `stair-tables.coax`) | 7 SRD classes at levels 1–5 with their subclasses, original bubble-beasts as SRD stat blocks, Wall of Stone and Wall of Fire, a real d20 | Classes and spells come from the records; the beasts stay as the world's own creatures in the SRD's shape |
| `coax/web/realms.ts` | `ruleset` in `realms.json`, the `data-ruleset` page attribute | Kept; gains the profile file |

Of the SRD druid list's 124 spells, Everglade has 37 today. The other 87
include Druidcraft, Guidance, Cure Wounds, Goodberry, Barkskin, Flame Blade,
Pass without Trace, Plant Growth, Dispel Magic and Heal. Its bar also carries
spells from other classes' lists (Meteor Swarm, Ray of Frost, Feather Fall,
Levitate, Telekinesis, Black Tentacles). Under this system those become a
declared `grant` in Everglade's profile; Ray of Frost already fits the Circle
of the Land's polar list.

## Work, in order

1. **Engine home:** move `verse/rules/srd5*.coax` to `aethyr/rules/srd/`,
   with Everglade and Hold the Stair loading the new path. No behaviour
   change; their tests pass as before.
2. **Import:** the PDF-to-records tool and the generated data for spells and
   spell lists first, then monsters, classes, feats, equipment, magic items,
   conditions. Counts checked against the PDF.
3. **Effect program and executor:** the step set above, with golden tests
   per spell written from the SRD text. Start with the 124 druid spells
   (Everglade's class), then every list.
4. **Time adapter:** turn-based and real-time, with the combat model's
   6-second round.
5. **Ruleset profiles:** the profile format, tweaks applied in the engine,
   Everglade's and Hold the Stair's profiles written so their play doesn't
   change.
6. **Everglade on the records:** its 43 abilities and 4 aimed spells read
   their numbers from the records; the 87 missing druid spells become
   playable with default looks.
7. **Hold the Stair on the records:** its classes, levels and spells read
   from the records.
8. **Shared surfaces:** the spellbook page, cards, `index.md` text, voice
   casting by name, agent tools.
9. **Made worlds:** the ruleset in Aethyrforge's spec and edit model, and a
   maker's choices.

## Open questions for the owner

- Turn-based mode: needed now, or only real time until a world asks?
- Character creation: does every player get a full SRD character sheet
  (species, background, class, level), or does each world hand out a
  pre-built character, as Everglade's druid and Hold the Stair's party do?
- Progression in shared worlds: do levels and XP persist per player across
  sessions and worlds?

## Related

- [2026-11-03-dnd-campaign-creator-spec.md](2026-11-03-dnd-campaign-creator-spec.md):
  the campaign creator and table for game masters, built on this system.
