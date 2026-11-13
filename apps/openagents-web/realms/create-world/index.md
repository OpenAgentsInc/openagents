# WorldBuilder

Build a 3D world by chatting, at https://openagents.com/create-world. You start on an empty grid with one line to type in at the bottom. Say what to put where, and it appears. All 785 Quaternius models (trees, rocks, plants, houses, castles, furniture, animals and more) are available.

## What you can say

- "put a tree here": click or tap the ground first to choose where "here" is.
- "add five rocks around it", "put a house to the left of the tree"
- "move that further back", "make it bigger", "turn it around"
- "delete the rocks", "fill the area with grass and flowers", "undo"

Drag to turn the view. Scroll or pinch to zoom. UNDO takes back the last change without asking the model.

## Limits, shown on the page

- $50 of AI a day, shared by everyone. When it runs out, WorldBuilder rests until midnight, Chicago time. The page shows today's spend.
- Each visitor can send 4 commands at once, then 1 every 1.5 seconds. If you send too fast, the page shows how long to wait.
- A world holds up to 4,096 things.
- No sign-in. Your world is kept in this browser.

## How it is made

- `apps/openagents-web/worldbuilder.coax` is the page. `worldbuilder-plan.coax` carries out the edits. `worldbuilder-face.c` binds them for the browser, and `worldbuilder.wgsl` (from `gpu/worldbuilder.coax`) draws them, one instanced draw for each model.
- `coax/web/api/worldbuilder.c` asks Gemini on Vertex for typed edits (place, move, rotate, scale, delete, duplicate, undo, clear) with a JSON schema. `worldbuilder-policy.coax` meters each call at Vertex prices and enforces the daily cap and the anti-flood limit. `migrations/0141_worldbuilder.sql` keeps the day's spend.
- `metaverse/tools/assets/worldbuilder_pack.coax` packs the models into `assets/worldbuilder.pack.gz`. The models are by Quaternius (CC0, or the Quaternius Asset License).
