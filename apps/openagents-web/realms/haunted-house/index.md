# Haunted House

A Verse realm you walk through in the browser at https://openagents.com/realms/haunted-house: a house at night with nine haunts to set off, characters to talk to, doors to open, and a quest, Make it yours.

It is the same Coax code the OpenAgents iOS app runs, compiled to WebAssembly and drawn with WebGPU:

- `verse/haunt.coax` plays the house: its rooms, objects, lights, people, the nine haunt zones and what they do, doors, conversations and sounds. The house itself is `Haunt.pb` (a protobuf document, `proto/haunt.proto`) with its geometry (`Haunt.vertices`, `Haunt.indices`) and twelve texture sheets.
- `verse/player.coax` walks you through it, against its walls; `verse/grid.coax` reads a phone's thumbs.
- `apps/openagents-web/realm.coax` draws the HUD and decides the controls; `realm-face.c` binds them for the browser as the iOS app's view controller does; `realm.wgsl` draws the house with the Verse engine's own light, fog and tone functions (`verse/haunt-light.coax`, `verse/light.coax`).

## Controls

There is one camera, behind you. Indoors it comes in closer, and any wall, beam or roof between it and you isn't drawn (`cut_away` in `realm.wgsl`), so it never loses you. On screen there are only your name (or Guest), the minimap, one voice button and the hotbar; a panel shows only while it's needed.

- Computer: W A S D or the arrow keys move you, drag the mouse to look, Space jumps, Shift walks, X does a little spin, E acts on what you face (talks to a person, does the quest's step at its orb, opens a door; in a conversation T asks a typed question, E asks for a hint and Escape says goodbye), the wheel moves the camera in and out, M or the minimap opens the map, C opens the chat.
- Placing, as in Minecraft: 1 to 5 (or a hotbar slot) takes a jack-o'-lantern, candlestick, ghost, spider or tombstone in hand; a click on the floor puts it there. Escape or the slot again puts it away. With nothing in hand, a click on a thing picks it (it shows lit): drag it to move it, the wheel or + and - size it, Q and E (or R) turn it, Delete removes it, a click on nothing lets go. Ctrl or Cmd+Z undoes, with Shift (or Ctrl+Y) redoes. Each change is kept, and in a world you own, published.
- Phone: a finger that starts on the stick (bottom left) walks; anywhere else it looks, and a tap places what's in hand or picks a thing (a person in reach talks to you). Drag a picked thing to move it, pinch to size it, twist two fingers to turn it; a quick two-finger tap undoes. The round button on the right jumps; held, it spins. The voice button sits under the minimap.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## The quest: Make it yours

Six steps, one at a time, each at an orange orb (the HUD names the step, and a glow on the screen's edge points to an orb out of view): walk in, pick something from the hotbar, place it, save your house, make it your own, tell us how it went (a typed answer). There's no sharing step: a save made at the old share or invite step goes on to the next. The house, your changes, the step you're on and your answer are kept in this browser and come back on your next visit.

## Talking to the characters

The characters are never quite still: they breathe, shift their weight and glance about, now and then with a little hop, each at their own times. Within four metres they turn to you; in a conversation they nod and gesture as they talk; and when a haunt goes off near them they jump back with a shiver (`verse_haunt_life` in `verse/haunt.coax`).

A question you ask goes to our server, where Google's Gemini answers as the character: from who they are, what players taught them and the last ten lines of the conversation (`POST /api/playtest/npc/ask`). When the server can't answer (an error, the day's answers used up, or nothing back in 12 seconds), the character answers from their own lines. What you teach a character is added to what they know in this house and kept with your save.

## Other players and the chat

Everyone else in the same world is drawn where they stand, a pale figure with their name over it on a plate: the page posts where you are about every second (`POST /api/playtest/presence/<room>`, the room being the world's id) and is answered with everyone seen in the last six seconds, so another player shows within two. A world holds sixteen at once. The chat (CHAT, or C) shows how many are here and the last lines said; a line goes to everyone here (`POST /api/playtest/chat/<room>`, at most 500 characters, five in ten seconds) and is kept, with your account's name or "Guest" and four letters of your player id. A guest takes part too: a browser without a player key is given one.

## A player's own world

A link with a world's id after `#` (`/realms/haunted-house#planet-…`) opens that player's world: what its owner last published (`GET /api/playtest/world/<id>`) over the house. Its owner's edits are published as each one is made (a placing, or a drag, size or turn when it ends) and at the quest's save step; anyone else's are offered to the owner instead (`POST /api/playtest/change/<id>`). The owner sees the newest offer waiting, with who made it, and presses ACCEPT (the world is published with it) or DECLINE (nothing changes); `GET /api/playtest/log/<id>` lists every publish and offer. Nothing is paid. `coax/web/api/realm-flow.ts` plays this in headless Chrome.

## On the screen

Your name (your npub shortened, or Guest) and the quest's step at the top left; the minimap with the room's name at the top right; the voice button; the hotbar at the bottom; on a phone the stick and the jump circle. The house's notices, who or what is near, a picked thing's name and how to change it, a conversation, the chat and the map show only while they matter. Everything is drawn by Coax in the house's own plain look: each panel and button a dark wood plate with one thin bronze line, its words parchment, in the Filament stroke font.

## What the iOS app has that the web doesn't yet

Editing characters and moving zones, importing and switching worlds, and the bloom over bright lights. The Haunted House itself keeps your changes in this browser (and on the server once you join with an email); a player's own world is published to the server.

## Needs

A browser with WebGPU: a current Chrome or Edge, or Safari 26. About 6.6 MB arrives on the first visit (the geometry, the texture sheets and the document; sounds as they first play), and is cached after.

## Credits

Original layout and art by OpenAgents (Apache-2.0); sounds CC0; Quaternius CC0 props (`apps/openagents-ios/OpenAgents/Resources/Haunt-NOTICE.txt`).
