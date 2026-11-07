# Everglade Experimental meadow

The unlisted experimental world uses a separate Nature glade pack. Its 180-metre
square ground keeps Character Create's terrain heights, sky, grass meshes,
lighting and central planting. Extra grass concentrates around the former
boundary and extends into the outer meadow; rocks and the distant town are
omitted. Walking is bounded eight metres inside the ground's edge.

The [scene declaration](../../assets/registry/scenes/everglade-experimental.json)
pins the source asset revisions, source scene, wardrobe pack, generator and
runtime bytes. Rebuild it from the shared asset library:

```sh
nature_root="$(coax/web/tool.sh verse/tools/assets/registry.coax source nature "$HOME/.openagents/assets")"
coax/web/tool.sh verse/tools/assets/character_glade_pack.coax "$nature_root" \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack \
  apps/openagents-web/realms/everglade-experimental/assets/glade.pack --experimental
python3 verse/tools/assets/character_glade_oracle.py \
  apps/openagents-web/realms/everglade-experimental/assets/glade.pack \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack --experimental
```
