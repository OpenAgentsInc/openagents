# Everglade Experimental meadow

The unlisted experimental world uses a separate Nature glade pack. Its 180-metre
square ground keeps Character Create's sky, grass meshes, lighting and central
planting. Extra grass extends into the meadow, and rocks and the original
background town are omitted. Walking is bounded eight metres inside the ground's
edge.

The world contains the four distinct layouts from Quaternius's Medieval Village
Godot sample scene: House C, Tower, Houses A, and Houses D. House C2 is an exact
local-layout duplicate of House C and is not duplicated. Eighteen placements combine
these layouts into larger settlements, using 2,183 source-authored pieces with
their original transforms, meshes and base-color textures. Buildings supply
nearby collision shapes to the Everglade walker: the source
collision meshes become individual axis-aligned boxes, and the interior stairs
become sloped walkable surfaces. Non-box hulls are approximated by their bounds;
interactive doors have not yet been authored. The homes and their surroundings
are furnished with 112 placements of 20 Fantasy Props Standard models; these props
have colliders derived from their placed geometry. The terrain is levelled under the
sites and blends back into the meadow. The extra geometry uses a single padded
texture atlas; the original Character Create pack retains its 32 MiB budget.

The experimental pack's showcase profile raises its budget and
adds fantasy-appropriate Ultimate Food models, 106 Ultimate RPG models,
48 Modular Dungeon pieces and 12 Ultimate Animated Animals. Food and RPG
models stand in rows south of spawn, animals run in place to their west, and dungeon
pieces are displayed in the southeast meadow. The animals use ten baked
poses of the original Gallop clips, including bone-parented antlers. The showcase
pieces do not yet supply collision shapes.
Modern packaged foods and fast food are omitted from the world; their source
assets remain in the shared registry. Their reserved layout slots are left
empty so the other specimens keep their positions.

The remaining 74 Fantasy Props Standard models have a display area in the
western meadow. The Imp and Gremlin (the Puglin model) from the Bestiary Dungeon Monsters Standard
pack stand near spawn. Their source pack contains no animations, so they cycle
through six compatible Universal Animation Library 1 motions every three seconds. Three additional imps and three gremlins wait at their places until the player comes near and in sight, then pursue and attack using the SRD combat rules. Nothing wakes, attacks or lands a hit while the player stands within 12 metres of spawn, or for 3 seconds after spawning or respawning. A Goblin Warrior's Shortbow, an Ogre's Javelin and a Hill Giant's Trash Lob fly as a visible arrow or rock after a draw or wind-up on the attacker's attack clip, with a release sound at the attacker; the attack resolves when it arrives, so stepping out of its path makes it miss and fly on past. A hit from off-screen shows a red wedge at the screen's edge toward the attacker, fading over about a second. Player health is shown on a bar, hits raise damage numbers, and after the death animation the enemies return toward their starting places. Press R or click the respawn prompt to return to spawn at full health; defeated monsters stay defeated. Their original
Quaternius Asset License v1.0 is retained in the registry and scene catalog.

## Housewright showcase

Three levelled yards east and north-east of the food rows show each basic house
from the [Housewright spec](../../everglade/docs/housewright-profession-spec.md)
frozen at every stage of its build. Each tier is one row running west to east,
from the bare site to the finished house: the branch lean-to (3 stages), the
woven hut (4), the wattle-and-daub cottage (7), the timber house (5) and the
stone cottage (5). Rows face each other in pairs across 6.8 m walks:

- The hut and the lean-to, on the yard centred at (26.3, 46). Their walk runs
  along z 47.
- The cottage, facing north, and the timber house and stone cottage, facing
  south, on the yards centred at (10.8, 72.2) and (60, 78.5). Their walk runs
  along z 72, from x -20 to x 75.

Houses in a row stand at least 3 m apart. Each house has a sign on a post in
front of it, right of its doorway. Its board shows the tier and the stage, for
example "Woven hut" over "2 of 4: walls woven", when you are within 16 m. The
nearest labels are drawn first, and a label that would overlap a nearer one is
left out.

The parts are data in `everglade/world/housewright.coax`; the staged-build
piece is `aethyr/world/build-site.coax`, and the packer meshes the parts
with `aethyr/world/build-site-mesh.coax`. The layout and the signs are in
`verse/experimental-terrain.coax`. The timber house uses the Medieval Village
kit's wall, door, window and roof modules. Colliders match what each stage
shows: you can walk between bare frame posts, and finished walls block you
everywhere except through the doorway.

To look at the showcase without playing, add `?px=1009&pz=1047&yaw=90&dist=6`
(the hut and lean-to walk) or `?px=978&pz=1072&yaw=90&dist=6` (the long walk)
to a local page. `&pitch=DEGREES` tilts the camera down; for example
`?px=1020&pz=1060&yaw=0&dist=45&pitch=50` looks over the whole showcase.

A build site can also show any progress between stages (#13106). The packer
stores each part once, with a reveal key from 0 to 1 on every vertex, and the
scene draws one instanced record per part. The record carries the part's pose
and how far it has arrived, so a post swings up from the ground, a beam is
raised into place, wattle is woven row by row, daub spreads in patches and
thatch goes on course by course. `bs_fractions`, `bs_arrival` and `bs_used`
in `aethyr/world/build-site.coax` give each part's progress, the part that
is arriving and where, and the materials used so far. A whole-number progress
draws exactly that stage. Open `?stage=build-cottage` to watch one
wattle-and-daub cottage go up on the meadow at (62, 66), two seconds a stage.

### Time-lapses

`?stage=house-leanto`, `house-hut`, `house-cottage`, `house-timber`, `house-stone` and
`house-row` film one house building on the open grass at (-60, -38), south of the glade's village, or the five finished side by
side. Each one owns the camera (a keyed track, `aethyr/cam/track.coax`) and the sun (the
glade's day, `env_glade_day_profile`), and draws no HUD. On a local page, `&sx=` and `&sz=` (metres
plus 1000) and `&turn=` stand it somewhere else. See [Gameplay clips](clips.md).

## Fight back

You fight the monsters with your class's level 1 kit
(`everglade/zone-combat.coax`, #13025).

- The bar at the bottom holds your class's level 1 abilities, on keys 1 to N. Slot 1 is the
  ability from Character Create's CLASS row. The others are the class's cantrips, 1st-level
  spells, and features that work with no ally present. For example, a Wizard has Fire Bolt,
  Chill Touch, Ray of Sickness, False Life, and Thunderwave, and a Fighter has Second Wind and
  Attack. Hover over a slot to see its card. You can also click a slot.
- Everything goes the way your character faces. Nothing picks a target for you, and you can't
  click a monster to target it.
  - A bolt or ray (Fire Bolt, Eldritch Blast, Starry Wisp, Guiding Bolt, a bow's Attack) flies
    straight ahead to its SRD range. It strikes the first monster whose body its line crosses,
    with about 5 degrees of aim help. Otherwise it flies on and fizzles.
  - A blow (a sword's Attack, Sneak Attack, Chill Touch) strikes the nearest monster within
    reach in a 90-degree arc ahead, or swings at the air.
  - A single-target spell with a saving throw (Sacred Flame, Vicious Mockery, Hex) takes the
    nearest monster in range within a 30-degree cone ahead. With no one there, its icon flashes
    red and no turn is spent.
  - Self and area abilities (Second Wind, Rage, Thunderwave, Entangle) work from where you stand,
    with the area set the way you face.
- A heal adds to your Hit Points. One second passes between uses.
- The SRD 5e engine (`rules/srd/`, through `fivebar.coax`) rolls every attack, save, and
  damage roll offline. The Imp has AC 13 and 10 Hit Points. The Gremlin has AC 12 and 13 Hit
  Points. Each monster shows its name and Hit Points, and damage numbers rise off it.
- A monster at 0 Hit Points dies. It stops, a puff of dust rises, and it sinks into the ground
  over 2.4 seconds.
- CLASSES, a column under ANIMATIONS on the left (or at the top when ANIMATIONS is hidden), lists
  the twelve SRD classes. Click one to become that class at level 1. The bar reloads with its
  kit. Click CLASSES to fold the column.
- Melee classes hold a weapon from the Ultimate RPG kit: the Barbarian a greataxe (Axe_Double),
  the Fighter a greatsword (Sword_big), the Paladin a golden longsword (Sword_Golden), and the
  Ranger and Rogue a shortsword (Sword, smaller). The Monk fights unarmed, and casters hold
  nothing. Changing class in CLASSES swaps the weapon.
- One-handed weapons are fixed to the right hand's bone, so they follow every animation.
- The greataxe and greatsword are held in both hands. No Universal Animation Library clip holds
  a weapon in two hands, so OpenAgents authored its own: a two-handed ready stance, the same
  hold over the run, and a diagonal cleave, one set for each weapon. While you hold a
  two-hander, these replace the idle and run, so the legs still run. The hold and run carry
  the weapon low across the body, its head out to your left. The right fist is low on the grip
  and the left fist is up the haft (about 40 cm for the greataxe, 16 cm for the greatsword).
  The weapon runs from the right fist through the left, so both hands stay on it.
- The cleave lifts the weapon up and back over your right shoulder, with the head behind and
  above your head and your arms clear of your face. It then cuts from high right to low left
  across the front of your body, follows through low on your left and returns to the hold. On
  the greataxe's downswing, your left hand slides down the haft toward your right.
- A blow with a weapon (Attack, Sneak Attack, Divine Smite) plays a swing over what you're
  doing, blended like the jump. One-handers use the Universal Animation Library 1 sword
  attack, and the blow lands at the frame where the hand moves fastest forward or down.
  Two-handers use the authored cleave, and the blow lands at its impact, 0.55 s into the
  1.2 s swing. In both cases the blow lands then, not at the key press. A two-hander's blow
  takes the whole body, even on the run, and is blended over the two-handed hold, never over
  the jump. A jump with a two-hander blends in a tenth of the jump pose, not the usual 45%,
  because the jump's free arms would swing the weapon into you.
- The two-handed clips are authored in Blender by `verse/tools/assets/two_hand_clips.py` over
  the Universal Animation Library 1 rig: the haft is keyed in the chest's space, each wrist is
  pinned to it with IK, and the result is baked to plain keys at 60 frames a second.
- The weapon keeps clear of your body. The head is a 12 cm sphere and the neck a capsule,
  and the weapon's real mesh boxes must stay at least 8 cm from both and outside a 15 cm
  torso capsule. The Blender script checks this every 1/120 s on the library rig and fails
  the bake if the weapon comes closer. Tests in `everglade/scene/charactercreator.coax`
  check it again every 1/60 s after retargeting onto both game bodies, through every clip and
  through the blends into and out of the cleave, the jump and the idle-to-run change.
- The `.blend` source, the GLB and
  their provenance are the registered package `openagents-two-hand-clips` (see
  `assets/registry/README.md`). `verse/tools/clip_rig.py` retargets them from
  `verse/packs/everglade-two-hand.json` into `verse/everglade/two-hand-rig.coax`.

Thirty-six additional flower, plant, fern, clover, mushroom, bush and grass variants grow in
the eastern meadow. The remaining sixty nature models, including trees, rocks and paths,
stand in display rows in the northwestern meadow. The experimental pack budget is 512 MiB
and accommodates up to 128 texture layers. Its deterministic gzip transport is
tracked in Git, and the browser decompresses it before reading the pack.

A walled and roofed stone dungeon stands southwest of spawn. Its entrance is at (-30.5, -44), on the side that faces the village. The dungeon is 55 by 110 metres on a 5-metre grid and runs south past the meadow's edge to z = -154. It has 166 cells of floor, 4,150 square metres in all. The old 20-by-20-metre dungeon, 400 square metres, is now its entrance hall. Sixteen rooms and passages meet at 17 arched doorways:

- The entrance hall.
- An east passage to the guard room.
- A west passage to the barracks.
- A 20-by-40-metre great hall with columns, which both routes reach.
- Three dead ends: a crypt, a storeroom and a treasure nook.
- An antechamber.
- The Warlord's hall at the far end.

`verse/experimental-dungeon.coax` holds the layout. The scene packer builds the Modular Dungeon pieces from it, and the walker, the camera and the monsters collide with boxes computed from the same layout:

- Floor_Modular and Wall_Modular are placed at 2.5 times, with walls 5 metres tall. The walls meet at corners and doorway jambs without gaps or overlapping faces.
- Each doorway is an Arch. Its posts and lintel are also colliders.
- Furniture stops you where it stands: tables, chairs, barrels, crates, chests, pedestals, statues, columns and braziers.
- The roof is ten-metre slabs of Floor_Modular.

Torches and the fire bowls are the only light inside. The glade shader darkens everything under the roof and lights it from the nine lights nearest you, which flicker. Inside, the sky's haze and the sun's veil are off. The outer walls' faces and the roof keep the daylight. The walls stand 0.3 metres into the floor and slabs lie under it, so no seam shows the sky.

On a local page, `?cell=N&yaw=D` starts you in dungeon cell N (row × 11 + column) turned D degrees, for headless screenshots. For example, `?cell=203` is the Warlord's hall.

The outer walls, the walls between rooms, the roof and the entrance are drawn with the village. The rest of the inside is eight parts by room, and each part is drawn only while you can see into one of its rooms. A test checks every line of sight between cells. Monsters inside are drawn only in those parts.

Monsters fill the dungeon: the imp and gremlin that were in the entrance hall, plus 39 more, for 41 in all. They wait in groups by room and get tougher deeper in:

- Imps and gremlins near the entrance.
- Imp fiends (big imps) and Puglin brutes (big gremlins) in the hall, crypt and antechamber.
- The Puglin Warlord: 2.4 times a gremlin's size, AC 16, 110 Hit Points, a 3d8+4 axe.

A monster wakes when it sees you within 8 metres, or 13 for the Warlord, and never through a wall. It then pursues you with the same SRD melee as the village's. Nameplates and aimed abilities also need a line of sight. The player and camera are stopped by every wall, arch, column and solid piece of furniture.

The follow camera checks the village's nearby building collision boxes along
the full boom, including the shoulder offset. It pulls in immediately when a
wall blocks the view and eases back out after the obstruction clears.

A watchmaker’s workshop stands east of spawn at (15, -5). Its multi-storey
building has a clock on the front gable, workbenches and storage on two floors,
and a walkable stair between them. The clock is decorative and does not tell
time.

Your home is a one-room wooden house about 20 metres west of spawn, centred
at (-20, -5). Its open doorway faces spawn at (-18, -4): walk west from spawn
and go straight in. Inside, a workbench stands against the back wall, with a
candlestick on it, a wall lantern, a barrel of apples and a crate. The rest of
the floor is left clear. The workbench has the stable id `home/workbench` and
a place to stand in front of it at (-20.25, -5.6). It will become the crafting
table and the place you command golems from
([Golemancy](../../everglade/docs/golemancy-profession-spec.md)); crafting is not
built yet. The parts live where they belong:

- Engine: `aethyr/world/building.coax` places a building at a frame turned in
  quarters and holds the 4 m one-room house's wall boxes, its doorway and the
  way a monster walks round to the doorway. The neighbourhood's houses use the
  same boxes. `aethyr/world/interact.coax` holds interactable spots: a stable
  id, a place to stand, a facing and a reach.
- Game: `everglade/world/home.coax` is the home itself: its kit pieces, its
  furniture and the workbench's spot. Any Everglade zone can place it.
- Zone wiring: `verse/experimental-terrain.coax` places it (`exp_home`), and
  the village, props, hostiles and the scene packer read that frame.

The walls and the workbench stop you and the monsters, the doorway is open,
and a monster following you inside walks round to the doorway. The house uses
Medieval Village MegaKit pieces (worn-wood walls, a door wall with an open
door, two glazed windows, a dark wood floor, a round-tile roof with wooden
gables), and its furniture comes from Fantasy Props. Both kits are CC0.

The [scene declaration](../../assets/registry/scenes/everglade-experimental.json)
pins the Nature and Village asset revisions, the archived source scene, extracted
layout, wardrobe pack, generators, shader and runtime bytes. Rebuild from the
shared library (the archived scene is read as data; its scripts are never run):

```sh
nature_root="$(coax/web/tool.sh verse/tools/assets/registry.coax source nature "$HOME/.openagents/assets")"
village_root="$(coax/web/tool.sh verse/tools/assets/registry.coax source village "$HOME/.openagents/assets")"
props_root="$(coax/web/tool.sh verse/tools/assets/registry.coax source fantasy-props "$HOME/.openagents/assets")"
food_root="$(coax/web/tool.sh verse/tools/assets/registry.coax source ultimate-food "$HOME/.openagents/assets")"
rpg_root="$(coax/web/tool.sh verse/tools/assets/registry.coax source ultimate-rpg "$HOME/.openagents/assets")"
dungeon_root="$(coax/web/tool.sh verse/tools/assets/registry.coax source dungeon "$HOME/.openagents/assets")"
animal_root="$(coax/web/tool.sh verse/tools/assets/registry.coax source ultimate-animals-static "$HOME/.openagents/assets")"
animal_original="$(coax/web/tool.sh verse/tools/assets/registry.coax source ultimate-animals-source "$HOME/.openagents/assets")"
animal_frames="$HOME/.openagents/assets/cache/ultimate-animals-run"
monster_root="$(coax/web/tool.sh verse/tools/assets/registry.coax source bestiary-dungeon "$HOME/.openagents/assets")"
ual_root="$(coax/web/tool.sh verse/tools/assets/registry.coax source ual1 "$HOME/.openagents/assets")"
monster_frames="$HOME/.openagents/assets/cache/bestiary-dungeon-scene"
/Applications/Blender.app/Contents/MacOS/Blender --background --factory-startup --disable-autoexec \
  --python-exit-code 1 --python verse/tools/assets/animal_bake.py -- "$animal_original" "$animal_frames"
/Applications/Blender.app/Contents/MacOS/Blender --background --factory-startup --disable-autoexec \
  --python-exit-code 1 --python verse/tools/assets/bestiary_bake.py -- "$monster_root" "$monster_frames" \
  "$ual_root/Unreal-Godot/UAL1_Standard.glb"
unzip -p "$village_root/Engine Projects/Godot.zip" \
  'Godot/addons/quaternius/medievalvillagemegakit/L_SampleScene_1.tscn' > /tmp/everglade-village-source.tscn
coax/web/tool.sh verse/tools/assets/village_layout.coax /tmp/everglade-village-source.tscn \
  > assets/registry/scenes/everglade-experimental-village.layout
coax/web/tool.sh verse/tools/assets/character_glade_pack.coax "$nature_root" \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack \
  apps/openagents-web/realms/everglade-experimental/assets/glade.pack --experimental \
  "$village_root" assets/registry/scenes/everglade-experimental-village.layout "$props_root" \
  assets/registry/scenes/everglade-experimental-showcase.layout "$HOME/.openagents/assets/objects" \
  "$food_root" "$rpg_root" "$dungeon_root" "$animal_root" "$animal_frames" \
  "$monster_root" "$monster_frames" assets/registry/scenes/everglade-experimental-extra-props.layout \
  assets/registry/scenes/everglade-experimental-extra-nature.layout \
  verse/experimental-scene-colliders.coax "$HOME/.openagents/assets/cache/0ad-gltf"
coax/web/tool.sh verse/tools/assets/village_collision.coax "$village_root" \
  assets/registry/scenes/everglade-experimental-village.layout > verse/experimental-village-data.coax
python3 verse/tools/assets/character_glade_oracle.py \
  apps/openagents-web/realms/everglade-experimental/assets/glade.pack \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack --experimental --village --props --showcase --animal-run --bestiary --extra-props --extra-nature
horse_frames="$HOME/.openagents/assets/cache/horse-white-mount"
/Applications/Blender.app/Contents/MacOS/Blender --background --factory-startup --disable-autoexec \
  --python-exit-code 1 --python verse/tools/assets/horse_bake.py -- "$animal_original" "$horse_frames"
coax/web/tool.sh verse/tools/assets/character_glade_pack.coax --append-horse \
  apps/openagents-web/realms/everglade-experimental/assets/glade.pack /tmp/glade-horse.pack "$horse_frames"
mv /tmp/glade-horse.pack apps/openagents-web/realms/everglade-experimental/assets/glade.pack
gzip -n -6 -c apps/openagents-web/realms/everglade-experimental/assets/glade.pack \
  > apps/openagents-web/realms/everglade-experimental/assets/glade.pack.gz
```

Run the oracle before you append the horse: it checks the scene pack's own vertex and index layout, which the appended horse meshes extend.

A full scene pack already holds the build parts. To add them to a pack that
lacks them without a full repack, append them:

```sh
coax/web/tool.sh verse/tools/assets/character_glade_pack.coax --append-build \
  apps/openagents-web/realms/everglade-experimental/assets/glade.pack /tmp/glade-build.pack "$village_root"
```

The append refuses a pack that already has build parts.

The meadow's ground (`C glade/meadow`) is baked from `exp_height`. When a
site moves, bake the ground again in place, check it and write the samples
that the terrain's check reads (`verse/tools/assets/meadow_pack.coax`,
[#13129](https://github.com/OpenAgentsInc/openagents/issues/13129)):

```sh
gzip -dc apps/openagents-web/realms/everglade-experimental/assets/glade.pack.gz > /tmp/glade.pack
coax/web/tool.sh verse/tools/assets/character_glade_pack.coax --replace-meadow /tmp/glade.pack /tmp/glade-new.pack
coax/web/tool.sh verse/tools/assets/character_glade_pack.coax --check-meadow /tmp/glade-new.pack \
  verse/experimental-meadow-baked.coax
gzip -n -6 -c /tmp/glade-new.pack > apps/openagents-web/realms/everglade-experimental/assets/glade.pack.gz
```

`--check-meadow` fails when a packed ground vertex is more than 2 mm off
`exp_height`. The check "the packed meadow ground is exp_height's at every
sampled point" in `verse/experimental-terrain.coax` fails when a site moves
and the ground wasn't baked again.

The glade's grass seeds (`C glade/grass0`, `1` and `2`: 50,690 tufts) are kept
sorted by the 8 m cell they stand in (`verse/experimental-grass-grid.coax`,
[#13147](https://github.com/OpenAgentsInc/openagents/issues/13147)), so the page
draws only the cells in view, nearest first
(`charactercreator.coax` `grass_cull`). A cell draws fewer of its seeds the
farther it is, by the same law as every grass field in the engine
(`verse/everglade2/meadow-grid.coax` `e2_meadow_density`): all of them to 16 m,
a quarter fewer every 12 m past that, none past 140 m, each distance scaled by
the quality's grass reach. The builder sorts the seeds it writes.
After any bake that changes the grass, write where each cell's seeds start and
commit that file with the pack:

```sh
gzip -dc apps/openagents-web/realms/everglade-experimental/assets/glade.pack.gz > /tmp/glade.pack
coax/web/tool.sh verse/tools/assets/grass_cells.coax --sort /tmp/glade.pack /tmp/glade-new.pack \
  verse/experimental-grass-baked.coax
coax/web/tool.sh verse/tools/assets/grass_cells.coax --check /tmp/glade-new.pack verse/experimental-grass-baked.coax
gzip -n -6 -c /tmp/glade-new.pack > apps/openagents-web/realms/everglade-experimental/assets/glade.pack.gz
```

`--sort` leaves a sorted pack as it is. A pack whose seed counts differ from
the file's is drawn whole, every seed, as before the sort.

The Archery Range realm ([spec](../../everglade/docs/archery-range-spec.md))
draws its butts, sky faces, posts and chalk line from the same pack, as one mesh
(`C glade/range`, built by `everglade/range/place.coax`). Append it the same way,
then compress the pack again:

```sh
coax/web/tool.sh verse/tools/assets/character_glade_pack.coax --append-range \
  apps/openagents-web/realms/everglade-experimental/assets/glade.pack /tmp/glade-range.pack
```

The scene packer also writes the placed assets’ world-space collision bounds to `verse/experimental-scene-colliders.coax`. Tree collision uses trunk geometry, and living hostiles use moving body capsules. Rebuild the bounds whenever the scene pack changes.

A white horse stands just ahead of the spawn at (4, 6). Walk up to it and press
**E** (or tap MOUNT on a phone) to ride: W and S go and slow, A and D turn, Shift
gallops at 14 m/s against 7.5 m/s running on foot, and E gets you off. It stays
where you leave it. You can draw and loose the Longbow from the saddle. The
horse is Quaternius's CC0 Horse_White, lightened to white and baked by
`verse/tools/assets/horse_bake.py` (Idle, Walk and Gallop, appended to the pack
as `glade/horse0` to `glade/horse35`). See the
[mounts spec](../../everglade/docs/mounts-spec.md) for the engine
(`aethyr/action/mount.coax`, `aethyr/action/ride-pose.coax`), game
(`everglade/mounts.coax`: the horse, the dragon and the rider's keys;
`everglade/zone-mounts.coax`: mounts standing in a zone, E, the saddle and the
prompt; `everglade/zone-ride.coax`: a zone's keys handed to them and the
dragon's Fire Breath) and zone (`apps/openagents-web/experimental-stages.coax`:
the film stages) layers. Mounting and getting off go through the action
pipeline (`aethyr/action/deed.coax`): a press asks with your conditions, so you
can't mount while unconscious, paralyzed or grappled. To look at riding without
playing, add `?ride=1` (in the saddle), `?ride=2` (galloping) or `?ride=3&class=7`
(a Ranger drawing the bow in the saddle) to a local page.

A red dragon stands in the open ahead and to the left of spawn, facing back over
the village. E mounts it from its flank; you sit at the base of its neck. Space
takes off, beats the wings and (held) climbs, Shift dives, A and D turn it (level, under you),
and X lands. E in the air says "Land first". F, or the Fire Breath slot the bar
gains while you ride the dragon, breathes fire ahead of it, on the ground or in the
air: the SRD Adult Red Dragon's 60-foot cone from its jaws along its heading, tipped
down as far as you look down. The dragon joins the battle as its own creature, so the
breath acts on its own turn through the shared rules: a DC 21 Dexterity save, 59 (17d6)
fire damage, half on a success, and Burning on a failure. It recharges on a 5 or 6
(Recharge 5-6), rolled at the start of each of its rounds; while it is spent, the
slot's picture sweeps over the time to the next roll. The torrent pours from the jaws
for over a second, the grass under the cone catches and is left black, and each monster
it reaches flinches and cries out or falls. The mount's keys show as one small line
under the bar, never over the horse or the dragon. It walks at 10 m/s and flies at 20 m/s
(the SRD Adult Red Dragon's Speed 40 ft and Fly 80 ft), on the shared winged flight
(`aethyr/action/flight.coax`). Four staged shots ride on their own and hide the HUD:
`?stage=dragonback` (up off the meadow and over the village and castle),
`?stage=horseback` (a gallop down the meadow past the autumn trees),
`?stage=dragon-fire` (a swoop down low over a band of gremlins in the meadow west of
spawn, and Fire Breath across them) and `?stage=horse-archer` (a Ranger canters at two
gremlins south-west of the village, reins in and shoots the first with the Longbow until
an arrow hits; each shot is the real SRD attack, so a take can miss before it hits),
recorded with `coax/web/tool.sh clip`. Add `&view=side` or `&view=rear` to dragonback
or horseback to watch the seat close up from beside or behind the rider. An arrow that
hits flashes at the monster's chest.

Experimental uses Everglade’s shared sound bank for ambience, footsteps and combat cues. The browser starts audio on the first click, tap or key; the ambient bed is offered again for visitors who interact after the scene loads.

Every ability has its own sounds (`xc_sound` in `everglade/zone-combat.coax`): a cast or launch sound as you use it (a blow's whoosh as its swing starts), often with a second layer under it, an impact where it lands on a monster, and a fizzle where a spell meets no one or misses. A spell without its own entry sounds like its damage type, or else its school. No two abilities a class has at once, at any level from 1 to 6, share a cast sound. The greataxe and greatsword swing slower and lower than the one-handed swords and land a heavier hit. Imps and gremlins have their own hurt and death sounds. Most sounds are the Everglade bank's picks; the ones it has no file for (Vicious Mockery's warble, Sacred Flame's choir, Rage's roar, the heavy swing, the fizzle, the monsters' voices and a few more) are synthesized from recipes in `aethyr/audio/experimental-sfx.coax` (see `aethyr/audio/SOURCES.md`). Each play takes one of the sound's picks at random and varies its pitch by up to 7% and its level by up to 15%, so a repeat never sounds identical.

The castle spans 56 by 54 metres, with nine towers surrounding four connected courtyards and a central keep. Its main arched entrance is at (34, -20). The walls and towers share a level foundation, with a smooth transition to the surrounding meadow. The walls and towers supply colliders to the player and camera.

Bushes use walk-through foliage volumes at 80% of normal movement pace; their bounds do not block the player or camera. Movement returns to normal outside the foliage.

Nearby solid scene objects with gaps narrower than the player are grouped where a bounded collision footprint fits, reducing the chance of being squeezed between opposing item boxes.

Enemies yield when the player pushes through them. If the player becomes trapped in intersecting props, the walker temporarily releases overlapping props while retaining structural walls; an unresolved structural trap returns the player to the last clear position, or the spawn if none is available.

## Private NWN reference specimen

[Lady Aribeth's installed NWN model](everglade-experimental-aribeth.md) has been
rendered in a private local copy of this world's scene pack. The note records
its original model/texture identities, conversion tools, actual headless render
and limitations. The public scene pack does not include the NWN payloads.

[Ten private NWN comparison pairs](everglade-experimental-comparison.md) add
labeled props, furniture, vegetation, a weapon and a creature beside this realm's
existing Quaternius counterparts in the local specimen pack.

[Three private NWN terrain sections](everglade-experimental-biomes.md) compare
native campaign forest, farmland and desert tiles in small patches at
the northern meadow edge, retaining the rest of the existing grass and scenery.

[Aribeth's four native animations](everglade-experimental-aribeth-animations.md)
now cycle in the local specimen scene, with an on-character clip label and a
close local view. The biome sections and comparison gallery remain present.
