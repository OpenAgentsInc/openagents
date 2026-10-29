# Everglade, the forest city

Status: plan and blockout, October 28, 2026 (eg-design); the forest and
its paths, October 29 (eg-forest); the kit buildings and seven
interiors, October 29 (eg-buildings); its people, who talk and remember
you, October 29 (eg-people). The city is
walkable at https://openagents.com/realms/everglade (not on the galaxy until
eg-launch; `/verse/galaxy?realms=all` shows it). The order of the work after
this is in [realms-port.md](realms-port.md).
The previous Everglade, on the Rust engine, is kept for parity in
[`verse/everglade/previous/`](../../verse/everglade/previous/README.md): its map and specifications.

Everglade is a conventional MMORPG hub in the woods: where players gather,
get quests and set out from. It is the same for everyone and players can't
change it, so quests and characters can rely on it. Only an admin can,
in edit mode or by voice (below), and every change is kept as the realm's
published edits. The brief is the
Everglade section of `~/work/aethyr-demo/docs/realms.md`; the shape and the
paper map come from the old OpenAgents Everglade (`0b401561a0^:docs/verse/everglade.md`),
a glade around a commons grown into a small town. This plan keeps its
commons, fountain, observatory and market stalls and drops the workshop and
the Agent Studio (they can come later as one building).

![Everglade's plan](everglade-map.svg)

One source holds the layout: [`verse/everglade/plan.coax`](../../verse/everglade/plan.coax).
The realm draws it, its checks walk it, and
[`verse/everglade/map.py`](../../verse/everglade/map.py) draws the map above
and the walking times below from the same numbers. Change the city there,
then run the checks and the map:

```sh
coax/bin/coax test --library verse/everglade/plan.coax --steps 200000000
python3 verse/everglade/map.py docs/verse/everglade-map.svg
python3 verse/everglade/map.py --times
```

## Scale

- Metres. North is +z and the fountain is the origin. The axes are the
  engine's (yaw 0 faces +z, a quarter turn faces +x), so +x is **west**:
  seen from above with north up, +x is to the left, as on the map.
- The ground is 208 m square (104 cells of 2 m); you may walk 200 m of it.
- The palisade encloses 127 m east to west and 115 m north to south. Its
  timber walls are 4.5 m high and 0.8 m thick; the gate towers are 7 to
  7.5 m.
- Streets and trails are 3 m wide. Buildings keep at least 2 m from each
  other and from the palisade; props at least 1 m from anything.
- You walk at 3.5 m/s and run at 7.5 m/s (`verse/player.coax`): the city
  is half a minute's walk across, and the farthest trail end under a minute
  from the South Gate. That keeps a hub dense enough to meet people in and
  small enough to learn in one visit.
- The ground is `verse/terrain.coax`'s heightfield, with the bowl's rim
  moved out to 80 m (`Ground.rim_from`, `rim`): gentle hills inside the
  walls, flat under every building and square, rising into the forest
  past the trails' ends.

## The city

| Place | Where | Size | What it is for |
| --- | --- | --- | --- |
| **The road in** | South, from the edge of the world to the South Gate | 3 m road, 42 m | Where you arrive. It passes the stables. |
| **Stables** | Beside the road, west of it, outside the gate | 12 × 12 m, 6 m | Mounts later; the stable hand. |
| **South Gate** | The palisade's south wall | 6 m gap, two 3 m towers | The main gate; two guards. |
| **The palisade** | Round the city | 4.5 m walls, four gates | Keeps the city a place; every way out is a gate. |
| **Inn Street** | South Gate to the commons | 3 m street, 44 m | The way in: the inn and houses. |
| **The inn** | Inn Street, east side | 16 × 14 m, two storeys | The common room with a hearth, rooms upstairs; where parties meet. The innkeeper and the inn's cat. |
| **The commons** | The middle | paved, 32 m across | The fountain (6 m across), the quest board and the town crier; the ring street round it. The errand cat waits by the fountain. |
| **The market** | West of the commons, on Market Street | paved, 26 m across | Six stalls in two rows either side of the street (3 × 2 m each), three sellers. |
| **General store** | The market's north side | 12 × 10 m | Supplies; the shopkeeper. |
| **Smithy** | North-west of the market, by the West Gate | 12 × 10 m | Arms and tools; the smith. |
| **Alchemist** | South-west of the market | 10 × 10 m | Potions; the alchemist. |
| **Guild Row** | East of the commons | 3 m street, 52 m | The fighters' hall (14 × 12 m) on the north side and the rangers' lodge (14 × 12 m) on the south side, facing each other. Where you pick a class and train. |
| **Mages' tower** | North-east, on Mages' Lane off the temple's forecourt | 10 × 10 m, 14 m tall | The third guild, set apart near the stars; the archmage and the guild's familiar (a cat). |
| **Temple Hill** | North of the commons, up the Temple Walk | forecourt 18 m across | The temple (18 × 18 m, 12 m) for healing and lore; the priest. |
| **The observatory** | North-west of the temple, on Stargazers' Lane | 8 × 8 m, 18 m tall with its dome | Lore and the view over the forest; the astronomer. |
| **North Gate** | The north wall, near the observatory | 5 m gap | Out to the ruins and the crypt. |
| **East Gate** | The east wall, at Guild Row's end | 5 m gap | Out to the deep wood. |
| **West Gate** | The west wall, at Market Street's end | 5 m gap | Out to the river crossing. |
| **The deep wood** | East, by the Deep Wood Trail | waystone | The woodwarden; the first quest area past the trees. |
| **The ruins** | North-west, at the end of the Ruins Trail | three broken pillars | The ruin scholar. |
| **The crypt** | North-west, off the North Trail's fork | 8 × 6 m | A crypt door; a dungeon later (the Haunted House is the first one a quest can send you to). |
| **The river crossing** | West, past the West Gate | a 4 m river and a bridge | The ferryman on the far bank. The river runs the length of the world in a carved bed; you cross by the bridge, or wade and swim across in the glade beside it. |
| Houses | Six plots round the streets | 8 to 10 m a side | Homes for the city's people (eg-people), filler that keeps the streets streets. |

## Walking times

At a walk (3.5 m/s), by the streets, then straight to the door
(`python3 verse/everglade/map.py --times`). Running takes under half as long.

| Place | From the South Gate | From the fountain |
| --- | --- | --- |
| The Inn | 20 m, 5 s | 49 m, 15 s |
| General Store | 71 m, 20 s | 28 m, 10 s |
| Smithy | 80 m, 25 s | 50 m, 15 s |
| Alchemist | 69 m, 20 s | 53 m, 15 s |
| Fighters' Hall | 71 m, 20 s | 38 m, 10 s |
| Rangers' Lodge | 63 m, 20 s | 41 m, 10 s |
| Mages' Tower | 95 m, 25 s | 35 m, 10 s |
| Temple | 89 m, 25 s | 21 m, 5 s |
| Observatory | 99 m, 30 s | 39 m, 10 s |
| Stables | 16 m, 5 s | 82 m, 25 s |
| The Road In | 29 m, 10 s | 97 m, 30 s |
| The Deep Wood | 123 m, 35 s | 93 m, 25 s |
| The Ruins | 148 m, 40 s | 81 m, 25 s |
| The Crypt | 168 m, 50 s | 109 m, 30 s |
| The River Crossing | 103 m, 30 s | 96 m, 25 s |
| North Gate | 126 m, 35 s | 69 m, 20 s |

## Kit pieces

Everything is from the three CC0 Quaternius kits aethyr-cpp already packs
(its SCN4 `scene.bin`, read by `verse/tools/pack_realm.py`): Stylized Nature
(SN), Medieval Village (MV) and Fantasy Props (FP). The names are the kits'
meshes. Where a kit has no piece, the line says what is assembled instead.

| Place | Pieces |
| --- | --- |
| Palisade and gates | MV `Roof_Log` stood upright as posts, `Prop_Support` braces; gates `DoorFrame_Round_WoodDark` with `Door_8_Round`; towers `Wall_UnevenBrick_Straight`, `Corner_Exterior_Wood`, `Roof_Tower_RoundTiles`, `Stairs_Exterior_Straight`. No kit has a log wall: posts repeat along each wall. |
| Houses and walls of every building | MV `Wall_Plaster_*`, `Wall_UnevenBrick_*`, `Corner_Exterior_*`, `Window_*`, `WindowShutters_*`, `Door_1_Flat`/`Door_2_Round`, `Roof_RoundTiles_*`, `Roof_Front_Brick*`, `Overhang_*`, `Balcony_*`, `Prop_Chimney`, `Prop_Vine*` on the 2 m grid. |
| Commons | No fountain in the kits: a basin ring of MV `Prop_ExteriorBorder_Straight1/Corner` on `Floor_Brick`, a column of `Prop_Brick*`, water drawn by the engine. Quest board: two MV `Prop_Support` posts, a `Floor_WoodDark_Half1` board, FP `Scroll_1`, `Scroll_2`; the crier's FP `Banner_1`. SN `Bush_Common_Flowers`, `Flower_3_Group` round the ring. |
| Market | FP `Stall_Empty`, `Stall_Cart_Empty`, `FarmCrate_Apple`/`Carrot`/`Empty`, `Barrel_Apples`, `Crate_Wooden`, `Bag`, `Coin_Pile`, `Pot_1`, `Vase_2`/`Vase_4`. |
| General store | FP `Shelf_Arch`, `Shelf_Simple`, `Cabinet`, `Barrel`, `Rope_1..3`, `Pouch_Large`, `Bucket_Wooden_1`, `Lantern_Wall`. |
| Smithy | FP `Anvil`, `Anvil_Log`, `Whetstone`, `WeaponStand`, `Sword_Bronze`, `Axe_Bronze`, `Pickaxe_Bronze`, `Shield_Wooden`, `Bucket_Metal`, `Chain_Coil`; MV `Prop_Chimney2` over the forge. |
| Alchemist | FP `Cauldron`, `Potion_1/2/4`, `SmallBottles_1`, `Shelf_Small_Bottles`, `Bottle_1`, `BookStand`, `CandleStick`. |
| Inn | MV walls, `Stair_Interior_Rails`, `Floor_WoodDark`, `Balcony_Simple_Straight`; FP `Table_Large`, `Chair_1`, `Bench`, `Stool`, `Mug`, `Table_Plate`, `Chandelier`, `Barrel_Holder`, `Bed_Twin1/2`, `Nightstand_Shelf`, `CandleStick_Triple`; the hearth from MV `Prop_Brick*` under `Prop_Chimney`. |
| Fighters' hall | FP `Dummy`, `WeaponStand`, `Shield_Wooden`, `Sword_Bronze`, `Banner_2`, `Bench`. |
| Rangers' lodge | FP `Peg_Rack`, `Rope_2`, `Axe_Bronze`, `Bag`, `Table_Large`; SN `Pine_*` in the yard. No bow in the kits: one is modelled with the eg-buildings work. |
| Mages' tower | MV `Wall_UnevenBrick_*`, `Roof_Tower_RoundTiles`; FP `Bookcase_2`, `BookGroup_*`, `Book_Stack_*`, `Scroll_*`, `CandleStick_Stand`. |
| Temple | MV `Wall_UnevenBrick_*`, `Wall_Arch`, `DoorFrame_Round_Brick`, `Door_4_Round`, `Stairs_Exterior_Platform`; FP `CandleStick_Stand`, `Chalice`, `Bench`, `Banner_1`. |
| Observatory | MV `Wall_UnevenBrick_*`, `Stairs_Exterior_*`, `Balcony_Cross_*`, `Roof_Tower_RoundTiles`. No telescope or dome in the kits: both are assembled (a dome from the engine's sphere is the blockout's). |
| Stables | MV `Roof_Wooden_2x1*`, `Prop_WoodenFence_Single`/`Extension1/2`, `Prop_Wagon`; FP `Bucket_Wooden_1`, `FarmCrate_Empty`, `Bag`. |
| Forest and trails | SN `CommonTree_1/3/4` (+ `_Leaves`), `Pine_1/4/5`, `TwistedTree_2/4/5`, `Bush_Common`, `Fern_1`, `Grass_*`, `Clover_*`, `Flower_*`, `Mushroom_Common`, `Plant_1/7`, `Rock_Medium_1..3`. |
| Ruins | SN `Rock_Medium_*`; MV `Prop_Brick1..4`, `Wall_UnevenBrick_Straight` broken off, `Wall_Arch`; FP `Vase_Rubble_Medium`. |
| Crypt | MV `Wall_UnevenBrick_Door_Round`, `DoorFrame_Round_Brick`, `Door_4_Round`; SN `DeadTree_1..3`, `Rock_Medium_*`. |
| River crossing | MV `Floor_WoodDark` planks and `Prop_WoodenFence_Single` rails for the bridge; SN `Rock_Medium_*` on the banks; water drawn by the engine (the water system, docs/verse/water.md). |
| Characters | You are the previous Everglade's Ranger (below, "You: the Ranger"); the people are still plain figures, to be aethyr's figure parts with the `Peasant` and `Ranger` outfits; the cats are aethyr's `Custom_Cat`. |

## Where characters stand

Each character's day starts at a spot in the plan (`EG_SPOT`: x, z, facing).
Every spot is checked to be in the open and reachable on foot.

## The people (eg-people)

`verse/everglade/people.coax` names whoever stands at each spot and gives
them a role, a greeting and what everyone in the city knows of it; the cats
are cats, who talk as cats would. Walk up to one (2.8 m) and press E, or tap
TALK: you type, and the question goes to the site's character chat
(`/api/playtest/npc/ask`) with the conversation so far, the city's controls
(so nobody explains the Haunted House's edit mode) and this visit's stamp.
They remember you: the server keeps every question and answer with the
player key that asked it (`coax/web/api/migrations/0031_npc_memory.sql`), and
a character is told what that player said to it, and what it answered, on
their earlier visits, the last twelve exchanges, oldest first. Nothing one
player said reaches another, and each character remembers only its own
conversations. A browser without a player key is given one when it first
talks. They stand at their spots all day for now; schedules are for later.

| Who | Where |
| --- | --- |
| Two gate guards | Outside the South Gate, either side of the road |
| Stable hand | By the stables' door on the road in |
| Town crier | At the quest board on the commons |
| Errand cat | By the fountain |
| Innkeeper, the inn's cat | At the inn's door on Inn Street |
| Shopkeeper | At the general store's door, behind the north stalls |
| Fruit, cloth and trinket sellers | Behind three of the stalls |
| Smith | At the smithy's door |
| Alchemist | At the alchemist's door |
| Fighters' master, rangers' warden | At their halls' doors, facing each other across Guild Row |
| Archmage, the guild's familiar (a cat) | At the mages' tower's door |
| Priest | On the temple's steps |
| Astronomer | At the observatory's door |
| Woodwarden | On the Deep Wood Trail by the waystone |
| Ferryman | On the river's far bank, by the bridge |
| Ruin scholar | Among the ruins |

## The quests (eg-quests)

The quests are data an agent can write, in `verse/everglade/quests.coax`: a
title, what the board says of it, and up to three steps, each one of three
kinds: GO (reach one of the plan's places, within 12 m), TALK (talk to one of
the plan's people) and RETURN (hand it in at the quest board, or to Oswin the
crier). Every quest ends with RETURN. Each also names what the person it
sends you to says when you come (their first line instead of their
greeting), what the character chat is told of it (so Moss knows why you
came), what the board says when you hand it in, and its reward. The checks
hold every step to a real place or person, a talk step's person near the
place before it, and exactly one quest earning the flock.

| Quest | Steps | Reward |
| --- | --- | --- |
| The Waystone's Word | out by the East Gate to the deep wood; talk to Moss by the waystone; hand it in | |
| Older Than the City | up the North Trail to the ruins; talk to Corin; hand it in | |
| Across the River | out by the West Gate to the river; cross the bridge and talk to Old Tam; hand it in | the bird flock |

At the board on the commons press E (on a phone, tap READ THE QUEST BOARD):
a row a quest with NEW, UNDER WAY, HAND IN or DONE, what the picked one asks,
and TAKE IT or HAND IT IN. 1 2 3 or W and S pick, Enter does it, Esc closes.
`#quest-board` starts you there. While a quest is under way its name and
the step you are on show under the title with how far it is, and a QUEST
mark with the distance stands over where it sends you. Several can be under
way at once; the HUD follows the one you last moved on.

Progress is a stage per quest, kept in this browser for its playtest player
(the host's save slot `everglade`, IndexedDB else localStorage) as
`eg-quests 1`, `player=ID` and `q=S,S,S`. A save made for another player
is not read: their quests start new. It is not on the server, so another
browser or device starts afresh.

## The bird flock (eg-companion)

Hand in Across the River and a flock of seven birds follows you, each
trailing a ribbon of its own colour that narrows to nothing. `verse/flock.coax`
is the engine piece: the birds ride a slow ring over and just ahead of you,
each pulled to its place by a critically damped spring, kept apart, under a
top speed (16 m/s) and acceleration (40 m/s²). They step at a fixed 120 Hz
whatever the frame rate, at most a quarter second a frame, so a phone's uneven
frames neither fling nor stall them; the checks run a phone's uneven frames
round a bend and hold every step under those limits, and a runner never more
than 6.5 m from the flock. Each bird keeps its last ten places (one every
0.08 s) for its ribbon. A link to another place has the flock waiting there.

## What players can and can't do

- **Can:** walk, run and jump anywhere the ground is open; go into the
  buildings the plan gives interiors (the inn, the shops, the guild halls,
  the temple; eg-buildings); talk to the characters (eg-people); take quests
  from the board, a character or the inn, and go out by the gates to the
  quest areas and back (eg-quests); earn the bird-flock companion
  (eg-companion); see the whole city from above (MAP); wade, swim and dive
  in the river by the bridge.
- **Can't:** edit anything. No building, placing, removing, painting or
  terrain tools; no player-made objects stay in the city. An admin
  (`admins.conf`, logged in at /login) is the one exception: see Edit mode. Everglade stays
  put so quests can rely on it; making things is the Sandbox's job. No
  fighting inside the walls. The world ends at its edge, and past the
  trails' ends the forest keeps you to the ways out (the river's glade by
  the bridge is open: you may swim across there).

## The blockout

`/realms/everglade` (module `everglade`, save slot `everglade`) is the plan
as grey boxes at full size, built on the realm host
([realm-host.md](realm-host.md)):

- `verse/everglade/plan.coax`: the layout and its checks.
- `apps/openagents-web/everglade.coax`, `everglade-face.c`, `everglade.wgsl`:
  the realm. It draws the ground and streets (`verse/terrain.coax`), the
  palisade, every building as a box with a gable roof, the props, the
  forest (below), you as the Ranger (below), the characters as plain
  figures and cats, and each
  place's name as you come near. The free orbit
  (`verse/camera.coax`) follows you and keeps out of the walls; walking is
  `verse/player.coax` among the plan's solids, thumbs `verse/grid.coax`.
- M or MAP lifts the camera over the whole city; `#above` opens there, and a
  place's id in the hash starts you at it (`#commons`, `#market`, `#guilds`,
  `#temple`, `#observatory`, `#inn`, `#road`, `#deep-wood`, `#ruins`,
  `#crypt`, `#river`, `#gate`, `#north-gate`).

## You: the Ranger (eg-ranger)

You walk Everglade as the character you were in the previous Everglade (the
Rust engine's, removed in the Nuke; [previous/](../../verse/everglade/previous/README.md)):
the hooded green **Ranger**, Quaternius's Universal male Ranger, the
character the owner calls the Ranger Druid. It is taken whole, byte for
byte, from that engine's last pinned Everglade pack
(`assets/verse/everglade/a82df378….vtp` at `853417f2`, checked by its
SHA-256), not rebuilt from the glTFs, so its mesh, proportions, textures and
clips are exactly the old ones:

- **Mesh:** the `Male_Ranger` outfit (hood, tunic, belts, bracer, trousers,
  boots) with the head and neck of `Superhero_Male_FullBody`, and no hair or
  eyebrows (the hood covers them): 5 parts, 31,396 triangles, 27,402
  vertices, 75 joints.
- **Textures:** the old ones at their own resolution, the outfit 512, the
  body's and head's skin 256, the teeth 128 and the eyes 128; each is a
  layer of the realm's 512-texel texture array, the smaller ones repeated
  texel for texel (no resampling).
- **Clips:** the Universal Animation Library's idle, walk, run, jump,
  backpedal, strafe left and right, and the tree-chopping swing, with the
  old engine's seconds and metres per loop.

`verse/tools/pack_realm.py` (manifest `"character"`) copies it into the pack
(32 bytes a vertex: position, normal and layer, uv, four joints and
weights) and writes its rig to `verse/everglade/ranger.coax`. `verse/skin.coax`
plays it as the old engine did (`Motion::of`, `ClipPlayer`): standing below
0.1 m/s idles, below 5 m/s walks, faster runs, off the ground jumps; a
change of clip blends over 0.22 s from the pose it left; a gait advances by
the ground covered (seconds a loop times metres moved over metres a loop),
so the feet keep to the ground; keys are sampled as the old engine did
(linear translation, spherical rotation). Each frame it writes the 75
joints' skinning matrices to a store the shader reads; the GPU skins the
vertices (four joints each) and draws the Ranger in one draw at your feet,
turned to face the way you go. The controller is unchanged: walking,
running, jumping, the camera and their speeds are `verse/player.coax` and
`verse/camera.coax`'s.

The checks: the pack's catalog names the Ranger with its 75 joints; the
controller's state picks idle, walk, run and jump as the old engine did;
the pose at eight joints, three clips and several times matches the old
engine's sampling (worked out in `pack_realm.py` from the old pack) within
a millimetre; and a phone's whole frame, the Ranger's 31,396 triangles
with the trees, stays under 700,000.

What differs from the old Everglade, and why:

- **Backpedal, strafes and the swing are packed but not chosen.** The old
  controller kept facing the camera and stepped backward or sideways; ours
  always turns you to face the way you go, so you never walk backward or
  sideways, and there is no chopping yet.
- **Speeds:** the old controller ran at 6.4 m/s; ours walks at 3.5 and runs
  at 7.5. The clips are the same; the gait follows the ground covered, so
  the run plays a little faster than it did.
- **Light:** the realm's own lighting (sun, sky, fog) and a soft round
  shadow under you, not the old engine's physically based light and cast
  shadows, so the Ranger reads slightly flatter.

## The forest

Past a 6 m apron of open grass round the palisade, everything is woods
(`everglade_in_woods` in `plan.coax`) but the trails, 3 m either side of
their middle, and six clearings where they end: the stables, where you
arrive on the road in, the deep wood's waystone, the ruins, the crypt and
the river's far bank, and the river's glade either side of the bridge
(an ellipse 14 m across and 52 m along it). You can't walk into the woods and the camera never
backs into them, so the only ways out of the city are the trails.

- **Trees:** one every 4.2 m, jittered, wherever the woods are, with a
  metre clear of open ground round each trunk and none in the river (about
  a thousand). Four kinds from Stylized Nature: `Pine_5`, `Pine_4`,
  `CommonTree_3`, `CommonTree_4`, each with its `_Leaves`.
- **Along the tree line:** `Bush_Common_Flowers`, `Fern_1`,
  `Rock_Medium_1` and `_2`.
- **The pack:** `verse/packs/everglade.json`, compiled to
  `realms/everglade/assets/everglade.pack` (6.0 MB: 6,021,712 bytes,
  budget 6,750,000) with `licences.md`; it carries the town's 44 Medieval
  Village and Fantasy Props pieces as well as the forest's, and the Ranger
  (1.7 MB of it). The budget was 5 MB before the Ranger and rose by the
  Ranger's bytes, rounded up. It also carries 1,024 identity indices (`"identity"`), so
  the city's shapes, drawn from the vertex index alone, share the pack's
  index buffer.
- **The phone:** each frame the trees in view and nearer than 55 m (42 m on
  a phone) are the kit's, at most 80 of each kind (40 on a phone), nearest
  first; the rest are a cone each. The checks hold a phone's kit
  triangles under 700,000 from the road, the gates and the trails, every
  way it looks.

The checks (`plan.coax`):

- **Nothing overlaps:** buildings keep 2 m apart and from the palisade,
  props 1 m; only the stables stand outside the walls; the palisade's pieces
  meet without overlapping.
- **The streets keep clear:** every link of the path graph keeps 0.5 m
  clear of every solid on either side of its 3 m strip.
- **The paths connect:** one network from the South Gate; a link runs
  through the middle of every gate; the river trail crosses on the bridge.
- **Everything is reachable on foot:** a half-metre grid of the world, each
  solid widened by a walker's radius, filled from the road in, reaches every
  door, every character, every path node, every gate, the quest board, the
  stalls, every trail's end and the river's far bank. The river is not in
  the way: in its glade you reach both banks and the water between them;
  past the glade the woods close in on it.
- **The river stays in its bed:** its level falls south at least 8 mm a
  ground row and stands 0.2 m under the ground beside it; the bed is 2.2 m
  under the level at the middle; the river trail climbs onto the bridge's
  deck and off it within the trails' slope.
- **The palisade keeps you in** but for its four gates.
- **The woods keep you to the trails:** no cell in the woods is reachable,
  straight out from each wall the forest stops you inside the apron, the
  far bank is reached by the bridge and every trail stays open to its end.
- **The ground** is level under each building and square, rises into the
  forest, and no street or trail climbs steeper than 1 in 5.

## The buildings

Each building is put together from its footprint in `plan.coax`, in
`building()` in `everglade.coax`:

- **Walls:** the kit's 2 m plaster modules (brick for the smithy, the
  temple, the mages' tower and the observatory), one or more 3.1 m storeys,
  with arched windows along each wall, timber corner posts and a door on the
  side its door faces. Walk-in buildings' doors stand open.
- **Roofs:** `Roof_RoundTiles_6x8` stretched over the footprint with its
  ridge along the long side, and a brick-and-plaster gable at each end; the
  mages' tower has the tower roof and the observatory its dome.
- **The palisade:** rows of logs, with brick towers either side of each
  gate and a log lintel over it.
- **The market stalls** are the kit's stall with crates of apples.
- **Rooms:** the inn, the general store, the smithy, the alchemist, the
  fighters' hall, the rangers' lodge and the temple can be walked into
  (`EG_INTERIOR` in `plan.coax`). Each has a floor and its furniture:
  the inn's tables, chairs, mugs, hearth, benches, barrels and stair; the
  store's shelves and counter; the smithy's anvil, forge and weapon stand;
  the alchemist's cauldron and shelves; the fighters' dummies, racks and
  banners; the rangers' pegs, table and packs; the temple's benches and
  altar. The plan gives each walk-in building four walls with a doorway
  1.9 m wide and checks every room is reachable through it and that the
  wall beside the doorway is shut. `#in-inn`, `#in-store`, `#in-smithy`,
  `#in-alchemist`, `#in-fighters`, `#in-rangers` and `#in-temple` start you
  inside, far enough in that the camera stays in the room.
- **Draw cost:** every kit piece of the town is one instance of one of
  the pack's meshes; the checks hold each mesh's count under its cap and
  the town within 3,061 pieces.

## The river (W3 of the water system)

The river is one body of the water system (docs/verse/water.md, W1 to W3
and W6's ripples), drawn by its shader and read by the swimmer:

- **Its bed:** `everglade_carve_river` in `plan.coax` cuts it into the
  ground after the terrain is built: a level from north (10 m up the
  hills) to south that never rises, the bed a V 2.2 m deep at the middle
  and 0.25 m at its banks 2 m out, climbing one in one to the ground (in
  the south woods, a ravine). The bridge (8 m long, on two piles in the
  stream) spans the cut at the trail's height.
- **Its water:** `verse/everglade/river.coax`, a W1 river along x = 88 from
  the north edge to the south, 6 m wide, on that level, with a 0.45 m/s
  current southward round the bridge's piles and a light chop. The ground
  under it shades as silt, darker the deeper.
- **You in it:** past 0.4 m you wade, past 1.25 m you swim, at half speed
  either way (a sixth less for each level of Exhaustion); afloat you can't
  jump and the current carries you. Hold C to dive. Under water you hold
  your breath for a minute (BREATH shows over the hint); then a level of
  Exhaustion every 6 s, and at the sixth you come to on the nearer bank.
  Surfacing removes them.
- **Swimming across is allowed** beside the bridge: the glade opens the
  river from 40 m south of the crossing to 12 m north of it. Up and down
  the river past the glade the forest closes in on both banks.
- **On the water:** three ducks paddle round their stretch of the glade and
  six lily pads ride on their stems, floating by the physics' buoyancy.
- **Ripples:** a splash where you come into the water and rings as you
  move through it.

## What's next

eg-launch ([realms-port.md](realms-port.md)).

## Edit mode (admins only, eg-edit)

An admin gets EDIT beside MAP at the top right (B on a keyboard); nobody
else does. Edit mode is the Haunted House's (`realm.coax` draw_edit), in
`apps/openagents-web/everglade-edit.coax`: EDITING over the middle with what
just happened, and along the bottom the hotbar CRATE, BARREL, BENCH, FENCE,
STALL (1 to 5 place one in front of you), UNDO (Z), REDO (Y), ORIGINAL and
DONE (B or Esc). Click or tap a placed thing to pick it; its row moves it
LEFT, RIGHT, AWAY and NEAR (the arrows), TURNs it (R), makes it BIGGER or
SMALLER (+ and -) or DELETEs it (Delete); Esc lets go. Click or tap one of
the city's people or cats to pick them, and DELETE takes them away (REMOVED
MARTA). On a phone the hotbar is two rows and the picked thing's row is two
rows of four. A change that would put a thing past the edge, in the woods,
in a building or in another thing is refused and says why. Placed things
are walls from the ground they stand on; a bench or anything else no
higher than a step can be stood on, and the soft shadow falls on its top
(everglade.wgsl shade_shadows_at). The list is published a
moment after the last change and at once on DONE. ORIGINAL asks twice, then
puts the designed city back for everyone (`POST
/api/playtest/world/everglade/reset`); UNDO after it brings the edits back
and publishes them again. Edit mode and voice edits share one list, one
undo and one redo. A removed person is a `p SPOT` line in the list; a
quest whose giver is gone (Moss, Corin, Old Tam) is off the quest board,
and one under way can't be handed in until they're back
(`verse/everglade/quests.coax` eg_quest_open).

## Voice edits (admins only, eg-edit)

An admin sees one HOLD TO TALK button at the bottom middle (hold V on a
computer, press and hold on a phone). The mic is open only while it is held.
The voice host hears the sentence and the page sends it to
`POST /api/playtest/realm/everglade/edit`, which checks admin_who and asks
Gemini for one edit under a response schema of the realm's catalogue
(`migrations/0045_realm_voice_edit.sql`): place 1 to 6 of crate, barrel,
bench, fence or stall in front of you; remove (the nearest placed thing
or person, a person or cat by name, or with `all` many at once: "remove
these crates", "get rid of those benches", "clear this", every one near
and the whole row the nearest was placed with: migration
`0046_realm_voice_remove.sql`); or undo. No words are matched. While the
server answers, what was heard shows; then the result (PLACED 4 CRATES,
PLACED A BARREL, REMOVED MARTA). A sentence with no edit in it shows only
DIDN'T CATCH AN EDIT IN: '...'. The page does the edit
(`verse/everglade/edits.coax`, `apps/openagents-web/everglade-voice.coax`):
a row starts clear of you (your radius, a margin and half the thing) and
goes further out where it doesn't fit, saying how many didn't; it refuses
a place in a building, the woods or another thing; and publishes the edits as
Everglade's world (`POST /api/playtest/world/everglade`, admin site session
only). Everyone sees them. `POST /api/playtest/world/everglade/reset` puts
the designed city back; every version is kept in the realm's log. The
switch is per realm: `"voice": "everglade"` in `apps/openagents-web/realms.json`.
