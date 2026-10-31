# A First Realm

Where a project made from a drop first opens, at https://openagents.com/realms/drop. When someone drops 3D models on NEW PROJECT at https://openagents.com/projects and publishes the project, its models are stood on an empty floor at `/projects/<slug>/realm`, linked from the project's page, so the project has somewhere to walk on day one. This page is the same realm with the fixture realm's kit pieces, so every release checks it.

## What it does

- The models stand in a ring round where you start, each facing in, on the floor. One bigger than 5 m is shown smaller; one made in centimetres is shown bigger.
- You carry a lamp among them. W and S (or the up and down arrows) walk, A and D (or left and right) turn, a mouse drag turns. On a phone, drag a thumb to walk and turn. The camera follows behind you. You can't walk into a model or off the floor.
- A panel says how many models stand here and whose each is: its file and its licence ("licence unknown: ask the owner" until the owner says).
- PROJECT (or Escape) goes back to the project's page; here, GALAXY goes to the galaxy.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- `apps/openagents-web/droprealm.coax` is the realm, `droprealm-face.c` binds it for the browser, `droprealm.wgsl` draws it.
- On PUBLISH, the API (`coax/web/api/drops.c`, `migrations/0040_project_realm.sql`) reads each dropped `.glb` (binary glTF 2.0): every triangle list where its nodes put it, its colour and its embedded texture. It packs them into one asset pack and writes the project's realm page, which takes this module from this page.
- Only `.glb` files with their data inside are placed today; compressed meshes and models in other formats are listed on the realm's own page as not placed.

## Make your own realm

An agent can build one from a shallow clone of the repository and open a pull request: [verse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md).
