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

## Save Player and use it in Everglade

SAVE PLAYER explicitly commits the current character to this browser's active
Aethyr player appearance. USE IN EVERGLADE saves the current choices and opens
Everglade only after storage confirms the exact saved record. Automatic draft
saving remains separate: trying another outfit or pressing RESET does not
replace the active player until explicitly saved. PLAYER SAVED confirms the
write; RETRY SAVE reports refused browser storage and never navigates away.

The versioned origin-wide key is `aethyr.player.appearance.v1`. Its bounded
payload is the existing CC2 record, parsed by `aethyr/character-appearance.coax`
in both the creator and Everglade. Invalid records leave live choices intact.
This is browser-local appearance, independent of login identity, quest saves,
room permissions, movement, spell stats or inventory. It is not account/cloud
sync; other clients retain the existing multiplayer LOOK contract.

The optional `coaxial_player_appearance_v1` capability opts a face into loading
message16 before page facts. Action16 writes the shared profile; message17
acknowledges the exact bytes or an empty failure. The UI keeps a pending
snapshot, so another edit cannot be mistaken for the acknowledged selection.

Everglade's appended `everglade-avatar.pack` preserves its original catalog,
vertices, indices and texture layers, then adds all76 source garments. It uses
the existing 24-byte people vertices,65-joint male/female rigs, three colorways
and cloth dye rules. The player uses Everglade's existing feet, facing, world
collision and camera. Its body samples the people rig's idle/walk/run and spell
clips; the normal Ranger and its gear are hidden for that local actor. When no
valid complete appearance is available, the ordinary Ranger remains visible.
Eye color uses the palette's previously unused hair-alpha word; NPC palette
extents and all existing joint bounds stay unchanged.

The pack writer runs the production `pack_read` before writing. It emits VPK2
for the enlarged texture table and retains sequential CC0 source declarations.
Build the normal local room worker with `coax/web/tool.sh coax/web/room/worker.coax
apps/openagents-web/build/everglade-room.wasm`. Everglade requires an admitted
room seat for movement, including its default one-seat browser worker. A native
development room and API are available with `coax/web/tool.sh coax/web/room/local.coax
--room 8847 --page 8817 --static`.

Rebuild and independently validate the appended wardrobe:

```sh
coax/web/tool.sh verse/tools/assets/wardrobe_pack.coax \
  apps/openagents-web/realms/everglade/assets/everglade.pack \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack \
  apps/openagents-web/realms/everglade/assets/everglade-avatar.pack
python3 verse/tools/assets/wardrobe_pack_oracle.py \
  apps/openagents-web/realms/everglade/assets/everglade.pack \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack \
  apps/openagents-web/realms/everglade/assets/everglade-avatar.pack
coax/web/tool.sh coax/web/tests/player-appearance.coax \
  http://127.0.0.1:8817/worlds/character-creator
```

## Entering Everglade after creation

A valid saved Character Create profile is already a completed appearance choice.
Everglade must join directly without automatically opening its legacy Ranger
name/class/skin/cloak/trim picker. The server still owns the name and room seat.
This works whether the profile loads before or after the room ticket; a late
valid profile releases only the automatic picker hold. Invalid records do not
release it, and an explicitly opened in-world picker remains manual.

The browser handoff test waits for an actual room seat without pressing Enter
to dismiss a dialog, and rejects any automatic picker on reload.

## Compact sidebar

Desktop appearance choices, Reset/Surprise Me and Save/Use actions share one
compact left sidebar. Rows are 40 display pixels apart with 36-pixel hit areas.
Labels and values sit on one line; the right side remains unobstructed. Narrow
or short layouts use the existing single bottom panel.

The tinted panel is 75% opaque: the private post pass composites it over the
fully rendered world, then the shared host draws crisp HUD strokes on the final
canvas. It does not use an opaque depth mask. Scene-only comparison mode omits
the panel and all controls. Appearance records and compatibility keys remain
unchanged. See `docs/verification/character-create-sidebar-2026-11-06/`.
