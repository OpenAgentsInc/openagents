# Khala Code Message-Triggered Codex Fleet Delegation

Date: 2026-07-01
Status: implementation note for the Codex-wrapper Khala Code path
Transcript target: `docs/transcripts/245.md`

## Decision

The new Codex-wrapper Khala Code should make transcript 245 true by registering
Khala Fleet as a local Codex MCP server, not by reintroducing the legacy
Khala-native prompt/tool loop.

Normal message flow:

```text
user message
  -> Khala Code Desktop
  -> Codex app-server thread/turn
  -> Codex tool router
  -> local MCP server: khala_fleet
  -> codex_spawn
  -> khala.fleet.delegate
  -> Pylon assignment
  -> isolated worker Codex home
```

This keeps Codex app-server as the single-agent harness authority while giving
Codex a real tool for swarm delegation. A casual prompt such as "fan out 10
Codex workers to open issues" can now be answered by a Codex MCP tool call
rather than by UI-only buttons or a shadow TypeScript coding harness.

## Local Bridge

Khala Code registers a Codex MCP server named `khala_fleet` before default
desktop chat turns. The server is launched with:

```toml
[mcp_servers.khala_fleet]
command = "bun"
args = ["<openagents>/clients/khala-code-desktop/src/bun/khala-fleet-mcp-server.ts"]
cwd = "<openagents>"
enabled = true
default_tools_approval_mode = "prompt"
enabled_tools = ["pylon_ensure", "codex_fleet_status", "codex_spawn"]
```

The bridge can be disabled for debugging with:

```sh
KHALA_CODE_DESKTOP_FLEET_MCP_BRIDGE=0
```

The server intentionally exposes only the Fleet/Pylon supplemental tools. It
does not expose filesystem, shell, patch, browser, or other Codex-equivalent
Khala tools; those remain owned by Codex app-server on the default path.

## Authority Boundary

- The primary user session remains the user's normal Codex app-server thread.
- Worker execution still uses isolated Pylon Codex homes under the existing
  Fleet account model.
- Codex MCP approval mode is `prompt`, so message-triggered delegation asks for
  explicit approval at the Codex tool boundary.
- Once approved, `codex_spawn` still runs the deterministic
  `khala.fleet.delegate` program:
  `ensure_pylon -> advertise_capacity -> select_account -> prepare_work ->
  dispatch -> verify_closeout`.
- Raw transcripts, local paths, provider payloads, credentials, and bearer
  material are not projected through the MCP bridge.

## Transcript 245 Closure

The old transcript failure was:

```text
codex_spawn_failed: No Pylon Codex assignment capacity is available right now
```

That failure came from a missing deterministic precondition:
capacity was not advertised before dispatch. In the new system, the same user
intent enters through a Codex message, but the actual work still lands in the
same deterministic bundle. `dispatch` can recover through `advertise_capacity`
instead of dead-ending, and the UI can show the Codex MCP tool item plus the
`khala.fleet.delegate` trace.

## Verification

Focused checks:

```sh
bun test clients/khala-code-desktop/tests/codex-fleet-mcp-bridge.test.ts \
  packages/khala-tools/src/mcp.test.ts
```

Recording checks remain:

```sh
bun run --cwd clients/khala-code-desktop smoke:part2-ui
bun clients/khala-code-desktop/scripts/part2-delegation-smoke.ts
```
