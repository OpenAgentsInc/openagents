# Fixture Realm

The realm host's test world at https://openagents.com/realm/fixture. Every Verse realm on openagents.com opens the same way: one table (`apps/openagents-web/realms.json`) names its world on the galaxy, its page, its module, its asset pack and its save slot. This realm is the smallest one that uses all of it, so the host is checked by something other than the Haunted House.

## What it does

- A light stands on a round floor. Move it with W A S D or the arrow keys, or drag it; on a phone, drag it.
- It stays where you leave it: the realm keeps where it stands and how many times you came in this browser, in its own save slot (`fixture`), apart from every other realm's save.
- GALAXY (or Escape) goes back to the galaxy. The galaxy shows this realm only at https://openagents.com/verse/galaxy?realms=all.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- `apps/openagents-web/fixture.coax` is the realm, `fixture-face.c` binds it for the browser, `fixture.wgsl` draws it.
- The page is written from `realms.json` by `coax/web/realms.ts`. The contract every realm follows is `docs/verse/realm-host.md` in the OpenAgents repository.
