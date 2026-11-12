# Character Studio

`/worlds/character-creator` is a live Aethyr wardrobe for the Quaternius humans,
and it is a place in Everglade II: Character Create's glade is where Everglade II
begins (#13023). The page boots Everglade II itself (`everglade2`, its slot and
pack) in the glade, in Character Create's mode: a studio camera on your
character, the wardrobe down the left, and your keys and pointer on the wardrobe.
You pick one of the twelve SRD 5.2.1 classes on the CLASS row. USE IN EVERGLADE
saves the character, plays a short run out of the glade and down the path, then
hands you the walk. There is one page, one Wasm world, one renderer and one clock
the whole way: nothing loads, nothing fades, and no frame or iframe opens. The
address bar changes to `/worlds/everglade2` in place (`history.replaceState`).

The studio and the run are Coax in `everglade/client/create.coax`,
loaded into `everglade/client/client.coax`. The wardrobe's choices and record are
`charactercreator.coax`'s (`Creator`, `change`, `value`, `save`), with the class
added to the record (`CC3`). The glade is Everglade II's own
(`docs/verse/everglade2.md`).

## Through the portal from the lander at /newlander

The lander at `https://openagents.com/newlander`, the homepage's module with `data-page="3"`
(`newland.coax`, `newland-portal.coax`, `apps/openagents-web/newlander/index.html`), has one
Everglade portal in the Grid and one button, CREATE YOUR CHARACTER. The homepage itself has
neither. A press fades the lander's panels and flies the camera into the portal. As soon as the
portal's light covers the view (about 1.3 s in, `PORTAL_COVER`), the tab goes on to
`https://everglade.world/play?via=portal`: the address bar changes to everglade.world there, and
Everglade's own arrival continues on the same light, with its loading bar, while the engine,
shaders, pack and textures download. The lander loads nothing of Everglade before that. The
browser keeps each site's downloads apart (its HTTP cache is split by the site in the address bar),
so what openagents.com fetched would be fetched again under everglade.world; a headless check of
`rel=prefetch` (plain, `as=document` and `as=fetch`) across two sites confirmed nothing carries over.
Instead the lander's head opens the connection to everglade.world (`preconnect`, `dns-prefetch`) and
fetches the `/play` page ahead with speculation rules, so the tab's navigation starts at once. Back to
a lander the browser kept in memory shows it as it was before the press (message 602).

The old unlisted address of the lander answers 301 to `/newlander` (the Coaxial load balancer's URL
map, `production` path matcher).

The light is one function, `portal_field` in `apps/openagents-web/gpu/portal.coax`,
drawn by both pages at the same scale (`PORTAL_HANDOFF`) and on the same clock
(the wall clock's seconds, from each page's `now=` fact). Until Character
Create's engine draws, its loading screen is the light, moving: a CSS swirl and
rings in the light's colours, run by the compositor while the engine compiles,
with the loading bar low on a dark strip (`"arrival": "portal"` in `realms.json`). A failure still shows the loading screen's message. The engine
then draws the light (`e2_arrival` in `gpu/everglade2.coax`) and holds it until
the glade has drawn 2 frames and the page is in sight (not hidden, message 601). Over the next 2.9 s the glade opens through the
light from the middle, the camera comes round and in to the studio's framing,
your character pops up from its feet, and the wardrobe fades in. A key, click
or scroll skips to the end. A plain visit to `/worlds/character-creator` has no
arrival.

Everglade at `/worlds/everglade` no longer has a Character Create entrance. It
still reads the saved record, class included, when you open it.

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
screens. The appearance record is the 56-byte `CC2` record, or the 57-byte `CC3`
record when a class is chosen (`CC3` is `CC2` plus one class letter, `a` for
Barbarian to `l` for Wizard). It contains no asset addresses or renderer state.
Invalid records leave the current appearance intact.

## Rebuild the asset pack

The checked-in pack and provenance receipt are in
`apps/openagents-web/realms/character-creator/assets/`. The exporter reads the
source Blender files with embedded scripts disabled, evaluates clothing in the
armature rest pose, preserves UVs/normals/skin weights, and uses the existing
people vertex encoder. It retains the base pack's checked heads/hair and appends
the Source clothing and 512px base-color layers. Identical encoded vertices are
shared within a piece. Normal and ORM files remain source authoring inputs;
Character Create uses base color with glade sunlight, geometry shadows, sky/ground
fill and a soft warm front bounce; its private shader stays in linear HDR through
the shared output transform. Normal/ORM sampling is not yet implemented.

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
coax/bin/coax test --library everglade/client/client.coax --only "create:"
coax/bin/coax test --library aethyr/render/adoption.coax
cd apps/openagents-web && ../../coax/web/wasm.sh everglade/client/client.coax everglade2-face.c \
  build/everglade2.wasm everglade-step.coax ../../aethyr/sticks.coax
bun coax/web/realms.ts
coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:8873
```

Rendering adoption binds the `character-creator`/`everglade2` pair as a tracked
legacy consumer, as Everglade II's own page is. The realm table's `place_of` says
the page is a place in Everglade II: it takes Everglade II's module, slot, pack
and `data-realm`, and only its address, its font and its first mode differ
(`coax/web/realms.ts`).

## Save Player and use it in Everglade

SAVE PLAYER explicitly commits the current character to this browser's active
Aethyr player appearance. USE IN EVERGLADE saves the current choices and opens
Everglade only after storage confirms the exact saved record. Automatic draft
saving remains separate: trying another outfit or pressing RESET does not
replace the active player until explicitly saved. PLAYER SAVED confirms the
write; RETRY SAVE reports refused browser storage and never navigates away.

The versioned origin-wide key is `aethyr.player.appearance.v1`. Its bounded
payload is the CC2 or CC3 record, parsed by `aethyr/character-appearance.coax`
in both the creator and Everglade. Invalid records leave live choices intact.
This is browser-local appearance, independent of login identity, quest saves,
room permissions, movement or inventory. The class it carries sets your bar in
Everglade (see "Classes"). It is not account/cloud
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

A valid saved Character Create record is already a completed appearance choice.
Everglade II restores it on its own page (message 16) and draws you in it, so a
reload of `/worlds/everglade2` keeps your look and class. There is no room or
seat in Everglade II.

## Classes

The CLASS row is the first row of the sidebar. It offers the twelve classes of
the SRD 5.2.1: Barbarian, Bard, Cleric, Druid, Fighter, Monk, Paladin, Ranger,
Rogue, Sorcerer, Warlock and Wizard. It starts at Druid. The class is saved in
the character record (`CC3`), so a reload keeps it, and Everglade restores it
with the rest of your appearance.

Each class is a level 1 character built by the SRD 5e engine
(`everglade/shared/fivebar.coax` `fb_cl_hero`), with the class's Hit Points,
Armor Class, ability scores and spell save DC. Its bar has one ability in slot 1
(key 1) and Repair on R:

| Class | Slot 1 | Kind |
| --- | --- | --- |
| Barbarian | Rage | Bonus Action, on you |
| Bard | Vicious Mockery | Cantrip, 60 ft |
| Cleric | Sacred Flame | Cantrip, 60 ft |
| Druid | Starry Wisp | Cantrip, 60 ft |
| Fighter | Second Wind | Bonus Action, on you |
| Monk | Martial Arts | Unarmed Strike, 5 ft |
| Paladin | Lay On Hands | Bonus Action, on you |
| Ranger | Hunter's Mark | 1st-level spell, 90 ft |
| Rogue | Sneak Attack | Shortbow, 80 ft |
| Sorcerer | Fire Bolt | Cantrip, 120 ft |
| Warlock | Eldritch Blast | Cantrip, 120 ft |
| Wizard | Fire Bolt | Cantrip, 120 ft |

An ability that acts on you goes at once. An aimed one is picked with 1; a
click, a tap or 1 again casts it where you aim. The page rolls nothing: the cast
goes to the room (`C` with slot 96 + class), the room's 5e engine resolves it
(`everglade5e-room-rules.coax`), and every page plays its look when the room's
cast event arrives. Offline, the local room worker does the same. The room
refuses any other slot for a class's seat. Each slot's card is the SRD's text
for what the slot does here.

The page's side is `apps/openagents-web/everglade-classes.coax`. The hello's
`KIND` word is `srd-barbarian` to `srd-wizard`. To show a class's bar without
Character Create, add `?kit=srd-NAME` to Everglade's address; `&card=1` also
shows slot 1's card. The icons and cards are generated with
`coax/web/tool.sh verse/tools/everglade_hotbar.coax` (the `cl-*` pictures).

The legacy CHARACTER picker's druid and Warlock are unchanged. A record without a
class (`CC2`) keeps Everglade's own druid.

## The road to Everglade

USE IN EVERGLADE saves first. Nothing moves until the save is confirmed
(message 17). Then the run plays, in the same world, with no cut:

1. The controls and their glass panel fade out over 0.8 seconds, and the camera
   swings round behind you over 3 seconds.
2. You run out of the glade and down Everglade II's path on your own for 20
   seconds. The run uses Everglade II's own controller (forward at a jog,
   steered 5 m ahead along the path), as held keys would. The camera keeps its
   own track: a three-quarter view that sweeps across behind you, pulls out and
   cranes up midway, and comes in to straight behind you.
3. The orbit turns to look where the run's camera looks
   (`verse_orbit_toward`), and the camera eases into the follow camera over
   1.5 seconds, so there is no jump.
4. The keys are yours, about 90 m down the path.

Any key or a click during the run skips to step 3. The key is used for the skip
and does nothing else. The address bar says `/worlds/everglade2` from the start
of the run (action 19, `history.replaceState`). The run is in
`everglade/client/create.coax` (`e2c_walk`, `e2c_shot`, `e2c_frame`). Everglade II
doesn't show the class bar yet.

Check: `coax/bin/coax test --library everglade/client/create.coax`
(the "create:" tests).

## Compact sidebar

Desktop class and appearance choices, Reset/Surprise Me and Save/Use actions
share one compact left sidebar. Rows are 40 display pixels apart with 36-pixel hit areas.
Labels and values sit on one line; the right side remains unobstructed. Narrow
or short layouts use the existing single bottom panel.

A shade (one of Everglade's HUD plates) sits under the sidebar, and on a phone
under the bottom panel and the title, so the labels read over any sky. Drag
turns your character, P (or DRAG: PAN on a phone) switches the drag to panning,
and the wheel zooms. Appearance records and compatibility keys remain unchanged.

## Camera ground bound

The studio camera's eye stays at least 0.28 m above the walk's ground at its
panned and zoomed position (`cz_studio`). The sky is Everglade's.
