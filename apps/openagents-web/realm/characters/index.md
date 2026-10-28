# Characters

A Verse realm at https://openagents.com/realm/characters that shows the engine's characters: figures built from a few dozen rounded bones, posed and moved by the engine and drawn in a night garden.

- Two shadow visitors: soft silhouettes, black all through, their edges a fuzzy fringe that thins into the night, lit by nothing but a rim of moonlight.
- A guest drawn lit, in its coat's colours, to compare.

Choose a figure by tapping or clicking it, then its gait (STAND, WALK, RUN) and its look (SOFT, a silhouette, or LIT). Drag to turn the view; the wheel zooms. On a computer, 1, 2 and 3 pick the gait, L the look, and the arrow keys or Tab the figure.

## How it is made

- `verse/figure.coax` is the engine's figure: a rig (a person, or a four-legged animal), a shape (size, legs, body, girth, head, and for an animal its snout, ears, tail and coat) and a pose (each joint's turn, held to the joint's limits). It places the posed bones in the world and works out the gaits from a clock.
- `apps/openagents-web/characters.coax` is the realm: the figures, where they walk, the view and the controls.
- `apps/openagents-web/characters.wgsl` draws it: each pixel's ray is marched through the figures' bones, melted into one smooth body; a soft silhouette's edge is how near the ray passed, so it fades out instead of ending at a line.

Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.
