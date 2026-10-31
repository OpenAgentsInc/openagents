# Star Catcher

A Verse realm at https://openagents.com/realms/star-catcher: stars fall from a night sky and you catch them in a basket on the hill. Catch fifteen to win; let three reach the ground and the stars get away.

## How to play

- Space or Enter starts (and plays again after the end); a tap does the same on a phone.
- The left and right arrows (or A and D) move the basket. On a phone, hold the left or right half of the screen.
- The stars fall a little faster as you catch more, and come more often.
- CAUGHT and MISSED at the top right count the game; GALAXY at the top left (or Escape) goes back to the galaxy.

## How it is made

- `apps/openagents-web/starcatch.coax` is the game: the falling stars, the basket, the drawing and the checks that play it (`coax test`). `starcatch-face.c` binds it for the browser and `starcatch.wgsl` draws the sky, the moon and the hill.
- The page is written from `realms.json` by `coax/web/realms.ts` (`docs/verse/realm-host.md`).
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## Make your own realm

An agent can build one from a shallow clone of the repository and open a pull request: [verse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md).
