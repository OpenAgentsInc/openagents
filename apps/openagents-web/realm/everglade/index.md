# Everglade

Everglade, the forest city, at https://openagents.com/realm/everglade. This is its blockout: the whole city in grey boxes at its real size, so it can be walked and judged before it is built from the art kits. The plan is https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/everglade.md and its map https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/everglade-map.svg.

## What is here

- The road in from the south, past the stables, to the South Gate in a timber palisade. Three more gates lead out: west to the deep wood, east to the river crossing, north to the ruins and the crypt.
- The commons in the middle: the fountain, the quest board and the town crier.
- The market to the east: six stalls, the general store, the smithy and the alchemist.
- The inn on Inn Street, between the gate and the commons.
- Guild Row to the west: the fighters' hall and the rangers' lodge; the mages' tower stands to the north-west.
- Temple Hill to the north: the temple and the observatory.
- Where each of the city's characters will stand, as plain figures: gate guards, the stable hand, the crier, the innkeeper, the shopkeepers, the smith, the alchemist, the guild masters, the priest, the astronomer, and three cats.

Names float over each place as you come near. The city is about 130 metres across inside its walls: half a minute's walk from the South Gate to the temple.

## Controls

- Computer: W A S D walk, Shift runs, Space jumps, drag the mouse or the arrow keys turn the camera, the wheel zooms, M (or the MAP button) lifts the camera over the whole city and back.
- Phone: your left thumb walks, your right thumb turns the camera, a tap jumps, MAP at the top right shows the city from above.
- A link can start you at a place: `#commons`, `#market`, `#guilds`, `#temple`, `#observatory`, `#inn`, `#road`, `#deep-wood`, `#ruins`, `#crypt`, `#river`, `#gate`, `#north-gate`; `#above` opens on the map.
- Nobody can change Everglade: there is no editing here.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- `verse/everglade/plan.coax` is the city's layout: the palisade, every building and prop as a box, the streets and trails, the squares, where each character stands, and the checks that everything is reachable on foot, nothing overlaps and the paths connect.
- `apps/openagents-web/everglade.coax` draws it and the HUD, `everglade-face.c` binds it for the browser, `everglade.wgsl` shades it. The ground and paths are `verse/terrain.coax`, the camera `verse/camera.coax`, walking `verse/player.coax`, and a phone's thumbs `verse/grid.coax`, as in the Garden.
- Nothing is downloaded but the page, its code and a 4 KB index list. The art comes later from the CC0 Quaternius kits (Stylized Nature, Medieval Village, Fantasy Props).
