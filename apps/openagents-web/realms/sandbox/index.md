# Sandbox

Open ground in the Verse that everyone builds on together, at https://openagents.com/realms/sandbox. There is one Sandbox, shared: what anyone builds there is published, and everyone who comes sees it.

## What is here

- Flat, pale ground a hundred metres across.
- A ring of eight lamps round the starting plot.
- A builder's yard: three crates, a barrel, a workbench and a bench to move, turn, resize or remove.
- Rocks far out.
- The Builder, who says what the place is for. Walk up to him and press E to talk; a typed question gets an answer as in the Haunted House.

## Building

- Computer: B opens edit mode. P makes a set piece from a sentence (below). 1 to 5 place a crate, a barrel, a lamp, a tree or a rock in front of you; click a thing to pick it, then the arrows move it, R turns it, + and - resize it, Delete removes it. Z undoes and Y redoes. B, Escape or DONE play again, and your changes are published for everyone.
- Phone: EDIT opens edit mode; the hotbar's slots place things, a tap picks one, and DONE publishes.
- Walking: W A S D or the arrow keys (you run), drag the mouse to look, Space jumps, Shift walks, the wheel zooms, C opens the chat with everyone here.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- The Sandbox is a world document like the Haunted House's (`Sandbox.pb`, `proto/haunt.proto`), written by `verse/tools/pack_sandbox.py` after the demo's Sandbox. It uses the house's meshes, so it shares the house's geometry and texture sheets.
- The Haunted House's engine and edit tools play it: `verse/haunt.coax`, `apps/openagents-web/realm.coax` and `realm-face.c`. The page opens the shared world `sandbox` from the server (`GET /api/playtest/world/sandbox`) over the document, and edits are published to it when you press DONE (`POST /api/playtest/world/sandbox`). The server keeps it open to everyone's publishing (`coax/web/api/migrations/0029_sandbox_open.sql`); every publish is in its log (`GET /api/playtest/log/sandbox`). A browser without a player key is given one.

## Set pieces from one sentence

- In edit mode, MAKE A SET PIECE (or P) takes one sentence, such as "a flock of birds follows me" or "a ring of blue lanterns", and the world gets that piece in front of you.
- There are four pieces: a flock of birds (crows or bats) that flies round whoever is walking, a ring of lanterns in a colour, a grove of trees (round, pine, dead or mixed), and a pond ringed with stones. Each has a count, a kind, a colour or a size where it makes sense.
- Z takes the whole piece back in one step. The piece is published to the shared Sandbox at once, so everyone sees it.
- How it works: the sentence goes to our server (`POST /api/playtest/sandbox/piece`), where Google's Gemini picks one piece from this fixed list and its fields, under a schema of the list's own words. The server checks the answer against the list (`coax/web/api/migrations/0036_sandbox_pieces.sql`) and keeps the sentence with what was picked. The page checks the answer again and lays the piece out with a seeded generator (`verse/setpiece.coax`). The Sandbox's engine (`verse/haunt.coax`) draws it. A sentence the list can't make, like a castle, gets an answer saying what it can make.

## Make your own realm

An agent can build one from a shallow clone of the repository and open a pull request: [verse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md).
