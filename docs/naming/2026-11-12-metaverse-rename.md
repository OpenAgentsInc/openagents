# The OpenAgents Metaverse rename

**Decided 2026-11-12 (GitHub #13405).** This decision replaces the
2026-10-31 engine naming decision in
[docs/verse/engine-name.md](../verse/engine-name.md).

| Old name | New name | Short form |
| --- | --- | --- |
| The Verse | OpenAgents Metaverse | the Metaverse |
| Aethyr, Aethyr Engine, the Verse engine | OpenAgents Metaverse Engine | Metaverse Engine |
| Aethyrforge | (no name; part of the Metaverse Engine) | |
| Verse worlds | Metaverse realms | realms |
| Verse OS | Metaverse OS | |

Coax, Psionic and Realms keep their names. "OpenAgents Engine" is not a
name; don't use it.

**There is no "Forge"** (owner, 2026-11-12, later the same day). Aethyrforge
folds into the Metaverse Engine: creating and editing realms are features of
the engine, not a separate product. Where the activity needs a word, use a
plain verb ("create a realm", "edit") or a plain noun ("the realm editor"),
never a brand.

This document is the inventory and the plan. It lists what changes, what
stays for compatibility, and when each compatibility name retires.

## What each name means

- **OpenAgents** is the company and the agent platform.
- **OpenAgents Metaverse** is the network of persistent, connected worlds
  that people and AI agents live in and make.
- **OpenAgents Metaverse Engine** is the real-time game, simulation,
  rendering, networking and world-execution engine, built in Coax. The same
  engine runs robotics, scientific simulation and training environments;
  the name is its product identity, not a limit on what it does.
- Creating, editing, playing, forking and publishing realms are features of
  the Metaverse Engine. They have no product name of their own.
- **Realms** are the individual worlds: Everglade, Fairy Forest, Dead Gears
  and the worlds users make.

## Writing rules

- Write "the OpenAgents Metaverse" the first time on a page, then "the
  Metaverse".
- Write "the OpenAgents Metaverse Engine" the first time, then "the
  Metaverse Engine". It takes "the", as "the Verse" did; "Aethyr" did not.
- Where Aethyrforge was named, write "the Metaverse Engine", or "the realm
  editor" when the sentence is about the editing code or screen.
- An individual destination is a realm.
- Replace the names only. Don't restyle the copy around them.
- Don't rewrite transcripts, dated reports, audits, handoffs, verification
  evidence or the changelog of a release. They keep the names they were
  written with.

## Inventory (origin/main at 6c6f85e4f8, 2026-11-12)

Counts leave out `docs/verification/`, which is recorded evidence.

| What | Files | Occurrences |
| --- | ---: | ---: |
| `Aethyr` as a word | 520 | 1,406 |
| `Aethyrforge` | 153 | 387 |
| `the Verse` / `The Verse` | 265 | 641 |
| `Verse` as a word (includes the row above) | 595 | 1,842 |
| `aethyr` in a path, symbol or ID | 2,169 | 12,049 |
| `aethyrforge` in a path, symbol or ID | 122 | 561 |
| `verse` in a path, symbol or ID | 1,911 | 17,720 |
| Tracked files under `aethyr/` | 2,031 | |
| Tracked files under `verse/` | 620 | |

Where the prose names are: `aethyr/book/`, `apps/openagents-web/` (pages,
project pages, link previews), `docs/verse/`, `docs/unreal/`, `docs/books/`,
`coax/web/` (API text, room text), `psionic/book/`, `coax/book/`,
`openagents/book/`, `nostr/book/`, `everglade/book/`, `os/nix/`,
`components/`, `README.md`, `AGENTS.md` and `INVARIANTS.md`.

Not project naming, and left alone: "reverse", "traverse", "universe",
"inverse", Epic's Verse language (`docs/research/`, `docs/unreal/`), a verse
of a song or poem, the `cordiverse` GitHub organisation, the
`OpenAgentsInc/aethyr-cpp` repository and its `aethyr-demo` Cloud Run
service (a different, older product that keeps its name in history), the
`projects/` clones, the separate `forge` repository and the workspace root.

## Paths

| Old path | New path | Notes |
| --- | --- | --- |
| `aethyr/` | `metaverse-engine/` | The engine, its book, tests and tools |
| `aethyr/forge/` | `metaverse-engine/editor/` | Stays in the engine folder: `host/`, `world/`, `content/` and `tools/` load it |
| `verse/` | `metaverse/` | The realms and their content |
| `verse/everglade2/` | Stays until it moves to `everglade/world/` | Everglade II's live world. AAA P1.2 (#13268) moves it, so this rename does not move it twice |
| `verse/everglade/` | Stays until it is split or retired | Everglade v1 and pieces still in use (guard AI, rigs, edits). AAA P1.3 (#13269) |
| `apps/aethyrforge-desktop/` | `apps/metaverse-engine-desktop/` | The desktop app |
| `docs/verse/` | `docs/metaverse/` | |
| `apps/openagents-web/verse*`, `gpu/verse.*`, `carriers/verse.coax` | `metaverse*` | Page sources |
| `apps/openagents-web/verse/` | `apps/openagents-web/metaverse/` | Done in stage 5: the landing's page folder is its address |
| `apps/openagents-web/projects/verse/` | Stays | The folder is the project's slug, its name on the relay and in the bucket. The address `/projects/metaverse` is an alias; see Routes |
| `assets/verse/` | `assets/metaverse/` | |
| `os/verse-*.coax` | `os/metaverse-*.coax` | |
| `aethyr/layer-allowlist.txt`, `aethyr/consumers.txt`, `aethyr/model-files.txt` | Move with the folder | `contexts.txt`, `docs/architecture/contexts.md`, `docs/reuse.md`, `AGENTS.md` and `coax/web/tool.sh layer-check` change in the same commit |

Paths that stay:

- `proto/aethyr/`, `proto/aethyrforge/`, `proto/verse/`,
  `proto/openagents/verse/`, `proto/descriptors/verse*.pb`. Published
  schema paths and `$id` URLs. See Compatibility.
- `docs/verification/**`. Recorded evidence, named for the run it records.
- `coax/web/api/migrations/*.sql`. History.
- `psionic/fixtures/verse-villain/` and the other recorded Psionic
  fixtures whose names are pinned by their manifests. They move only with a
  fixture re-record.

Each folder move is one commit: `git mv` plus a scripted rewrite of every
path reference, with no other change. The script is rerun on a fresh
`origin/main` when the push loses a race, so no other agent's change is
dropped.

The `aethyr/` and `verse/` moves wait for #13404 to close, because that
issue is moving files in and out of both folders.

## Code symbols

These are implementation details and change after the folders move, one
scripted commit per prefix:

| Old | New | Examples |
| --- | --- | --- |
| `aethyr_`, `AETHYR_`, `Aethyr` + capital | `engine_`, `ENGINE_`, `Engine` + capital | `aethyr_voice_step`, `AETHYR_RENDER_TYPED`, `AethyrHost` |
| `#module "aethyr.…"` | `#module "engine.…"` | `aethyr.render.passes` |
| `verse_`, `VERSE_`, `Verse` + capital | `metaverse_`, `METAVERSE_`, `Metaverse` + capital | `verse_style_luma`, `VERSE_PACK_DIR`, `VerseState` |
| `aethyrforge_`, `AETHYRFORGE_`, `forge_` in `aethyr/forge/`, `#module "aethyr.forge.…"` | `editor_`, `EDITOR_`, `#module "engine.editor.…"` | `AETHYRFORGE_SCENE_TOOL`, `forge_save_body` |

`engine` is already the context's name in `contexts.txt`, so `engine_` is
the engine's symbol prefix. It is a prefix, not a product name.

Unreal exporter types (`AethyrProbe`, `AethyrBridge`, `AethyrGameMode` and
the rest under `aethyr/export/` and `docs/unreal/`) are Unreal class and
module names that saved `.uasset` and `.uproject` files point at. They
change only with a core redirect in the exporter's `DefaultEngine.ini`.

## Compatibility: names that stay for now

A name in this table is a published or stored identity. It keeps working.
None of them is a product name, and none appears in UI text. Each has the
condition that retires it.

| Identifier | Where it lives | Why it stays | Retires when |
| --- | --- | --- | --- |
| `aethyr.content.v1` | `proto/registry.json`, content documents on disk and in rooms | Stored document header | Content tables v2 ships with a `metaverse.content.v2` header and reads v1 |
| `aethyrforge.scene-command-v1`, `-v2`, `aethyrforge.scene-session.v1`, `aethyrforge.npcs.v1`, `aethyrforge.npc-command-v1`, `aethyrforge.raid-command-v1`, `aethyrforge.voice-coder.v1`, `aethyrforge.microphone-capture.v1` | `proto/aethyrforge/*.schema.json`, `https://openagents.com/proto/aethyrforge/…` `$id` URLs, saved scene sessions, agent tools | Published schemas; AGENTS.md: a published contract changes only with a new version | The next version of each schema is published as `metaverse.editor.…` under `proto/metaverse/`; the old `$id` URL keeps serving for as long as a saved session of that version can be opened |
| `aethyr.player.appearance.v1` | Browser `localStorage`, `proto/aethyr/player-appearance-v1.json` | Players' saved looks | Appearance v2; the page reads v1 once and writes v2 |
| `aethyr.physics-lab.v1` and the other `proto/aethyr/*.json` profiles | `proto/aethyr/` | Published profiles | Next version of each |
| `aethyr-transport-v1`, `aethyr-projectile-v1` | `proto/registry.json` row IDs, `docs/protocol/aethyr-transport-v1.md` | Wire profile IDs that production rooms and pages agree on | The next transport or projectile version, registered as `metaverse-…` |
| `aethyr.render.*.v1…v7`, `aethyr.host.*.v1`, `aethyr.metal.*.v1`, `aethyr.stream-walk-*`, `aethyr.recorded-source.v1`, `aethyr.native-gpu-worker.v1`, `aethyr.streaming-scene.v1`, `aethyr.realm` | Render packages, host observation and effect records, recorded fixtures, `docs/verification/` evidence | Format IDs inside recorded files; changing one invalidates every recorded golden | Each format's next version; the reader accepts the old ID until its fixtures are re-recorded |
| `VPK1` and the `/api/verse/packs/…` routes | Asset packs in the bucket, pages in production, the desktop app | Stored pack magic and a route that cached pages call | Packs v2. `/api/metaverse/packs/` is the main route (`verse_packs.c`) and `/api/verse/packs/` answers the same way, not as a redirect. The pages (`host.js`, `character-host.js`) move to the new route once the API that has it is in production; the old route stays until no request has used it for 30 days |
| `verse.player`, `verse.quality`, `verse.quality.settled`, `verse.settings`, `verse.sound`, `verse.gyro`, `verse.voice.speaker`, `verse.hud`, `verse.haunt.save`, `verse.world.v1`, `verse-place 1` | Browser `localStorage`, saved places | Players' saved settings, identity and places | A storage version that copies each key to `metaverse.…` once, then 90 days |
| Postgres schema `verse` (`verse.item_outbox`, `verse.character_outbox`, …), 123 migrations | Production database | Renaming a schema needs a migration and a deploy in step with every query; migrations are history | A planned database change adds `metaverse` as the schema and leaves `verse` as views for one release. Not part of this rename |
| `/verse`, `/verse/galaxy`, `/projects/verse` | Published links, link previews, the sitemap | Saved links must keep working | Never removed: each is a permanent redirect. See Routes |
| `/verse-os/` | The OS image's update address (`verseos.image.updateUrl`) | Installed machines ask it | Moves with the OS rename, not with the pages |
| The project slug `verse` (`projects/verse/project.json`, the relay's `d` tag, the bucket's `projects/verse/`) | The relay, the bucket, `openagents project …` | A project's slug is its identity on the relay; a new slug is a new project | The owner republishes the project as `metaverse` with the project keys. Until then `/projects/metaverse` is an alias for it |
| `/api/playtest/…` | API | Not a Verse name; unchanged | |
| `/forge` | Published links | A stable public link | Never removed: it stays as a permanent redirect to the realm creation page. No UI text says "Forge" |
| `/realms/<slug>` | Published links | Already the right name; unchanged | |
| `~/.openagents/aethyrforge/`, `~/.openagents/bin/aethyrforge-scene`, `~/.openagents/verse/` | Installed machines | Users' workspaces, setup state and private builds | The desktop app reads `~/.openagents/metaverse-engine/` first and moves an old folder there on first start; the old path is dropped two releases later. `aethyrforge-scene` stays as a link to `metaverse-scene` for the same two releases |
| `Aethyrforge.app`, bundle IDs `com.openagents.aethyrforge`, `com.openagents.aethyrforge.experimental`, `com.openagents.aethyr`, `com.openagents.verse` | Installed apps, macOS permissions (microphone), signing | A new bundle ID is a new app to macOS and loses granted permissions | The app's display name changes to "OpenAgents Metaverse Engine" now. The bundle ID changes only with a signed release that moves the user's data and permissions |
| `openagents-aethyrforge.service`, `verse.service`, the `services.verse*` NixOS options | Deployed NixOS machines | Unit names that running machines and deploy tooling address | The next OS image build renames the units and keeps the old names as aliases for one image |
| `https://openagents.gitbook.io/building-aethyr-world-engine` | The published engine book | Published link | The space is renamed "Building the OpenAgents Metaverse Engine" and GitBook redirects the old slug. The publication file records both |
| `https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md` and other `blob/main/aethyr/…`, `blob/main/verse/…`, `blob/main/docs/verse/…` links | Agent discovery text, pages in production | GitHub does not redirect a moved file | Links in this repository change with the folder moves. Links outside it can't be fixed; `verse/SKILL.md` stays as a one-line pointer file until the pages that name it are redeployed |
| Unreal class and module names (`AethyrProbe`, `AethyrBridge`, …) | Unreal project files | Saved assets point at them | A core redirect lands with the rename |

## Routes

Stage 5. The rules are in two places that must agree: `coax/web/site-routes.coax`
(the dev server, with its checks) and the load balancer's url map, written by
`coax/web/api/lb.sh` ("The Metaverse's addresses"). `coax/web/tool.sh
site-routes-check` asks the dev server every address below; `coax/web/smoke.sh`
asks staging and production.

| Address | Answer |
| --- | --- |
| `/metaverse`, `/metaverse/index.md`, `/metaverse/og-verse.jpg` | The landing: `apps/openagents-web/metaverse/` |
| `/verse`, `/verse/*` | 301 to `/metaverse`, `/metaverse/*`, path and query kept |
| `/verse/galaxy`, `/metaverse/galaxy` and under them | 301 to `/worlds`, query dropped (the galaxy was retired on 2026-10-31) |
| `/projects/metaverse`, `/projects/metaverse/*` | The project page: the site's `projects/verse/`, by a rewrite |
| `/projects/verse`, `/projects/verse/*` | 301 to `/projects/metaverse`, `/projects/metaverse/*`, path and query kept |
| `/api/metaverse/packs/<name>`, `/api/metaverse/packs/fetch/<sid>` | The pack routes |
| `/api/verse/packs/…` | The same routes, answered the same way (an alias, not a redirect) |
| `/worlds/<slug>`, `/realms/<slug>` | Unchanged |

Pages the project tool writes (`projects/verse/index.html`, `projects/index.md`,
`projects/list.coax`) and links between projects are made from the slug, so
they still say `/projects/verse`. They work through the redirect. They change
when the project is republished under the new slug.

`/forge` answers 404 in production today (checked 2026-11-12). Its redirect to
the realm creation page is not part of stage 5.

Deploy order, each step safe on its own:

1. `coax/web/api/lb.sh`. It adds the `/projects/metaverse` alias, the
   `/projects/verse` redirect and the `/metaverse/galaxy` redirect. It leaves
   `/verse` alone on a host whose `/metaverse` doesn't answer yet, and says so.
2. The API deploy (`/api/metaverse/packs/`, migration 0178).
3. The web release (`/metaverse` is in the bucket; links point at it).
4. `coax/web/api/lb.sh` again. Now it adds the `/verse` redirect on both hosts.
5. `coax/web/smoke.sh https://openagents.com` and the same for staging.

A web release before step 1 is safe except for one link: `llms.txt` names
`/projects/metaverse/index.md`, which answers 404 until step 1.

## Root folders

Owner rule, 2026-11-12: the repository root is for projects that have a
book. Realms and other world code without a book go under the realms folder,
`verse/` today and `metaverse/` after the move. Splice moves to
`verse/splice/` under #13404 and comes along. This rename creates no new
root folder without a book.

Root folders with a book: `aethyr/` (becomes `metaverse-engine/`),
`bitcoin/`, `coax/`, `coder/`, `everglade/`, `nostr/`, `openagents/`,
`psionic/`.

Root folders without a book, recorded for the owner to decide. None of them
moves in this rename.

| Folder | What it is | Looks out of place? |
| --- | --- | --- |
| `apps/` | The apps: web, iOS, CLI, desktop, Everglade's apps | No. Apps are products built from the projects |
| `assets/` | The shared asset registry and the Metaverse's icons, fonts and models | No |
| `bench/` | The voice benchmark (`bench/voice/`) | Maybe. It could sit under `psionic/`, which it measures |
| `components/` | The one UI component system for every app | Maybe. It is a project with a spec and no book |
| `docs/` | Documents | No |
| `orbit/` | Orbit League's room code (`orbit/server/`) | Yes. It is a realm's code, so it belongs under the realms folder |
| `os/` | Metaverse OS: the machine's programs and the NixOS modules | Maybe. A project with a spec and no book |
| `pay/` | The payments context's money rules | Maybe. A bounded context with no book; the Bitcoin book covers its subject |
| `plans/` | Plan documents | Yes. Plans are documents and fit under `docs/` |
| `proto/` | Protocol schemas, descriptors and the wire registry | No |
| `rules/` | The SRD 5.2.1 combat rules, used by Everglade and other worlds | Maybe. World code without a book; the Everglade book covers it |
| `scripts/` | Repository scripts | No |
| `tools/` | Two standalone Coax tools (`coax-nostr`, `coax-unicode`) | Maybe. They could sit under `coax/` and `nostr/` |

## Text that still says the old names, and why

Found while changing UI text (stage 2b). Each needs more than a text edit.

| Where | What it says | Why it stays for now | Changes when |
| --- | --- | --- | --- |
| `coax/web/api/schema.sql`, served realm pages | Changed by migration `0178_metaverse_page_text.sql`: the pages say "the Metaverse" once the API with that migration is deployed | | Done in the repository; live with the next API deploy |
| `coax/web/api/schema.sql` model prompts, `coax/web/agent/chat.coax` lines 150 and 152 | NPC and chat-agent prompts that say "the Verse" | Prompts are agent behaviour, which this rename must not change without an evaluation | A prompt change with its own check of the answers |
| `apps/openagents-web/metaverse/og-verse.jpg` | "ENTER THE VERSE" and "OPENAGENTS.COM/VERSE" drawn into the picture | A picture; `og-card.py` needs a page shot to redraw it | The card is redrawn after the web release that shows the new landing title |
| `coax/web/goldens/landing.png`, `goldens/linux/landing.png` | The old landing title | Goldens are re-recorded from staging | The next staging release (`bun coax/web/goldens.ts … --only landing --update`) |
| `aethyr/ui/arwes-paint.coax` line 205 | Draws "AETHYR / ONLINE" in the HUD lab | Asserted by two checks; a drawn string | The symbol stage |
| The macOS app name in `coax/web/voice.js` line 233 ("Microphone → Aethyrforge") | The installed app's name in System Settings | Must match the installed app | The app's display name changes |
| The relay's copy of the project ("Verse") | `/projects` and the project pages read the relay | Needs `openagents project publish` with the project keys | The owner or the coordinator publishes the project |
| "the old Rust Verse engine" in three comments | Names the pre-reset engine | History | Never |

## Stages

1. **Inventory.** This document.
2. **Names in prose.** Active docs, books, UI labels, CLI help, link
   previews, README, AGENTS.md and INVARIANTS.md say the new names. No path
   or symbol changes.
3. **Folders.** `aethyr/`, `verse/`, `apps/aethyrforge-desktop/`,
   `docs/verse/` and the page sources move, one commit each. Waits for
   #13404.
4. **Symbols and modules.** One scripted commit per prefix.
5. **Routes and aliases.** `/metaverse`, `/projects/metaverse`,
   `/api/metaverse/packs/`, `~/.openagents/metaverse-engine/`, with the old forms
   redirecting or read as fallbacks, and tests for each. The web routes and
   the served page text are done (see Routes); `~/.openagents/metaverse-engine/`
   is not.
6. **Verification.** layer-check, wire-check, tools-check, the touched
   suites, page builds and a walk of saved links.
7. **Cleanup.** A search for the old names finds only this document's
   compatibility table, history and recorded evidence.

Mechanical commits and functional commits are separate.
