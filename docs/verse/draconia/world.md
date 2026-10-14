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

## Walk round her now (local, private assets)

A first walkable cut, before the courtyard zone exists: Draconia standing
in a candlelit potions hall. It runs only on a computer that has her
private MetaHuman exports (openagents#11576).

```sh
scripts/verse/draconia-world.sh
```

- **Controls:**
  - Click to look with the mouse.
  - W, A, S, D walk, Shift runs, Space jumps.
  - F12 saves a screenshot to `~/Downloads`.
  - Escape frees the mouse; press it again to quit.
  - The window title shows the frame rate.
- **Start-up:**
  - The first build takes a few minutes and keeps its target in
    `~/work/openagents-target-castle`.
  - After that the world opens in about 3 s.
  - A new parity round is imported once, in about 10–20 s, and then
    cached in `~/work/film-out/draconia/world/cache/`.
- **Her model:**
  - The parity pipeline's latest round, read at run time from
    `~/work/film-out/draconia/metahuman/latest/mh.neutral.glb` (its `ROUND`
    file names the round), or `build-rig-neutral/` before that path
    exists. New rounds show up with no code change.
  - The parity builds stop at the robe's hem, so
    `scripts/verse/draconia_world_body.py` (Blender, run by the launcher
    when a round or her body is newer) adds her lower body:
    - her MetaHuman legs and hands, the legs in dark tights;
    - plain black shoes;
    - a pleated charcoal skirt;
    - the robe carried down to the shin.
  - It writes `~/work/film-out/draconia/world/draconia_lower.glb`.
- **Missing assets:** without them the launcher says which file is
  missing and stops. `--no-draconia` walks the empty castle.
- **Licence:** MetaHuman assets are private. They stay in
  `~/work/film-out` and are never committed, published, or used, nor
  their renders, for AI training. The world is for local use only.

### What it is

- **The hall:** the great crypt's vaulted nave, aisles, chapels and dais
  (`scripts/blender/great_crypt.py`), dressed as a potions hall from the
  crypt lab's props. It has cauldrons, jar shelves, bookshelves, alchemy
  benches, candelabra and braziers; the cult's circle, bones, cages and
  sarcophagi are left out.
- **Light:**
  - A light at every flame, with flicker, plus cauldron glows and the
    moon through the far window.
  - Her own warm key, a neutral fill, a teal rim, and a back light.
  - Four lights cast shadows, chosen by how much each one shows.
  - Height fog.
- **Walking:** the hall's collision boxes and each prop's footprint, through
  the physics character controller (0.22 m radius, 0.42 m steps). She has
  a low collider, so the camera can come to 25 cm from her face.
- **Her idle:** she is static geometry, so the world rigs her itself.
  - Seven joints: root, chest, head, both eyes, both upper lids.
  - Weights come from where each vertex sits.
  - The pose is computed each frame and handed to the renderer
    (`Renderer::set_pose_override`).
  - She breathes (4.4 s), shifts her weight (9 s), and blinks every
    2.5–6 s.
  - Within 5.5 m and in front of her, her head turns to you (up to 34°)
    and her eyes follow.
- **Detail:**
  - Within 7 m she draws at full detail: the strand groom over the hair
    cards, and 2048-pixel maps (`--textures 4096` for the full maps).
  - Further away, the strands drop and the cards remain.
  - Hair is drawn with alpha to coverage on the 4x multisampled target
    (`Renderer::hair_coverage`). The film path's stochastic cutoff needs
    averaged sub-frames, and in one frame it fizzes.

### Evidence and measures

- **Stills:** `scripts/verse/draconia-world.sh --capture ~/Downloads` writes
  `draconia-world-{wide,approach,face,three-quarter,full}.png`.
- **Clip:** `--walk OUT.mp4` renders a 22 s, 60 fps, 1080p walk: down from
  the landing to her face and round her.
- **Frame rate:** `--benchmark 20` flies that walk in the window at 1080p
  and prints the frame rate.
  - Measured on the owner's Mac, 2026-10-14, round 21: 95.8 fps average
    and 82.7 fps within 3 m of her face.
  - Frame time: median 8.4 ms, 95th percentile 16.8 ms.
- **Collisions:** `--check-walk` walks into her and into a wall and prints
  where the walker stopped.
