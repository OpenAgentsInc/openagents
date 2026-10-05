# Druid demo: an Archdruid on the web

Status: specification, October 4, 2026; the Grove implements the four-row
bar as of October 5 ([Implemented](#implemented)). The owner asked for a web
demo, ready to show the next day, of a high-level druid in a field of training
dummies with a full action bar of druid abilities from the System Reference
Document. This page researches what a level 20 SRD druid can do, lays out the
action bar, maps each ability to what Verse already implements, and specifies
the build.

Every ability follows the [combat model](combat-model.md): real time, MMO
style, with SRD dice rolled behind the scenes. Spell slots and rest-limited
uses become mana, cooldowns, and recharging charges; attack rolls, damage
dice, and saving throws still roll, against a save DC of 19 and a +11 spell
attack. Damage figures in the action bar are the dice's averages; tooltips
show each spell's dice range.

The abilities come from the
[System Reference Document 5.2.1](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf)
(SRD 5.2.1), the 2024 rules, rather than the SRD 5.1 that
[SRD-5.1-NOTICE.md](SRD-5.1-NOTICE.md) covers. See [Attribution](#attribution).

## The goal

A visitor opens `openagents.com/druid`, which is full screen like
`/everglade`, and plays an Archdruid in a meadow. Training dummies stand at
several distances. Four action bar rows hold every class feature and every
prepared spell. Each ability shows its effect in the world, its damage and debuffs
in a combat log, and its effect on the dummies' hit points and
conditions. A **Choose Land** button swaps the land spells, and **Long Rest**
refills everything. The demo needs no sign-in, no server, and no model calls.

## The level 20 druid in SRD 5.2.1

### Class features

| Level | Feature | What it does at level 20 |
| --- | --- | --- |
| 1 | Spellcasting | Wisdom casting; 22 prepared spells; slots 4/3/3/3/3/2/2/1/1 (levels 1 to 9). |
| 1 | Druidic | Always has *Speak with Animals* prepared. |
| 1 | Primal Order | **Magician**: one extra cantrip (five in all), and Wisdom added to Arcana and Nature checks. The other choice, Warden, gives martial weapons and medium armor. |
| 2 | Wild Shape | Bonus Action; four uses, one back on a Short Rest and all back on a Long Rest. Lasts hours equal to half the druid level. Grants Temporary Hit Points equal to the druid level (20). Eight known forms, Challenge Rating 1 or lower, flying forms allowed. |
| 2 | Wild Companion | Spend a Wild Shape use or a spell slot to cast *Find Familiar* with no material component; the familiar is a Fey. |
| 5 | Wild Resurgence | Spend a spell slot to regain a Wild Shape use when none are left (once per turn), or spend a Wild Shape use for a level 1 slot (once per Long Rest). |
| 7 | Elemental Fury | **Potent Spellcasting**: add Wisdom to druid cantrip damage. The other choice, Primal Strike, adds 1d8 elemental damage to weapon and beast attacks. |
| 15 | Improved Elemental Fury | Potent Spellcasting: druid cantrips with a range of 10 feet or more gain 300 feet of range. |
| 18 | Beast Spells | Cast spells while in Wild Shape, except spells with a costed or consumed material component. |
| 19 | Epic Boon | A feat; Boon of Dimensional Travel is recommended. |
| 20 | Archdruid | **Evergreen Wild Shape** (regain a use on rolling Initiative with none left); **Nature Magician** (convert Wild Shape uses into one spell slot, two levels per use, once per Long Rest); **Longevity**. |

The SRD's only druid subclass is the **Circle of the Land**:

- **Level 3, Circle of the Land Spells:** after each Long Rest, choose a land
  and gain its spells, which don't count against the prepared total.
- **Level 3, Land's Aid:** spend a Wild Shape use for a 10-foot burst within
  60 feet. Enemies make a Constitution save or take 4d6 necrotic damage, half
  on a success. One creature of your choice heals 4d6 Hit Points. Both
  amounts scale with druid level and reach 4d6 at level 14.
- **Level 6, Natural Recovery:** cast one Circle spell without a slot once per
  Long Rest, and recover slot levels on a Short Rest.
- **Level 10, Nature's Ward:** immunity to Poisoned, plus a resistance that
  depends on the chosen land.
- **Level 14, Nature's Sanctuary:** spend a Wild Shape use to raise a
  15-foot cube of spectral trees that gives allies half cover and the land's
  resistance.

Land spells by druid level, from the SRD table:

| Land | 3 | 5 | 7 | 9 |
| --- | --- | --- | --- | --- |
| Arid | Blur, Burning Hands, Fire Bolt | Fireball | Blight | Wall of Stone |
| Polar | Fog Cloud, Hold Person, Ray of Frost | Sleet Storm | Ice Storm | Cone of Cold |
| Temperate | Misty Step, Shocking Grasp, Sleep | Lightning Bolt | Freedom of Movement | Tree Stride |
| Tropical | Acid Splash, Ray of Sickness, Web | Stinking Cloud | Polymorph | Insect Plague |

### Numbers the demo uses

- **Health and mana:** 163 health and 300 mana; mana regenerates 10 a second
  out of combat.
- **Spell power:** from Wisdom 20; it scales every spell's base value.
  Cantrips add the Potent Spellcasting bonus.
- **Tiers:** mana, cooldown, and cast time by spell level, from the
  [tier table](combat-model.md#spell-tiers).
- **Wild Shape:** four charges, one back every 30 seconds; each form grants a
  20-point shield.

## The action bar

Four rows of twelve, on keys `1` to `=`, with `Shift`, `Ctrl`, and `Alt` for
the rows below. Every slot shows its game-icons.net icon, its key, its
remaining charges or cooldown, and a tooltip with its SRD summary. **Status**
says where each ability stands today:

- **Done:** a Verse spell solver already exists (`verse-world` spells or the
  chamber kit).
- **Port:** the effect exists elsewhere in Verse and needs a druid version.
- **New:** nothing exists yet.

### Row 1: Wild Shape and druid features

| Key | Ability | Status | Demo behavior |
| --- | --- | --- | --- |
| 1 | Wild Shape: Brown Bear | New | Become a bear (a 20-point shield); bite and claw attacks on dummies. |
| 2 | Wild Shape: Dire Wolf | New | Fast wolf; a bite that knocks a dummy down for 1.5 s. |
| 3 | Wild Shape: Giant Eagle | New | Flight with Everglade's levitate altitude controls. |
| 4 | Wild Shape: Giant Spider | New | Climb, and a web attack that roots. |
| 5 | Return to Form | New | Drops the form, off the global cooldown. |
| 6 | Wild Companion | New | A Fey owl familiar that circles the druid and marks targets. |
| 7 | Land's Aid | New | Flower-and-thorn burst: 18 necrotic to enemies in a 3 m burst; heals one ally 14. |
| 8 | Nature's Sanctuary | Port | A 15-foot cube of spectral trees giving half cover; reuse Wall of Stone's placement. |
| 9 | Choose Land | New | Cycles Arid, Polar, Temperate, and Tropical, and swaps row 4's land slots. |
| 0 | Nature Magician | New | Converts Wild Shape charges into mana (30 per charge). |
| - | Wild Resurgence | New | Trades 40 mana for a Wild Shape charge. |
| = | Long Rest | New | Demo control: refills mana, charges, cooldowns, and dummies. |

### Row 2: cantrips and level 1 to 2 (`Shift`)

| Key | Ability | Level | Status | Demo behavior |
| --- | --- | --- | --- | --- |
| 1 | Produce Flame | 0 | Port | Flame in hand; hurled at a dummy for 23 fire (reuse Fire Bolt's projectile). |
| 2 | Starry Wisp | 0 | New | A mote of starlight for 23 radiant; the target sheds light and can't be invisible. |
| 3 | Shillelagh | 0 | New | Staff glows; melee with Wisdom for force damage. |
| 4 | Poison Spray | 0 | New | A bolt for 31 poison, as a green puff. |
| 5 | Elementalism | 0 | New | Cosmetic gust, ember, or earth tremor near a dummy. |
| 6 | Thunderwave | 1 | Done | Exists in the chamber kit and the physics layer; damages and pushes dummies. |
| 7 | Entangle | 1 | New | Grasping vines in a 6 m square; roots for 3 s, with diminishing returns. |
| 8 | Faerie Fire | 1 | New | Outlines dummies; they take 20% more damage for 10 s. |
| 9 | Ice Knife | 1 | New | Shard hits for 5 piercing, then bursts for 7 cold around it. |
| 0 | Healing Word | 1 | New | Off-GCD heal at range, as a green rune. |
| - | Moonbeam | 2 | New | A movable column of silver light; 11 radiant a second inside. |
| = | Gust of Wind | 2 | Done | Already in the physics layer; pushes dummies and props in a line. |

### Row 3: levels 2 to 6 (`Ctrl`)

| Key | Ability | Level | Status | Demo behavior |
| --- | --- | --- | --- | --- |
| 1 | Spike Growth | 2 | New | 6 m of thorny ground; 5 piercing per 1.5 m moved. |
| 2 | Call Lightning | 3 | New | A storm cloud; a bolt every 6 s for 16 lightning. |
| 3 | Conjure Animals | 3 | New | A spectral pack that damages dummies it moves past. |
| 4 | Wind Wall | 3 | Done | Already in the physics layer and in Everglade's hotbar. |
| 5 | Ice Storm | 4 | New | Hail in a 6 m cylinder: 11 bludgeoning plus 14 cold; leaves difficult ground. |
| 6 | Wall of Fire | 4 | New | An 18 m burning line: 22 fire a second. |
| 7 | Polymorph | 4 | New | Turns a dummy into a harmless beast for 8 s, with diminishing returns. |
| 8 | Mass Cure Wounds | 5 | New | A wave of green light that heals several targets. |
| 9 | Sunbeam | 6 | New | A radiant beam, recast while held: 27 radiant and a 2 s blind. |
| 0 | Wall of Thorns | 6 | New | A wall of brambles: 31 piercing to anything crossing. |
| - | Fire Storm | 7 | New | Ten 3 m cubes of flame: 38 fire. |
| = | Reverse Gravity | 7 | Done | Already in the physics layer; dummies and props fall upward. |

### Row 4: levels 8 and 9, and land spells (`Alt`)

| Key | Ability | Level | Status | Demo behavior |
| --- | --- | --- | --- | --- |
| 1 | Sunburst | 8 | New | An 18 m burst: 42 radiant and a 3 s blind. |
| 2 | Storm of Vengeance | 9 | New | A spreading storm with a new effect each round: thunder, acid rain, lightning, hail. |
| 3 | Shapechange | 9 | New | Take any form, here a dragon silhouette, while keeping druid spellcasting. |
| 4 | Speak with Animals | 1 | New | Druidic's always-prepared spell; the familiar answers in the log. |
| 5 to = | Land spells | 0 to 5 | Varies | The current land's six spells, below. |

**Land slots** (Choose Land swaps all six):

| Land | Spells in row 4, keys 5 to = | Status |
| --- | --- | --- |
| Arid (default) | Fire Bolt, Burning Hands, Blur, Fireball, Blight, Wall of Stone | Fire Bolt, Fireball, and Wall of Stone are **Done**; the rest are New. |
| Polar | Ray of Frost, Fog Cloud, Hold Person, Sleet Storm, Ice Storm, Cone of Cold | New. |
| Temperate | Shocking Grasp, Sleep, Misty Step, Lightning Bolt, Freedom of Movement, Tree Stride | Misty Step is **Done**; the rest are New. |
| Tropical | Acid Splash, Ray of Sickness, Web, Stinking Cloud, Polymorph, Insect Plague | Web is **Done**; the rest are New. |

### Prepared spells

The bar's leveled druid spells are the druid's 22 prepared spells:

- **Level 1:** Entangle, Faerie Fire, Healing Word, Ice Knife, Thunderwave.
- **Level 2:** Moonbeam, Spike Growth, Gust of Wind.
- **Level 3:** Call Lightning, Conjure Animals, Wind Wall.
- **Level 4:** Ice Storm, Wall of Fire, Polymorph.
- **Level 5:** Mass Cure Wounds.
- **Level 6:** Sunbeam, Wall of Thorns.
- **Level 7:** Fire Storm, Reverse Gravity.
- **Level 8:** Sunburst.
- **Level 9:** Storm of Vengeance, Shapechange.

Land spells and *Speak with Animals* don't count against the 22. The five
cantrips come from four druid cantrips plus Primal Order (Magician).

The rest of the druid list stays reachable through a **Spellbook** panel that
lists every SRD 5.2.1 druid spell by level. Any spell there can be dragged
onto the bar in place of a prepared one, as a druid re-prepares after a Long
Rest. Spells with no demo effect yet are marked and cast as a labeled
placeholder burst.

## What Verse has today

- **Spell solvers on shared physics** in `verse-world` (`spells/`): Thunderwave,
  Gust of Wind, Wind Wall, Wall of Stone, and Reverse Gravity, all druid spells,
  plus Telekinesis, Levitate, Feather Fall, Black Tentacles, and Meteor Swarm,
  which aren't. The chamber kit adds Fire Bolt, Fireball, Misty Step, and
  Web, which arrive through land spells. These carry seeded dice, saving throws,
  mana, cooldowns, maintained effects, and fields.
- **Everglade in the browser** (`crates/everglade-web`, `openagents.com/everglade`)
  runs the shared `WorldRuntime` on WebGPU and WebGL2. It already has the icon
  hotbar with held controls and four spells (Feather Fall, Wall of Stone, Wind
  Wall, and Reverse Gravity) through `zones/everglade/spells.rs`.
- **Assets:**
  - The Everglade pack: the Stylized Nature MegaKit's meadow, trees, rocks, and
    flowers, and a training dummy (`props/Dummy`).
  - The Ranger character on the Universal rig, with walk, run, strafe, and
    backpedal clips from the Universal Animation Library.
  - Sky, height fog, baked light, and sun shadows from tonight's lighting work.
- **Missing:**
  - Beast models for Wild Shape: no kit in hand has a bear, wolf, eagle, or
    spider.
  - A dummy health and condition model on the client.
  - A combat log.
  - Druid icons beyond the nine spells already vendored.
  - A spellbook panel.

The bar has 46 slots, two of them demo controls (Choose Land and Long Rest).
Nine abilities are **Done**: seven on the default Arid bar (Thunderwave, Gust
of Wind, Wind Wall, Reverse Gravity, Fire Bolt, Fireball, and Wall of Stone),
plus Misty Step and Web in other lands. Two are **Port** (Produce Flame and
Nature's Sanctuary), and the rest are **New**.

## The build

### World: the Grove zone

A new zone, `ZoneId::Grove`, built on the Everglade pack so no new asset
download stands between the demo and tomorrow:

- A meadow cleared from Everglade's nature placements: a ring of trees, rocks,
  and flowers around a 60-meter field, under Everglade's sky and lighting.
- Training dummies (`props/Dummy`), each with a plaque and a floating health
  bar:

| Dummy | Count | Placement | AC | HP | Traits |
| --- | --- | --- | --- | --- | --- |
| Straw | 3 | 10 m, 20 m, 30 m | 10 | 100 | None. |
| Armored | 1 | 15 m | 18 | 200 | Resists piercing and slashing. |
| Warded | 1 | 25 m | 13 | 150 | Resists fire; shows the land choice paying off. |
| Big | 1 | Clustered with the others | 15 | 300 | Large; for area spells. |
| Flying target | 1 | 15 m up, on a post | 12 | 60 | Wild Shape: Giant Eagle reaches it. |

- Dummies regenerate after 10 seconds untouched, and **Long Rest** resets
  them.
- The player spawns as the Archdruid: the Ranger rig in a druid outfit tint
  with a staff. The "Druid" outfit uses the Universal outfits pack's Peasant
  or Ranger parts recolored green and brown.

### Rules: a druid kit in the runtime

A `zones::grove` module, single-player and client-side, holds the following.

**Druid state:** the druid's stats, mana, cooldowns, Wild Shape charges,
concentration, the chosen land, the current form, and temporary hit points.

**Dummy state:** each dummy's AC, HP, saving throw modifiers, resistances,
and conditions (Restrained, Prone, Blinded, Poisoned, Faerie Fire,
Polymorphed, Burning), with condition icons shown above the health bar.

**Dice behind the scenes:** `verse-world`'s seeded `dice::Dice` rolls each
attack, damage, and save, so a page load replays from its seed. The log and
floating numbers show the outcome ("Miss", "Resisted", a crit), not the
procedure. Repeated hard control on one dummy also gets diminishing returns.

**Timing:** mana and cooldowns from the tier table, a 1-second global
cooldown, and a separate 1-second lane for off-GCD abilities (Wild Shape,
Healing Word). A new maintained effect replaces the old one. The playable
Grove drops mana and every cooldown so the owner can spam spells: each press
casts, and a held key recasts six times a second.

**Physics:** spells marked **Done** call their existing `verse-world`
solvers, as Everglade's four spells do. Their dummies are physics bodies, so
Thunderwave, Gust of Wind, and Reverse Gravity move them.

**Wild Shape:**
- Swaps the player's model and controller speeds: bear slow and sturdy,
  wolf fast, eagle flying with the Levitate-style altitude controls already in
  Everglade, spider climbing.
- Replaces the action bar's row 2 with the form's attacks.
- Under Beast Spells, the other rows still cast.
- Temporary hit points absorb damage first.

### Effects

Each ability gets a readable effect in the world plus a log line. Effects are
procedural, like the existing spell effects:

- particle bursts;
- ground decals: rings, squares, and lines;
- columns (Moonbeam, Sunbeam);
- walls built from repeated placed models: thorns from the nature kit's
  bushes, fire from flame cards;
- a storm cloud sprite layer (Call Lightning, Storm of Vengeance) with
  lightning strokes from the line renderer.

Damage numbers float off dummies, colored by damage type. All shaders follow
the platform rules of the sky pass: no derivatives in non-uniform control
flow, and no storage buffers on the Low tier.

### Interface

- **Action bar:** the four-row icon tray described above, extended from
  `zones::everglade::hotbar` to four rows of twelve, with a mana-cost badge
  and a cooldown sweep.
- **Resource bar:** health, mana, Wild Shape charges, the shield, the current land, the current form, and concentration.
- **Combat log:** an overlay at the lower left. Each entry gives the
  damage by type, the mitigation, and any debuff, for example
  "Fire Bolt hits Armored Dummy: 22 fire, 6 mitigated".
- **Tooltips:** each slot's SRD summary in our own words, with mana,
  cooldown, range and area in meters, damage, and debuffs.
- **Spellbook:** the panel described in [Prepared spells](#prepared-spells).
- **Touch:** on phones the bar collapses to one row with a row switcher, as on
  the iOS app.

### Web delivery

- **Page:** `openagents.com/druid` serves the same `everglade-web` WebAssembly
  module started in Grove mode (or a sibling `grove-web` entry), full screen,
  with the loading status over the canvas.
- **Assets:** the existing pinned Everglade pack plus any icons. No new pack
  is needed for tomorrow.
- **Deploy:** the Cloud Build path recorded in
  [the web deploy record](../deployment/openagents-web.md).

### Icons

Each new slot needs a game-icons.net icon, vendored and credited in
[`assets/verse/icons/game-icons/CREDITS.md`](../../assets/verse/icons/game-icons/CREDITS.md)
like the existing ones. That's about 35 icons. A slot whose icon is missing
shows the spell's school color and its initials until one is chosen.

## Scope for tomorrow, and after

| Tier | What ships | Effort |
| --- | --- | --- |
| **Demo (tomorrow)** | The Grove field and dummies; the four-row bar with every slot, tooltip, and log line; all nine Done spells; at least 14 more spells with real effects: Produce Flame, Starry Wisp, Poison Spray, Entangle, Faerie Fire, Ice Knife, Healing Word, Moonbeam, Spike Growth, Call Lightning, Ice Storm, Wall of Fire, Sunbeam, Sunburst. Choose Land swaps slots; Long Rest; Wild Shape as a stylized form change (tinted silhouette and speed) until beast models exist. Every other slot casts its labeled placeholder burst and logs its Verse numbers. | Two or three focused agents in one day, integrated and deployed the same night. |
| **Next** | The remaining spells' effects (Wall of Thorns, Fire Storm, Storm of Vengeance, Shapechange, Conjure Animals, Polymorph, and Insect Plague first); Land's Aid; Nature's Sanctuary; Wild Companion; the Spellbook panel. | About a week. |
| **Beasts** | Real Wild Shape forms once a CC0 animal pack is in hand: bear, wolf, eagle, and spider with their own attacks and animations. | Blocked on an asset pack. |

## Implemented

As of October 5 (#10609), the Grove (`crates/verse/src/zones/grove/`) has
the four rows above, adapted to the playable demo: no mana, no cooldowns,
and no charges, so every press casts and a held key recasts six times a
second.

- **The bar:** `slots.rs` holds the rows and each ability's icon and
  tooltip sentence; `hotbar.rs` draws them. Row 1 is on `1` to `=`, and
  `Shift`, `Ctrl`, and `Alt` reach rows 2 to 4. A desktop stacks all four
  rows; a screen under 640 points either way shows one row and a switcher.
  Row 4's last two slots are empty, since the land spells fill keys 5 to 0.
  In a browser on Windows or Linux, `Ctrl` with a digit may switch tabs
  first; clicking a slot always works.
- **Tooltips:** each card gives the sentence, the key, the level, the range
  in meters, the area, the damage range from the dice, the attack or save,
  the condition and its seconds, and concentration.
- **Spells with effects:** every **Done** spell and Produce Flame, Starry
  Wisp, Shillelagh, Poison Spray, Elementalism, Entangle, Faerie Fire, Ice
  Knife, Healing Word, Moonbeam, Spike Growth, Call Lightning, Conjure
  Animals, Ice Storm, Wall of Fire, Polymorph, Mass Cure Wounds, Sunbeam,
  Wall of Thorns, Fire Storm, Sunburst, Storm of Vengeance, Land's Aid, and
  every land spell except Blur and Freedom of Movement. Each lands by its
  area (`cast.rs`): a projectile, the target, a burst, a cone, a line, or a
  lasting zone or wall (`aura.rs`) that acts each second, each round, or as
  a dummy is pushed through it. A maintained spell ends the druid's other
  maintained area.
- **Placeholders:** Wild Companion, Nature's Sanctuary, Nature Magician,
  Wild Resurgence, Shapechange, Blur, and Freedom of Movement cast a labeled
  burst and say so in the log. Speak with Animals answers in the log.
- **Choose Land** cycles Arid, Polar, Temperate, and Tropical and swaps row
  4's six land slots.
- **Wild Shape:** the Giant Spider and the generated stylized bear, wolf,
  and eagle are pack forms (see [Beast models](#beast-models)). Each swaps
  the druid's model and pace and puts its attacks on row 2's first slots;
  the eagle flies with Everglade's levitation, Jump climbing and X
  descending.
- **Conditions:** timed debuffs on the dummies, shown as tags with their
  seconds over the health bars: rooted, knocked down, blinded, poisoned
  (3 poison a second), outlined (a fifth more damage), polymorphed (drawn
  small), paralyzed, asleep (damage wakes it), slowed, and starlit. Hard
  control diminishes within 15 seconds, and the third is immune.
- **Combat log:** the lower left shows the land and the form over the newest
  eight lines, with damage by type, crits, halved saves, resistances, and
  conditions.
- **Effects:** sprite particles from the [effect pipeline](particles.md)
  (`assets/verse/fx/effects/grove_*.toml`) and line-drawn rings, cones,
  beams, lightning strokes, columns, walls, and vines. Live effects, areas,
  particles, and floating numbers each have a cap, oldest first, so spam
  stays bounded.

Not built yet: the Spellbook panel, the resource bar (the playable Grove
has no mana or charges to show), Wild Shape's temporary hit points, and the
spider's climbing.

## Beast models

None of the kits on this Mac has a bear, wolf, or eagle. Quaternius's **Easy
Animated Enemy Pack** (January 2019, CC0, in `~/Downloads`) has an animated
**Spider**, plus a Rat, Frog, Snake, and Wasp, as FBX, OBJ, and Blender files.

The Spider is the first real Wild Shape form (#10610).
`scripts/blender/enemy_pack.py` converts it to glTF
([the Blender pipeline](blender-pipeline.md)), and
`scripts/blender/beasts_admit.py` admits it, with the generated bear, wolf,
and eagle, into the Everglade pack's `beasts` set. The pack compiler turns
each skinned model in that set into one of the pack's **forms**: a skinned
character beside the player's, with its own skeleton and clips
(`everglade_pack::compile::forms`). A gait's distance per loop is measured
from the clip, so the feet keep pace with the ground.

In the Grove, **Wild Shape: Giant Spider** (key `4`) swaps the druid's
character for the spider at 1.5 times its modeled size, a Large creature's
3 m. It plays idle standing, its walk by speed for every gait, and its
attack clip on a bite, and it moves at 1.25 times the druid's pace. Its
**Bite** (+5, 1d8 + 3 piercing and 2d6 poison, 3.5 m reach) and **Web** (+5
at 18 m; a hit roots for 6 s) take row 2's first two slots (`Shift+1` and
`Shift+2`), and the druid's spells still cast. **Return to Form** (`5`),
Long Rest, and leaving the Grove end it. Climbing isn't implemented. The
stylized bear (key `1`, slow, with a bite and a claw that knocks down),
wolf (`2`, fast, with a bite that knocks down), and eagle (`3`, flying, with
its talons) work the same way.

The Circle of the Land is the only SRD druid subclass; the Circle of the Moon
isn't in the SRD and stays out.

## Attribution

This work includes material taken from the System Reference Document 5.2.1
("SRD 5.2.1") by Wizards of the Coast LLC and available at
<https://dnd.wizards.com/resources/systems-reference-document>. The SRD 5.2.1
is licensed under the Creative Commons Attribution 4.0 International License
available at <https://creativecommons.org/licenses/by/4.0/legalcode>.
