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
piece. Female has no counterpart for the separate Noble arm guards and Noble
belt. Those slots skip while browsing Female and remain hidden after a body
switch. Peasant, Ranger, Wizard, Noble and original Tideglass presets choose complete outfits; individual pieces can then be
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
screens. The 56-byte `CC2` appearance record saves through the host's existing
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

## Tideglass and selector revision

Heavy armor (Knight armor, sleeves, boots, helmet, horns and legs, metal shoulder
pieces, gorget and arm guards) is excluded from browsing and drawing. Source
payloads remain in the pack for provenance. Legacy CC1 saves migrate with heavy
pieces disabled; CC2 adds the original outfit slot and six eye colors. Eye color
changes only the brown iris mask on the existing eye texture, retaining pupils,
whites and highlights. The studio instance marker isolates these fields from
battle status effects. Male/Female and Color labels are used throughout the UI.

Tideglass is two skinned catalog entries with the existing 24-byte people vertex
format and 65-joint animation palettes. Source sleeves are returned to rest pose;
new cape, seams and map panels borrow nearest garment weights. The mask binds
rigidly to Head. The Male fit widens the design by 5.5%; this is a stylized fit,
not simulated cloth. The original hood/mask are bundled with the outfit.

The authored Blender scene remains at
`~/ore-wow-private/original-clothing/tideglass-wayfarer.blend`. After rebuilding
the unmodified Fantasy pack, append it with:

```sh
'/Applications/Blender.app/Contents/MacOS/Blender' --background --disable-autoexec \
  --python verse/tools/humans/pack_tideglass.py -- \
  "$HOME/ore-wow-private/original-clothing/tideglass-wayfarer.blend" \
  "$HOME/Downloads/Modular Character Outfits - Fantasy[Source]" \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack
```

The receipt includes the design SHA-256, per-fit geometry counts and final pack
identity. No Blender runtime is used in the selector.

## Face inspection and headwear

The page header is “Character Create”. Crown is excluded from browsing, drawing
and restored appearances. A hood suppresses hair while retaining a selected
Male beard as a separate draw. Tideglass's closed mask still hides facial parts.
Zoom now reaches 0.65 m (formerly 2.2 m); below 2.2 m, the camera target rises
toward the face. The upper distance stays 6 m. The headless Metal capture
`docs/verification/character-creator-2026-11-06/hood-beard-close.png` shows the
actual hood, beard and closest camera together. Eleven targeted creator/studio
checks pass; the release producer is 592,520 bytes.
