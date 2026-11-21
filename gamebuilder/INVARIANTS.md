# Invariants — gamebuilder/INVARIANTS.md

> Scope: GameBuilder 3D level/game creation core, command table, CLI/MCP, play loop, sync, publish, and shared game types (#13854). Indexed from root [INVARIANTS.md](../INVARIANTS.md).

## Every GameBuilder action lives in one command table (`gamebuilder/core/commands.coax`)
The CLI (`gamebuilder/cli/main.coax`), MCP server (`gamebuilder/core/mcp.coax`), hosted HTTP API (`gamebuilder serve --accounts`), and embedded Desktop/iOS views all dispatch through the same typed command table (`gamebuilder/core/commands.coax`) with typed JSON responses. Never add a surface-only command outside the table.

## Coax owns editing, physics, play, sync and publish state
All level mutation, snapping, undo/redo, play-loop stepping, lobby/party rules, and publish admission live in Coax under `gamebuilder/core/` and `gamebuilder/shared/`. Host surfaces (`apps/openagents-desktop/`, `apps/openagents-ios/`, `gamebuilder/web/`) provide rendering, input, audio, and persistence effects only.

## GameBuilder consumes OpenAgents directly from the monorepo
GameBuilder has no `openagents.lock`, `vendor/openagents`, or standalone native desktop/mobile app shell (`host/`). All engine, component, and toolchain imports use direct repo-relative `#load` paths checked by `coax/web/tool.sh layer-check` against the `gamebuilder` context in `contexts.txt`.

## Binary assets live in `assets/registry/collections/gamebuilder.lock.json`
Large binary assets (`village.pack`, `scrappers.gbm`, baked meshes) are never committed to git; they are pinned by SHA-256 in `assets/registry/collections/gamebuilder.lock.json` and hydrated into `~/.openagents/assets/sha256/` via `coax/web/tool.sh assets-sync --group gamebuilder`.
