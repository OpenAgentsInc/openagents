# Dead Gears

A Verse realm at https://openagents.com/realms/dead-gears: a ship's hold to hold against three waves of mutants. Between waves you repair the reactor; three repairs and the ship holds.

## How to play

- Keyboard: WASD walk, Shift runs, drag the mouse (or the arrow keys) to turn, F fires the cutter (hold it), R reloads, C takes cover at a cover block, Q changes the cutter's mode, G lifts a crate and throws it, T stuns what is in front of you, E (held at the reactor) repairs it, Escape pauses.
- Phone: the left thumb walks, the right thumb looks; FIRE (REPAIR between waves), COVER, RELOAD, MODE, LIFT and STUN are buttons at the bottom right, PAUSE at the top right.
- The cutter has three modes. SEVER cuts a limb off (a crawler loses legs, a reaver an arm); HEAD takes the head, which keeps crawling on its own; CORE hits the body hardest.
- Lift costs 15 psi and holds a crate in front of you; throw it into a creature to stun and hurt it. Stun costs 35 psi and freezes everything in front of you for a moment. Psi comes back while you don't use it.
- The support engineer follows you, fights beside you and walks over to help repair.
- Each wave comes later in the day: the afternoon, the golden hour, then dusk, and the lamps come up as the light goes. Alarms flash as a wave comes.
- The fight is kept in this browser at each wave and each repair: come back and it goes on from there. AGAIN starts over; GALAXY goes back to the galaxy.
- `?hour=17.5` holds the deck at that hour; `?spawn=200` adds 200 more mutants, to see a crowd.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- The game is the original Dead Gears from the Aethyr demo, ported to Coax. `verse/deadgears/` holds its entity system (`ecs.coax`), the hold's collisions and walking grid (`arena.coax`) and its rules (`rules.coax`), with every check of the original's tests.
- `apps/openagents-web/deadgears.coax` is the page: the ship's hull, the cover, the reactor, the cargo, the engineers and the creatures, all boxes as the original drew them; the camera, the controls, the HUD, the hour and the lamps, and the saves. `deadgears-face.c` binds it for the browser and `deadgears.wgsl` draws it.
- The page is written from `realms.json` by `coax/web/realms.ts`. The contract every realm follows is `docs/verse/realm-host.md` in the OpenAgents repository.
