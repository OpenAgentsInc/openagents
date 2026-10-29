# Sandbox

Open ground in the Verse that everyone builds on together, at https://openagents.com/realm/sandbox. There is one Sandbox, shared: what anyone builds there is published, and everyone who comes sees it.

## What is here

- Flat, pale ground a hundred metres across.
- A ring of eight lamps round the starting plot.
- A builder's yard: three crates, a barrel, a workbench and a bench to move, turn, resize or remove.
- Rocks far out.
- The Builder, who says what the place is for. Walk up to him and press E to talk; a typed question gets an answer as in the Haunted House.

## Building

- Computer: B opens edit mode. 1 to 5 place a crate, a barrel, a lamp, a tree or a rock in front of you; click a thing to pick it, then the arrows move it, R turns it, + and - resize it, Delete removes it. Z undoes and Y redoes. B, Escape or DONE play again, and your changes are published for everyone.
- Phone: EDIT opens edit mode; the hotbar's slots place things, a tap picks one, and DONE publishes.
- Walking: W A S D or the arrow keys, drag the mouse to look, Space jumps, Shift runs, the wheel zooms, C opens the chat with everyone here.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- The Sandbox is a world document like the Haunted House's (`Sandbox.pb`, `proto/haunt.proto`), written by `verse/tools/pack_sandbox.py` after the demo's Sandbox. It uses the house's meshes, so it shares the house's geometry and texture sheets.
- The Haunted House's engine and edit tools play it: `verse/haunt.coax`, `apps/openagents-web/realm.coax` and `realm-face.c`. The page opens the shared world `sandbox` from the server (`GET /api/playtest/world/sandbox`) over the document, and edits are published to it when you press DONE (`POST /api/playtest/world/sandbox`). The server keeps it open to everyone's publishing (`coax/web/api/migrations/0029_sandbox_open.sql`); every publish is in its log (`GET /api/playtest/log/sandbox`). A browser without a player key is given one.

## Not here yet

One-sentence set pieces ("build a seasteading city", "a flock of birds follows me") chosen by a typed planner, from the demo's plan.
