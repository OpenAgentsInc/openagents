# Everglade, the forest city

Status: plan and blockout, October 28, 2026 (eg-design); the forest and
its paths, October 29 (eg-forest). The blockout is
walkable at https://openagents.com/realm/everglade (not on the galaxy until
eg-launch; `/verse/galaxy?realms=all` shows it). The order of the work after
this is in [realms-port.md](realms-port.md).

Everglade is a conventional MMORPG hub in the woods: where players gather,
get quests and set out from. It is the same for everyone and nobody can
change it, so quests and characters can rely on it. The brief is the
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
| **The river crossing** | West, past the West Gate | a 4 m river and a bridge | The ferryman on the far bank. The river runs the length of the world; the bridge is the only way over. |
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
| River crossing | MV `Floor_WoodDark` planks and `Prop_WoodenFence_Single` rails for the bridge; SN `Rock_Medium_*` on the banks; water drawn by the engine. |
| Characters | aethyr's figure parts with the `Peasant` and `Ranger` outfits; the cats are aethyr's `Custom_Cat`. |

## Where characters stand

Each character's day starts at a spot in the plan (`EG_SPOT`: x, z, facing);
eg-people gives them names, schedules and memory. Every spot is checked to
be in the open and reachable on foot.

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

## What players can and can't do

- **Can:** walk, run and jump anywhere the ground is open; go into the
  buildings the plan gives interiors (the inn, the shops, the guild halls,
  the temple; eg-buildings); talk to the characters (eg-people); take quests
  from the board, a character or the inn, and go out by the gates to the
  quest areas and back (eg-quests); earn the bird-flock companion
  (eg-companion); see the whole city from above (MAP).
- **Can't:** edit anything. No building, placing, removing, painting or
  terrain tools; no player-made objects stay in the city. Everglade stays
  put so quests can rely on it; making things is the Sandbox's job. No
  fighting inside the walls. The world ends at its edge, and past the
  trails' ends the forest and the river keep you to the ways out.

## The blockout

`/realm/everglade` (module `everglade`, save slot `everglade`) is the plan
as grey boxes at full size, built on the realm host
([realm-host.md](realm-host.md)):

- `verse/everglade/plan.coax`: the layout and its checks.
- `apps/openagents-web/everglade.coax`, `everglade-face.c`, `everglade.wgsl`:
  the realm. It draws the ground and streets (`verse/terrain.coax`), the
  palisade, every building as a box with a gable roof, the props, the
  forest (below), the characters as plain figures and cats, and each
  place's name as you come near. The free orbit
  (`verse/camera.coax`) follows you and keeps out of the walls; walking is
  `verse/player.coax` among the plan's solids, thumbs `verse/grid.coax`.
- M or MAP lifts the camera over the whole city; `#above` opens there, and a
  place's id in the hash starts you at it (`#commons`, `#market`, `#guilds`,
  `#temple`, `#observatory`, `#inn`, `#road`, `#deep-wood`, `#ruins`,
  `#crypt`, `#river`, `#gate`, `#north-gate`).

## The forest

Past a 6 m apron of open grass round the palisade, everything is woods
(`everglade_in_woods` in `plan.coax`) but the trails, 3 m either side of
their middle, and six clearings where they end: the stables, where you
arrive on the road in, the deep wood's waystone, the ruins, the crypt and
the river's far bank. You can't walk into the woods and the camera never
backs into them, so the only ways out of the city are the trails.

- **Trees:** one every 4.2 m, jittered, wherever the woods are, with a
  metre clear of open ground round each trunk and none on the river (about
  a thousand). Four kinds from Stylized Nature: `Pine_5`, `Pine_4`,
  `CommonTree_3`, `CommonTree_4`, each with its `_Leaves`.
- **Along the tree line:** `Bush_Common_Flowers`, `Fern_1`,
  `Rock_Medium_1` and `_2`.
- **The pack:** `verse/packs/everglade.json`, compiled to
  `realm/everglade/assets/everglade.pack` (1.8 MB, budget 3 MB) with
  `licences.md`. It also carries 1,024 identity indices (`"identity"`), so
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
  stalls, every trail's end and the river's far bank.
- **The palisade keeps you in** but for its four gates.
- **The woods keep you to the trails:** no cell in the woods is reachable,
  straight out from each wall the forest stops you inside the apron, the
  far bank is reached by the bridge and every trail stays open to its end.
- **The ground** is level under each building and square, rises into the
  forest, and no street or trail climbs steeper than 1 in 5.

## What's next

eg-buildings (the kit buildings and the interiors above), eg-people,
eg-quests, eg-companion and eg-launch, in that order
([realms-port.md](realms-port.md)).
