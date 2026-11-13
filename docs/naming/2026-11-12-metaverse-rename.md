# The OpenAgents Metaverse rename

**Decided 2026-11-12 (GitHub #13405).** This decision replaces the
2026-10-31 engine naming decision in
[docs/verse/engine-name.md](../verse/engine-name.md).

| Old name | New name | Short form |
| --- | --- | --- |
| The Verse | OpenAgents Metaverse | the Metaverse |
| Aethyr, Aethyr Engine, the Verse engine | OpenAgents Metaverse Engine | Metaverse Engine |
| Aethyrforge | OpenAgents Forge | Forge |
| Verse worlds | Metaverse realms | realms |
| Verse OS | Metaverse OS | |

Coax, Psionic and Realms keep their names. "OpenAgents Engine" is not a
name; don't use it.

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
- **OpenAgents Forge** is where people and agents create, edit, play, fork
  and publish realms.
- **Realms** are the individual worlds: Everglade, Fairy Forest, Dead Gears
  and the worlds users make.

## Writing rules

- Write "the OpenAgents Metaverse" the first time on a page, then "the
  Metaverse".
- Write "the OpenAgents Metaverse Engine" the first time, then "the
  Metaverse Engine". It takes "the", as "the Verse" did; "Aethyr" did not.
- Write "OpenAgents Forge" the first time, then "Forge". It takes no "the".
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
| `aethyr/forge/` | `metaverse-engine/forge/` | Stays in the engine folder: `host/`, `world/`, `content/` and `tools/` load it, so a top-level folder would break the layer rule |
| `verse/` | `metaverse/` | The realms and their content |
| `apps/aethyrforge-desktop/` | `apps/forge-desktop/` | The desktop app |
| `docs/verse/` | `docs/metaverse/` | |
| `apps/openagents-web/verse*`, `gpu/verse.*`, `carriers/verse.coax` | `metaverse*` | Page sources |
| `apps/openagents-web/projects/verse/` | `projects/metaverse/` | Old URL redirects; see Routes |
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
| `aethyrforge_`, `AETHYRFORGE_` | `forge_`, `FORGE_` | `AETHYRFORGE_SCENE_TOOL` |

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
| `aethyrforge.scene-command-v1`, `-v2`, `aethyrforge.scene-session.v1`, `aethyrforge.npcs.v1`, `aethyrforge.npc-command-v1`, `aethyrforge.raid-command-v1`, `aethyrforge.voice-coder.v1`, `aethyrforge.microphone-capture.v1` | `proto/aethyrforge/*.schema.json`, `https://openagents.com/proto/aethyrforge/…` `$id` URLs, saved scene sessions, agent tools | Published schemas; AGENTS.md: a published contract changes only with a new version | The next version of each schema is published as `forge.…` under `proto/forge/`; the old `$id` URL keeps serving for as long as a saved session of that version can be opened |
| `aethyr.player.appearance.v1` | Browser `localStorage`, `proto/aethyr/player-appearance-v1.json` | Players' saved looks | Appearance v2; the page reads v1 once and writes v2 |
| `aethyr.physics-lab.v1` and the other `proto/aethyr/*.json` profiles | `proto/aethyr/` | Published profiles | Next version of each |
| `aethyr-transport-v1`, `aethyr-projectile-v1` | `proto/registry.json` row IDs, `docs/protocol/aethyr-transport-v1.md` | Wire profile IDs that production rooms and pages agree on | The next transport or projectile version, registered as `metaverse-…` |
| `aethyr.render.*.v1…v7`, `aethyr.host.*.v1`, `aethyr.metal.*.v1`, `aethyr.stream-walk-*`, `aethyr.recorded-source.v1`, `aethyr.native-gpu-worker.v1`, `aethyr.streaming-scene.v1`, `aethyr.realm` | Render packages, host observation and effect records, recorded fixtures, `docs/verification/` evidence | Format IDs inside recorded files; changing one invalidates every recorded golden | Each format's next version; the reader accepts the old ID until its fixtures are re-recorded |
| `VPK1` and the `/api/verse/packs/…` routes | Asset packs in the bucket, pages in production, the desktop app | Stored pack magic and a route that cached pages call | Packs v2; `/api/metaverse/packs/` is added as the main route first and `/api/verse/packs/` stays until no request has used it for 30 days |
| `verse.player`, `verse.quality`, `verse.quality.settled`, `verse.settings`, `verse.sound`, `verse.gyro`, `verse.voice.speaker`, `verse.hud`, `verse.haunt.save`, `verse.world.v1`, `verse-place 1` | Browser `localStorage`, saved places | Players' saved settings, identity and places | A storage version that copies each key to `metaverse.…` once, then 90 days |
| Postgres schema `verse` (`verse.item_outbox`, `verse.character_outbox`, …), 123 migrations | Production database | Renaming a schema needs a migration and a deploy in step with every query; migrations are history | A planned database change adds `metaverse` as the schema and leaves `verse` as views for one release. Not part of this rename |
| `/verse`, `/verse/galaxy`, `/projects/verse`, `/verse-os/` | Published links, link previews, the sitemap | Saved links must keep working | Never removed: each becomes a permanent redirect to `/metaverse…` or `/projects/metaverse` |
| `/api/playtest/…` | API | Not a Verse name; unchanged | |
| `/forge`, `/realms/<slug>` | Published links | Already the right names; unchanged | |
| `~/.openagents/aethyrforge/`, `~/.openagents/bin/aethyrforge-scene`, `~/.openagents/verse/` | Installed machines | Users' workspaces, setup state and private builds | Forge reads `~/.openagents/forge/` first and moves an old folder there on first start; the old path is dropped two releases later. `aethyrforge-scene` stays as a link to `forge-scene` for the same two releases |
| `Aethyrforge.app`, bundle IDs `com.openagents.aethyrforge`, `com.openagents.aethyrforge.experimental`, `com.openagents.aethyr`, `com.openagents.verse` | Installed apps, macOS permissions (microphone), signing | A new bundle ID is a new app to macOS and loses granted permissions | The app's display name changes to "OpenAgents Forge" now. The bundle ID changes only with a signed release that moves the user's data and permissions |
| `openagents-aethyrforge.service`, `verse.service`, the `services.verse*` NixOS options | Deployed NixOS machines | Unit names that running machines and deploy tooling address | The next OS image build renames the units and keeps the old names as aliases for one image |
| `https://openagents.gitbook.io/building-aethyr-world-engine` | The published engine book | Published link | The space is renamed "Building the OpenAgents Metaverse Engine" and GitBook redirects the old slug. The publication file records both |
| `https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md` and other `blob/main/aethyr/…`, `blob/main/verse/…`, `blob/main/docs/verse/…` links | Agent discovery text, pages in production | GitHub does not redirect a moved file | Links in this repository change with the folder moves. Links outside it can't be fixed; `verse/SKILL.md` stays as a one-line pointer file until the pages that name it are redeployed |
| Unreal class and module names (`AethyrProbe`, `AethyrBridge`, …) | Unreal project files | Saved assets point at them | A core redirect lands with the rename |

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
   `/api/metaverse/packs/`, `~/.openagents/forge/`, with the old forms
   redirecting or read as fallbacks, and tests for each.
6. **Verification.** layer-check, wire-check, tools-check, the touched
   suites, page builds and a walk of saved links.
7. **Cleanup.** A search for the old names finds only this document's
   compatibility table, history and recorded evidence.

Mechanical commits and functional commits are separate.
