# OpenAgents Core (`openagents/core/`)

Pure Coax autonomous coding agent core (`agent-core` bounded context), paired with the unified agent runtime (`openagents/runtime/`), resident daemon (`openagents/daemon/`), and external delegated workers (`openagents/workers/`). Built with checked native Coax primitives (`@id` nominal types, generational `Handle(T)`, `#authority;` zero-survivor mutation boundaries, caller-owned bounded buffers, and zero-dependency state machines).

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
| **Ch 14 — Configuration & Enterprise Policy** | Layer 4 | `config.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 15 — Model Context Protocol (MCP)** | Layer 5 | `mcp.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 16 — Skills, Plugins, Output Styles & LSP** | Layer 5 | `extensions.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 17 — Background Task Runtime** | Layer 6 | `tasks.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 18 — Subagents, Context Forking & Worktrees** | Layer 6 | `subagent.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 19 — Agent Teams, Swarms & Coordinator Mode** | Layer 6 | `swarm.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 20 — Native Terminal Screen Renderer** | Layer 7 | `screen.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 21 — The Interactive REPL** | Layer 7 | `repl.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 22 — Cold Start Mastery** | Layer 8 | `cli_boot.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 23 — Headless SDK & Control Protocols** | Layer 8 | `sdk_protocol.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |
| **Ch 24 — Remote Sessions, Bridge & Direct Connect** | Layer 8 | `remote_bridge.coax` (`#authority`) | `coax test` + `coax mutate --root-only` (0 survivors) | Complete |

## Verification

```sh
./coax/bin/coax test openagents/core/core.coax --library
./coax/bin/coax test openagents/runtime/oa-worker.coax --library
./coax/bin/coax mutate openagents/core/ids.coax --root-only
./coax/bin/coax mutate openagents/core/scheduler.coax --root-only
./coax/web/tool.sh layer-check
```
