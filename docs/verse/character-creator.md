# Character Studio

`/worlds/character-creator` is a live Aethyr wardrobe for the Quaternius humans.
It uses the existing people renderer and animation clips, with an independent
appearance save slot. Its controls and appearance rules are Coax in
`apps/openagents-web/charactercreator.coax`; `fivelab.coax` dispatches this exact
page to the studio instead of starting a battle.

## The complete Fantasy Source wardrobe

The input is `~/Downloads/Modular Character Outfits - Fantasy[Source]`.
Its `All_Male.blend` contains 39 individual clothing meshes and its
`All_Female.blend` contains 37. All 76 are included, with three base-color
textures for each of Knight, Noble, Peasant, Ranger and Wizard (15 images).
The standalone glTF directory has 64 grouped exports; using only those would
lose independent controls for some guards, belts, bracers and ornaments.
Rig widgets and the source fitting bodies are not clothing assets.

`MODULAR PART` browses aligned garment slots; `WEAR SELECTED PART` toggles the
piece. Body B has no counterpart for the separate Noble arm guards and Noble
belt. Those slots skip while browsing Body B and remain hidden after a body
switch. Five presets choose complete outfits; individual pieces can then be
mixed freely. Alternative armor/cloth, shoulder and belt variants can overlap
when enabled together, so disable the unwanted alternative. This is explicit
modular assembly, not a cloth simulation or automatic garment fitting system.

The studio reuses the existing head, hairstyles, beard, 65-joint body palettes
and idle/walk/run/cast clips. A closed helmet suppresses head and hair; a hood
suppresses hair. Skin and hair multiply the corresponding materials' color.
Cloth dyes use the existing pale/green dye masks, preserving texture detail;
NO DYE shows the original colorway. Metal and saturated decorative regions are
not uniformly repainted by that mask.

Tab selects a control, arrows change it, and Space cycles animations. Drag
turns the model and adjusts the viewing pitch; the wheel zooms. Controls form
two side columns on desktop and a two-column panel below the preview on narrow
screens. The 54-byte `CC1` appearance record saves through the host's existing
action 4/IndexedDB save slot and restores on message 4. It contains no asset
addresses or renderer state. Invalid records leave the current appearance intact.

## Rebuild the asset pack

The checked-in pack and provenance receipt are in
`apps/openagents-web/realms/character-creator/assets/`. The exporter reads the
source Blender files with embedded scripts disabled, evaluates clothing in the
armature rest pose, preserves UVs/normals/skin weights, and uses the existing
people vertex encoder. It retains the base pack's checked heads/hair and appends
the Source clothing and 512px base-color layers. Identical encoded vertices are
shared within a piece. Normal and ORM files remain source authoring inputs;
the existing Aethyr people shader uses base color and stylized lighting.

```sh
'/Applications/Blender.app/Contents/MacOS/Blender' --background --disable-autoexec \
  --python verse/tools/humans/pack_fantasy_source.py -- \
  "$HOME/Downloads/Modular Character Outfits - Fantasy[Source]" \
  apps/openagents-web/realms/everglade/assets/everglade.pack \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack
```

The receipt records source `.blend` hashes, every piece, palette joint names,
all 15 source texture hashes, the base pack hash, and the final pack hash.
The source license is CC0 1.0 Universal (`License_Source.txt`).

## Build and check

```sh
coax/bin/coax test apps/openagents-web/charactercreator.coax
coax/bin/coax test aethyr/render/adoption.coax
coax/bin/coax mutate aethyr/render/adoption.coax --root-only
coax/web/tool.sh coax/web/wgsl-module.coax \
  apps/openagents-web/gpu/fivelab.coax apps/openagents-web/fivelab.wgsl \
  apps/openagents-web/gpu/fivelab.module --check
COAXIAL_MODE=dev coax/web/wasm.sh \
  apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c \
  apps/openagents-web/build/fivelab.wasm \
  apps/openagents-web/everglade-step.coax aethyr/sticks.coax
# Generate page metadata as the normal site build does.
bun coax/web/realms.ts
coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:8817 --static
# With the server running in another terminal, checks real input and reload.
coax/web/tool.sh coax/web/tests/charactercreator.coax \
  http://127.0.0.1:8817/worlds/character-creator
```

Rendering adoption binds the exact `character-creator`/`fivelab` pair as an
explicit legacy consumer; it does not claim a qualified shared/native renderer
profile. The change does not alter existing battle selection or its look rules.

The shared fivelab Wasm budget is 625,000 bytes after a measured 590,971-byte
release build with the wardrobe UI and save codec (about 5% headroom). The
asset pack is separately bounded to 32 MiB.
