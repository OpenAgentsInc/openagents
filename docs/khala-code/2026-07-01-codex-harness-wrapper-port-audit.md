# Khala Code Codex Harness Wrapper / Port Audit

Date: 2026-07-01
Status: product pivot audit and implementation direction
Scope: Khala Code Desktop, Khala CLI/headless surfaces where they overlap with
Codex, Pylon/Codex delegation, and the local Codex reference checkout.

## Executive decision

Khala Code should pivot from "a Khala-native coding harness inspired by Codex"
to "a direct desktop/web wrapper around the Codex harness, with Khala swarm
coordination layered around it."

The product should require a working Codex install and Codex authentication as
the default user contract. Khala Code should then bootstrap or connect to Codex
app-server, preserve Codex Core/TUI semantics, and render those semantics through
our desktop web shell. The Khala-specific value should live in:

- our sidebar and multi-pane desktop navigation;
- fleet/account visibility;
- Pylon and Khala swarm delegation;
- unified inbox and proof/receipt views;
- web-native transcript, diff, file, terminal, and trace rendering;
- optional Gym/proof/demo surfaces.

The default agent loop should not be the current
`runKhalaCodeDesktopChatTurn` hosted-Khala plus `@openagentsinc/khala-tools`
loop. That loop has been useful as a fast local prototype, but it is now the
wrong abstraction for 100 percent Codex feature parity. Codex already has the
session model, approvals, sandboxing, tool routing, plugin/skill/MCP policy,
slash commands, app-server API, realtime, remote-control, rollout, and
multi-agent mechanics we need. Rebuilding those in TypeScript will put Khala
Code permanently behind upstream Codex.

## Evidence reviewed

OpenAgents working context:

- Reviewed the last 20 OpenAgents commits on `origin/main`, ending at
  `e477f617b936` (`feat(pylon): worker 3/5...`) and starting at
  `e6eb7038b112` (`Reconcile Codex fleet capacity`).
- The recent chain is heavily Khala/Codex oriented:
  - Khala Code composer visual smoke coverage.
  - Unified Inbox view.
  - Codex spawn progress replacement hardening.
  - Pylon/Codex turn ingest and token proof paths.
  - Codex account concurrency and fleet slot accounting.
  - Khala spawn objective bounding and token proof backfills.
  - Pylon assignment lease freshness/accounting.

Codex reference context:

- Ran `git pull --ff-only` in `projects/repos/codex`.
- Codex `main` fast-forwarded from `cfead68e5d` to `db887d03e1`.
- The pulled delta removed full text websocket trace output and adjusted TUI
  safety notice wording.
- Reviewed recent Codex commits around:
  - app-server environment RPC;
  - app-server JSON shutdown logs;
  - runtime plugin marketplace policy;
  - remote plugin defaults;
  - model metadata for skills instructions;
  - safety buffering and security notice wording;
  - unified exec pushed process events;
  - stable synthesized call output IDs;
  - namespace preservation for custom tool calls;
  - reasoning effort/catalog changes.

OpenAgents checkout note:

- The normal `openagents/` checkout has an unresolved conflict in
  `clients/khala-code-desktop/tests/khala-chat-runtime.test.ts`.
- This audit was written from a clean detached worktree at
  `openagents-worktrees/khala-code-codex-port-audit-20260701` based on
  `origin/main` so that existing work was not touched.

## Current Khala Code architecture

Khala Code Desktop is currently an Electrobun app:

- Bun host: `clients/khala-code-desktop/src/bun/index.ts`.
- RPC layer: `clients/khala-code-desktop/src/bun/rpc-handlers.ts` and
  `src/shared/rpc.ts`.
- UI shell: `clients/khala-code-desktop/src/ui/main.ts`, `sidebar.ts`,
  `inbox.ts`, `fleet-status.ts`, `gym-pane.ts`.
- Tool/chat runtime:
  `clients/khala-code-desktop/src/bun/khala-chat-runtime.ts`.
- Pylon/Codex fleet tools:
  `clients/khala-code-desktop/src/bun/khala-codex-fleet-tools.ts`.
- Shared tool runtime: `packages/khala-tools`.

The current desktop chat loop:

1. Builds a Khala-specific system prompt.
2. Routes model traffic through hosted OpenAgents/Khala.
3. Converts `@openagentsinc/khala-tools` definitions into OpenAI-compatible
   function tools.
4. Executes tool calls locally through a TypeScript dispatcher.
5. Emits desktop transcript events over Electrobun RPC.
6. Adds Pylon/Codex delegation as just another tool family:
   `pylon_ensure`, `codex_fleet_status`, `codex_spawn`.

That has produced useful primitives:

- working desktop shell;
- command composer/HUD;
- sidebar;
- fleet panel;
- unified inbox seed;
- Gym/proof pane;
- headless JSONL mode;
- Pylon/Codex account and spawn tooling;
- live `codex_spawn` progress mechanics;
- a TypeScript tool dispatcher with approval events, output bounds, MCP,
  session rollout, apply patch, process execution, and redaction.

But this is still a parallel harness. It is not Codex. It does not inherit
Codex's app-server protocol, exact item model, slash dispatch behavior, TUI
state machines, app/plugin/skill policy, auth/account model, rollout database,
permissions profiles, sandbox/retry orchestrator, or upstream tests.

## Codex architecture that Khala Code should wrap

Codex is already split in the way a desktop wrapper wants:

- `codex-rs/core`: agent session, tool planning, turn context, context
  construction, approvals, sandbox orchestration, unified exec, MCP, plugins,
  skills, multi-agent, rollout/session state.
- `codex-rs/tui`: interactive terminal UI, slash command parsing/dispatch,
  popups, composer behavior, status/diff/usage displays, keyboard handling.
- `codex-rs/app-server`: JSON-RPC server for rich clients.
- `codex-rs/app-server-protocol`: typed v2 protocol and generated schemas.
- `codex-rs/cli`: main user CLI, login/logout, exec/review, MCP/plugin
  commands, app-server, app launcher, sandbox, resume/fork/archive/delete.
- `codex-rs/exec-server`: process execution service and remote execution
  support.
- `codex-rs/tools`: tool spec, namespaces, dynamic tools, tool search,
  Responses API shape.

The key interface is `codex app-server`, not terminal scraping and not a
TypeScript reimplementation. Its README defines a local JSON-RPC protocol over
stdio, websocket, and unix socket. It exposes exactly the primitives Khala Code
needs:

- initialize/initialized handshake with client metadata;
- thread start/resume/fork/list/read/archive/delete/unarchive/name;
- turn start/steer/interrupt/compact;
- item notifications for agent messages, reasoning, commands, diffs, MCP calls,
  dynamic tools, subagents, and approvals;
- command execution with streaming output/write/resize/terminate;
- process spawn/write/resize/kill;
- fs read/write/copy/remove/watch;
- model list and provider capabilities;
- permission profile list;
- feature flags;
- environment add/info;
- collaboration modes;
- skills list/config/write and change notifications;
- hooks list;
- plugin and marketplace management;
- app/connector list;
- MCP server status, resource read, tool calls, OAuth login;
- realtime text/audio sessions;
- remote control pairing/status/client management;
- feedback upload;
- config read/write/batch write/config requirements.

That API is the proper harness boundary for Khala Code Desktop.

## Slash command parity target

The current Codex TUI built-in slash command inventory from
`codex-rs/tui/src/slash_command.rs` is:

- `/model`
- `/ide`
- `/permissions`
- `/keymap`
- `/vim`
- `/setup-default-sandbox`
- `/sandbox-add-read-dir`
- `/experimental`
- `/approve`
- `/memories`
- `/skills`
- `/import`
- `/hooks`
- `/review`
- `/rename`
- `/new`
- `/archive`
- `/delete`
- `/resume`
- `/fork`
- `/app`
- `/init`
- `/compact`
- `/plan`
- `/goal`
- `/agent`
- `/subagents`
- `/side`
- `/btw`
- `/copy`
- `/raw`
- `/diff`
- `/mention`
- `/status`
- `/usage`
- `/debug-config`
- `/title`
- `/statusline`
- `/theme`
- `/pets` and `/pet`
- `/mcp`
- `/apps`
- `/plugins`
- `/logout`
- `/quit`
- `/exit`
- `/feedback`
- `/rollout`
- `/ps`
- `/stop` and `/clean`
- `/clear`
- `/personality`
- `/test-approval` in debug builds
- `/debug-m-drop` and `/debug-m-update` debug memory commands

Codex also injects model service-tier commands after `/model` when enabled.

Parity is not just making these strings autocomplete. The TUI has command
availability rules:

- some commands accept inline args;
- some are hidden or disabled by feature flags;
- some are unavailable during an active task;
- some remain available in side conversations;
- `/resume` has stricter active-turn blocking;
- Windows-only commands are conditionally visible;
- app/desktop commands are platform gated;
- aliases such as `/clean` and `/pet` must resolve exactly.

Khala Code should render these through web UI, but the behavior contract must
come from Codex. When a slash command is TUI-local and not currently represented
as app-server RPC, we should either:

1. call the equivalent app-server method;
2. add or upstream a small app-server method in Codex;
3. keep a Khala UI-only adapter that delegates back to Codex state, with a
   parity test against the Codex enum/dispatch behavior.

Free-form recreation in Khala's chat prompt is not acceptable for parity.

## Feature parity matrix

| Area | Codex authority | Current Khala status | Pivot requirement |
| --- | --- | --- | --- |
| Auth/account | `codex login`, `codex logout`, app-server account processors, Codex home | Khala has hosted OpenAgents token plus Codex fleet account probes | Require Codex auth for default harness; use Codex account state directly; keep isolated Pylon account homes for swarm workers |
| Conversation/session | Codex thread/turn/item model, rollouts, sqlite/thread store | Khala has ad hoc message array and sessionId | Replace with app-server thread and turn lifecycle |
| Streaming | app-server `turn/*`, `item/*`, deltas, command output deltas | Khala emits `message_start/delta/replace/done` | Map Codex notifications to web transcript items without losing item ids or statuses |
| Tools | Codex tool router, ToolSpec, ToolSearch, MCP, dynamic tools, extensions | `@openagentsinc/khala-tools` duplicates many basics | Default to Codex tools; retain Khala tools only as outer/supplemental tools where Codex lacks Khala swarm behavior |
| Shell/process | Codex unified exec, exec-server, command/exec, process/spawn | Khala has `exec_command` and `write_stdin` TypeScript tools | Use Codex command/process APIs for parity |
| Apply patch/diff | Codex apply patch handler, file change approval items | Khala has TypeScript apply_patch and edit/write | Use Codex patch/file-change items; avoid divergent patch grammar/approval behavior |
| Approvals | Codex approval policy, permission profiles, guardian, execpolicy/network amendments | Khala has permission service and approval cache | Render Codex approval requests; do not emulate policy |
| Sandbox | Codex sandbox manager, permission profiles, Windows setup | Khala has macOS seatbelt helper and owner-local permission modes | Use Codex sandbox/permissions as source of truth |
| Slash commands | Codex TUI enum/dispatch and app-server equivalents | Khala currently has no full slash command bus | Implement command palette/slash parser against Codex command inventory and app-server methods |
| Model/settings | Codex model catalog, reasoning, service tier, personality, collaboration mode | Khala uses hosted Khala/OpenRouter routing | Expose Codex model/settings controls; Khala backend selection becomes optional swarm metadata, not primary chat |
| MCP/plugins/apps | Codex app-server processors, marketplace policy, skills watcher | Khala has standalone MCP client/server and external MCP wrapping | Use Codex processors for default; project Khala-specific connector health into our sidebar/inbox |
| Skills/memory | Codex skills list/read/config, memory mode/reset | Khala has progressive disclosure and session rollout pieces | Use Codex skill/memory APIs; avoid parallel memory governance for Codex sessions |
| Multi-agent | Codex subagents/multi-agent v1/v2, agent picker, side conversations | Khala has Pylon/Codex swarm delegation | Preserve Codex subagents, then add Khala swarm as an outer fleet layer |
| Realtime/remote | Codex realtime and remoteControl APIs | Khala desktop does not own this yet | Wrap or hide until app-server support is wired |
| UI chrome | TUI bottom pane and popups | Khala has web sidebar/inbox/fleet/gym | Keep Khala UI advantages while mapping to Codex state |

## Main finding: app-server should become the kernel

The fastest path to parity is not to port every Rust file to TypeScript. It is
to treat `codex app-server` as the local kernel process and make Khala Code a
rich desktop client.

Concretely:

1. Khala Code starts by checking for `codex` on PATH or a configured Codex
   binary path.
2. It checks Codex auth/status and guides the user to install/sign in if absent.
3. It starts or connects to `codex app-server` using stdio or unix socket.
4. It sends `initialize` with a Khala Code client identity.
5. It creates/resumes/forks Codex threads through `thread/*`.
6. It starts and steers turns through `turn/*`.
7. It renders app-server notifications into Khala's web transcript.
8. It uses app-server config/model/plugin/skills/MCP/process/fs APIs for all
   Codex-equivalent behavior.
9. It layers Pylon/Khala swarm controls outside the Codex thread, not inside a
   fake replacement harness.

This keeps us current with upstream Codex. It also lets users trust that when
they use Khala Code, they are still using Codex's real local semantics.

## What to keep from current Khala Code

Keep these as product advantages:

- Electrobun desktop shell.
- Web preview mode for quick testing.
- Sidebar navigation.
- Unified Inbox concept.
- Fleet status panel.
- Pylon/Codex account connection and capacity views.
- `codex_spawn` as Khala swarm delegation, but not as the default single-agent
  chat mechanism.
- Gym/proof panes.
- Composer HUD and visual smoke infrastructure.
- On-device decider experiments as optional secondary affordances, not the
  default Codex harness.
- Headless JSONL mode, if it can drive app-server turns.

Keep `@openagentsinc/khala-tools` as a useful library for:

- Khala-only tools exposed to Khala swarm orchestration;
- Pylon delegation helpers;
- MCP server compatibility surfaces where they do not duplicate Codex default
  behavior;
- fixture/testing tools.

Do not keep it as the default Codex-parity tool router.

## What to retire or demote

Demote these from the primary path:

- `runKhalaCodeDesktopChatTurn` as the main chat runtime.
- Hosted Khala/OpenRouter as the default model path for "Khala Code" coding.
- The Khala system prompt as the authority for Codex behavior.
- TypeScript implementations of Codex-equivalent tools as first-class parity
  targets.
- Any attempt to recreate slash commands through prompt instruction instead of
  command dispatch.

They can survive behind a compatibility or "Khala native" feature flag, but the
default product promise should be: "Khala Code wraps your Codex."

## Target architecture

Recommended process layout:

```text
Khala Code Desktop
  Web UI
    sidebar
    transcript
    command palette/slash menu
    inbox
    fleet board
    proof/gym panes
  Electrobun/Bun host
    Codex app-server client
    Codex process supervisor
    Khala swarm/Pylon client
    local config adapter
  Local services
    codex app-server
    codex exec-server as managed by Codex
    pylon node when swarm is enabled
  User state
    CODEX_HOME or default ~/.codex for the main user's Codex
    isolated Pylon account homes for additional Codex fleet accounts
```

Important state split:

- The primary local user session may use the user's normal Codex home, because
  the product is now explicitly a Codex wrapper.
- Khala fleet accounts must remain isolated under Pylon account homes. The
  existing "never clobber the owner's live Codex session" rule still applies to
  login flows and multi-account fleet connect.
- Khala Code should never run `codex login` against default `~/.codex` without
  explicit user intent and visible warning, because login flow start can rewrite
  auth state. For app-server wrapping, prefer detecting status and opening the
  normal Codex login UX/command guidance.

## Implementation plan

### Phase 0: freeze the promise

Define the new product promise:

- "Khala Code requires Codex and wraps Codex for local coding."
- "Feature parity is measured against Codex app-server/TUI semantics."
- "Khala adds desktop navigation and swarm coordination."

Add a feature flag split:

- `codex_harness` as the default new path.
- `khala_native_runtime` as legacy/fallback.

### Phase 1: app-server bootstrap and minimal chat

Add a Bun-side Codex app-server client:

- spawn `codex app-server --stdio` or connect to the app-server daemon socket;
- send `initialize` and `initialized`;
- expose typed request/notification helpers;
- start a thread with cwd/workspace roots;
- submit `turn/start`;
- interrupt with `turn/interrupt`;
- render agent message deltas and completed items.

Initial user-visible parity bar:

- new thread;
- submit prompt;
- stream assistant text;
- stream reasoning summaries if available;
- interrupt;
- show completed/failed/interrupted turn state;
- preserve thread id and reload it.

### Phase 2: item renderer parity

Map app-server `ThreadItem` variants into Khala web components:

- user message;
- hook prompt;
- agent message;
- plan;
- reasoning;
- command execution;
- file change;
- MCP tool call;
- dynamic tool call;
- collab/subagent tool call;
- subagent activity;
- patch/file-change deltas;
- command output deltas;
- approval prompts;
- network approval contexts;
- guardian/security assessments.

This is where the web UI can beat the TUI. Rich cards, side-by-side diffs,
terminal panes, file preview, copy controls, progress timelines, and filters are
desktop advantages. The data and lifecycle still come from Codex.

### Phase 3: slash command parity

Implement a Khala command registry whose source of truth is Codex:

- generate or scrape the Codex slash command inventory during development;
- store command metadata in a checked fixture with the Codex commit hash;
- add a parity test that fails when the Codex enum changes;
- dispatch every command to an app-server method or a narrowly scoped web UI
  action that mutates Codex state.

Command groups:

- session: `/new`, `/resume`, `/fork`, `/archive`, `/delete`, `/clear`,
  `/rename`, `/app`, `/rollout`;
- turn/task: `/compact`, `/review`, `/plan`, `/goal`, `/side`, `/btw`,
  `/agent`, `/subagents`;
- workspace: `/diff`, `/mention`, `/ide`, `/init`;
- settings: `/model`, `/permissions`, `/keymap`, `/vim`, `/experimental`,
  `/memories`, `/personality`, `/theme`, `/title`, `/statusline`, `/pets`;
- ecosystem: `/skills`, `/hooks`, `/mcp`, `/apps`, `/plugins`, `/import`;
- account/diagnostics: `/status`, `/usage`, `/debug-config`, `/feedback`,
  `/logout`;
- background terminals: `/ps`, `/stop` and `/clean`;
- exit: `/quit`, `/exit`.

Where app-server lacks a direct method, add the minimum upstream/app-server
surface instead of baking behavior into Khala.

### Phase 4: Codex session sidebar

Replace representative sidebar items with real Codex data:

- active thread;
- recent threads from `thread/list`;
- archived filter;
- cwd/project grouping;
- search;
- fork tree/parent-child relationships;
- loaded/in-progress status;
- goal badge;
- dirty/diff indicators when available.

Keep Khala-specific top-level sections:

- Chat;
- Inbox;
- Fleet;
- Gym/Proofs;
- Settings.

But Chat should be a Codex thread, not a Khala-native synthetic conversation.

### Phase 5: approvals, permissions, sandbox, settings

Render Codex approval requests exactly:

- command execution decisions:
  - accept;
  - accept for session;
  - accept with execpolicy amendment;
  - apply network policy amendment;
  - decline;
  - cancel;
- file change decisions:
  - accept;
  - accept for session;
  - decline;
  - cancel;
- MCP and dynamic tool approval states;
- permission profile selection;
- sandbox policy and Windows sandbox setup;
- network constraints and managed requirements.

The UI can be nicer than the TUI, but it must send Codex's typed decisions.
Khala should not translate these into its own permission enum on the default
path.

### Phase 6: plugins, skills, MCP, apps

Use Codex app-server APIs:

- `skills/list`;
- `skills/extraRoots/set`;
- `skills/config/write`;
- `hooks/list`;
- `marketplace/*`;
- `plugin/*`;
- `app/list`;
- `mcpServerStatus/list`;
- `mcpServer/resource/read`;
- `mcpServer/tool/call`;
- `mcpServer/oauth/login`.

Khala's Inbox can aggregate:

- MCP auth failures;
- plugin install/auth requirements;
- disabled-by-admin availability;
- skill changes;
- hook diagnostics.

But the underlying state and policy must remain Codex-owned.

### Phase 7: Khala swarm layer

After single-session parity is real, re-add the swarm as a first-class desktop
advantage:

- Pylon account connection/status remains in Fleet.
- `codex_spawn` becomes "delegate this Codex-backed task to my Khala swarm."
- Each swarm worker is a Codex-capable worker with isolated Codex home.
- Fleet board shows capacity, active assignments, token proof, closeout state,
  and queue/refill policy.
- Inbox receives worker approval/block/review events.
- A user can promote a current Codex task into a Khala swarm fanout, but the
  individual worker execution still uses Codex harness semantics.

This sequence keeps the product honest: first match Codex one-user behavior,
then add Khala multi-worker behavior.

## Gaps and risks

### P0: parallel harness drift

Every feature implemented in `@openagentsinc/khala-tools` as a Codex-equivalent
runtime becomes drift debt. Codex changed app-server, safety notices, tool
namespace preservation, plugin policy, skills instructions, unified exec, and
reasoning catalog behavior in the recent commit window alone. Khala will not
catch that by hand.

Mitigation: make Codex app-server the default harness and keep Khala tools out
of the parity path.

### P0: slash commands are UI state machines, not prompts

Codex slash commands are not merely strings. They have feature gates, inline
argument rules, active-task availability, side-conversation restrictions,
platform gates, popups, and app-server side effects.

Mitigation: build command dispatch against Codex metadata/methods and add a
parity test against `codex-rs/tui/src/slash_command.rs`.

### P0: approval/sandbox semantics cannot be approximated

The Codex orchestrator combines approval policy, permission profiles, guardian,
execpolicy/network amendments, sandbox selection, retry semantics, and tool
hooks. A nicer UI is fine. A different policy is not parity.

Mitigation: render and answer Codex approval requests directly.

### P1: app-server APIs include experimental surfaces

Many valuable APIs are marked experimental or "under development." They are
still the right local integration boundary, but Khala must expect schema drift.

Mitigation: generate TypeScript schemas from the exact Codex binary in use,
version the client adapter, and keep contract tests with fixture notifications.

### P1: auth and Codex home safety

Requiring Codex means using Codex auth state. Fleet connect still needs isolated
homes. Confusing these two paths can clobber the user's active Codex session.

Mitigation: make the "main wrapper Codex home" and "fleet worker Codex homes"
visibly separate in code and UI.

### P1: item renderer completeness

A partial item renderer will feel like a regression versus Codex TUI. Missing
items include reasoning, plan, command output, file changes, MCP calls, dynamic
tools, subagents, approvals, network approvals, and guardian assessments.

Mitigation: build from app-server `ThreadItem` and notification fixtures before
adding visual flourish.

### P1: Codex TUI logic not all exposed as app-server methods

Some slash command behavior is TUI local. If Khala clones it in TypeScript, it
will drift.

Mitigation: prefer app-server additions or generated command metadata. Keep
Khala adapters tiny and test them against upstream Codex fixtures.

### P2: Pylon delegation currently talks through CLI/processes

The Pylon/Codex delegation layer mostly routes through local commands and
assignment lifecycle streams. That remains useful for swarm, but it should not
be confused with the single-user Codex harness.

Mitigation: split "Codex wrapper session" from "Khala swarm delegation" in
product copy, UI, and code modules.

## Concrete first engineering slice

The smallest useful pivot slice is:

1. Add `clients/khala-code-desktop/src/bun/codex-app-server-client.ts`.
2. Spawn/connect to `codex app-server --stdio`.
3. Implement initialize/initialized and JSON-RPC request id handling.
4. Add RPC methods:
   - `codexHarnessStatus`;
   - `codexThreadStart`;
   - `codexTurnStart`;
   - `codexTurnInterrupt`;
   - `codexThreadList`;
   - `codexThreadResume`.
5. Add a transcript adapter from app-server notifications to current Khala
   message events.
6. Gate the current chat submit path:
   - default: Codex app-server harness;
   - fallback flag: legacy Khala native runtime.
7. Render at least:
   - user message;
   - assistant delta/message;
   - reasoning summary;
   - command execution start/output/completion;
   - file change start/completion;
   - approval request.
8. Add fixtures from a mocked app-server stream.
9. Add a local smoke that fails clearly when `codex` is missing.

This gives the product its new spine without blocking on every slash command.

## Verification plan

Parity should be tested mechanically:

- App-server schema generation:
  - run `codex app-server generate-ts --out <tmp>`;
  - verify the Khala adapter compiles against that generated schema.
- Slash inventory:
  - parse or snapshot `SlashCommand` from Codex;
  - compare against Khala's command registry.
- Notification rendering:
  - replay fixture streams containing every `ThreadItem` and delta family;
  - assert stable DOM snapshots for transcript cards.
- Approval routing:
  - fixture command/file/MCP approval requests;
  - assert the exact typed response payload sent to app-server.
- Process/terminal:
  - mock `command/exec/outputDelta`, `process/outputDelta`, resize, terminate.
- Session:
  - thread start/resume/fork/archive/delete/unarchive/name.
- Settings:
  - model list, permissions list, feature flag list, config read/write.
- Live smoke:
  - skip when `codex` is unavailable;
  - start app-server;
  - create temporary thread in a temp cwd;
  - run a harmless prompt;
  - interrupt and resume;
  - close the app-server process cleanly.

Do not declare 100 percent parity until slash commands, item rendering, approval
decisions, session lifecycle, model/settings controls, MCP/plugins/skills, and
command execution all pass fixture and at least one live smoke.

## Product copy implications

Old framing:

- "Khala Code is our local coding agent runtime."
- "Hosted Khala/OpenRouter drives desktop coding."
- "Codex appears as a fleet delegate."

New framing:

- "Khala Code is a desktop wrapper for your Codex, with Khala swarm controls."
- "Install/sign in to Codex first."
- "Use Codex normally in Khala's desktop UI."
- "When you want more throughput, connect more Codex accounts to Khala Fleet and
  delegate work through Pylon."

This is a cleaner promise. It is also easier to trust.

## Bottom line

Codex app-server is the harness boundary. Khala Code should become a faithful,
web-native Codex client first, then a Khala swarm console second. The existing
Khala desktop shell, sidebar, inbox, fleet, and proof surfaces are good
advantages. The existing Khala-native tool loop is good prototype scaffolding,
but it should no longer be the default path for the product now that the goal is
100 percent Codex feature parity.
