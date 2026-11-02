# Build your own world

At https://openagents.com/worlds/new, the step after the Haunted House. You say the world you want, the page shows it as a short board, you check the features to keep, and MAKE IT makes the world and takes you into it.

## What it does

- First, a few setup questions one at a time, each with big choices, SAY IT or SKIP: 2D (side-on) or 3D; the camera (third person, first person or top-down); the art style (low poly or painterly); the ground; the time of day. What you answer is fixed, and the model fills in the rest from what you say. CHANGE on the board goes through them again.
- Tap TALK (or press Space), say the world you want, and tap it again to send. The microphone is open only while TALK is on; after a few quiet seconds it sends on its own. TYPE IT types it instead.
- The board shows the world's name, 2D or 3D and its camera, its style (low poly or painterly), its ground and time of day, and a checkbox per feature: IN YOUR WORLD (water, trees, rocks, plants, lanterns, a flock, props, characters) and VISITORS CAN (build, fly, jump, a map, talk to characters, voice).
- Tap a row to check it on or off. Talk again to change the board; your checks go with it.
- MAKE IT makes the world from only the checked features and takes you to `/worlds/<slug>`. It is empty except what you checked: characters are there only if you checked them.
- The world is yours: your playtest key's, or your account's when you're logged in. Log in later and it moves to your account.
- WORLDS (or Escape) goes to the [worlds list](https://openagents.com/worlds).
- Without WebGPU the page shows only its name and a link here.

## How it is made

- `apps/openagents-web/builder.coax` is the page, `builder-face.c` binds it, `builder.wgsl` draws it.
- The words go to `POST /api/playtest/realms/plan` (`coax/web/api/realms.c`, `migrations/0048_realm_builder.sql`, `0049_realm_builder_setup.sql`): Gemini answers in a fixed schema, and the server keeps only features from its catalogue of what the engine builds today and writes each row's words itself.
- MAKE IT is `POST /api/playtest/realms` with the plan and the checks. The server writes the world's page to the site with no deploy.
- The full spec: `docs/verse/realm-builder.md`.
