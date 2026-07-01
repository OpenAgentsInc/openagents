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

## Issue #7786: Approvals, Permissions, Sandbox, And Guardian Decisions

Status: implemented

Khala Code Desktop now answers Codex app-server approval requests through the
same JSON-RPC server-request channel that Codex uses. The app-server host can
write a response with the original server request id, and desktop RPC exposes
`codexApprovalRespond()` for command execution, file change, and permission
approval requests.

The approval response builder pins Codex's generated protocol shapes:

- command decisions: `accept`, `acceptForSession`, `decline`, `cancel`,
  `acceptWithExecpolicyAmendment`, and `applyNetworkPolicyAmendment`;
- file-change decisions: `accept`, `acceptForSession`, `decline`, and
  `cancel`;
- permission grants: turn-scoped, session-scoped, strict turn review, and
  empty-permission decline responses.

Approval cards now preserve request ids, available decisions, command/cwd
context, requested permissions, proposed execpolicy amendments, proposed network
policy amendments, and network context in the Codex item metadata. Pending
approval cards render desktop buttons that send typed Codex responses rather
than sending prompt text or using the legacy Khala permission dispatcher.

The UI keeps amendment choices explicit: execpolicy and network policy
amendments only appear when Codex supplied the proposed amendment payload, and
network amendments are sent back as `network_policy_amendment` exactly as
app-server expects.

Validation:

- `bun test clients/khala-code-desktop/tests/codex-approval-decisions.test.ts clients/khala-code-desktop/tests/codex-app-server-client.test.ts clients/khala-code-desktop/tests/codex-thread-item-projector.test.ts clients/khala-code-desktop/tests/rpc-handlers.test.ts clients/khala-code-desktop/tests/app-shell.test.ts`
- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun run --cwd clients/khala-code-desktop verify`

## Issue #7787: Models, Config, Usage, Personality, And Collaboration Settings

Status: implemented

Khala Code Desktop now treats Codex app-server as the source of truth for the
default wrapper settings surface instead of duplicating model or permission
state in Khala preferences.

The desktop RPC exposes:

- `codexSettingsRead()` to fan out across `config/read`, `model/list`,
  `modelProvider/capabilities/read`, `permissionProfile/list`,
  `configRequirements/read`, `account/usage/read`, and
  `collaborationMode/list`;
- `codexConfigValueWrite()` to persist a single Codex config key through
  `config/value/write` and then re-read the app-server settings projection.

The shared settings projector preserves the Codex model catalog, reasoning
effort options, model-advertised service tiers, provider capability flags,
permission profile allowance state, managed requirements, config origins,
usage summary, and collaboration-mode presets. It intentionally projects only
safe setting fields instead of returning the raw Codex config object.

The desktop shell now has a `Settings` sidebar view with Codex-backed controls
for model, reasoning effort, service tier, permission profile, and personality,
plus readouts for provider capabilities, approval policy, reviewer, sandbox
mode, usage, and managed requirements. Writes go through app-server config
mutation only; denial responses surface as local status instead of falling back
to a Khala-local preference cache.

Validation:

- `bun test clients/khala-code-desktop/tests/codex-settings.test.ts clients/khala-code-desktop/tests/rpc-handlers.test.ts clients/khala-code-desktop/tests/app-shell.test.ts`
- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun run --cwd clients/khala-code-desktop verify`

## Issue #7788: Codex Session Sidebar And Thread Navigation

Status: implemented

Khala Code Desktop now has a Codex-backed thread navigator for the Chat surface.
The top-level Khala sections remain `Chat`, `Inbox`, `Fleet status`, `Gym`, and
`Settings`, while the Chat view gets an adjacent Codex thread rail backed by
app-server thread APIs.

The desktop runtime now supports:

- `thread/list` with search, archived filtering, active-thread projection, and
  cwd/project grouping;
- `thread/read` with optional turn loading;
- `thread/resume` transcript restoration through the Codex ThreadItem
  projector;
- `thread/fork` with forked thread id persistence for the active desktop
  session;
- `thread/archive`, `thread/unarchive`, `thread/delete`, and `thread/name/set`
  lifecycle operations.

The shared thread projector preserves Codex thread ids, session ids,
fork/parent relationships, model provider, source, cwd grouping, runtime
status, recency timestamps, and derived badges for running, failed, forked,
child, and git-backed threads. Search and grouping are read-only projections
over `thread/list`; they do not mutate Codex state.

Selecting a thread calls `thread/resume`, persists the active Codex thread id,
and replaces the visible transcript with messages replayed from Codex turns.
New ordinary chat turns also update the active thread id from the Codex turn
response, so reload keeps the selected Codex thread identity instead of falling
back to a synthetic local chat.

Validation:

- `bun test clients/khala-code-desktop/tests/codex-threads.test.ts clients/khala-code-desktop/tests/codex-app-server-chat-runtime.test.ts clients/khala-code-desktop/tests/rpc-handlers.test.ts clients/khala-code-desktop/tests/app-shell.test.ts`
- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun run --cwd clients/khala-code-desktop verify`

## Issue #7789: Plugins, Skills, MCP, Apps, Hooks, And Inbox Diagnostics

Status: implemented

Khala Code Desktop now reads Codex ecosystem state from Codex app-server
instead of maintaining a parallel plugin, skill, app, hook, or MCP registry.
The desktop RPC exposes `codexEcosystemRead()` as a fan-out across:

- `skills/list`
- `hooks/list`
- `plugin/list`
- `plugin/installed`
- `app/list`
- `mcpServerStatus/list`

The same RPC projection records recent app-server invalidation/auth events from
`skills/changed`, `app/list/updated`, `mcpServer/startupStatus/updated`, and
`mcpServer/oauthLogin/completed`. The projection keeps Khala-only swarm and
fleet helpers in a separate `Khala desktop extensions` section so Codex
connectors remain visibly distinct from desktop advantages.

Direct pass-through RPCs now cover Codex ecosystem actions without a Khala-side
runtime fork:

- skill roots and enablement: `skills/extraRoots/set`,
  `skills/config/write`;
- marketplace and plugin operations: `marketplace/add`,
  `marketplace/remove`, `marketplace/upgrade`, `plugin/install`, and
  `plugin/uninstall`;
- MCP operations: `mcpServer/resource/read`, `mcpServer/tool/call`,
  `mcpServer/oauth/login`, and `config/mcpServer/reload`.

Settings now renders Codex ecosystem health counts for skills, hooks, plugins,
marketplaces, apps, MCP servers, and Khala desktop extensions. Unified Inbox
turns ecosystem diagnostics into actionable rows, including MCP auth failures,
MCP startup/login failures, disabled/admin-managed plugin state, install/auth
requirements, disabled connectors, unknown app-server states, and skill-change
invalidation notices.

Validation:

- `bun test clients/khala-code-desktop/tests/codex-ecosystem.test.ts clients/khala-code-desktop/tests/rpc-handlers.test.ts clients/khala-code-desktop/tests/app-shell.test.ts`
- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun run --cwd clients/khala-code-desktop verify`

## Issue #7790: Demote Legacy Khala Native Runtime And Khala Tools

Status: implemented

Khala Code Desktop now makes the runtime split explicit in the desktop RPC
contract. Backend projections carry `runtimeMode`, and tool catalogs carry both
`runtimeMode` and `catalogKind`:

- default: `codex_harness` with `codex_harness_supplemental`;
- opt-in legacy: `khala_native_runtime` with `khala_native_legacy`;
- Codex turn responses are labeled `codex_app_server`.

The default `toolCatalog()` no longer exposes Codex-equivalent Khala tools. It
returns only the supplemental Pylon/Codex fleet tools that remain useful around
the Codex harness:

- `pylon_ensure`
- `codex_fleet_status`
- `codex_spawn`

The full Khala-native registry still exists for explicit legacy/fallback mode
and tests, but filesystem, shell, patch, and local search helpers are labeled
`legacy_codex_equivalent`. Normal desktop chat submit does not fall back to that
runtime when Codex app-server is missing; it fails on the Codex path instead.
When the user explicitly sets `KHALA_CODE_DESKTOP_RUNTIME=khala_native_runtime`
or `KHALA_CODE_DESKTOP_LEGACY_KHALA_NATIVE_RUNTIME=1`, the returned transcript
starts with a visible system banner explaining that the legacy Khala-native
runtime handled the turn.

The README Tools section now documents the default supplemental catalog first
and moves the Codex-equivalent Khala tools under the explicit legacy/fallback
mode.

Validation:

- `bun test clients/khala-code-desktop/tests/khala-chat-runtime.test.ts clients/khala-code-desktop/tests/rpc-handlers.test.ts clients/khala-code-desktop/tests/headless.test.ts clients/khala-code-desktop/tests/codex-app-server-chat-runtime.test.ts`
- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun run --cwd clients/khala-code-desktop verify`

## Issue #7791: Re-layer Pylon/Khala Swarm Delegation On Codex Sessions

Status: implemented

The Fleet surface now projects Khala Code as a Codex-wrapper hierarchy instead
of a second local coding harness. `codexFleetStatus()` returns `sessionLayers`
that distinguish the main local Codex session from Khala swarm worker Codex
sessions. Worker accounts carry `sessionRole`, `homeRole`, queue/refill policy,
cooldown state, readiness, and capacity. Active assignments carry worker
session metadata with Codex-harness runtime, isolated-home policy, transcript
refs, closeout status, blocker refs, review state, and token proof.

The Fleet board graph adds a `main-codex-session` node and caveat so the graph
shows the main Codex chat path separately from the Pylon capacity gate and
worker pool. The Fleet list adds a `Codex sessions` section and renames linked
accounts as `Worker Codex accounts`.

`codex_spawn` copy now describes delegation from the main Codex-backed task to
isolated Pylon worker homes. The new `codexFleetPromoteThread()` RPC wraps
`spawnCodexInstances()` for promotion from a current Codex thread into a swarm
fanout request. The request requires an origin `sessionId`/`threadId`, an
explicit objective, and `includeTranscript: false`; it only carries allowed
public refs and an optional user-written summary into the worker prompt.

Inbox assignment routing now uses worker metadata: approval blockers become
`approval_required`, blocker refs become `run_blocked`, and normal closeout /
proof review remains `ready_for_review`. The Fleet connect path also has a
focused regression asserting display-only default account refs are rejected
before any Codex login can touch the main user home.

Validation:

- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun test clients/khala-code-desktop/tests/rpc-handlers.test.ts clients/khala-code-desktop/tests/khala-codex-fleet-tools.test.ts clients/khala-code-desktop/tests/fleet-board-projection.test.ts clients/khala-code-desktop/tests/app-shell.test.ts`
- `bun run --cwd clients/khala-code-desktop verify`

## Issue #7792: Headless JSONL And Preview Modes On App-server

Status: implemented

`khala code --json` now uses the Codex app-server chat runtime instead of the
legacy Khala-native chat turn. The headless runner creates a Codex-backed
thread through `startThread()`, streams projected Codex item notifications as
JSONL, submits the prompt through `startTurn()`, and writes a single final JSON
object to stdout. The event stream preserves desktop ids and adds Codex
correlation fields including `thread_id`, `codex_turn_id`, turn status, backend
kind, and app-server tool-catalog metadata.

Missing Codex/app-server/auth setup now fails as a structured headless error:
stderr receives `turn.failed` with `status:
codex_app_server_unavailable`, stdout receives one final `ok: false` JSON
object, and the CLI exits nonzero. The headless path also has an interrupt smoke
hook, `KHALA_CODE_HEADLESS_INTERRUPT_AFTER_MS`, that calls the Codex
`interruptTurn()` API for bounded automation tests.

The preview and smoke docs now state which harness each lane exercises:

- `smoke:composer-visual` / `smoke:composer-visual-preview` is a preview UI
  geometry lane labeled `preview_ui_codex_harness_shell`; it does not submit a
  model turn and does not exercise legacy Khala shell/process tools.
- `smoke:codex-spawn-live` is the guarded Pylon/Codex worker delegation lane
  labeled `pylon_codex_spawn_live`.

Validation:

- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun test clients/khala-code-desktop/tests/headless.test.ts clients/khala-code-desktop/tests/headless-events.test.ts clients/khala-code-desktop/tests/composer-visual-smoke.test.ts clients/khala-code-desktop/tests/app-shell.test.ts clients/khala-code-desktop/tests/rpc-handlers.test.ts`
- `bun run --cwd clients/khala-code-desktop verify`

## Issue #7793: Parity Contract Tests, Fixtures, And Live Smoke Suite

Status: implemented

Khala Code now has a first-class Codex parity contract in
`src/bun/codex-parity-contract.ts` and
`docs/khala-code/2026-07-01-codex-parity-contract.md`. The contract pins the
reference Codex checkout at `db887d03e1f907467e33271572dffb73bceecd6b`, records
the generated app-server schema files required for parity, and lists the
client-request methods, server-request methods, notifications, and `ThreadItem`
variants that must remain covered.

The new `codex-parity-contract.test.ts` suite verifies the pinned reference
checkout, schema files, app-server method inventory, server requests,
notifications, ThreadItem variants, slash-command dispatch mapping, and the
coverage matrix that distinguishes Codex-wrapper fixture tests from the legacy
Khala-native fallback guard.

The new guarded live smoke is exposed as `smoke:codex-parity-live`. By default it
returns a structured skip result without touching Codex. With
`KHALA_CODE_DESKTOP_CODEX_PARITY_LIVE_SMOKE=1` or `--require-live`, it fails
loudly if Codex is missing or unauthenticated; otherwise it starts app-server,
creates and resumes a temporary thread, starts a harmless turn, attempts
`turn/interrupt`, and shuts down cleanly.

Validation:

- `bun run --cwd clients/khala-code-desktop typecheck`
- `bun test clients/khala-code-desktop/tests/codex-parity-contract.test.ts clients/khala-code-desktop/tests/codex-parity-live-smoke.test.ts clients/khala-code-desktop/tests/codex-slash-commands.test.ts clients/khala-code-desktop/tests/codex-app-server-chat-runtime.test.ts clients/khala-code-desktop/tests/codex-thread-item-projector.test.ts clients/khala-code-desktop/tests/codex-approval-decisions.test.ts`
- `bun run --cwd clients/khala-code-desktop smoke:codex-parity-live`
- `bun run --cwd clients/khala-code-desktop verify`
