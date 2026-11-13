# Everglade II

Everglade II (`/worlds/everglade2`) is the first piece of Everglade v2. Everglade v2 is
rebuilt from scratch, one piece at a time, apart from Everglade v1 (`/worlds/everglade`), which
stays as it is.

This piece is Character Create's glade, one winding path out of it through dense woods, and a
clearing at the path's end. Beside the path, a short walk from the glade, the woods open onto the
city's clearing, where the city will be built, with a brook along its east edge (#13093). It has
no town, gate, walls, people, raid, or HUD yet. You walk the
character you saved in Character Create, on Everglade's controller and follow camera. Up to 16
players share a room (`EG_SEATS` in `coax/web/room/room-everglade.coax`); when a room is full, the
next player joins another.

## Walk it

Character Create (`/worlds/character-creator`) is a place in this world: its studio stands in the
glade, and USE IN EVERGLADE runs you out of the glade and down the path, then hands you the keys
(`everglade/client/create.coax`, `docs/metaverse/character-creator.md`).

The studio shows a class pane beside the wardrobe. On a desktop it is a glass panel at the right
of the screen. On a phone it is two narrow columns in the sky on either side of your character. It
shows the class from the CLASS row: what the class is and how it plays, its Hit Die, primary
ability, saving throws, and armor and weapon training, its level 1 features, and the ability on
slot 1 of its bar with that ability's picture. The pane changes when the class changes. The facts
follow the SRD 5.2.1 classes (CC-BY-4.0, attributed in the source).

- Open `/worlds/everglade2`.
- W, A, S, and D run at a jog (4.25 m/s). Hold Shift to walk (1.5 m/s). Space jumps.
- Drag to look. You can look 1.2 radians above or below the horizon. Scroll to zoom. The arrow
  keys turn and tilt the camera.
- On a touch screen, the left thumb walks and the right thumb looks.
- `?at=METRES` starts you that far down the path, for example `/worlds/everglade2?at=300`. The
  clearing starts at about 440 m.
- `?walk` walks you down the path on your own, with the camera behind you, and stops in the
  clearing. Any key takes over. Add it to `?at=`, for example `/worlds/everglade2?at=200&walk`.

## The land

`everglade/world/forest.coax` holds the land as plain functions that the CPU and the shaders
share:

- Inside the glade (within 14 m of the middle and north of z = -14), the ground is exactly the
  glade's own (`cg_height`). A test checks this.
- The path leaves the glade on the glade's own road and winds 440 m south through a valley
  (`e2_road_x`). Its grade never climbs steeper than one in five.
- The valley walls rise into wooded ridges away from the path.
- `e2_forest` is how thick the woods stand: thick on both sides of the path and crowding it, none
  in the glade or the clearings. The woods close round the glade on every side, from 17 m out of
  its middle (`e2_wooded`), so the only open ground is the glade and the two clearings.
- The clearing at the path's end is a wide bowl, about 68 m across (`e2_end`).
- The city's clearing (`e2_city`, #13093) is where the city will be built: a level meadow about
  108 m across, east of the path from about z = -60 to z = -170. Its floor is the path's level
  beside it, so the path runs onto it with no step, and no tree stands on it. Walking south from
  the glade, the woods on your left open onto it after about 70 m.
- A brook (`e2_brook`) runs along the city clearing's east edge, out of the woods to the north and
  back into them to the south. It is 4.4 m wide and 0.55 m deep, shallow enough to wade, with open
  banks you may walk along (`e2_by_brook`).
- Both clearings and the brook are placed with Everglade's shared clearing module
  (`everglade/world/clearing.coax`): `clearing_open` (the ragged open ground), `clearing_floor`
  (the ground levelled in), `clearing_ring` (the band of trees past the edge), and `stream_x`,
  `stream_dist`, `stream_bed` and `stream_keep` (the channel, its banks, and where trees may
  stand). `forest.coax` only says where they lie. `e2_clearing` is the larger of the two
  clearings' open ground, so the planting, the grass, the walk and the camp follow both.
- `e2_autumn` colours the tree crowns. `e2_gold` colours the ground and grass: the glade's own
  gold inside it, and a gentle share outside it, so the woods' floor and the clearing stay
  green.

### The admitted ground and its baked light (#12995, #13059)

`everglade/world/ground.coax` holds Everglade II's land as a set of Metaverse Engine terrain tiles
(`metaverse-engine/world/terrain-tile.coax`) against Everglade II's ground material table (meadow, autumn meadow,
forest litter, path dirt), with four weights a sample from `e2_gold`, `e2_forest` and `e2_path`.
All tiles lie on one lattice, the glade's 0.65 m samples from (-32.5, -60), each worked out once
in f64 and rounded once, so a sample two tiles share is the same point and height in both, bit
for bit.

- The glade: tile 0, the pack's old grid exactly (100 cells of 0.65 m from (-32.5, -60) to
  (32.5, 5), each split on the same diagonal as the pack's triangles).
- The path: 65 m tiles in rows south of the glade. Rows 0 to 4 are 100 cells of 0.65 m; rows 5
  to 7, round the clearing, are 50 cells of 1.3 m. A cell is solid where the old ribbon ran, on
  the city's clearing and on the brook's banks beside it, a
  hole elsewhere, and a tile with no solid cell is left out: 24 tiles in all.
- Seams: neighbours of one spacing repeat their shared edge exactly. Where a 0.65 m row meets a
  1.3 m row (z -385), the finer edge is stitched onto the coarser one (LOD profile v1), so no
  edge cracks. Every tile draws at its stored level: nothing swaps as you walk.

The walk stands on the admitted triangles (`e2g_y`) on every tile, so your feet and the drawn
ground agree at every sample and along every triangle; the pack draws that surface 0.035 m lower,
a declared decorative offset. Past the tiles (the coarse ground) the walk reads `e2_height`, and
`e2_probe` 98 counts those reads.

The asset route. The pack tool converts the land once and writes the whole set into the pack's
catalog as one record (E2T1: a header, each tile's header and revision, its holes, and every live
sample) in base64 `G` lines. The page reads that record as the catalog arrives
(`e2g_record_read`). Every tile is admitted against the page's own table and must come out at the
revision the record names; then the seams and the set's revision are checked, and only then does
the set publish, whole. A refusal is counted and the set published before stays. Without a record,
or when it refuses, the page converts the land itself. `e2_probe` 90 to 97 report the route (1 the
pack, 2 converted), the tiles, publishes, refusals, the first refusal's code, the KiB resident, and
the set's and the glade's revisions.

The frame. Tiles keep f64 world origins; the page reads them through one frame origin (`frame_x`,
`frame_z`), and the queries, the bake and the shader's store all use it. Everglade II runs at frame
origin 0; the tests check that a set placed 1e8 m out answers the same queries, draws the same
grid and bakes the same light, and that a frame too far from a tile refuses.

`coax/web/everglade2-terrain-check.coax` is the receipt: it walks the glade, the path and the
fine-to-coarse seam headless at 2400x1400, DPR 2, and reports load time, frame times, GPU pass
time, Wasm memory and the ground's facts. It passes only on the pack route with no refusal and no
memory growth.

The woods' and the glade's tree crowns (sphere proxies per kit) are then baked onto the tile as
ambient occlusion (`metaverse-engine/render/bake.coax`, keyed by the tile's revision, the crowns' digest and one world-state
axis, the realm's variant: a bake for other land, crowns or state refuses and the live bake stays).
A crown of radius r whose centre is D away along d hides (r / D)^2 max(0, n . d) of the
cosine-weighted sky; AO = clamp(1 - sum, 0.6, 1), with no bounce, eased to open sky over the last 6 m inside the tile's edge so leaving it is no step in light. Your character
takes the AO at your middle (u[62]) on your indirect light only, never the sun, so you darken under
the canopy; the ground takes a quarter of it (below). `e2_probe` 9 is the bake's status, 10 the AO at you, 11 its crowns.

The ground draws from the table and the bake (#13052). The page uploads both as one store
(`e2g_gpu`: the grid, the four materials, the live AO), and the ground shader
(`everglade/client/gpu/everglade2.coax` `e2_ground_albedo`, `e2_ground_ao`) takes the same weights at each pixel
(`forest.coax` `e2_ground_layer`) and evaluates `metaverse-engine/render/terrain-shade.coax`'s equations.
The pack's vertex colours are no longer read. The table holds the pack's colours re-encoded for
its 2.2 power, so each decodes to the same linear colour. The litter's autumn tint is a layer share
(a new material-table term: autumn's weight against the meadow's under it, `aethyr.terrain.materials`
"ATM2"). The pack's grain is the table's value noise. Mixing in linear light brightens a layer's
edge by up to 0.06 (sRGB) against the pack's sRGB mix. The ground's sky light takes a quarter of
the bake's AO (`E2G_GROUND_AO_SHARE`): the realm's own darkening under the woods stays, and the full
bake on top made the woods' path 15 to 30% darker. Measured against the frames before: within 4% at
the darkest spots under the crowns, within 2% elsewhere, unchanged off the tile.

## The planting

`everglade/world/plant.coax` places every kit piece as a pure function of the land, cell by
16 m cell. The page plants all of them at load, in a fraction of a second:

- Trees stand on a jittered 2.6 m grid where the woods are thick, never on the path or in the
  clearing. The first trunks stand 2.6 m from the path's middle, and trees grow larger away
  from the path, so their crowns make a continuous canopy behind the near trunks. There are
  oaks, birches, and pines, with autumn crowns in the autumn patches.
- The understory has ferns, bushes, flowers, mushrooms, clover, and rocks. It is thickest on the
  verges and between the trunks within 35 m of the path.
- Now and then a boulder stands in the woods.

That comes to about 100,000 pieces. Tests check that none stand on the path, that no tree stands
in the clearing, and that trees crowd the path.

## The pack

`metaverse/tools/assets/everglade2_pack.coax` writes `realms/everglade2/assets/everglade2.pack`
(57 MB). The pack holds:

- Character Create's people pack (the wardrobe and the rig).
- The glade's own composition, placed on Everglade II's land.
- The ground: the admitted tiles' own samples (their solid cells only), the coarse 4 m ground
  out to the valley's edges, and the tiles' record in the catalog. The ground vertices carry no
  colour: the shader takes the ground's colour from the material table.
- The 36 kit pieces, each at the origin. Each tree has three levels of detail: whole; two crown
  cards in three with bark on the lower half; and one crown card in three with bark on the
  trunk only.

The tracked pack is sealed: `pack_read` in `metaverse-engine/editor/pack.coax` admits it as it
is, and `coax/remote/check.sh reports` fails when it doesn't (`packs-admitted`). The tools below
append to a working form the admission refuses (the `VPK1` magic over 48 entries, a ten-digit
`bytes` line, each tool's own source numbers). So every append has three steps: take the working
form, run the tool on it, and seal the result. Sealing numbers the sources 0, 1, ... in the order
their `L` lines stand and records each old number in a `was NEW OLD` line, which the working form
reads back.

```sh
P=apps/openagents-web/realms/everglade2/assets/everglade2.pack
coax/web/tool.sh metaverse-engine/tools/pack-seal.coax --working "$P" /tmp/everglade2-working.pack
# ... run the append tools below on /tmp/everglade2-working.pack ...
coax/web/tool.sh metaverse-engine/tools/pack-seal.coax /tmp/everglade2-working.pack "$P"
```

`everglade2_plant.coax` (the web build's planting step) reads the sealed pack and seals what it
writes. Experimental's `glade.pack.gz` is sealed the same way (inflate it, seal, `gzip -n -6`).

Build it from the repository root:

```sh
A="$HOME/.openagents/assets"; src() { coax/web/tool.sh metaverse/tools/assets/registry.coax source "$1" "$A"; }
M="$A/cache/everglade2-monsters"; B=/Applications/Blender.app/Contents/MacOS/Blender
$B --background --factory-startup --disable-autoexec --python-exit-code 1 --python metaverse/tools/assets/animal_bake.py -- \
  "$(src ultimate-animals)" "$M" e2 "$(src dungeon)" "$(src fantasy-props)" "$(src ultimate-rpg)"
$B --background --factory-startup --disable-autoexec --python-exit-code 1 --python metaverse/tools/assets/bestiary_bake.py -- \
  "$(src bestiary-dungeon)" "$M" "$(src ual)/Unreal-Godot/UAL1_Standard.glb" e2
coax/web/tool.sh metaverse/tools/assets/everglade2_pack.coax "$(src nature)" \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack \
  apps/openagents-web/realms/everglade2/assets/everglade2.pack "$M" "$(src bestiary-dungeon)"
```

The two bakes write the monsters' frames (the Wolf's Idle, Gallop, Attack and Death; the goblin
Puglin's Idle, Jog, Sword, Shoot and Death, eight frames each), the goblin camp's props (the Dungeon
Woodfire and the Fantasy Props stall, crates, barrel and bench) and the guard's Sword. The pack holds
them as `glade/monster0..31`, `glade/gremlin0..39` and `glade/weapon1`; the camp stands in the glade's
environment where `everglade/world/camp.coax` puts it.

The path's guard is the SRD Fighter at level 10 (`rs_path_guard` in `rules/srd/npcs.coax`):
AC 19 (Chain Mail, a Shield and the Defense style), 84 Hit Points, Str 18, Proficiency +4, and two
Longsword attacks a turn at +8 for 1d8 + 4, with Second Wind and Action Surge. He never strikes first.
Once you strike him, he fights until you drop, leave his 28 m leash, or stop fighting for 15 s. You
see and hear each of his attacks, 0.7 s apart. The room check
`e2 guard: an SRD level 10 Fighter` covers his numbers and his attacks a turn.

Then add the Ranger's kit: Experimental's Longbow, its string and its Arrow (`glade/weapon4..6`),
copied out of its `glade.pack` onto the Sword's vertex base and shaded by vertex colour alone
(`metaverse/tools/assets/weapon_pack.coax`). Without them a Ranger here carries nothing:

```sh
gzip -dc apps/openagents-web/realms/everglade-experimental/assets/glade.pack.gz > /tmp/glade.pack
coax/web/tool.sh metaverse/tools/assets/character_glade_pack.coax --append-weapons \
  BASE.pack apps/openagents-web/realms/everglade2/assets/everglade2.pack /tmp/glade.pack
```

Then add the characters' material maps (#13012). The tool appends the Fantasy Source's normal and
ORM maps for each outfit family, and the base bodies' normal and roughness maps, as 14 linear
data layers at 512 px, between the colour layers and the sounds. That brings the pack to 57 MB.
`build.sh` then writes the range-loaded `everglade2.vpk3` from it:

```sh
coax/web/tool.sh metaverse/tools/humans/pack_material_layers.coax \
  "$(coax/web/tool.sh metaverse/tools/assets/registry.coax source fantasy "$HOME/.openagents/assets")" \
  apps/openagents-web/realms/everglade2/assets/everglade2.pack /tmp/everglade2-data.pack
mv /tmp/everglade2-data.pack apps/openagents-web/realms/everglade2/assets/everglade2.pack
```

`metaverse-engine/character/material.coax` holds each colour layer's material record: its kind (skin,
cloth, hair, eye or metal), roughness, metalness, sheen and the data layers it reads. The page
stores the table (store 3) and `cc_player_fs` in `everglade/client/gpu/everglade2.coax` samples the maps and
shades with `metaverse-engine/render/character-material.coax`. Roughness never goes below 0.55, or 0.7 on
metal, so characters stay matte. `?maps=0` turns the maps off, and `?film` adds skin wrap, cloth
sheen and hair highlights.

### The cat (#13112)

The glade has a cat (`everglade/creatures/cat.coax`, the game layer's, wired in by
`everglade/client/wild.coax`): a black and white tuxedo cat. It sits, stands, wanders
within 5 m of its home south-east of where you start, and when you come within 4 m it sits and
turns to look at you. Its sitting head is turned 0.8 rad to its left (`EG_CAT_LOOK`), so the cat
turns its body that much away from you and you see the sit in three-quarter view.
`?stage=cat` is its shot: it sits on the glade's path and looks at a low camera that closes in
over ten seconds, with no HUD and no room. The pack holds its 64 frames as `glade/cat0..63`
(Idle 8, Walk 8 and Sit 48; `assets/registry/README.md` has how the cat is built). Bake the frames,
append them to the pack with the tool, then rebuild the `.vpk3`:

```sh
Blender --background --factory-startup --disable-autoexec --python-exit-code 1 \
  --python metaverse/tools/assets/cat_bake.py -- ~/.openagents/assets/cache/cat-e2
coax/web/tool.sh metaverse/tools/assets/character_glade_pack.coax --append-cat \
  BASE.pack apps/openagents-web/realms/everglade2/assets/everglade2.pack ~/.openagents/assets/cache/cat-e2
coax/web/tool.sh clip "http://127.0.0.1:8871/worlds/everglade2?stage=cat" cat.mp4 --size 1920x1080 --seconds 10
```

## The page

`everglade/client/client.coax` and `everglade2-face.c` run the page:

- The face steps Everglade's controller (`everglade-step.coax`). The page lifts your feet onto
  the land, and keeps you in the glade's 26 m circle, on the path's corridor, or in the
  clearing. Thick woods stand wherever you may not go, so no edge of the world shows open ground.
- Every tree trunk and rock stops you. A trunk is a capsule sized to its tree kind's trunk. A
  rock is its own mesh's convex hull (the OpenAgents Metaverse Engine's hulls, `metaverse-engine/physics/query.coax`): its top is ground you
  stand on, its sides stop you where you see them. You step onto a rock lower than 0.55 m, jump
  onto a taller one and stand on its top, walk up a gentle face and slide off a steep one. The
  hulls are baked from the Stylized Nature meshes by
  `metaverse/tools/assets/everglade2_rock_hulls.coax` into `everglade/world/rock-hulls.coax`, whose
  check compares each hull to its mesh's bounds. Both are scaled and turned with the placed
  piece, the glade's own trees and rocks included; the page and the room step you on the same
  hulls. Pebbles, a few centimetres high, don't stop you. `everglade/client/walk-test.coax` walks into
  the glade's mossy rock from three sides, steps onto a low rock, jumps onto the mossy rock and
  stands on its top, walks into a trunk, and runs the whole path from the glade to the clearing
  down its middle and a metre to each side without being held up.
- Every collider comes from the same item list and transform as the drawn piece. The glade's
  pieces that stand on the path out (`e2_glade_kept`) are left out of both the pack and the
  colliders, so nothing unseen stands in your way.
- The run and the walk are Everglade II's own: the page sets the step's pace, so v1's speeds
  (7.5 and 3.5 m/s) stay as they are. The gait plays by the distance you cover (a 3.4 m running
  stride and a 1.3 m walking one), so the feet keep pace.
- Each frame, the page picks the pieces in view, sorts them into one run for each kit piece and
  level of detail, and draws each run as one instanced draw. About 75 draws make a frame.
- Every piece stands in one place at one size at every distance; with distance it only fades
  out in a screen dither, so nothing moves, grows or swaps as you walk (#13023). A tree draws
  whole to 32 m, two of its three crown cards to 52 m, and one past that. The cards it keeps are
  the same cards in the same places, and each change cross-fades over 8 m: the nearer level drops
  a share of its pixels and the farther level draws exactly those. Past 55% of its kit's reach
  (165 m for trees, 26 to 110 m for the small plants, 240 m for the big rocks) a kit thins out:
  the share left falls smoothly to none at the reach, and each piece dithers out as the share
  passes its own rank, a hash of where it stands, so the edge is ragged and never a line
  (`e2_piece_shown`, `e2_piece_looks`).
- Character Create's glade grass isn't drawn. At about 35 million vertices a frame it cost
  about 10 ms of GPU time; the blade grass covers the glade instead.
- Blade grass covers all of the ground except the bare path (`everglade/world/field.coax`). The
  GPU places one tuft in each cell of a grid fixed to the land, seeded by the cell, in four
  nested tiers round the eye: 0.5 m cells to 28 m, 1 m to 40 m, 2 m to 52 m, and 4 m to 140 m.
  Each coarse tuft stands exactly where one of the four finer tufts under it stood, with the same
  turn, height and width, all from the finest cell. Past 16 m, the other three dither out one by
  one at their own rank, so the grass only thins with distance and no tuft moves, widens or is
  replaced. Past 24 m the ground under the grass takes on the grass's colour by the same cover,
  so the field's far end fades into grass-coloured ground, not a line. The grass thins in at the path's
  edges and grows thinner, shorter, and darker under the trees. The page draws the blocks of 16
  by 16 cells in view.
- A jump plays its takeoff over 0.2 s, its loop while you are in the air, and its landing over
  0.35 s. It blends over the run or the idle pose at 0.45: it eases in over 0.12 s after takeoff
  and out over 0.18 s after landing, so the legs keep their stride. These came from Everglade
  Experimental (`charactercreator.coax`).
- The follow camera looks straight down your back at every tilt, with no shoulder offset, and
  keeps in front of the trunks between it and you. When you tilt up far enough that the eye
  would go under the land, the eye stays where it meets the land, 0.2 m above it, and the view
  keeps tilting up. As it does, the aim rises up to 2 m over your head, so you stand in the lower
  middle of the view and the sky fills the rest. The eye moves smoothly onto and off the land.
  Everglade II tilts from 1.2 radians below the horizon to 1.2 above.
- A frame-rate counter in the bottom-right corner shows frames a second, averaged over 0.5 s.
- The graphics governor (`metaverse-engine/core/quality.coax`) sets how many pixels the page draws. It lowers
  the resolution when frames run slow and raises it again when there's room.
- The key light's shadows are the Metaverse Engine's cascades (`metaverse-engine/render/cascades.coax`): four tiles in
  a 2048 atlas reaching 24, 60, 150 and 360 m round you (two, to 20 and 75 m, on a phone). Each tile's middle snaps to whole texels
  in the light's space and moves only in steps of 8 to 120 m, so shadows stay fixed to the land
  as you walk. The near tile casts every tree at its second level, the 60 m tile what the view
  draws, and the far tiles only the trees at their third level and the big rocks. The shader
  reads the finest tile that covers a point.

- The GPU leaves out the kit pieces nothing shows (#13050). The page still picks the pieces and
  their levels and fades; `everglade/client/cull.coax` hands the host each frame's camera and runs, and
  `everglade/client/gpu/everglade2-cull.coax` tests each picked piece's box against the frustum and the depth
  pyramid in two phases (`coax/web/instance-cull.js`). The first phase tests against last
  frame's pyramid and draws what passes. The pyramid is then built from that depth, and the
  second phase retests what the first hid and draws what is no longer hidden. A piece is left
  out only when depth this frame really drew is nearer than all of its box, so culling never
  makes a piece pop. Each run draws by two indirect draws, so it needs the adapter's
  `indirect-first-instance`; without it, and with `?cull=0`, the page draws every piece it
  picked. The shadow tiles' pieces aren't culled.

Read the counter in your own browser for the frame rate. Headless measurements on a busy Mac
put GPU time at 2400 by 1400 at about 16 ms at the glade (31 ms before the glade grass was
dropped) and 17 ms at the clearing, before the trees and grass only faded (#13023).

GPU culling, measured by `coax/web/everglade2-cull-check.coax` at 2400 by 1400 (dpr 1, every
device pixel, the view held still), each mode's median of six 2.5 s turns on a busy Mac (Apple
Metal 3, 2026-11-08). The image with the cull on is the image with it off, pixel for pixel, at
every view:

| View | Pieces picked | Frustum | Drawn first | Hidden | GPU off | GPU on |
|---|---:|---:|---:|---:|---:|---:|
| The glade | 4229 | 465 | 1162 | 2602 | 8.6 ms | 7.6 ms (-11%) |
| The woods (`?at=200`) | 4555 | 511 | 2091 | 1953 | 11.0 ms | 9.2 ms (-17%) |
| The clearing (`?at=470`) | 4160 | 489 | 2783 | 888 | 8.5 ms | 8.2 ms (-4%) |

The receipt is `docs/verification/gpu-cull-everglade2-13050/`.

`everglade/client/gpu/everglade2.coax` holds the passes. It uses Character Create's glade light, sky, fog, and
materials, adds the kit pieces' vertex stage and the grass tufts' shading, and gives the woods'
floor its leaf litter.

## Build and serve

Everglade II builds in seconds:

```sh
cd apps/openagents-web
../../coax/web/wasm.sh everglade/client/client.coax everglade2-face.c build/everglade2.wasm \
  everglade-step.coax ../../metaverse-engine/core/sticks.coax
cd ../..
coax/web/tool.sh coax/web/wgsl-module.coax everglade/client/gpu/everglade2.coax \
  apps/openagents-web/everglade2.wgsl everglade/client/gpu/everglade2.module
```

To serve it without the whole site's build, serve the app statically:

```sh
coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:8863 --static --cache 0
```

The page needs the shared host modules in `apps/openagents-web/build/` too
(`host-services`, `host-sockets`, `host-packs`, `latency-client`, and the others from
`build.sh`). A plain `coax serve` without `--static` runs the whole of `build.sh` before it
answers.

## Admin mode (this machine only)

Admin mode is a panel for doing anything in the world while you build it. It is Everglade
Experimental's left column: ANIMATIONS lists the clips, CLASSES switches among the twelve
classes, LEVEL sets 1 to 6, and REST sends SHORT, SLEEP, WATCH, HIT DIE, and END REST to the room.
Everglade II does not bake the ANIMATIONS clips yet: their 1.4 MB of keys does not fit beside its
state, so a chosen clip does not play. CLASSES and LEVEL also tell the room (`X class srd-CLASS-L`),
so your casts are the new class's even when your seat's ticket holds later hellos. KNOCK OUT makes the guard you drop stay at 1 HP, Unconscious. Under them are the admin
rows:

- FULL HEAL, and GOD, which keeps your Hit Points full.
- NOCLIP, which flies you through trees and rocks with W, A, S, and D (Shift is slower).
- CLICK TP, after which a click on the ground takes you there. GO TO takes typed `X Z`
  metres and Enter. PATH -50 M and PATH +50 M move you along the path, as `?at=` does.
- TIME steps the time of day by two hours. Reload the page to go back to the room's day.
- GUARD KILL, GUARD RESET (a new guard now), GUARD OFF, and GUARD ON.
- Your position and distance down the path. The frame rate is in the bottom-right corner.

Backquote (`` ` ``) shows and hides the panel. It starts shown. The panel does not hold the
cursor: a click on the world takes the mouse for looking, and Tab or Escape frees it for the panel.

## Mouse buttons

As in Everglade Experimental, while the mouse looks round (`everglade/client/mouse.coax`):

- The left button casts your last pressed slot, or your Attack.
- A Ranger's right button held draws the Longbow and its release looses the arrow at the reticle,
  through the room as the Attack. The left button while drawing lets the bow down.
- Every other press is the world's: a click takes the mouse back for looking, and a drag turns.

A cast plays the same body clip as Experimental's (`cast-pose.coax`): a slash for a blow, the spell
clip for a spell, and the bow's draw pose while the bow is drawn.

To open it, build a dev `everglade2.wasm` and run a local room and page server:

```sh
cd apps/openagents-web
COAXIAL_MODE=dev ../../coax/web/wasm.sh everglade/client/client.coax everglade2-face.c build/everglade2.wasm \
  everglade-step.coax ../../metaverse-engine/core/sticks.coax
cd ../..
coax/web/tool.sh coax/web/room/local.coax --room 8950 --page 8949 --static
```

Then open `http://127.0.0.1:8949/worlds/everglade2?admin&relay=ws://localhost:8950`.

Admin mode works only on this machine. Three checks guard it:

- It exists only in dev builds. `everglade/client/client.coax` calls `everglade/client/admin.coax` only inside
  `#if BUILD_DEBUG` blocks. `coax/web/wasm.sh` compiles with `--build-profile release` unless
  `COAXIAL_MODE=dev`, so the release linker drops the unit's code. `build.sh` fails a release
  `everglade2.wasm` that contains the panel's words or its room lines.
- In a dev build, it turns on only for a page from `127.0.0.1` or `localhost` whose address has
  `?admin`.
- The room acts on the panel's `X` lines only when it runs with `--dev` and listens on
  `127.0.0.1` (`everglade/server/admin.coax`). Production's room drops them. The room checks
  `e2 admin:` test this.
