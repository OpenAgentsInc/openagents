# Flawed (`flawed/`)

Pure Coax autonomous coding agent harness built bottom-up, chapter-by-chapter from the `cc` reverse-engineering manuscript (`~/work/cc/book`), replacing TypeScript/Bun/React/Zod workarounds with checked native Coax primitives (`@id` nominal types, generational `Handle(T)`, `#authority;` zero-survivor mutation boundaries, caller-owned bounded buffers, and zero-dependency state machines).

## Chapter Progress

| Chapter | Layer | Modules | Gate | Status |
| :--- | :--- | :--- | :--- | :--- |
| **Ch 01 — The Atoms** | Layer 0 | `ids.coax` (`#authority`), `state.coax`, `file_cache.coax`, `scheduler.coax` (`#authority`), `core.coax`, `tools/flawed-cli.coax` | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 02 — The Tool Contract** | Layer 1 | `tool.coax` (`#authority`), `glob_tool.coax`, `flawed-cli glob` | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 03 — Command & Message Contracts** | Layer 1 | `command.coax` (`#authority`), `messages.coax`, `message_wire.coax` (`#authority`), `flawed-cli route` | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 04 — Prompt Engineering & Cache Boundaries** | Layer 2 | `prompt_cache.coax` (`#authority`), `prompt_context.coax`, `flawed-cli cache-plan` | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 05 — Talking to the Model** | Layer 2 | `api_policy.coax` (`#authority`), `sse_stream.coax`, `flawed-cli cost-check` | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 06 — Executing Tools** | Layer 2 | `tool_exec.coax` (`#authority`), `tool_spill.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 07 — The Heartbeat Query Loop** | Layer 2 | `query_loop.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 08 — Session Envelope & Transcripts** | Layer 2 | `session.coax` (`#authority`), `transcript_dag.coax` | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 09 — Five-Layer Compaction Pipeline** | Layer 3 | `compaction.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 10 — Memory Systems & Auto-Dream** | Layer 3 | `memory.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 11 — The Permission Engine** | Layer 4 | `permissions.coax` (`#authority`), `perm_match.coax` (`#authority`), `flawed-cli perm-check` | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 12 — Shell, Sandboxing & Workspace Tools** | Layer 4 | `shell_sandbox.coax` (`#authority`), `workspace_tools.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 13 — The Hook System** | Layer 4 | `hooks.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |

## Verification

```sh
./coax/bin/coax test flawed/core.coax --library
./coax/bin/coax mutate flawed/ids.coax --root-only
./coax/bin/coax mutate flawed/scheduler.coax --root-only
./coax/web/tool.sh layer-check
./coax/web/tool.sh flawed/tools/flawed-cli.coax selftest
./coax/web/tool.sh flawed/tools/flawed-cli.coax inspect flawed/README.md
```
