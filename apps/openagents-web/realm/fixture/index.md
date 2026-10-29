# Fixture Realm

The realm host's test world at https://openagents.com/realm/fixture. Every Verse realm on openagents.com opens the same way: one table (`apps/openagents-web/realms.json`) names its world on the galaxy, its page, its module, its asset pack and its save slot. This realm is the smallest one that uses all of it, so the host is checked by something other than the Haunted House.

## What it does

- A lamp stands on a round floor ringed with kit pieces: a barrel, a chest, an anvil, a wagon, a crate, a fence, a tree, a rock and a fern. Move the lamp with W A S D or the arrow keys, or drag it; on a phone, drag it. It lights what it nears.
- The pieces come in one asset pack (`fixture.pack`, about 1.2 MB of its 1.5 MB budget), streamed when you enter: a line says how much has come, then a panel names every source in the pack and its licence. All of them are CC0. The full list is [licences.md](licences.md).
- It stays where you leave it: the realm keeps where it stands and how many times you came in this browser, in its own save slot (`fixture`), apart from every other realm's save.
- GALAXY (or Escape) goes back to the galaxy. The galaxy shows this realm only at https://openagents.com/verse/galaxy?realms=all.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- `apps/openagents-web/fixture.coax` is the realm, `fixture-face.c` binds it for the browser, `fixture.wgsl` draws it.
- `verse/tools/pack_realm.py` compiles the pack from `verse/packs/fixture.json` and writes `licences.md`.
- The page is written from `realms.json` by `coax/web/realms.ts`. The contract every realm follows is `docs/verse/realm-host.md` in the OpenAgents repository.
