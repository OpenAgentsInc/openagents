# A World You Built

Every world made at https://openagents.com/worlds/new opens here: `/worlds/<slug>` is a page that opens this world for its slug, and https://openagents.com/worlds/built is this module's own page, with a small world of its own so every release checks it.

## What it does

- The world is only what its maker checked on the board: water, trees, rocks, ferns, lanterns, a flock, crates, barrels, a wagon, a fence, a camp, and characters. With nothing checked it is empty ground, and no one is in it unless characters were checked.
- Its ground (meadow, forest, desert, snow, stone or beach) and time of day (day, dawn, dusk or night) set its colours and light. A painterly world's light is softer.
- W and S (or the up and down arrows) walk, A and D turn, a mouse drag turns. On a phone, drag a thumb to walk and turn. Characters' names show when you're near.
- The owner gives visitors their tools in SETTINGS: BUILD, FLY, JUMP, MAP, talking to characters and voice. Only the tools that are on show.
- BUILD places a crate, barrel, tree, rock, fern, lantern, fence or chest in front of you. The owner's goes in at once. A visitor's goes to the owner as a card with ACCEPT and DECLINE.
- WORLDS (or Escape) goes to the [worlds list](https://openagents.com/worlds).

## How it is made

- `apps/openagents-web/myrealm.coax` is the world, `myrealm-face.c` binds it, `myrealm.wgsl` draws it, on the fixture world's pack (`realms/fixture/assets/fixture.pack`).
- The world's spec is `GET /api/playtest/realms/<slug>` (`coax/web/api/realms.c`, `migrations/0048_realm_builder.sql`). The features are laid out from the slug, so everyone sees the same world.
- The full spec: `docs/verse/realm-builder.md`.

## Make your own world

An agent can build one from a shallow clone of the repository and open a pull request: [verse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md).
