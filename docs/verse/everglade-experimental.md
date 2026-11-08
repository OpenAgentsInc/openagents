# Everglade Experimental meadow

The unlisted experimental world uses a separate Nature glade pack. Its 180-metre
square ground keeps Character Create's sky, grass meshes, lighting and central
planting. Extra grass extends into the meadow, and rocks and the original
background town are omitted. Walking is bounded eight metres inside the ground's
edge.

The world contains the four distinct layouts from Quaternius's Medieval Village
Godot sample scene: House C, Tower, Houses A, and Houses D. House C2 is an exact
local-layout duplicate of House C and is not duplicated. Nine placements combine
these layouts into larger settlements, using 1,400 source-authored pieces with
their original transforms, meshes and base-color textures. Buildings supply
nearby collision shapes to the Everglade walker: the source
collision meshes become individual axis-aligned boxes, and the interior stairs
become sloped walkable surfaces. Non-box hulls are approximated by their bounds;
interactive doors have not yet been authored. The homes and their surroundings
are furnished with 112 placements of 20 Fantasy Props Standard models; these props
are visual and do not yet have separate collision shapes. The terrain is levelled under the
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
through six compatible Universal Animation Library 1 motions every three seconds. Three additional imps and three gremlins pursue the player and make timed melee attacks using the SRD combat rules. Player health is shown on a bar, hits raise damage numbers, and after the death animation the enemies return toward their starting places. Press R or click the respawn prompt to return to spawn at full health; defeated monsters stay defeated. Their original
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
Thirty-six additional flower, plant, fern, clover, mushroom, bush and grass variants grow in
the eastern meadow. The remaining sixty nature models, including trees, rocks and paths,
stand in display rows in the northwestern meadow. The experimental pack budget is 512 MiB
and accommodates up to 128 texture layers. Its deterministic gzip transport is
tracked in Git, and the browser decompresses it before reading the pack.

A 20-by-20-metre open-roof stone dungeon stands northwest of spawn, with its entrance at (-32, -44). Five arches connect three halls furnished with columns, torches, treasure and dungeon decorations. An imp and a gremlin wait until a visitor comes near. The walls and columns stop the player and the camera.

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
  assets/registry/scenes/everglade-experimental-extra-nature.layout
gzip -n -6 -c apps/openagents-web/realms/everglade-experimental/assets/glade.pack \
  > apps/openagents-web/realms/everglade-experimental/assets/glade.pack.gz
coax/web/tool.sh verse/tools/assets/village_collision.coax "$village_root" \
  assets/registry/scenes/everglade-experimental-village.layout > verse/experimental-village-data.coax
python3 verse/tools/assets/character_glade_oracle.py \
  apps/openagents-web/realms/everglade-experimental/assets/glade.pack \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack --experimental --village --props --showcase --animal-run --bestiary --extra-props --extra-nature
```
