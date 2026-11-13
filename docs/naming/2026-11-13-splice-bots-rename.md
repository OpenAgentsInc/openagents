# The Splice Bots rename

Splice is now Splice Bots (GitHub #13486, 2026-11-13). This document says what
changed, what keeps working under the old name, when each old name retires, and
the order to deploy the change in. The pattern is the Metaverse rename's
([2026-11-12-metaverse-rename.md](2026-11-12-metaverse-rename.md)).

## What changed

| Before | After | Where |
| --- | --- | --- |
| Splice | Splice Bots | Every name a person sees: the page's title and link preview (`apps/openagents-web/realms.json`), the galaxy's name (`SPLICE BOTS`), the world's name on its card and in the Worlds list (`splice_world_name`), the agent reference's title, `llms.txt`, the world's refusals, docs and books |
| `/worlds/splice` | `/worlds/splice-bots` | The page's address, and the realm's slug |
| World ID and kind `splice` | `splice-bots` | `verse.world_docs`, its undo log, outbox, packs, files, scripts and attachments (migration `0188_splice_bots_world.sql`); the authority table (`metaverse-engine/editor/authority.coax`); `/api/worlds/splice-bots/...` |
| `metaverse/splice/` | `metaverse/splice-bots/` | The game's folder |
| Page module `splice` (`splice.coax`, `gpu/splice.coax`, `splice.wgsl`, `build/splice.wasm`, `data-app="splice"`) | `splice-bots` | The code route reads `metaverse/PAGE/PAGE.coax` and `metaverse/PAGE/gpu/PAGE.coax`, so the module follows its folder (`metaverse-engine/editor/code-wire.coax`). Its budget line in `wasm-budgets.txt` moved with it |
| `apps/openagents-web/realms/splice/` | `apps/openagents-web/realms/splice-bots/` | The page, its Markdown, its `llms.txt` and its fonts |
| Editor bar `splice · revision N` | `splice bots · revision N` | The bar shows the world's ID with each hyphen read as a space, for every world |

The title screen and the credits already said Splice Bots (#13482).

## The alias: the world's old ID

`metaverse-engine/editor/world-alias.coax` holds the one table of renamed
worlds: `splice` names `splice-bots`. Every reader of a world ID asks it first:

| Reader | What it does |
| --- | --- |
| The API (`coax/web/api/worlds.c`, through `world-policy.coax` `wr_world_id`) | `/api/worlds/splice/...` answers as `/api/worlds/splice-bots/...`, reads and writes. A chat turn's attachments that name `splice` open Splice Bots' |
| The dev server (`coax/web/serve.c` `world_url`, `site-routes.coax` `site_world`) | The same, before its world tools run |
| The dev server's world tools (`world-local.coax`, `asset-local.coax`, `files-local.coax`, through `metaverse-engine/tools/world-was.coax`) | On the first request after the rename, the files kept under `build/.worlds/splice.*` move to `splice-bots.*`, once, with the old ID rewritten in each document's, list's and event's text |
| The world's own code (`metaverse/splice-bots/world.coax` `spw_named`) | A document or an event saved under `splice` opens; it is saved again under `splice-bots` |
| Realm IDs (`verse.realm_world_id_ok`) | `splice` and `splice-bots` stay reserved: no realm takes either |

So a saved link, the CLI (`openagents realm world-* splice`), an MCP tool call,
a page cached before the deploy, and a stored document or event that names
`splice` all keep working.

**Retires when** the API has logged no request that names `/api/worlds/splice`
for 90 days after the deploy, and no saved document, event or attachments list
in production names `splice` (migration 0188 rewrites them, so this holds from
the deploy on). Then the `splice` row comes out of `world-alias.coax`; the
realm ID stays reserved.

## The redirect: the page's old address

`/worlds/splice` and everything under it move for good (301) to
`/worlds/splice-bots`, the rest of the path and the query kept, so
`/worlds/splice?edit` opens the editor. `/realms/splice` moves to
`/worlds/splice` first, then here. `/worlds/splicer` and other slugs that start
the same are not matched.

The rules are in two places that must agree: `coax/web/site-routes.coax` (the
dev server, with its checks) and the load balancer's url map, written by
`coax/web/api/lb.sh` (`MOVED_WORLDS`, ahead of `/worlds/*`).
`coax/web/tool.sh site-routes-check` asks the dev server; `coax/web/smoke.sh`
asks staging and production.

**Never removed**: the redirect is permanent, as `/verse` and `/forge` are.

## Names that stay `splice`

Each is a stored or internal identity. None is text a person reads.

| Identifier | Where | Why it stays |
| --- | --- | --- |
| Save slot `splice` (`realms.json` `"slot"`, `data-slot`) | Players' browsers | A new slot loses saved games |
| Browser storage space `splice` and `splice.run.v1` (`front.coax` `FR_STORE_SPACE`, `FR_RUN_HEAD`; `splice-face.c`) | Players' browsers | A new key loses the saved run and settings |
| Content set header `aethyr.content.v1 splice 2` | `content/splice.content`, stored dev and live tables in `verse.world_docs` | Stored table header; the world's code checks it |
| Font families `SplicePlexMono`, `SpliceMichroma` | `realms.json`, the page | Internal CSS names |
| File names `splice-game.coax`, `splice-cast.coax`, `splice-export.coax`, `splice-face.c`, `splice-ue-face.c`, `splice-run.aescript`, `content/splice.content` | `metaverse/splice-bots/` | Only the module's own file had to follow the folder; renaming the rest costs four concurrent agents a conflict and gives nothing a person sees |
| C and Coax symbols `splice_*`, `SPLICE_*`, `sp_*`, `spw_*`, the API's `splice-world` unit (`splice-world-unit.c`) | Code | Internal |
| Context name `splice` | `contexts.txt`, `docs/architecture/` | Internal layer name |
| The verb: "Splice the Digger", the SPLICE button, `EV_SPLICE_CORE` | The game | It is the game's verb, not its name |
| Unreal paths `/Game/Aethyr/Splice/...`, the export's game name | `metaverse-engine/export/unreal/`, `unreal-import.coax` | Saved Unreal assets point at them |
| Imported files' objects `PREFIX/worlds/splice/files/HASH` | The private bucket | Read only during an import; the pack they made moved with the row |
| Sample world IDs in fixtures (`proto/world/*.json`, engine and CLI checks) | Tests | A generic sample ID, not the real world |
| Dated docs, `docs/verification/`, migrations 0181 to 0187, analytics rows, logs, closed issues | History | History is not rewritten |

## Deploy order

Each step leaves the site working, so the order matters:

1. **API** with migration `0188_splice_bots_world.sql`. The row moves to `splice-bots`, and the old ID
   becomes the alias, so pages that still run the old build (`/api/worlds/splice/...`) keep working.
2. **Web release** (`coax/web/release.sh`): the page at `realms/splice-bots/`, `build/splice-bots.*.wasm`
   and `splice-bots.wgsl`. Until step 3, `/worlds/splice` still serves the old page's objects.
3. **Load balancer**: `coax/web/api/lb.sh`. `/worlds/splice*` now moves to `/worlds/splice-bots*`. Run it
   only after step 2, or the redirect leads to a page that is not there yet.
4. **Smoke** both hosts (`coax/web/smoke.sh`): `/worlds/splice*` moves to `/worlds/splice-bots*`.
