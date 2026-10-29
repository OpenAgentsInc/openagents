# Ice Cream Man

A Verse realm at https://openagents.com/realm/ice-cream-man: the Ice Cream Man's house, built in the browser from a seed, and the game played in it. Find the three check-out keys, then hold Use at the front door to escape before he scoops you.

## How to play

- Walk with W A S D or the arrow keys, hold Shift to sprint, and Space jumps. Drag to look around and scroll to zoom. Ctrl turns on Shift Lock (over the shoulder, with a crosshair); V switches between first and third person.
- Click to use your tool: 1 Hot Fudge, 2 the Fart Launcher, 3 the Key Ring, 4 and up the weapons you earn; Q picks the next one. E hides in a closet, refills at a fridge and, held at the front door, escapes. Esc opens the menu.
- On a phone: the stick on the left walks, drag on the right to look, and the Fire, Jump, Use, Sprint and View buttons sit under your right thumb. Tap a tool in the bar to pick it, or Next.
- The first visit starts with a four-step tour; Help in the menu shows it again.
- Add `#seed=12` (any number) to the address for another house; without one it is house 3. `#easy` and `#hard` set the difficulty, `#bot` lets the game's own bot play.
- Galaxy in the menu (or Escape on the end card) goes back to the galaxy, where Ice Cream Man has its own orbit at https://openagents.com/verse/galaxy.
- This browser keeps your settings (timer, jumpscares, sprint, difficulty), your best escape on each difficulty and whether you've seen the tour. A private window keeps nothing.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- `verse/icecream/` holds the game in Coax: `rules.coax` (the house and the rules, replaying every recorded trace of the original game step for step), `mesh.coax` (every room, piece of furniture and model as vertices, checked against the original C++), `camera.coax` (the moving camera), `play.coax` (the play loop: input, the 30 steps a second, the effects and the trace) and `scene.coax` (the house and everything that moves, packed for the GPU).
- `apps/openagents-web/icecream.coax` is the HUD and its cards, `icecream-face.c` binds both for the browser, `icecream.wgsl` draws them: banded light, each room's fluorescent flicker, the fog, the ink outlines and the HUD's plates.
- Add `#trace=600` and the page hands out the first 600 steps' inputs; `verse/icecream/test-browser.sh` plays the page headless and replays what it recorded on the native rules, step for step.
- The page is written from `realms.json` by `coax/web/realms.ts`. The contract every realm follows is `docs/verse/realm-host.md` in the OpenAgents repository.
