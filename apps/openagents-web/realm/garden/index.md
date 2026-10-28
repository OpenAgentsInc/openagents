# The Garden

A small open-air Verse realm you walk in the browser at https://openagents.com/realm/garden. It shows the Verse engine's world pieces at work, one at a time, as they are built for Planetary Villa (https://openagents.com/projects/planetary-villa).

## The camera

The free orbit (`verse/camera.coax`, Orbit): a third-person camera over your right shoulder that you turn freely around you. It turns with a mouse drag, the arrow keys (or Q and E), or your right thumb; the wheel (or + and -) zooms it from a metre and a half to twelve metres. A while after you last turned it, walking swings it gently in behind you, except when you walk toward it. When a tree, a hedge or the ground is between you and where it wants to be, it comes in at once to just short of it, keeping the same view direction, and backs out slowly when the way is clear.

## Controls

- Computer: W A S D walk, Shift runs, Space jumps, drag the mouse or hold the arrow keys (Q, E) to turn the camera, the wheel zooms.
- Phone: your left thumb walks, your right thumb turns the camera, a tap jumps.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- `verse/camera.coax` is the camera, `verse/player.coax` walks you among the trees, hedges and the pedestal (as in the Haunted House), and `verse/grid.coax` reads a phone's thumbs.
- `apps/openagents-web/garden.coax` lays out the garden and draws the HUD; `garden-face.c` binds it for the browser; `garden.wgsl` draws the ground, every object as a box, sphere, cylinder or cone, soft shadows and fog.
- Nothing is downloaded but the page, its code and a 4 KB index list.
