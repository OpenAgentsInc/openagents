# Splat Lab

A Verse world at https://openagents.com/worlds/splat-lab, not listed on the worlds page. It tests 3D Gaussian splats in the Verse's own WebGPU renderer. A splat is a soft, coloured, see-through blob with a direction and a stretch. Thousands of them, sorted from back to front and blended, can draw fire, smoke, spells or a captured place. The lab asks what splats could do for Everglade and the raid that burns it.

## Scenes

Press 1 to 5, or select a scene's button.

1. FIRE: a pyre on a ring of stones. Its flames, smoke and embers are about 22,500 splats that the GPU moves every frame.
2. SPELLS: a Fireball flies, bursts and leaves a mushroom of smoke, and an Eldritch Blast throws a wound beam of violet light at a standing stone.
3. TOWN: the raid. Ordinary mesh cottages line a street, and two roofs and a cart burn. The splat fire and smoke use the same depth buffer as the meshes, so a wall hides the flames behind it and smoke drifts in front of it.
4. SHRINE: Darian's ritual site, built from about 100,000 splats: standing stones, one of them fallen, an altar, a dead tree, and a chalk circle with violet ritual flame. On a local address, the altar also holds a captured skull (see Credits).
5. BENCH: a meadow of 16,384 to 1,048,576 splats. Press [ and ] or select - and + to change the count, and read the frame rate and the sort's time.

## Controls

- Drag to orbit the camera. The arrow keys orbit it too.
- Use the mouse wheel, a pinch, or the + and - keys to zoom.
- Shift-drag or drag with two fingers to pan.
- Hold W, A, S or D to fly forward, left, back or right. Hold Q to go down and E to go up.
- Press Space to pause, and C to put the camera back.
- Select WORLDS at the top right, or press Escape, to go to the [worlds list](https://openagents.com/worlds).

The readout at the top shows the frame rate, the number of splats, and, when your GPU reports timestamps, how long the sort and the drawing take.

## How it is made

- `apps/openagents-web/splatlab.coax` is the world: the scenes, the camera, the HUD, the authored splats, and the `.splat` and `.ply` loaders.
- `gpu/splatlab-fx.coax` makes each animated splat from its index and the time, sorts all of them with a bitonic sort in fragment passes, and projects each 3D Gaussian to a 2D ellipse on the screen.
- `gpu/splatlab.coax` draws the night sky, the meshes, the bloom and the final picture.
- `coax/web/host.js` runs the sort's passes before the frame (see its "Sorting" notes).
- [docs/verse/2026-11-03-splat-lab.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/2026-11-03-splat-lab.md) has the findings: what works, what it costs, and how splats could come to Everglade for the raid.

## Credits

The skull is the Skull Gaussian splat (`gs_Skull.splat`) from the [Babylon.js Assets repository](https://github.com/BabylonJS/Assets), licensed [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). It isn't in this repository or on openagents.com: a local server loads it from a cache outside the repository. Everything else in the lab is made in code.
