# Haunted House

A Verse realm you walk through in the browser at https://openagents.com/realm/haunted-house: a house at night with nine haunts to set off, characters to talk to, doors to open, and a quest, Make it yours.

It is the same Coax code the OpenAgents iOS app runs, compiled to WebAssembly and drawn with WebGPU:

- `verse/haunt.coax` plays the house: its rooms, objects, lights, people, the nine haunt zones and what they do, doors, conversations and sounds. The house itself is `Haunt.pb` (a protobuf document, `proto/haunt.proto`) with its geometry (`Haunt.vertices`, `Haunt.indices`) and twelve texture sheets.
- `verse/player.coax` walks you through it, against its walls; `verse/grid.coax` reads a phone's thumbs.
- `apps/openagents-web/realm.coax` draws the HUD and decides the controls; `realm-face.c` binds them for the browser as the iOS app's view controller does; `realm.wgsl` draws the house with the Verse engine's own light, fog and tone functions (`verse/haunt-light.coax`, `verse/light.coax`).

## Controls

- Computer: W A S D or the arrow keys walk, drag the mouse to look, Space jumps, Shift runs, E acts on what you face (talks to a person, does the quest's step at its orb, opens a door; in a conversation, E asks for a hint and Escape says goodbye), V changes the view, M opens the map, the wheel zooms (out of first person into third, and back in). B opens edit mode (the camera steps behind you): 1 to 5 place a jack-o'-lantern, a candlestick, a ghost, a spider or a tombstone in front of you; click a thing to pick it, then the arrows move it across and along your view, R turns it, + and - resize it, Delete removes it, and Escape lets go. With nothing picked, the nearest haunt zone (within three metres) shows: BIGGER, SMALLER, ONCE or ALWAYS, RESET. Z undoes and Y redoes (sixteen steps). B, Escape or DONE play again.
- Phone: your left thumb walks, your right thumb looks, a tap jumps. The HUD's buttons (VIEW, MAP, EDIT, EXIT, edit mode's things, edits, UNDO and REDO, and the prompt for what you face) take taps; in edit mode a tap on a thing picks it.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## The quest: Make it yours

Eight steps, one at a time, each at an orange orb (the HUD names the step, and a glow on the screen's edge points to an orb out of view): walk in, edit the hall, place something, save your house, share it (copies the link), make it your own, invite a friend (copies the link), tell us how it went (a typed answer). The house, your changes, the step you're on and your answer are kept in this browser and come back on your next visit.

## On the screen

The room you're in, the quest's progress and its step, the house's notices as haunts go off, who or what is near, a conversation, the map and the hotbar. Everything is drawn by Coax in the house's own plain look: each panel and button a dark wood plate with one thin bronze line, its words parchment, in the Filament stroke font.

## What the iOS app has that the web doesn't yet

Editing characters and moving zones, asking characters typed questions and teaching them, publishing to a server (Save keeps the house in this browser, and the survey answer stays there until the playtest server takes it), importing and switching worlds, realms of your own, and the bloom over bright lights.

## Needs

A browser with WebGPU: a current Chrome or Edge, or Safari 26. About 6.6 MB arrives on the first visit (the geometry, the texture sheets and the document; sounds as they first play), and is cached after.

## Credits

Original layout and art by OpenAgents (Apache-2.0); sounds CC0; Quaternius CC0 props (`apps/openagents-ios/OpenAgents/Resources/Haunt-NOTICE.txt`).
