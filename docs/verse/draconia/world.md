# Draconia: the playable world

The same courtyard the cinematic is shot in is also a place you can walk into
and duel in. The set is built once and used for both: shots are camera rigs
over the playable zone, as the Everglade film shots already are.

## The zone

- **Layout:** a castle courtyard at night, about 80 × 60 m.
  - Tall Gothic walls with arched tracery windows on three sides; a gate
    and a broken wall on the fourth.
  - A cobbled floor, low walls for cover, and rubble.
  - A tower beyond with a moon behind it.
  - From the courtyard, an arched passage leads to a sunlit classroom (the
    flashback room), which is a second, small interior.
- **What it's built from:**
  - The Modular Medieval Town kit for walls, arches and cobbles.
  - New Blender pieces for the tracery windows, the broken wall, and the
    rubble.
  - The kit stays outside the repo, per the medieval refactor's licensing
    rules.
- **Light:**
  - Night sky, moon key and fog.
  - Fires in braziers and wreckage, as point lights with flicker and ember
    particles.
  - Spells light the walls as they pass.
- **Breakable cover:** the low walls and parts of the main walls break
  under heavy spells (`docs/verse/destructible-buildings.md`).

## Wands and spells

Wands are a spell focus in the existing MMO combat model
(`docs/verse/combat-model.md`): real time, mana, cooldowns and cast bars,
with dice rolled behind the scenes.

- **Equipping.** The wand sits in the right hand. Casting plays a flick, and
  the spell leaves from the wand tip, not the chest.
- **Aiming.** The beam goes where the crosshair points. Holding the cast
  button channels a held spell.

| Spell (working name) | What it does | Effect |
| --- | --- | --- |
| Bolt | A fast single-target bolt | A thin straight beam with a hot core (the green of the reference is for D1; the original palette comes in D4) |
| Shield | Blocks bolts for 2 s; a perfect-timing block reflects one | A curved shimmering disc in front of the caster |
| Blast | A slower heavy bolt that breaks cover | A golden impact burst with fragments and dust |
| Light | Lights the wand tip; reveals hidden things | A small bright point light with a halo |
| Frost ring | Slows everyone near the caster | The ground frost wave from shot 14 |
| Guardian | Conjures a creature of light that drives back dark figures; a long channel and a long cooldown | The peacock guardian: emissive mesh, a fanned tail of light, particles |

Damage, saves and crits stay the combat model's dice. The names are ours,
not canon spell words.

## Duels

- **Training:** dummies on the cobbles, as in the Water Lab, with a damage
  readout.
- **Dueling an agent:** a student NPC duels you, run by the same agent
  system that runs Verse's townsfolk. Its choices (when to shield, when to
  blast) come from the Decisions API like other agent choices, not from
  keyword rules.
- **Multiplayer duels:** two players face off on the courtyard over Verse
  networking. Best of three, with a circle drawn on the cobbles.
- **The hooded figures** are a wave encounter at night. Bolts barely touch
  them; only Guardian drives them back. This is the cinematic's climax made
  playable.

## Phases

| Phase | Deliverable |
| --- | --- |
| **W1** | The courtyard set, walkable, with night light and fires. Shared with the cinematic (D1). |
| **W2** | Wands: equip, Bolt, Shield, Blast and Light; training dummies |
| **W3** | Duels against an agent student; breakable cover |
| **W4** | Frost ring, the Guardian, and the hooded-figure encounter; multiplayer duels; the sunlit classroom |

## Where it lives

- **Crate:** a new zone, `verse-zone-academy` (working name), like the
  other zone crates.
- **Entry:** reached from Everglade through a portal once the zone is
  public; until then, `verse --academy` for development.
- **The cinematic:** the D1 shot files point at this zone's scene, as a new
  `courtyard` scene kind in `film_shot.rs`.
