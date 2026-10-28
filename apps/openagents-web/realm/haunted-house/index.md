# Haunted House

A Metareal realm you walk through in the browser at https://openagents.com/realm/haunted-house: a house at night with nine haunts to set off, characters to talk to and doors to open.

It is the same Coax code the OpenAgents iOS app runs, compiled to WebAssembly and drawn with WebGPU:

- `metareal/haunt.coax` plays the house: its rooms, objects, lights, people, the nine haunt zones and what they do, doors, conversations and sounds. The house itself is `Haunt.pb` (a protobuf document, `proto/haunt.proto`) with its geometry (`Haunt.vertices`, `Haunt.indices`) and twelve texture sheets.
- `metareal/player.coax` walks you through it, against its walls; `metareal/grid.coax` reads a phone's thumbs.
- `apps/openagents-web/realm.coax` draws the HUD and decides the controls; `realm-face.c` binds them for the browser as the iOS app's view controller does; `realm.wgsl` draws the house with metareal's own light, fog and tone functions (`metareal/haunt-light.coax`, `metareal/light.coax`).

## Controls

- Computer: W A S D or the arrow keys walk, drag the mouse to look, Space jumps, Shift runs, E opens a door or talks to whoever is near (in a conversation, E asks for a hint and Escape says goodbye), V changes the view, M opens the map, the wheel zooms.
- Phone: your left thumb walks, your right thumb looks, a tap jumps. The HUD's buttons (VIEW, MAP, EXIT and the prompt for whoever is near) take taps.

## On the screen

The room you're in and how many of the nine haunts you've found, the house's notices as haunts go off, who or what is near, a conversation, the map and the hotbar. Everything is drawn by Coax: the Metareal HUD bar and the Filament stroke font.

## What the iOS app has that the web doesn't yet

Editing the house (moving, adding and removing objects, characters and zones), asking characters typed questions and teaching them, quests past exploring, saving, sharing, importing and switching worlds, and the bloom over bright lights. On the web the house starts fresh on every visit.

## Needs

A browser with WebGPU: a current Chrome or Edge, or Safari 26. About 6.6 MB arrives on the first visit (the geometry, the texture sheets and the document; sounds as they first play), and is cached after.

## Credits

Original layout and art by OpenAgents (Apache-2.0); sounds CC0; Quaternius CC0 props (`apps/openagents-ios/OpenAgents/Resources/Haunt-NOTICE.txt`).
