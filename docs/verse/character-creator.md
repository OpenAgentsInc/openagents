# Character Studio

`/worlds/character-creator` is a live Aethyr wardrobe for the Quaternius humans.
It uses the existing people renderer and animation clips, with an independent
appearance save slot. Its controls and appearance rules are Coax in
`apps/openagents-web/charactercreator.coax`; `fivelab.coax` dispatches this exact
page to the studio instead of starting a battle.

## The complete Fantasy Source wardrobe

The input is the pinned Fantasy Source snapshot in `~/.openagents/assets/originals/`
(see `assets/registry/packages/fantasy.json`).
Its `All_Male.blend` contains 39 individual clothing meshes and its
`All_Female.blend` contains 37. All 76 are included, with three base-color
textures for each of Knight, Noble, Peasant, Ranger and Wizard (15 images).
The standalone glTF directory has 64 grouped exports; using only those would
lose independent controls for some guards, belts, bracers and ornaments.
Rig widgets and the source fitting bodies are not clothing assets.

`MODULAR PART` browses aligned garment slots; `WEAR SELECTED PART` toggles the
piece. Female has no counterpart for the separate Noble arm guards and Noble
belt. Those slots skip while browsing Female and remain hidden after a body
switch. Peasant, Ranger, Wizard and Noble presets choose complete outfits; individual pieces can then be
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
SOURCE="$(coax/web/tool.sh verse/tools/assets/registry.coax source fantasy "$HOME/.openagents/assets")"
'/Applications/Blender.app/Contents/MacOS/Blender' --background --disable-autoexec \
  --python verse/tools/humans/pack_fantasy_source.py -- \
  "$SOURCE" \
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

## Selector appearance controls

Heavy armor and Crown are excluded from browsing and drawing; their original
Source payloads remain for provenance. Legacy saves migrate with those pieces
disabled. Six eye colors retain the texture's pupils, whites and highlights.
Male/Female and Color labels are used throughout the UI. The 40th garment slot
is reserved and unavailable; the CC2 appearance format remains compatible.

## Face inspection and headwear

The page header is “Character Create”. Crown is excluded from browsing, drawing
and restored appearances. A hood suppresses hair while retaining a selected
Male beard as a separate draw.
Zoom now reaches 0.65 m (formerly 2.2 m); below 2.2 m, the camera target rises
toward the face. The upper distance stays 6 m. The headless Metal capture
`docs/verification/character-creator-2026-11-06/hood-beard-close.png` shows the
actual hood, beard and closest camera together. Eleven targeted creator/studio
checks pass; the release producer is 592,520 bytes.

## Camera panning

Left mouse drag rotates; right mouse drag pans, with no mode toggle. On touch
screens, tap DRAG: ROTATE / DRAG: PAN or press P to toggle the drag behavior. Pan mode
translates the camera in its screen plane without changing character yaw or
view pitch. Translation uses camera distance and focal length, so panning stays
proportional to pointer travel when zoomed in. Both axes are bounded to two
metres. RESET centers the camera and returns to Rotate mode; pan is temporary
view state and is not part of the saved appearance. The touch mode control sits next to Eye Color on phones. Mouse gestures ignore
the touch mode. The optional `coaxial_pointer_buttons_v1` browser input hook
carries the DOM button number; the original pointer ABI remains unchanged.
The right button never changes appearance controls, and the initial button is
latched until its release or cancellation.

Only Peasant, Ranger, Wizard and Noble remain. The removed custom outfit's
geometry, palette textures and exporter are gone. Existing CC2 saves selecting
that removed slot migrate to Wizard while retaining body and eye color.

## Shared asset registry

The complete source packs now have a shared authoring catalog in
`assets/registry/README.md`, backed by `~/.openagents/assets/` source snapshots,
immutable artifacts and editable Blender forks. The wardrobe's stable IDs,
labels, availability and preset membership live in
`assets/registry/collections/character-create.json`. Regenerate the runtime
Coax tables and pinned revision lock with:

```sh
coax/web/tool.sh verse/tools/assets/registry.coax wardrobe
coax/web/tool.sh verse/tools/assets/registry.coax wardrobe --check
```

CC2's legacy slots remain frozen compatibility keys. Source discovery order
cannot change saved garments. Runtime readiness now requires each referenced
mesh, rather than an exact manually maintained count. The existing compiled
people pack, appearance codec and renderer ABI are unchanged.

## Nature glade background

The character now stands in the real 3D Nature glade described in
[character-glade.md](character-glade.md), composed against Quaternius's
`Preview_1.jpg`. Character Create loads `assets/glade.pack`, which preserves
all original wardrobe data and appends the static environment. Camera pitch,
zoom and pan apply to the environment; clothing and saved appearance retain
their existing behavior. The shared asset IDs and provenance are pinned in
`assets/registry/scenes/character-glade.json`.

## Main font: AR Kai C

Character Create uses the installed WoW Forever `arkai_c.ttf` (AR ZhongkaiGBK
Medium), identified by ore's `docs/wow-forever/fonts.md`. The original decoded
SHA-256 is `c1cabeae2ef13f2984221d37307639b47571e22edaa91ba5d938fc4871581396`.
The asset receipt retains the FileDataID, current archive extent and original
identity. The source has an invalid `vhea` version (0x00010001); the Coax tool
`verse/tools/humans/character_font.coax` changes it to 0x00010000 and recomputes
table/global checksums. Outlines, advances, cmap and names remain byte-identical.
Run it with `coax/web/tool.sh FILE.coax ORIGINAL.ttf WEB.ttf`.

The page manifest supplies its FontFace source and family. The browser loads it
before GPU setup and rasterizes the five existing atlas mip sizes. Character
Create lays every heading and control out in Coax using the source's exact
ASCII advances (`charactercreator-font.coax`), with the existing textured-glyph
vertex format. The tracked legacy fivelab shader now samples those glyphs beside
its frame-storage binding; battle text remains Filament. The shared canonical
renderer and font profile are unchanged. This is not a new native/shared font
qualification. Device recreation reuses the loaded FontFace and rebuilds its
atlas. Failure to load the requested font refuses startup instead of silently
substituting a different face.

Headless browser checks require the loaded family and exact measured advances
for A/i, then exercise input, save/reload and desktop/phone previews.
