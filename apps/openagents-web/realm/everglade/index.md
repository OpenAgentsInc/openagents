# Everglade

Everglade, the forest city, at https://openagents.com/realm/everglade. The whole city at its real size, built from art kits: timber and plaster houses with tiled roofs inside a log palisade, standing in a forest of kit trees. Seven of its buildings can be walked into. The plan is https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/everglade.md and its map https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/everglade-map.svg.

## What is here

- The road in from the south, past the stables, to the South Gate in a timber palisade. Three more gates lead out: east to the deep wood, west to the river crossing, north to the ruins and the crypt.
- The commons in the middle: the fountain, the quest board and the town crier.
- The market to the west: six stalls, the general store, the smithy and the alchemist.
- The inn on Inn Street, between the gate and the commons.
- Rooms you can walk into: the inn (tables, a hearth, a stair), the general store, the smithy, the alchemist, the fighters' hall, the rangers' lodge and the temple. Their doors stand open.
- Guild Row to the east: the fighters' hall and the rangers' lodge; the mages' tower stands to the north-east.
- Temple Hill to the north: the temple and the observatory.
- The forest all round: past a strip of open grass outside the walls the trees stand too thick to walk into, so the only ways out are the trails, to clearings at the stables, the deep wood's waystone, the ruins, the crypt and the river's far bank. Bushes, ferns and rocks line the trees' edge.
- Where each of the city's characters will stand, as plain figures: gate guards, the stable hand, the crier, the innkeeper, the shopkeepers, the smith, the alchemist, the guild masters, the priest, the astronomer, and three cats.

Names float over each place as you come near. The city is about 130 metres across inside its walls: half a minute's walk from the South Gate to the temple.

## Controls

- Computer: W A S D walk, Shift runs, Space jumps, drag the mouse or the arrow keys turn the camera, the wheel zooms, M (or the MAP button) lifts the camera over the whole city and back.
- Phone: your left thumb walks, your right thumb turns the camera, a tap jumps, MAP at the top right shows the city from above.
- A link can start you at a place: `#commons`, `#market`, `#guilds`, `#temple`, `#observatory`, `#inn`, `#road`, `#deep-wood`, `#ruins`, `#crypt`, `#river`, `#gate`, `#north-gate`; inside a building: `#in-inn`, `#in-store`, `#in-smithy`, `#in-alchemist`, `#in-fighters`, `#in-rangers`, `#in-temple`; `#above` opens on the map.
- Nobody can change Everglade: there is no editing here.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- `verse/everglade/plan.coax` is the city's layout: the palisade, every building and prop as a box, the streets and trails, the squares, where each character stands, and the checks that everything is reachable on foot, nothing overlaps and the paths connect.
- `apps/openagents-web/everglade.coax` draws it and the HUD, `everglade-face.c` binds it for the browser, `everglade.wgsl` shades it. The ground and paths are `verse/terrain.coax`, the camera `verse/camera.coax`, walking `verse/player.coax`, and a phone's thumbs `verse/grid.coax`, as in the Garden.
- The forest's trees, bushes, ferns and rocks are from the CC0 Quaternius Stylized Nature kit, and the walls, roofs, doors, windows, palisade and furniture from the CC0 Quaternius Medieval Village and Fantasy Props kits, all in one asset pack (`everglade.pack`, about 4.3 MB) streamed when you enter; the full list is [licences.md](licences.md). `verse/tools/pack_realm.py` compiles it from `verse/packs/everglade.json`. The trees nearest you are the kit's; farther ones are simple cones, so a phone keeps its frame rate. Each building's walls, doors and rooms are worked out from its footprint in the plan, which checks every room can be walked into and out of.
