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
texture atlas and a 64 MiB pack budget; the original Character Create pack
retains its 32 MiB budget.

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
unzip -p "$village_root/Engine Projects/Godot.zip" \
  'Godot/addons/quaternius/medievalvillagemegakit/L_SampleScene_1.tscn' > /tmp/everglade-village-source.tscn
coax/web/tool.sh verse/tools/assets/village_layout.coax /tmp/everglade-village-source.tscn \
  > assets/registry/scenes/everglade-experimental-village.layout
coax/web/tool.sh verse/tools/assets/character_glade_pack.coax "$nature_root" \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack \
  apps/openagents-web/realms/everglade-experimental/assets/glade.pack --experimental \
  "$village_root" assets/registry/scenes/everglade-experimental-village.layout "$props_root"
coax/web/tool.sh verse/tools/assets/village_collision.coax "$village_root" \
  assets/registry/scenes/everglade-experimental-village.layout > verse/experimental-village-data.coax
python3 verse/tools/assets/character_glade_oracle.py \
  apps/openagents-web/realms/everglade-experimental/assets/glade.pack \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack --experimental --village --props
```
