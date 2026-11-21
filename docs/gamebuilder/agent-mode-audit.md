# Agent mode audit: what exists for Claude changing a GameBuilder level

Written 2026-11-16. Read-only audit; nothing was changed in any repo.

Trees read:

- `gamebuilder` at its working tree (HEAD `a832cf2` or later).
- `openagents` main checkout at `19ff5d7715` ("MAIN").
- The pinned tree GameBuilder builds against, `../openagents-gamebuilder` at
  `42092fc2e9` ("PIN", `openagents.lock:4`).

PIN is 78 commits **ahead** of MAIN's checkout, not behind it. Every file this
audit relies on (`coax/lib/agent/claude/`, `coax/lib/agent/engine/claude.coax`,
`coax/lib/agent/world/`, `metaverse-engine/agent/`, `metaverse-engine/editor/`,
`metaverse-engine/host/apple/page-host.h`, `apps/openagents-cli/commands.coax`)
is byte-identical in both (`diff -q`). Paths below without a repo prefix are in
openagents and valid in PIN, so GameBuilder can `#load` them through
`vendor/openagents` today.

How to read the citations: "(inference)" marks something I concluded and did
not read. Section 7 lists which citations I read myself and which came from a
delegated sweep that I only spot-checked.

## 1. Summary

**What exists today**

- A complete Claude Agent SDK written in Coax, `coax/lib/agent/claude/`. It
  starts the installed `claude` CLI as a child process and speaks its
  stream-json protocol over stdin and stdout. It supports sessions and resume,
  streaming, tool allow and deny lists, permission prompts answered by the
  host, hooks, and in-process tools the host answers itself. It is tested and
  used by four programs.
- A shared engine on top of it, `coax/lib/agent/engine/claude.coax`: one
  long-lived `claude` per conversation, polled from a frame loop, folded into
  a session record.
- A "resident service" (`coder host serve`, installed as
  `~/.openagents/bin/openagents-environment`) that runs Claude on a computer
  for other clients. A phone or app reaches it over a Nostr relay with
  encrypted events, or locally through a Unix socket.
- Typed, agent-facing world edit formats in the engine: `world.edit.v1` (one
  JSON object per edit), `world.turn.v1` (the world as a model reads it) and
  `world.plan.v1` (the steps a model returns). The web editor already turns a
  model's plan into edits.
- In GameBuilder: Decide, one button that asks a hosted decision model for the
  next single edit and applies it through the same call a drag makes.

**What does not exist**

- GameBuilder has no Claude integration of any kind. No file in `coax/` or
  `host/Sources/` loads the Claude SDK or mentions Claude.
- GameBuilder's Coax core exports no call that takes an edit as data. The host
  can only send input events and press buttons (section 5.2).
- No OpenAgents app binds a Claude conversation to a world it is editing
  natively. That is milestone 2A of a spec that says it is not built
  (`docs/protocol/environments-claude-and-world-editing.md:3-4, 41`).

**Shortest path to a desktop agent mode with Claude on the same Mac**

1. Add two exports to GameBuilder's Coax core: one writes the level as text for
   the model, one applies a batch of edits and returns what happened. Both
   reuse engine codecs that already exist.
2. Run Claude with the Coax SDK, no built-in tools, and one in-process tool
   server whose tools are those level edits. `apps/everglade-desktop/next.coax`
   is the working template: same SDK, same in-process tool pattern, polled
   from a frame loop.
3. Decide where the `claude` process lives. This is the real blocker. The Mac
   app is sandboxed with only outbound network access
   (`host/GameBuilder-macOS.entitlements:5-8`), and a sandboxed app cannot
   usefully spawn the user's `claude`. Section 6 gives three ways round it and
   recommends one.

## 2. "The AI code we added to the iOS app yesterday"

**What I believe he means:** the shared app core and its paired-computer lane,
issue #13645, committed between 2026-11-15 23:42 and 2026-11-16 02:00 local
time. Confidence: fairly high, not certain.

Why not certain: by the calendar, "yesterday" (2026-11-15) also holds the
#13620 work, which added native task activity and permission controls to the
old iOS app (`6bc7933b1b`, 18:12, adding `apps/openagents-ios/home/native-task.coax`
and `apps/shared/chat-task-app.coax`). But `269227265a` (23:42) deleted the old
iOS system and moved `home/` to `apps/openagents-desktop/home/`, so the #13645
code is what the iOS app runs now.

The commits:

| Commit | Time | What it added |
| --- | --- | --- |
| `e9ce28a566` | 11-16 00:01 | Native services: `apps/shared/native/AppHTTP.swift`, `AppAccount.swift`, `AppDevice.swift`, `AppEffects.swift`, `AppWake.swift` |
| `19c6ce7f15` | 00:09 | The shared app core: `apps/shared/app/core/{core,dispatch,effects,host,state,task-client,url-policy}.coax` |
| `c68e4ca90d` | 00:14 | The core running in the iOS app: `apps/openagents-ios/AppCore.swift`, `core-host.c`, `core-host.h` |
| `e071d30f00` | 00:19 | The chat page: `apps/shared/app/view/page.coax` |
| `bbb7e59d0c` | 01:23 | Pairing a phone with a computer: `apps/shared/app/device/seal.coax` |
| `d567ece12e` | 01:41 | A conversation on a paired computer: `apps/shared/app/device/lane.coax` |
| `95aaa8b497` | 01:58 | The socket side: `apps/shared/native/AppDeviceLink.swift` |

**What it does.** The phone never runs an agent. It is a chat page whose
messages go to an agent somewhere else, by one of two routes.

Route A, logged in:

- HTTPS to `https://openagents.com` (`apps/shared/host/apple/AppAuth.swift:101-107`),
  limited to `/api/chat/*` and three account paths (`apps/shared/app/core/url-policy.coax:77`).
- Auth is the account's access token, added by `AppHTTP` (`apps/shared/native/AppHTTP.swift:91-93`).
- The reply is not streamed. The app polls a running turn's events every
  400 ms (`apps/shared/app/core/task-client.coax:32-34`).
- A worker on OpenAgents' side, or a task bound to an enrolled computer, runs
  the turn (`docs/protocol/account-chat-turn-v2alpha1.md:154-175`).

Route B, no account, "your own computer":

- Pairing: the computer prints an `openagents://pair?...` link; the app takes
  it and stores the pairing in the Keychain (`apps/shared/native/AppDeviceLink.swift:27-34, 82-120`).
- Transport: one WebSocket per relay, default `wss://relay.openagents.com`
  (`coder/tui/host.coax:58`). Requests and replies are Nostr events of kind
  3188, encrypted with NIP-44 (`coax/lib/nostr/host/browser.coax:1-41`).
- Auth: the phone's own secp256k1 key, a relay AUTH event of kind 22242
  (`apps/shared/app/device/seal.coax:105-107`), and a pairing grant that lasts
  7 days (`docs/protocol/environment-runtime-v1alpha1.md:126-136`).
- Methods: `DescribeEnvironment`, `ListSessions`, `ReadSession`, `OpenSession`,
  `SendMessage`, `StopTurn`, `RespondInteraction`
  (`docs/protocol/environment-runtime-v1alpha1.md:31-36`).
- Also polled, every 1500 ms (`apps/shared/app/device/lane.coax:34`).
- Permission prompts reach the phone as Allow and Deny actions
  (`apps/shared/app/core/dispatch.coax:32-34`). On the computer the answer
  becomes the Claude SDK's permission reply (`coder/tui/host-environment.coax:503-517`).

The computer side is the resident service, section 4.2.

**Why it matters for GameBuilder.** Route B needs only an outbound WebSocket.
That is exactly what GameBuilder's sandbox allows, on Mac and on iOS.

## 3. The Claude Agent SDK in the tree

**Where and what.** `coax/lib/agent/claude/`, written in Coax. It ports the
`claude` CLI's stream-json wire protocol, not the npm package, and tracks
`@anthropic-ai/claude-agent-sdk` 0.3.296, checked against Claude Code 2.1.312
(`coax/lib/agent/claude/PARITY.md:3-8`, `claude.coax:1-7`).

| File | What |
| --- | --- |
| `options.coax` | Options that become CLI flags and environment |
| `query.coax` | The query as a state machine over lines |
| `control.coax` | Control frames both ways |
| `mcp.coax` | In-process tool servers |
| `messages.coax`, `fields.coax` | Typed stream messages |
| `transport.coax` | The child process |

**How it launches Claude.** A subprocess, not HTTP.

- `cl_start` builds the arguments and environment and spawns the CLI
  (`transport.coax:36-57`). The effects are in C,
  `coax/platform/process-native.c`, using `fork` and `execve` (lines 297, 323).
- `claude` is found on `PATH`, then `~/.local/bin`, `~/.claude/local`,
  `/usr/local/bin`, `/opt/homebrew/bin` (`process-native.c:255-258`,
  `PARITY.md:113-115`).
- Fixed flags: `--output-format stream-json --verbose --input-format stream-json`
  (`options.coax:218-219`). The rest follow from options: `--model`, `--effort`,
  `--permission-prompt-tool=stdio`, `--resume`, `--continue`, `--allowedTools`,
  `--disallowedTools`, `--tools`, `--mcp-config`, `--setting-sources`,
  `--permission-mode`, `--include-partial-messages`, `--add-dir`,
  `--session-id`, `--no-session-persistence`, `--system-prompt`,
  `--append-system-prompt`, `--settings`, `--max-turns`, `--max-budget-usd`
  (`options.coax:220-262`).
- The child gets `BROWSER=true` and `NO_OPEN_BROWSER=1`, so it never opens a
  login page (`options.coax:268-272`). Callers remove `ANTHROPIC_API_KEY` so the
  user's own Claude Code login is used (`apps/everglade-desktop/next.coax:299`).
- The child runs in its own process group and is killed as a group
  (`transport.coax:126-138`).

**What it can do.**

- Sessions: persist or not, resume, continue, fork, fixed session id
  (`options.coax:101-105`).
- Streaming: partial messages (`options.coax:99`). `cl_next` blocks for one
  event; `cl_poll` is the same for a frame loop and returns `idle` when nothing
  has arrived (`transport.coax:74-101`).
- Tools: an allow list, a deny list, and the base tool set, which can be empty
  (`options.coax:86-93`).
- Permissions: six permission modes (`options.coax:60`). With
  `permission_handler` set, each tool use arrives as a `permission` event that
  the host answers with `cl_allow` or `cl_deny` (`PARITY.md:95`).
- In-process tools: up to 4 servers of 16 tools each (`options.coax:16-17`),
  added with `cl_add_server` and `cl_add_server_tool` (`options.coax:142-155`).
  A call arrives as a `tool_call` event; the host answers with `cl_tool_text`
  (`PARITY.md:97`). The model sees the tool as `mcp__<server>__<tool>`
  (`mcp.coax:5`). No separate MCP process is involved.
- Hooks: 33 event names, host-answered (`options.coax:28-34`).
- No heap, no callbacks: the host supplies the buffers and pulls events
  (`PARITY.md:23-44`).

**Not mapped yet:** subagent definitions, sandbox options, `--json-schema`
output, plugins, Windows (`PARITY.md:175-189`).

**The engine above it.** `coax/lib/agent/engine/claude.coax` keeps one
long-lived `claude` per conversation, sends each message as a user frame,
resumes with `--resume` after a restart, and folds the stream into a session
record of text, thinking, tool calls and results (lines 1-25). Entry points:
`e_attach` (585), `e_own` (593), `e_send` (679), `e_stop` (712), `e_answer`
(746), `e_pump` (839). By default it sets `permission_handler = true`
(line 564).

**Who uses it.**

- Everglade desktop's Next issue panel (`apps/everglade-desktop/next.coax:49-50`).
- The OpenAgents desktop recovery shell (`apps/openagents-desktop/zeron/modes.coax:1-22`).
- Coder, the terminal coding agent (`coder/tui/coder.coax:1-20`).
- The resident service (`coder/tui/host-environment.coax:223-225`).
- The hosted worker (`coax/web/agent/runner.c:1-40`).

**What is tested.**

- 55 `#test` blocks in `tests.coax` and `query_tests.coax`, covering message
  typing and the control protocol with scripted lines (`PARITY.md:134-146`).
- `smoke.sh`: a live run against the installed CLI. It checks the group kill,
  one plain turn, one turn that calls an in-process tool after a host-answered
  hook, and one turn whose `Bash` call the host denies (`PARITY.md:147-160`).
  The recorded pass is dated 2026-10-28. I did not run it.

## 4. Every app-to-agent integration under `openagents/apps`

### 4.1 `apps/everglade-desktop`: Next issue and Autopilot

- **Does:** a panel in a 3D desktop app. One Claude session picks a GitHub
  issue; a second one works it in a fresh git worktree
  (`apps/everglade-desktop/next.coax:1-18`).
- **Transport:** the Coax SDK in-process: the app itself spawns `claude` and
  polls it from the frame with no waiting (`next.coax:44, 239-249`).
- **The picker** runs with no settings files, an empty tool set and one
  in-process tool, `choose` (`next.coax:288-299`). The app answers the tool
  call (`next.coax:572`, replies at 431-463).
- **The coder** gets `Bash`, `Read`, `Edit`, `Write`, `Grep`, `Glob`, with
  subagents and web denied, and reports through an in-process `report` tool
  (`next.coax:346-359`).
- **Auth:** the user's Claude Code login; API key variables removed
  (`next.coax:299, 359`).
- **Host:** Swift spawns two helper binaries with `Process()` for context and
  worktrees (`apps/everglade-desktop/Sources/Workshop.swift:327, 363, 382`).
  The app is signed ad hoc with no entitlements file
  (`apps/everglade-desktop/build.sh:200`), so it is not sandboxed (inference
  from the absence of a sandbox key).
- **State:** working and the closest template for GameBuilder. `next-drive.coax`
  runs the flow headless. `desktop.coax:1-5` notes the mobile app has no agent;
  `apps/everglade-mobile/README.md:184-185` says why: the panel runs a `claude`
  process.

### 4.2 The resident service (`coder/tui/host*.coax`), used by the iOS and desktop apps

- **Does:** runs agents on a computer for paired clients. Verbs include
  `serve`, `pair`, `local-rpc`, `key` (`coder/tui/host.coax:906-934`).
- **Transport in:** the relay (section 2, route B), or a local Unix socket
  `~/.openagents/coder/host/service.sock`, mode 0600, with a peer user check
  (`docs/protocol/environment-runtime-v1alpha1.md:55-60`).
- **Transport out:** the Claude engine of section 3, one per slot, in the
  enrolled workspace, preferring `~/.local/bin/claude`
  (`coder/tui/host-environment.coax:219-226`). Four slots
  (`docs/protocol/environment-runtime-v1alpha1.md:78-101`).
- **Auth:** pairing grants; the user's Claude login on that computer.
- **Deployment:** a launchd service, `com.openagents.coax.environment`
  (`docs/protocol/environment-runtime-v1alpha1.md:148-156`).
- **State:** implemented as a "bootstrap profile". Physical-phone checks are
  still open. How the `openagents-environment` binary gets installed I could
  not find; that it is this code is an inference from matching verbs.

### 4.3 `apps/openagents-ios`

Section 2. Thin wrappers around `apps/shared/app/` (`apps/openagents-ios/app.coax:13-19`,
`AppCore.swift:22`). Tests: Coax checks for the core and the device lane, and
scripted cases with fakes under `apps/shared/app/cases/`.

### 4.4 `apps/openagents-desktop`

Three generations live side by side.

- **`zeron/`, the recovery shell.** Runs agents inside the app through the
  shared engine: Claude Code, Codex, Psionic and a lab engine
  (`zeron/modes.coax:1-22`, `zeron/screen.coax:231-258`). Subprocess, user's
  login.
- **`native/`, the shipping client.** Reaches "This computer" by running
  `~/.openagents/bin/openagents-environment local-rpc --request-file F`
  (`native/EnvironmentWindow.swift:170-179`). Not sandboxed: its entitlements
  have network client and server and no sandbox key (`native/entitlements.plist`).
- **`native/gpu/`, the compiled page from iOS on the Mac.** Does not replace
  the shipping client yet (`native/gpu/README.md:3-6`). `AppMacDevice` runs
  `openagents-environment key` and `local-rpc` through `Process`, with a 30 s
  cap and no fallback that owns a process itself
  (`apps/shared/native/AppMacDevice.swift:1-62`). A hidden
  `--inspect-resident-turn` flag qualifies one real Claude turn
  (`native/gpu/App.swift:28-37`).
- **Usage and Claude account connection** (`ed3736938b`):
  `apps/shared/app/usage/*`, `apps/shared/native/AppProvider.swift`. It reads
  usage and starts Claude Code's own login flow on the chosen computer.

### 4.5 `apps/openagents-cli`

- `coder.coax`: `openagents coder` execs Coder (lines 1-11).
- `chat.coax`: account chat over signed HTTP; never runs a model (lines 1-2).
- `agent.coax`: manages API accounts called agents; not a model (lines 1-7).
- `mcp.coax`: **an MCP server over stdio** that exposes the realm commands as
  tools, installed with `claude mcp add openagents -- openagents mcp` (lines
  1-15). Each call runs `openagents realm VERB --json`. This is the one place
  where Claude Code already edits worlds through typed tools. It edits worlds
  stored on openagents.com, not a local document.

### 4.6 `apps/metaverse-engine-desktop` (Aethyrforge)

- **Does:** a chat and voice panel in the desktop editor. A coding agent is
  given scene instructions and a command-line tool, and edits the scene by
  running the tool.
- **Transport:** the shared turn runner in-process, unattended; Claude runs
  with permissions bypassed and any approval that still asks is refused
  (`chat-launch.coax:1-37`). Engines: echo, claude, codex, microcoder, psionic,
  cursor, micro.
- **How the agent edits:** `scene-context.coax:2` tells it to run
  `"$AETHYRFORGE_SCENE_TOOL" show` and `apply 'JSON'`. The tool
  (`scene-cli.coax`) finds the running app through a session file and calls
  its loopback HTTP API at `http://127.0.0.1:PORT` (`scene-session.coax:1-12`).
- **Format:** its own JSON (`docs/protocol/aethyrforge-scene-v1.md`), not
  `world.edit.v1`. Place, move, remove and resize, at most 16 per command.
- **State:** working on Mac for the Everglade scene. The Linux port has no
  scene service (`linux-scene-context.txt:11-13`).

### 4.7 `apps/openagents-web`

- `chat-web.coax:993-1029`: in a world, the chat asks a hosted model for a
  typed plan (`world.plan.v1`) through `/api/chat/complete`, then the editor
  shell turns the plan into a `world.edit.v1` batch
  (`metaverse-engine/editor/shell.coax:901-958`). No subprocess; hosted models.
- `machine-web.coax:1-14`: pairs the browser with a computer's resident
  service over the relay.

### 4.8 Nothing agent-related

`apps/everglade-mobile`, `apps/verse-ui` (only `node_modules`),
`apps/openagents/apps` (an empty Xcode project shell). `apps/shared` holds the
shared pieces already described.

## 5. What exists for an agent changing a world or level

### 5.1 In OpenAgents

**Typed edits: `world.edit.v1`** (`metaverse-engine/editor/world-edit.coax`).

- One JSON object per edit: `spawn`, `despawn`, `set`, `move`, `rotate`,
  `scale`, `cell`, `terrain` (lines 1-38). Whole centimetres, degrees, percent.
- A batch carries a revision and is refused whole if the revision is stale
  (lines 16-19). At most 64 edits a batch (line 68).
- `we_read` parses one edit (line 195), `we_batch_read` a batch (line 220).
- `we_scene_apply` applies an entity edit as one undoable step and returns
  its inverse (lines 416-445).

**The world as a model reads it, and what it returns**
(`coax/lib/agent/world/turn.coax`).

- `world.turn.v1`: the world as compact text lines, written by `wt_write`
  (line 334). Up to 128 things in 24 KB (lines 64, 68).
- `world.plan.v1`: steps `place`, `move`, `turn`, `size`, `remove`, `undo`,
  `redo` and a few realm-only ones, at most 16 a plan (line 71).
- `wp_system` builds the system prompt (632), `wp_schema` the JSON schema
  (616), `wp_reply` validates the model's answer against the world it was
  shown (660-728).

**The saved document: `world.doc.v1`** (`metaverse-engine/editor/world-doc.coax`).
Line-based text with a digest; `world_save` (95) and `world_load` (202). It is
meant to be written by the engine, not by hand.

**The editor model: `editor.model.v1`**
(`metaverse-engine/editor/shell-model.coax`). What a page tells an editor
shell. The Apple page host exposes it read-only: `editorModel`,
`editorRevision`, `editorOwned` (`metaverse-engine/host/apple/page-host.h:100-103`).
I found no method there that pushes an edit in; that would go through
`message:text:` (line 104).

**The agent driver** (`metaverse-engine/agent/drive.coax`). This is for
*playing*, not authoring: it never changes the world (lines 1-18). Commands:
`tree`, `layout`, `status`, `quiet`, `target ID`, `taps`, `hit X Y`, `stop`,
`walk-to ID`, `face ID`, `mount ID`, `use ABILITY on ID` (lines 478-511). The
Apple host exposes it as `drive:` (`page-host.h:77-79`). Useful later for an
agent that play-tests a level.

**Command-line and MCP.** `apps/openagents-cli/commands.coax:740-763` lists
`world-place`, `world-move`, `world-turn`, `world-set`, `world-remove`,
`world-undo`, `world-redo`, `world-apply FILE.jsonl` and more, against
`/api/worlds/ID`. `openagents mcp` exposes the same table to Claude Code
(section 4.5).

### 5.2 In GameBuilder

**Decide** (`coax/decide.coax`, `host/Sources/Decide.swift`).

- One press sends one request to `https://openagents.com/api/v1/systemone`
  with model `auto` (`coax/decide.coax:21-22`), signed with the device's Nostr
  key under NIP-98 (lines 231-233). The key is in the Keychain
  (`host/Sources/Decide.swift:76-110`).
- The model writes no text. It answers six multiple-choice questions: which
  edit, which block, direction, distance, turn, size (`coax/decide.coax:184-219`).
- The scene is described to it as prose by `ed_state` (lines 132-151): each
  block's kind, id, position and size, the selection, the grid, and the last
  12 edits (`LOG_MAX`, line 26).
- `ed_carry_out` applies the answer: move, turn, size, add a block beside
  another, delete (lines 280-325).
- It is a remote decision model over HTTPS, not an agent and not Claude. It
  has five tests (lines 374-443).

**`her.coax` is not agent code.** It is the SPLICEBOTS character's movement
rules (`coax/her.coax:1-8`).

**The document.** A level is a `world.doc.v1` file (`coax/level.coax:1-23`),
one per level in the app's own Application Support folder
(`host/Sources/Levels.swift:10-17`). The core writes it with `ed_save`
(`coax/level.coax:118`) and reads it with `ed_open` (206). Undo history is not
saved (lines 18-19).

**Edit operations inside the core.**

- `ed_apply(e, WorldEdit)` moves, turns or sizes an existing block as one
  undoable step and logs it (`coax/editor.coax:617-628`). Not exported.
- `ed_spawn(e, kind, x, z, sx, sy, sz, yaw, scale)` adds a block
  (`coax/editor.coax:299`). Not exported.
- Both refuse when `ed_may_edit` is false: only a game's owners change it
  (`coax/games.coax:82`).
- A level holds at most 64 things (`ACTORS_MAX`, `coax/editor.coax:47`) of six
  kinds (lines 63-65).

**What the C header exports for editing.** Only what a person's hands do:
`ed_delete` (the selection, 758), `ed_undo`, `ed_redo` (780-781),
`ed_add_block` (787), `ed_set_tool` (747), and pointer and key events. There is
no export that says "move block 7 to here". An agent mode needs one.

**A play-time hook, debug only.** `RealmHost.drive` forwards a command to the
Everglade realm's agent driver (`host/Sources/Realm.swift:138-143`). Its only
caller is a debug check script (`host/Sources/ViewportView.swift:400-424`). It
drives the Everglade realm, not the level editor.

## 6. Gaps and a proposed minimal design

### 6.1 The constraint that decides the design: the sandbox

`host/GameBuilder-macOS.entitlements:5-8` grants the App Sandbox and outbound
network, nothing else. `project.yml:115-116` applies it with the hardened
runtime.

What that means for a local `claude`. None of this is read from the code; it is
my understanding of the macOS sandbox and should be tested with a ten-line
probe before any design is committed:

- A sandboxed app may `fork` and `exec`, but the child inherits the sandbox.
- The app sees a container as its home, so the SDK's lookups in `~/.local/bin`
  miss, and the real `claude` binary sits where the sandbox does not let the
  app read.
- Even if it started, `claude` inside the sandbox could not read the user's
  Claude Code login or write its own state under the real `~/.claude`.
- The resident service's Unix socket and its `openagents-environment` binary
  are outside the container too, so the route `AppMacDevice` uses is closed.
- Outbound TCP is allowed, including to `127.0.0.1`, and outbound WebSockets.
  Listening is not: that needs `network.server`.

Every OpenAgents app that spawns `claude` is unsandboxed (sections 4.1, 4.4).

### 6.2 Three ways round it

**A. Pair with the resident service over the relay (recommended first).**

- GameBuilder speaks route B of section 2 to the resident service on the same
  Mac. Outbound WebSocket only, so the entitlements stay as they are, and the
  same code serves iOS later.
- Reuse as-is: `apps/shared/app/device/seal.coax` and `lane.coax` (Coax), and
  `apps/shared/native/AppDeviceLink.swift` (Swift, compiled from the pin the
  way `project.yml:28-30` already compiles `AppAuth.swift`). GameBuilder
  already has a relay WebSocket and Nostr keys for the Lobby.
- Gap: the resident service runs Claude as a general coding agent in a
  workspace folder. It has no notion of GameBuilder's level tools. Claude would
  need a way to reach the level: either the service registers tools whose
  calls are relayed back to the app (new openagents work, close to milestone
  2A), or Claude edits a level file in the workspace and the app syncs it.
- Cost: a round trip through an internet relay to reach a process on the same
  machine, and polling at 1.5 s.

**B. A local helper outside the sandbox, reached over loopback.**

- A small unsandboxed command-line program, written in Coax, runs the SDK and
  `claude`, and listens on `127.0.0.1`. The app connects out to it.
- Reuse: the SDK and engine unchanged; the Aethyrforge session-file pattern in
  reverse.
- Gap: the helper must be installed and started outside the app. A sandboxed
  app cannot launch it. Loopback needs its own authentication, since any local
  process can connect.

**C. Claude Code drives the app: GameBuilder as an MCP server.**

- The user runs `claude` in a terminal with `claude mcp add gamebuilder -- gamebuilder-mcp`.
  The helper is a stdio MCP server, modelled on `apps/openagents-cli/mcp.coax`,
  and relays tool calls to the running app over loopback, with the app
  connecting out.
- No chat panel in the app at first: the terminal is the chat. Smallest amount
  of new interface, and the user's full Claude Code setup applies.
- Same install and loopback-auth gap as B.

**Not recommended: dropping the sandbox.** It would let the app use the SDK
directly, exactly as Everglade desktop does, and is the least code. But the Mac
App Store requires the sandbox, so it would fork the Mac build into a
developer-only variant. Worth it only as a throwaway prototype to prove the
edit tools while A, B or C is built.

### 6.3 What must be written whichever route is chosen

All of it is app logic, so it goes in Coax (`AGENTS.md`, "Logic is Coax").

1. **The level as text for the model.** A new export, for example
   `ed_agent_scene(e, out)`. Either reuse `ed_state`'s prose
   (`coax/decide.coax:132`) or write `world.turn.v1` with `wt_write`, which
   comes with a validator for the reply.
2. **Apply a batch of edits.** A new export, for example
   `ed_agent_apply(e, batch, out)`: parse with `we_batch_read`, apply each edit
   through `ed_apply`, `ed_spawn` and the delete path so undo, the edit log,
   `ed_may_edit`, grid snapping and the 64-thing limit all hold, and write back
   what was done or why it was refused.
   - Refuse a batch built against a stale revision, as the engine does. The
     person may have edited while the agent thought.
   - Make one agent turn one undo step, or say clearly that it is several.
3. **The tool definitions.** Names, descriptions and JSON schemas for the
   in-process server: read the level, place, move, turn, size, remove, undo.
   `cl_add_server_tool` takes them as strings (`options.coax:149`).
4. **The session and its panel state**: the transcript, the draft, running or
   waiting, Stop. `next.coax` shows the whole shape in one file.
5. **Tests.** A `#test` per rule (`AGENTS.md`, "Build and check"): a batch
   applies and undoes as one, a stale batch changes nothing, a non-owner's
   batch changes nothing, a full level refuses a spawn.

New Swift is limited to carrying bytes: a socket or WebSocket and a text
panel. Any new Swift or shell file needs a `Firm reason: N (...)` header
(`AGENTS.md:154-157`); reason 6, a platform SDK with no C API, fits the
socket, as it does in `host/Sources/Realm.swift:1`. A helper program for B or C
should be Coax, not Swift or shell.

### 6.4 Suggested order

1. Items 1, 2 and 5 of 6.3. They are needed by every route, are testable with
   no Claude at all, and would also let Decide return richer edits.
2. A throwaway unsandboxed debug build that wires the SDK in-process to those
   exports, to learn what tools and prompts make Claude good at level editing.
3. Then the shipping route: A if mobile matters soon, C if a terminal-first
   Mac workflow is acceptable.

### 6.5 Risks

- **Prompt injection through level content.** Level names and any text in a
  level are read by the model. With no built-in tools and only level-edit
  tools, the worst outcome is a bad level edit that undo takes back. Keep it
  that way: no `Bash`, no file tools, no web, no settings files, as the Next
  issue picker does (`next.coax:292-298`).
- **Whose login pays.** The SDK uses the Claude Code login on that Mac. A user
  without Claude Code installed gets an install message, not a fallback
  (`next.coax:86-87`).
- **Cost is not capped by default.** `max_budget_usd` and `max_turns` exist
  (`options.coax:80-82`); set them.
- **Buffers.** The Next panel keeps about 3 MB of fixed buffers per session
  (`next.coax:154-155`). Fine on a Mac; check before reusing on a phone.
- **Version drift.** The SDK tracks one CLI version; a Claude Code update can
  add message types. Unknown ones are passed over, not fatal (`PARITY.md:47-55`).
- **The pin.** Anything GameBuilder needs changed in the SDK or the resident
  service lands in openagents first, then `scripts/pin.sh bump`.

### 6.6 Open questions for the owner

1. Must the Mac build that has agent mode stay sandboxed (App Store), or may a
   direct-download build drop the sandbox? This picks the route.
2. Is the chat inside GameBuilder, or is driving the app from Claude Code in a
   terminal acceptable for a first version?
3. Is Claude's job only to edit the open level, or also to play-test it (the
   agent driver), write game rules, or make assets?
4. Should GameBuilder require the OpenAgents resident service to be installed,
   or carry its own helper?
5. One undo step per agent turn, or per edit?
6. Should Decide stay as it is beside agent mode, or become a mode of it?
7. Does agent mode edit games the user does not own by copying them first, as
   the editor does for a person?

## 7. What I could not determine, and how far to trust this

- **Read by me:** all of `coax/lib/agent/claude/options.coax`, `transport.coax`
  and `PARITY.md`; the head of `engine/claude.coax`; `next.coax` lines 1-420;
  all GameBuilder files cited; the commit list and stats.
- **From a delegated sweep, spot-checked only:** most line numbers in sections
  2, 4.2 to 4.7 and 5.1. I verified a sample (`AppMacDevice.swift`,
  `engine/claude.coax:564`, `host-environment.coax:219-226`, `build.sh:200`,
  `process-native.c`, the spec's header). Treat the rest as pointers to check
  before relying on an exact line.
- **Not determined:**
  - How `~/.openagents/bin/openagents-environment` is built and installed.
  - Whether the resident service is installed and running on the owner's Mac.
  - Whether the paired-computer route has passed on a physical phone.
  - The server side of `/api/worlds/ID`.
  - Whether GameBuilder's Mac build is distributed anywhere today:
    `scripts/release.sh:54` archives for iOS and I saw no Mac upload.
  - The sandbox behaviour in 6.1. It is reasoning, not a test.
- **Not run:** no tests, no smoke, no build.
