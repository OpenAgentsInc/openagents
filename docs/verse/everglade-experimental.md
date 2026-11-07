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

The experimental pack's showcase profile raises its budget to 128 MiB and
adds fantasy-appropriate Ultimate Food models, 106 Ultimate RPG models,
48 Modular Dungeon pieces and 12 Ultimate Animated Animals. Food and RPG
models stand in rows south of spawn, animals run in place to their west, and dungeon
pieces are displayed in the southeast meadow. The animals use ten baked
poses of the original Gallop clips, including bone-parented antlers. The showcase
pieces do not yet supply collision shapes.
Modern packaged foods and fast food are omitted from the world; their source
assets remain in the shared registry. Their reserved layout slots are left
empty so the other specimens keep their positions.

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
/Applications/Blender.app/Contents/MacOS/Blender --background --factory-startup --disable-autoexec \
  --python-exit-code 1 --python verse/tools/assets/animal_bake.py -- "$animal_original" "$animal_frames"
unzip -p "$village_root/Engine Projects/Godot.zip" \
  'Godot/addons/quaternius/medievalvillagemegakit/L_SampleScene_1.tscn' > /tmp/everglade-village-source.tscn
coax/web/tool.sh verse/tools/assets/village_layout.coax /tmp/everglade-village-source.tscn \
  > assets/registry/scenes/everglade-experimental-village.layout
coax/web/tool.sh verse/tools/assets/character_glade_pack.coax "$nature_root" \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack \
  apps/openagents-web/realms/everglade-experimental/assets/glade.pack --experimental \
  "$village_root" assets/registry/scenes/everglade-experimental-village.layout "$props_root" \
  assets/registry/scenes/everglade-experimental-showcase.layout "$HOME/.openagents/assets/objects" \
  "$food_root" "$rpg_root" "$dungeon_root" "$animal_root" "$animal_frames"
coax/web/tool.sh verse/tools/assets/village_collision.coax "$village_root" \
  assets/registry/scenes/everglade-experimental-village.layout > verse/experimental-village-data.coax
python3 verse/tools/assets/character_glade_oracle.py \
  apps/openagents-web/realms/everglade-experimental/assets/glade.pack \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack --experimental --village --props --showcase --animal-run
```
