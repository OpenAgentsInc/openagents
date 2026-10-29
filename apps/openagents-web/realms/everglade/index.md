# Everglade

Everglade, the forest city, at https://openagents.com/realms/everglade. The whole city at its real size, built from art kits: timber and plaster houses with tiled roofs inside a log palisade, standing in a forest of kit trees. Seven of its buildings can be walked into. The plan is https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/everglade.md and its map https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/everglade-map.svg.

## What is here

- The road in from the south, past the stables, to the South Gate in a timber palisade. Three more gates lead out: east to the deep wood, west to the river crossing, north to the ruins and the crypt.
- The commons in the middle: the fountain, the quest board and the town crier.
- The market to the west: six stalls, the general store, the smithy and the alchemist.
- The inn on Inn Street, between the gate and the commons.
- Rooms you can walk into: the inn (tables, a hearth, a stair), the general store, the smithy, the alchemist, the fighters' hall, the rangers' lodge and the temple. Their doors stand open.
- Guild Row to the east: the fighters' hall and the rangers' lodge; the mages' tower stands to the north-east.
- Temple Hill to the north: the temple and the observatory.
- The forest all round: past a strip of open grass outside the walls the trees stand too thick to walk into, so the only ways out are the trails, to clearings at the stables, the deep wood's waystone, the ruins, the crypt and the river's far bank, and the river's glade by the bridge. Bushes, ferns and rocks line the trees' edge.
- The river past the West Gate runs the length of the world in a carved bed. Cross by the bridge, or wade and swim across in the glade beside it: half speed in the water, the current carries you, hold C to dive. Under water you hold your breath for a minute; then you tire, and if you stay down too long you come to on the bank. Ducks and lily pads float on it.
- You walk as the Ranger, the hooded green ranger you were in the previous Everglade, with its walk, run, jump and idle.
- The city's people, as plain figures, and three cats: the gate guards Bram and Tilda, Pip the stable hand, Oswin the crier, Marta at the inn, Hollis at the general store, the stallholders Fern, Ada and Quill, Garrick the smith, Isolde the alchemist, Rook and Sable of the guilds, Elowen the archmage, Brother Anselm at the temple, Vega the astronomer, Moss the woodwarden, Old Tam the ferryman, Corin among the ruins, and the cats Pebble, Soot and Wisp.

## Talking to people

Walk up to anyone and press E (on a phone, tap TALK). Type what you want to say and press Enter; they answer as themselves. Esc or BYE ends it. They remember you: tell Marta your name or what you like, come back another day, and she can bring it up. What you say to one of them is kept for that one only, against this browser's player key (the same one the Haunted House uses).

Names float over each place as you come near. The city is about 130 metres across inside its walls: half a minute's walk from the South Gate to the temple.

## Quests

The quest board stands on the commons by the fountain. Walk up to it and press E (on a phone, tap READ THE QUEST BOARD). Three quests are posted:

- The Waystone's Word: out by the East Gate to the deep wood, to see that Moss the woodwarden is well.
- Older Than the City: up the North Trail to the ruins, to hear what Corin has found.
- Across the River: out by the West Gate, over the bridge to Old Tam the ferryman. Hand this one in and a flock of birds with coloured ribbons follows you from then on.

Take one, go where it says (the step you are on and how far it is show under EVERGLADE, and a QUEST mark stands over the place), talk to whoever it sends you to, and hand it in at the board or to Oswin the crier. Your quests are kept in this browser for your player, so they are there when you come back.

## Controls

- Computer: W A S D walk, Shift runs, Space jumps, C dives in the river, drag the mouse or the arrow keys turn the camera, the wheel zooms, M (or the MAP button) lifts the camera over the whole city and back.
- Phone: your left thumb walks, your right thumb turns the camera, a tap jumps, MAP at the top right shows the city from above.
- `#quest-board` starts you at the quest board.
- A link can start you beside someone, near enough to talk: `#marta`, `#old-tam`, `#pebble` and so on, by their name.
- A link can start you at a place: `#commons`, `#market`, `#guilds`, `#temple`, `#observatory`, `#inn`, `#road`, `#deep-wood`, `#ruins`, `#crypt`, `#river`, `#gate`, `#north-gate`; inside a building: `#in-inn`, `#in-store`, `#in-smithy`, `#in-alchemist`, `#in-fighters`, `#in-rangers`, `#in-temple`; `#above` opens on the map.
- Players can't change Everglade. An admin logged in at /login gets EDIT by MAP (B): a hotbar of crates, barrels, benches, fences and stalls (1 to 5), a click or tap picks a placed thing to move (arrows), turn (R), resize (+ and -) or delete, with UNDO (Z), REDO (Y), ORIGINAL (the designed city back) and DONE. An admin also gets HOLD TO TALK at the bottom middle (hold V, or press and hold on a phone) and edits by voice: "put two barrels here", "remove that", "undo". Everyone sees the edits.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- `verse/everglade/plan.coax` is the city's layout: the palisade, every building and prop as a box, the streets and trails, the squares, where each character stands, and the checks that everything is reachable on foot, nothing overlaps and the paths connect.
- The quests are `verse/everglade/quests.coax`, data an agent can write: each a title, a notice and steps (go to a place, talk to someone, hand it in), with checks that every step leads somewhere real. The bird flock is `verse/flock.coax`.
- The people are `verse/everglade/people.coax`: who stands at each spot, their role, greeting and what they know of the city. Talking goes to the site's character chat (`/api/playtest/npc/ask`, Gemini on Vertex AI), which keeps what each player said to each character and tells the character on their next visit (`coax/web/api/migrations/0031_npc_memory.sql`).
- `apps/openagents-web/everglade.coax` draws it and the HUD, `everglade-face.c` binds it for the browser, `everglade.wgsl` shades it. The river is `verse/everglade/river.coax` on the water system (`verse/water.coax`, `verse/water-look.coax`; https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/water.md). The ground and paths are `verse/terrain.coax`, the camera `verse/camera.coax`, walking `verse/player.coax`, and a phone's thumbs `verse/grid.coax`, as in the Garden.
- The forest's trees, bushes, ferns and rocks are from the CC0 Quaternius Stylized Nature kit, and the walls, roofs, doors, windows, palisade and furniture from the CC0 Quaternius Medieval Village and Fantasy Props kits, all in one asset pack (`everglade.pack`, about 6.0 MB) streamed when you enter, with the Ranger: Quaternius's CC0 Universal male Ranger, taken whole from the previous Everglade's pack, its clips from the CC0 Universal Animation Library, played by `verse/skin.coax` and skinned on the GPU; the full list is [licences.md](licences.md). `verse/tools/pack_realm.py` compiles it from `verse/packs/everglade.json`. The trees nearest you are the kit's; farther ones are simple cones, so a phone keeps its frame rate. Each building's walls, doors and rooms are worked out from its footprint in the plan, which checks every room can be walked into and out of.
