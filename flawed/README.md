# Flawed (`flawed/`)

Pure Coax autonomous coding agent harness built bottom-up, chapter-by-chapter from the `cc` reverse-engineering manuscript (`~/work/cc/book`), replacing TypeScript/Bun/React/Zod workarounds with checked native Coax primitives (`@id` nominal types, generational `Handle(T)`, `#authority;` zero-survivor mutation boundaries, caller-owned bounded buffers, and zero-dependency state machines).

## Chapter Progress

| Chapter | Layer | Modules | Gate | Status |
| :--- | :--- | :--- | :--- | :--- |
| **Ch 01 — The Atoms** | Layer 0 | `ids.coax` (`#authority`), `state.coax`, `file_cache.coax`, `scheduler.coax` (`#authority`), `core.coax`, `tools/flawed-cli.coax` | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 02 — The Tool Contract** | Layer 1 | `tool.coax` (`#authority`), `glob_tool.coax`, `flawed-cli glob` | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 03 — Command & Message Contracts** | Layer 1 | `command.coax` (`#authority`), `messages.coax`, `message_wire.coax` (`#authority`), `flawed-cli route` | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |

## Verification

```sh
./coax/bin/coax test flawed/core.coax --library
./coax/bin/coax mutate flawed/ids.coax --root-only
./coax/bin/coax mutate flawed/scheduler.coax --root-only
./coax/web/tool.sh layer-check
./coax/web/tool.sh flawed/tools/flawed-cli.coax selftest
./coax/web/tool.sh flawed/tools/flawed-cli.coax inspect flawed/README.md
```
