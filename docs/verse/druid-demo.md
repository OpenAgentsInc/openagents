# Druid demo: an Archdruid on the web

Status: proposed specification, October 4, 2026. The owner asked for a web
demo, ready to show the next day, of a high-level druid in a field of training
dummies with a full action bar of druid abilities from the System Reference
Document. This page researches what a level 20 SRD druid can do, lays out the
action bar, maps each ability to what Verse already implements, and specifies
the build.

The rules come from the
[System Reference Document 5.2.1](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf)
(SRD 5.2.1), the 2024 rules, rather than the SRD 5.1 that
[SRD-5.1-NOTICE.md](SRD-5.1-NOTICE.md) covers. See [Attribution](#attribution).

## The goal

A visitor opens `openagents.com/druid`, which is full screen like
`/everglade`, and plays an Archdruid in a meadow. Training dummies stand at
several distances. Four action bar rows hold every class feature and every
prepared spell. Each ability shows its effect in the world, its dice and saving
throws in a combat log, and its effect on the dummies' hit points and
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

- **Ability scores:** Wisdom 20 (+5) and proficiency bonus +6, so spell save
  DC 19 and spell attack +11.
- **Hit points:** level 20 with Constitution 14 gives 163.
- **Cantrips:** four dice from level 17, plus Wisdom from Potent Spellcasting.
  *Produce Flame* hurls for 4d8 + 5.
- **Slots and uses:** shown as pips, 4/3/3/3/3/2/2/1/1 spell slots plus four
  Wild Shape uses.

## The action bar

Four rows of twelve, on keys `1` to `=`, with `Shift`, `Ctrl`, and `Alt` for
the rows below. Every slot shows its game-icons.net icon, its key, its
remaining uses or slot level, and a tooltip with its SRD summary. **Status**
says where each ability stands today:

- **Done:** a Verse spell solver already exists (`verse-world` spells or the
  chamber kit).
- **Port:** the effect exists elsewhere in Verse and needs a druid version.
- **New:** nothing exists yet.

### Row 1: Wild Shape and druid features

| Key | Ability | Status | Demo behavior |
| --- | --- | --- | --- |
| 1 | Wild Shape: Brown Bear | New | Become a bear (20 temp HP); bite and claw attacks on dummies. |
| 2 | Wild Shape: Dire Wolf | New | Fast wolf; pack-tactics bite that can knock a dummy Prone. |
| 3 | Wild Shape: Giant Eagle | New | Flight (Wild Shape forms may fly from level 8). |
| 4 | Wild Shape: Giant Spider | New | Climb, and a web attack that Restrains. |
| 5 | Return to Form | New | Bonus Action; drop the form and keep the temp HP rules. |
| 6 | Wild Companion | New | A Fey owl familiar that circles the druid and can Help. |
| 7 | Land's Aid | New | Flower-and-thorn burst: Con save for 4d6 necrotic, heal one ally 4d6. |
| 8 | Nature's Sanctuary | Port | A 15-foot cube of spectral trees giving half cover; reuse Wall of Stone's placement. |
| 9 | Choose Land | New | Cycles Arid, Polar, Temperate, and Tropical, and swaps row 4's land slots. |
| 0 | Nature Magician | New | Converts Wild Shape uses into one slot (two levels per use). |
| - | Wild Resurgence | New | Trades a slot for a Wild Shape use, or the reverse once. |
| = | Long Rest | New | Demo control: refills slots, uses, and dummies. |

### Row 2: cantrips and level 1 to 2 (`Shift`)

| Key | Ability | Level | Status | Demo behavior |
| --- | --- | --- | --- | --- |
| 1 | Produce Flame | 0 | Port | Flame in hand; hurled at a dummy for 4d8 + 5 fire (reuse Fire Bolt's projectile). |
| 2 | Starry Wisp | 0 | New | A mote of starlight for 4d8 + 5 radiant; the target sheds light and can't be invisible. |
| 3 | Shillelagh | 0 | New | Staff glows; melee with Wisdom for force damage. |
| 4 | Poison Spray | 0 | New | Ranged spell attack for 4d12 + 5 poison, as a green puff. |
| 5 | Elementalism | 0 | New | Cosmetic gust, ember, or earth tremor near a dummy. |
| 6 | Thunderwave | 1 | Done | Exists in the chamber kit and the physics layer; pushes dummies on a failed Con save. |
| 7 | Entangle | 1 | New | Grasping vines in a 20-foot square; Str save or Restrained. |
| 8 | Faerie Fire | 1 | New | Outlines dummies; attacks on them have Advantage. |
| 9 | Ice Knife | 1 | New | Shard hits for 1d10, then bursts for 2d6 cold on a failed Dex save. |
| 0 | Healing Word | 1 | New | Bonus Action heal at range, as a green rune. |
| - | Moonbeam | 2 | New | A movable column of silver light; Con save for 2d10 radiant. |
| = | Gust of Wind | 2 | Done | Already in the physics layer; pushes dummies and props in a line. |

### Row 3: levels 2 to 6 (`Ctrl`)

| Key | Ability | Level | Status | Demo behavior |
| --- | --- | --- | --- | --- |
| 1 | Spike Growth | 2 | New | 20-foot thorny ground; 2d4 piercing per 5 feet moved. |
| 2 | Call Lightning | 3 | New | A storm cloud; each turn calls a bolt: Dex save for 3d10. |
| 3 | Conjure Animals | 3 | New | A spectral pack that damages dummies it moves past. |
| 4 | Wind Wall | 3 | Done | Already in the physics layer and in Everglade's hotbar. |
| 5 | Ice Storm | 4 | New | Hail in a 20-foot cylinder: 2d10 bludgeoning plus 4d6 cold; leaves difficult ground. |
| 6 | Wall of Fire | 4 | New | A 60-foot burning line: 5d8 fire. |
| 7 | Polymorph | 4 | New | Turns a dummy into a harmless beast (Wis save). |
| 8 | Mass Cure Wounds | 5 | New | A wave of green light that heals several targets. |
| 9 | Sunbeam | 6 | New | A radiant beam, recast each turn: 6d8, and Blinded on a failed Con save. |
| 0 | Wall of Thorns | 6 | New | A wall of brambles: 7d8 piercing. |
| - | Fire Storm | 7 | New | Ten 10-foot cubes of flame: 7d10 fire. |
| = | Reverse Gravity | 7 | Done | Already in the physics layer; dummies and props fall upward. |

### Row 4: levels 8 and 9, and land spells (`Alt`)

| Key | Ability | Level | Status | Demo behavior |
| --- | --- | --- | --- | --- |
| 1 | Sunburst | 8 | New | A 60-foot burst: 12d6 radiant, and Blinded on a failed Con save. |
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
  concentration, and fields.
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

**Druid state:** the druid's ability scores, spell slots, Wild Shape uses,
concentration, the chosen land, the current form, and temporary hit points.

**Dummy state:** each dummy's AC, HP, saving throw modifiers, resistances,
and conditions (Restrained, Prone, Blinded, Poisoned, Faerie Fire,
Polymorphed, Burning), with condition icons shown above the health bar.

**Dice:** SRD dice from `verse-world`'s seeded `dice::Dice`, so a page load
can be replayed from its seed. Attacks roll against AC, saves roll against
DC 19, and the result is half damage on a successful save where the spell
says so.

**Real-time timing:** the SRD's six-second round runs as a 6-second clock.
Durations and concentration tick on it. Casting costs a slot and has a short
global cooldown (1 second) so the bar feels like an action game, not a turn
sequence. Bonus-action features (Wild Shape, Healing Word) share a separate
1-second cooldown.

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
  `zones::everglade::hotbar` to four rows of twelve, with a slot-level badge
  and a cooldown sweep.
- **Resource bar:** spell slot pips by level, Wild Shape uses, temporary hit
  points, the current land, the current form, and concentration.
- **Combat log:** an overlay at the lower left. Each entry gives the roll,
  the DC, the save, the damage by type, and any condition, for example
  "Fire Bolt hits Armored Dummy (19 vs AC 18): 22 fire".
- **Tooltips:** each slot's SRD summary in our own words, with level, range,
  area, save, and damage.
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
| **Demo (tomorrow)** | The Grove field and dummies; the four-row bar with every slot, tooltip, and log line; all nine Done spells; at least 14 more spells with real effects: Produce Flame, Starry Wisp, Poison Spray, Entangle, Faerie Fire, Ice Knife, Healing Word, Moonbeam, Spike Growth, Call Lightning, Ice Storm, Wall of Fire, Sunbeam, Sunburst. Choose Land swaps slots; Long Rest; Wild Shape as a stylized form change (tinted silhouette and speed) until beast models exist. Every other slot casts its labeled placeholder burst and logs its SRD numbers. | Two or three focused agents in one day, integrated and deployed the same night. |
| **Next** | The remaining spells' effects (Wall of Thorns, Fire Storm, Storm of Vengeance, Shapechange, Conjure Animals, Polymorph, and Insect Plague first); Land's Aid; Nature's Sanctuary; Wild Companion; the Spellbook panel. | About a week. |
| **Beasts** | Real Wild Shape forms once a CC0 animal pack is in hand: bear, wolf, eagle, and spider with their own attacks and animations. | Blocked on an asset pack. |

## Open decisions for the owner

1. **Beast models.** The CC0 kits on this Mac have no animals. The options:
   - download a CC0 animated animal pack (Quaternius publishes animal packs
     under CC0) for real Wild Shape forms;
   - accept stylized silhouettes for tomorrow.
2. **Subclass.** Circle of the Land is the only SRD subclass. The Circle of
   the Moon is not in the SRD, so it stays out.
3. **Tuning.** The default is real-time rounds with tabletop dice and slots,
   as above. The chamber's mana-and-cooldown kit is the alternative if the demo
   should feel like an MMO rotation rather than a tabletop turn.

## Attribution

This work includes material taken from the System Reference Document 5.2.1
("SRD 5.2.1") by Wizards of the Coast LLC and available at
<https://dnd.wizards.com/resources/systems-reference-document>. The SRD 5.2.1
is licensed under the Creative Commons Attribution 4.0 International License
available at <https://creativecommons.org/licenses/by/4.0/legalcode>.
