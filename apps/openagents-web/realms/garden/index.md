# The Garden

A small open-air Verse realm you walk in the browser at https://openagents.com/realms/garden. It shows the Verse engine's world pieces at work, one at a time, as they are built for Planetary Villa (https://openagents.com/projects/planetary-villa).

## The camera

The free orbit (`verse/camera.coax`, Orbit): a third-person camera over your right shoulder that you turn freely around you. It turns with a mouse drag, the arrow keys, or your right thumb; the wheel (or + and -) zooms it from a metre and a half to twelve metres. A while after you last turned it, walking swings it gently in behind you, except when you walk toward it. When a tree, a hedge or the ground is between you and where it wants to be, it comes in at once to just short of it, keeping the same view direction, and backs out slowly when the way is clear.

## The ground and the paths

The ground (`verse/terrain.coax`) is a heightfield: rolling hills in a shallow bowl, levelled where the garden's paths run and flat on the plaza and under the pond. The paths are a graph of points and links: a ring round the plaza, the way in from the south, branches to the pond, the orchard and the meadow, and two loops joining them. Everything stands on the ground as it is drawn, and the camera keeps above it.

The dog follows you by the paths. From where it stands it works out the cheapest way to you, counting a metre over grass as 2.2 metres on a path, walks the paths when that is shorter, crosses the grass for the last few metres, runs when you are far off, and stops a couple of metres short of you.

## The ball

A ball lies at your feet where you start (`verse/throw.coax`). Walk up to it and press E (on a phone, the PICK UP button) to pick it up. While you hold it, a dotted arc shows where it would land; it goes the way the camera looks, higher the further up you look. Press E or click (on a phone, THROW) to throw it. It flies under gravity, bounces off the ground, trees, hedges and the pedestal, and rolls downhill until it settles. Then the dog runs for it, carries it back by the paths and drops it at your feet.

## The vase

On the pedestal in the middle of the plaza stands a blue vase with a few flowers in it (`verse/arrange.coax`). Stand beside the pedestal and press E (on a phone, the ARRANGE button): the camera comes in close over the vase, looking down into it. Click (or tap) inside the vase's mouth to put in a flower of the colour picked below (RED, GOLD, WHITE, VIOLET, or keys 1 to 4); it leans outward the nearer the rim it stands. Click a flower's head to take it out; the one under the mouse lights up first. Drag, or the arrow keys, walks the view round the vase. E, Escape or DONE brings the camera back to you. The flowers stay as you left them while the page is open.

## Controls

- Computer: W A S D walk, Shift runs, Space jumps, drag the mouse or hold the arrow keys to turn the camera, the wheel zooms, E picks up the ball and E or a click throws it, and by the pedestal E arranges the vase.
- Phone: your left thumb walks, your right thumb turns the camera, a tap jumps, the button at the bottom right picks up or throws the ball, or arranges the vase.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- `verse/camera.coax` is the camera, `verse/terrain.coax` the ground, the paths and the dog's way along them, `verse/throw.coax` the ball's flight, bounce and roll and the dog's fetch, `verse/arrange.coax` the close-up on the vase, `verse/player.coax` walks you among the trees, hedges and the pedestal (as in the Haunted House; on a flat floor, drawn on the ground), and `verse/grid.coax` reads a phone's thumbs.
- `apps/openagents-web/garden.coax` lays out the garden and draws the HUD; `garden-face.c` binds it for the browser; `garden.wgsl` draws the ground with its paths, every object as a box, sphere, cylinder or cone, soft shadows and fog.
- Nothing is downloaded but the page, its code and a 4 KB index list.
