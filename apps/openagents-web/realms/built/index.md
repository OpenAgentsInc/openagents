# A Realm You Built

Every realm made at https://openagents.com/realms/new opens here: `/realms/<slug>` is a page that opens this realm for its slug, and https://openagents.com/realms/built is this module's own page, with a small realm of its own so every release checks it.

## What it does

- The realm is only what its maker checked on the board: water, trees, rocks, ferns, lanterns, a flock, crates, barrels, a wagon, a fence, a camp, and characters. With nothing checked it is empty ground, and no one is in it unless characters were checked.
- Its ground (meadow, forest, desert, snow, stone or beach) and time of day (day, dawn, dusk or night) set its colours and light. A painterly realm's light is softer.
- W and S (or the up and down arrows) walk, A and D turn, a mouse drag turns. On a phone, drag a thumb to walk and turn. Characters' names show when you're near.
- The owner gives visitors their tools in SETTINGS: BUILD, FLY, JUMP, MAP, talking to characters and voice. Only the tools that are on show.
- BUILD places a crate, barrel, tree, rock, fern, lantern, fence or chest in front of you. The owner's goes in at once. A visitor's goes to the owner as a card with ACCEPT and DECLINE.
- GALAXY (or Escape) goes to the galaxy.

## How it is made

- `apps/openagents-web/myrealm.coax` is the realm, `myrealm-face.c` binds it, `myrealm.wgsl` draws it, on the fixture realm's pack (`realms/fixture/assets/fixture.pack`).
- The realm's spec is `GET /api/playtest/realms/<slug>` (`coax/web/api/realms.c`, `migrations/0048_realm_builder.sql`). The features are laid out from the slug, so everyone sees the same realm.
- The full spec: `docs/verse/realm-builder.md`.
