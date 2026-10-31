# Hold the Stair

Pitched by Marc on the October 30, 2026 recording: a balloon-popping
tower defense game, but with D&D.

Status: playable at openagents.com/realms/hold-the-stair (stair.coax, stair.wgsl).

## The game in one line

A wizard's tower is under siege by swarms of conjured bubble-beasts that
split when they burst. Hold the stair with a party of adventurers whose
attacks roll real dice.

## Style and originality

The genre is the popping tower defense: waves follow a winding path, you
place defenders beside it, and enemies come in layers that pop into weaker
ones. The rules are tabletop fantasy, taken only from the **SRD 5e**
(System Reference Document 5.2.1, CC BY 4.0), which we already use in
Everglade. None of another game's names, monkeys, balloons, maps or art, and
no non-SRD monsters or setting material.

## The enemies: bubble-beasts

A rogue conjurer's spell makes **bubble-beasts**: floating, glowing
creatures in layers. Pop the outer skin and the one inside comes out,
smaller and faster. Each layer is a creature with an SRD stat block (armor
class, hit points, saving throws, resistances), so defenses matter:

| Layer | Colour | What it is | Pops into |
|---|---|---|---|
| Mote | Pale blue | Slow, AC 10, 1 hit point | Nothing |
| Wisp | Green | AC 11, resists cold | Two motes |
| Glimmer | Gold | Fast, AC 13 | Two wisps |
| Ember | Red | Immune to fire | A glimmer and a wisp |
| Frostling | White | Immune to cold | Two glimmers |
| Ironskin | Grey | Resists slashing, piercing and bludgeoning from non-magical attacks | Two embers |
| Shadow | Black | Invisible to defenders without truesight or detection | Two frostlings |
| Hulk | Purple, huge | A boss: AC 17, many hit points, regenerates | Four ironskins |

Damage types and resistances are the strategy: a fire wizard is wasted on a
wave of embers, a cleric's radiant light is great against shadows.

## The defenders: your party

Each defender is an SRD class you place on the map. It attacks on its turn,
which comes round every few seconds:

- **Fighter:** cheap and steady. One melee attack per turn, two at level 5
  (Extra Attack). Great against ironskins with a magic weapon.
- **Rogue:** Sneak Attack. Extra dice against a target that another
  defender is also hitting.
- **Wizard:** area spells from the SRD: Fire Bolt, Magic Missile (never
  misses, so great against shadows once seen), Fireball, Ice Storm, Lightning
  Bolt in a line. Spell slots refresh between waves.
- **Cleric:** Sacred Flame and Spirit Guardians (an aura that hurts
  everything that passes); Daylight reveals shadows.
- **Ranger:** long range, chooses a favoured enemy layer.
- **Bard:** no attacks. Inspiration: nearby defenders add a d6 to hit.
- **Paladin:** a holy aura that makes nearby defenders' attacks magical.

**Dice are real.** Every attack rolls a d20 plus the defender's bonus
against the target's AC; a natural 20 is a critical hit and doubles the
dice; a save against a spell rolls the creature's save. The red damage
numbers rise over each target (Everglade's).

**Upgrades** are levels, 1 to 10, bought with gold between and during
waves. Each level is a class feature from the SRD, chosen from a short
list (subclass picks at level 3: an evoker wizard sculpts spells around
other defenders, a life cleric heals the tower).

## How it plays

- **Maps** are dungeon floors: the bubble-beasts climb the stairs from the
  cellar to the top of the tower. Each map's path winds; some have two.
- **Waves:** 40 waves to win a map, a hulk every 10. Lives are the tower's
  hit points: every bubble-beast that reaches the top takes some.
- **Gold** comes from popping layers, and between waves.
- **Spells you cast yourself:** the tower's own wizard (you) has a few
  spells per map: Wall of Stone across the path, Wall of Fire, Slow on a
  wave.
- **Phone:** tap a spot to place a defender, long-press to see its card,
  drag to swap two (the hotbar's rules). A top-down camera that pinches to
  zoom.
- **Co-op:** two players share the gold and each places their own party.

## What it reuses

| Need | What we have | Where |
|---|---|---|
| SRD 5e dice, spells, saves and objects | The ruleset Everglade opts into | `verse/rules/srd5.coax` |
| Creatures, armor class, hit points, attacks | Stat blocks and attack rolls | `verse/rules/srd5-combat.coax` |
| Spells with real effects | 48 spell slots: Fire Storm, Wall of Fire, Moonbeam, Magic Missile and the rest | `apps/openagents-web/everglade-spells.coax` |
| Homing projectiles | WANTED 5's Magic Missile at a levitating player | Everglade |
| Walls that break into rubble | Wall of Stone with AC and hit points; rubble damage | `verse/demolish.coax`, Everglade |
| Red damage numbers | Rising red numbers over whoever takes damage | Everglade |
| Waves of enemies on a rules engine | Dead Gears' waves, entities and event log | `verse/deadgears/rules.coax`, `verse/deadgears/ecs.coax` |
| Walking a path | Characters following garden paths | `verse/terrain.coax` |
| Placing things on a grid | Minecraft-style placing from the playtest redesign | `verse/edit/`, `verse/arrange.coax` |
| Tap, long-press, drag to swap | Everglade's hotbar | `apps/openagents-web/everglade-hotbar.coax` |

## What the engine needs

1. **Paths as lanes** (`verse/terrain.coax`): a path with a distance along
   it, so "first", "last", "strongest" and "closest" targeting are cheap,
   and enemies that move along it at their speed.
2. **Splitting enemies**: an entity that, at 0 hit points, spawns its inner
   layers at its spot along the path with their own stat blocks. An event
   in the rules log, so replays and analytics see it.
3. **Turn timers in real time**: each defender has an initiative and a turn
   every few seconds. The SRD's six-second round, scaled. Simultaneous turns
   resolved in a fixed order so the game is deterministic.
4. **Area queries**: "everything in this 20-foot sphere" and "everything in
   this line" against hundreds of moving enemies, every turn. A spatial
   grid in the entity system.
5. **Hundreds of enemies on a phone**: instanced bubbles (one mesh,
   per-instance colour and size), and particles for pops.
6. **A top-down tactical camera** (`verse/camera.coax`): pinch zoom, pan,
   and a slight tilt.
7. **Seeded dice for replays and co-op**: one seed per map so a replay or a
   shared match rolls the same dice (Ice Cream Man is already seeded).
8. **Content as data**: layers, defenders, levels and maps as text tables
   that an agent can write and the engine checks, like motions
   (`verse/motion.coax`) and set pieces (`verse/setpiece.coax`).

## Milestones

1. **One lane.** One map, motes to embers, a fighter and a wizard, real dice.
2. **The party.** All seven classes, levels 1 to 5, upgrades.
3. **The layers.** All eight bubble-beasts, resistances, shadows and
   detection.
4. **The tower.** Three maps, 40 waves, hulks, your own spells.
5. **Together.** Two-player co-op on the room service.

## Open questions

- How much of the dice should show? Every roll as a number, or only crits
  and misses?
- Does a six-second SRD round scaled to two seconds feel right?
