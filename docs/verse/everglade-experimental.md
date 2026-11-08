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

## Fight back

You fight the monsters with your class's level 1 kit
(`apps/openagents-web/experimental-combat.coax`, #13025).

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
- The SRD 5e engine (`aethyr/rules/srd/`, through `fivebar.coax`) rolls every attack, save, and
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
  the bake if the weapon comes closer. Tests in `apps/openagents-web/charactercreator.coax`
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
  verse/experimental-scene-colliders.coax
gzip -n -6 -c apps/openagents-web/realms/everglade-experimental/assets/glade.pack \
  > apps/openagents-web/realms/everglade-experimental/assets/glade.pack.gz
coax/web/tool.sh verse/tools/assets/village_collision.coax "$village_root" \
  assets/registry/scenes/everglade-experimental-village.layout > verse/experimental-village-data.coax
python3 verse/tools/assets/character_glade_oracle.py \
  apps/openagents-web/realms/everglade-experimental/assets/glade.pack \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack --experimental --village --props --showcase --animal-run --bestiary --extra-props --extra-nature
```

The scene packer also writes the placed assets’ world-space collision bounds to `verse/experimental-scene-colliders.coax`. Tree collision uses trunk geometry, and living hostiles use moving body capsules. Rebuild the bounds whenever the scene pack changes.

Experimental uses Everglade’s shared sound bank for ambience, footsteps and combat cues. The browser starts audio on the first click, tap or key; the ambient bed is offered again for visitors who interact after the scene loads.

Every ability has its own sounds (`xc_sound` in `apps/openagents-web/experimental-combat.coax`): a cast or launch sound as you use it (a blow's whoosh as its swing starts), often with a second layer under it, an impact where it lands on a monster, and a fizzle where a spell meets no one or misses. A spell without its own entry sounds like its damage type, or else its school. No two abilities a class has at once, at any level from 1 to 6, share a cast sound. The greataxe and greatsword swing slower and lower than the one-handed swords and land a heavier hit. Imps and gremlins have their own hurt and death sounds. Most sounds are the Everglade bank's picks; the ones it has no file for (Vicious Mockery's warble, Sacred Flame's choir, Rage's roar, the heavy swing, the fizzle, the monsters' voices and a few more) are synthesized from recipes in `aethyr/audio/experimental-sfx.coax` (see `aethyr/audio/SOURCES.md`). Each play takes one of the sound's picks at random and varies its pitch by up to 7% and its level by up to 15%, so a repeat never sounds identical.

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
