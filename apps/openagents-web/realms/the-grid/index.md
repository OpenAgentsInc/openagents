# The Grid

The bare Verse world, at https://openagents.com/realms/the-grid: a floor of lines that runs out 264 metres in every direction and fades into the dark, with the places the Verse began with standing by the start.

## What is here

- The Pylon terminal, four metres ahead of where you start, and the help desk ("Ask us"), the suggestion box and the build board near it.
- The Foreman, a robot three times the size of the others, standing by the start.
- Robots walking their rounds, and one waving.
- The gym with its boards.
- The gate to Everglade: walk into it and you go to [Everglade](https://openagents.com/realms/everglade).
- Everyone else walking the Grid right now, on the web or in the OpenAgents app, each with their name over them. The homepage shows them walking too.

Each place's name shows over it as you come near. Walk up to a station and press E (on a phone, tap the button that shows) to open its panel: what it is, a few answers, and links to the site's pages. The suggestion box takes a line of text and sends it to us. Esc or CLOSE shuts the panel.

## Playing

- Computer: W A S D or the arrow keys move and turn (you run), drag the mouse to look, Space jumps, Shift walks, the wheel or + and - zoom.
- Phone: the left thumb walks, the right thumb looks, a tap jumps.
- GALAXY, top right, goes back to the [galaxy](https://openagents.com/verse/galaxy); HOME, beside it, to the [homepage](https://openagents.com/), whose WALK AROUND brings you back here.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- The art is OpenAgents' own Grid scene from the first Verse demo, packed for the browser by `verse/tools/pack_grid.py`: the faces and lines in their flat colours, the robot and its walk, idle and wave clips, and the robots' recorded rounds.
- The page is `apps/openagents-web/thegrid.coax` with `thegrid-face.c`, drawn by `thegrid.wgsl`; you walk with the engine's controller (`verse/player.coax`) and see it through its camera (`verse/camera.coax`). You and everyone else are the human character from `Cast.bin` (`verse/cast.coax`), the one the app's own Grid uses; who is here comes from the playtest room `the-grid` (`thegrid-people.coax`).
