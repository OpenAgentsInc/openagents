# Khala Code Codex Wrapper Implementation Log

Date: 2026-07-01
Tracking epic: <https://github.com/OpenAgentsInc/openagents/issues/7780>
Audit: `docs/khala-code/2026-07-01-codex-harness-wrapper-port-audit.md`

## Issue #7781: Codex Install And Auth Gate

Status: implemented

Khala Code Desktop now has a typed Codex harness readiness projection for the
main user Codex session. The probe checks:

- the Codex command source (`PATH`, `KHALA_CODE_CODEX_BINARY`,
  `KHALA_CODE_CODEX_COMMAND`, or explicit test input);
- `codex --version` availability and version text;
- the main user Codex home (`CODEX_HOME` or default `~/.codex`);
- `auth.json` presence, JSON shape, and token-field presence without exposing
  token values;
- the distinction between the main user Codex home and isolated Pylon fleet
  worker homes.

The desktop RPC now exposes `codexHarnessStatus()`, composes
`codexAccountsStatus()` with that harness gate, and marks `codingStatus()` as
blocked when Codex is missing, unsigned, or invalid. Unified Inbox projects
missing main Codex setup as a critical local blocker.

The README now states the pivot clearly: the default Khala Code harness requires
Codex, while the hosted Khala/OpenRouter runtime is legacy/fallback during the
transition.

Validation:

- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun run --cwd clients/khala-code-desktop test`
- `bun run --cwd clients/khala-code-desktop verify`

## Issue #7782: App-Server Supervisor And Typed Client

Status: implemented

Khala Code Desktop now has an idle-by-default Codex app-server host in the Bun
process. The host supervises `codex app-server --stdio`, speaks newline-delimited
JSON-RPC, and performs the required `initialize` request followed by the
`initialized` notification. It records bounded diagnostics, maps request errors,
tracks pending requests, supports timeouts, exposes notification subscriptions,
and provides start, stop, restart, status, request, and dispose methods.

The desktop RPC now exposes:

- `codexAppServerStatus()`
- `codexAppServerStart()`
- `codexAppServerStop()`
- `codexAppServerRestart()`

The desktop app constructs one host and disposes it on process exit or signal.
The host does not auto-start during ordinary readiness polling; later issues can
use it as the kernel for thread and turn lifecycle without spawning Codex from
every status check.

Validation:

- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun run --cwd clients/khala-code-desktop test`
- `bun run --cwd clients/khala-code-desktop verify`

## Issue #7783: Codex Thread And Turn Runtime

Status: implemented

Khala Code Desktop now routes the default chat submit path through the local
Codex app-server thread and turn lifecycle. The existing Khala-native hosted
runtime remains available only behind `KHALA_CODE_DESKTOP_RUNTIME=khala_native_runtime`
or `KHALA_CODE_DESKTOP_LEGACY_KHALA_NATIVE_RUNTIME=1`.

The desktop host now has a Codex chat runtime that:

- starts `thread/start` for new desktop sessions;
- persists desktop `sessionId` to Codex `threadId` mappings in
  `~/.khala-code/codex-sessions.json` by default;
- resumes persisted sessions with `thread/resume`;
- sends user prompts through `turn/start`;
- streams `item/agentMessage/delta`, `item/started`, `item/completed`, and
  `turn/completed` notifications back into the existing transcript event model;
- exposes RPC methods for `codexThreadStart`, `codexThreadResume`,
  `codexThreadList`, `codexTurnStart`, `codexTurnSteer`,
  `codexTurnInterrupt`, and `codexThreadCompact`;
- maps the desktop stop action to Codex `turn/interrupt`.

The browser shell now keeps the desktop session id stable in local storage so a
reloaded app can use the persisted Codex thread mapping on the next turn.

Validation:

- `bun test clients/khala-code-desktop/tests/codex-app-server-chat-runtime.test.ts clients/khala-code-desktop/tests/rpc-handlers.test.ts clients/khala-code-desktop/tests/app-shell.test.ts`
- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun run --cwd clients/khala-code-desktop verify`

## Issue #7784: Codex ThreadItem Renderer

Status: implemented

Khala Code Desktop now projects Codex app-server `ThreadItem` lifecycle into
stable transcript cards. The projector preserves Codex item ids, item types,
thread ids, turn ids, request ids, and statuses in the desktop message metadata
while streaming updates into the existing transcript event model.

The renderer covers:

- user messages when replaying history;
- agent messages and deltas;
- reasoning summaries and text deltas;
- plan deltas;
- command execution output and final status;
- file change and patch update lifecycle;
- MCP and dynamic tool calls;
- collab/subagent tool calls and subagent activity;
- web search, image view, sleep, image generation, review mode, and context
  compaction items;
- command, file-change, permission, and auto-review approval prompts;
- safe diagnostic cards for unknown future Codex item variants.

The browser transcript now renders Codex item cards with distinct status styling,
bounded expandable output, copy controls, and the existing Markdown, code, and
diff renderers inside each card. The app-server client also forwards
server-to-client requests that include JSON-RPC ids so approval prompts can be
rendered instead of being mistaken for unmatched responses.

Validation:

- `bun test clients/khala-code-desktop/tests/codex-thread-item-projector.test.ts clients/khala-code-desktop/tests/codex-app-server-client.test.ts clients/khala-code-desktop/tests/codex-app-server-chat-runtime.test.ts clients/khala-code-desktop/tests/rpc-handlers.test.ts clients/khala-code-desktop/tests/app-shell.test.ts`
- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun run --cwd clients/khala-code-desktop verify`

## Issue #7785: Slash Command And Command Palette Parity

Status: implemented

Khala Code Desktop now has a typed Codex slash-command registry whose inventory
matches `codex-rs/tui/src/slash_command.rs`, including aliases such as
`/pet` -> `/pets` and `/clean` -> `/stop`. The registry records Codex
availability rules for active turns, side conversations, debug gates, platform
gates, inline arguments, grouping, and dispatch coverage.

The desktop RPC now exposes:

- `slashCommandList()` for palette/autocomplete metadata with availability;
- `slashCommandDispatch()` for executing slash commands as commands instead of
  prompt text.

Commands with direct app-server backing dispatch through the Codex app-server
where possible, including thread lifecycle, thread rename, goal, review, MCP,
apps, plugins, models, permissions, experimental features, usage, logout, and
background terminal list/cleanup. Commands still owned by Codex TUI popups or
missing upstream app-server methods return structured gap results with the
tracked dependency instead of silently degrading to chat prompts.

The browser composer now shows a compact slash-command palette when the draft
starts with `/`, disables commands unavailable in the current Codex-equivalent
state, and intercepts slash command submit before ordinary chat turn startup.
Client-owned commands such as copy, clear, and status run in the desktop shell;
the rest use the app-server or report their explicit dependency.

Validation:

- `bun test clients/khala-code-desktop/tests/codex-slash-commands.test.ts clients/khala-code-desktop/tests/rpc-handlers.test.ts clients/khala-code-desktop/tests/app-shell.test.ts`
- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun run --cwd clients/khala-code-desktop verify`
