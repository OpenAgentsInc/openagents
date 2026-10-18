# Coder Desktop patches

Keep fork-specific changes here so upstream updates can review each integration.

- OpenAgents #12084: add an isolated Kotlin Views transcript renderer over
  Rust row placements and display lists, with shared fallback shaping,
  bounded row caches, and stable viewport anchors. Recorded synthetic Rust
  geometry backs stub-core tests; B1 integration, Kotlin execution, Android
  glyph checks, and emulator performance evidence remain queued.

- OpenAgents #12086: Android can supply Standard Play Integrity through a
  UniFFI callback on the existing key sign-in controller. Rust binds its
  digest to the challenge and identity, sends the token through the SDK,
  and keeps proof-of-work fallback. The Kotlin provider is reserved for
  B1 integration; it is not registered by an Android shell yet. The shared
  sign-in flow reserves event and challenge before making an account, so
  concurrent replays create no account as a side effect.

- OpenAgents #12083: the Android Views shell in `apps/android` uses the shared
  Rust mobile core, brand and version metadata, and both Android ABIs. The
  offline demo uses Rust's chat fixture with the bundled Geist fonts and
  `oa-tokens` colors; local commands live in `scripts/android/app.sh`. The app
  wires the shared push coordinator through its signed-in client provider and
  selects the Firebase or no-push source set from local Firebase configuration.
  Backup rules exclude encrypted account storage from cloud backup and device
  transfer. The app owns one encrypted storage adapter and exposes it to the
  Rust account and wallet integration. The `ComponentActivity` owns typed photo,
  document, camera, and permission results; QR scanning forwards bounded raw
  text through a Rust classifier callback. The checked-in UniFFI source still
  lacks A2/A3's `PushWakeKind`, `CoreClient.handlePushWake`, `AndroidSecretStorage`,
  `SecretRead`, `SecretMutation`, `CoreAccount.newWithSecureStorage`, and
  `androidWalletKeyStore` bindings, as well as the app-link classifier binding.
  The Rust account and wallet call sites remain pending binding generation.
  `scripts/android/build-core.sh` must regenerate the bindings and both ABI
  libraries before the app can compile.

- OpenAgents #12081: Android startup initializes the JVM context and platform
  certificate verifier before any live Rust client can start. Kotlin retries
  after failed setup, ignores repeated successful callbacks, and rejects calls
  that arrive before initialization.

- OpenAgents #12081: Android startup initializes the JVM context and platform
  certificate verifier before any live Rust client can start. Kotlin retries
  after failed setup, ignores repeated successful callbacks, and rejects calls
  that arrive before initialization.

- [OpenAgents #12075](https://github.com/OpenAgentsInc/openagents/issues/12075):
  Queued messages publish their ID after attachments and delivery settings, so
  concurrent drains read complete rows. Engine fixtures use SDK sign-in,
  retain WorkOS cases behind its compatibility feature, and check temporary
  image reads and lowercase repository identities.

- OpenAgents #12059: voice has a three-tool relay to the chosen saved agent,
  records spoken owner messages in that chat, and attaches the calling
  viewport's current chat through the active call lease.

- OpenAgents #12041: `list_runs` reads local terminal agent runs through
  `coder_lease::runs` and reports whether a claim's holder is active or its
  claim should be released. Every agent can use the tool and its stall window.

- OpenAgents #12036: `machine_status` includes owner steps and deploy permission
  facts through the public `coder_cloud::owner_status` API. Each agent can
  choose its repository and deployment; `NewEntries` tracks notifications by ID.
  Check-ins append these facts and notify the owner once per new item, using
  the saved agent definition and the existing owner-notification hook.

- OpenAgents #12071: agent builds disable incremental compilation and keep
  Cargo fingerprints under each manifest's path hash within a shared target.

- OpenAgents #12061: the SDK stores the account's voice agent by definition,
  chat, and host IDs. A shared Rust Native picker supplies desktop Settings
  and the phone form, reached from both call controls. Voice reads the choice
  again before each question.

- OpenAgents #12035: a generic session-keyed claim activity helper joins claim
  holders to working chats, reports chatless claims, and treats stalled work as
  unclaimed in coordinator prompts before the six-hour claim expiry.

- OpenAgents #12060: each voice call uses a fresh chat and isolated Codex home,
  copies authentication only, refuses messages after hang-up, and keeps
  compaction notices out of assistant text.

- [OpenAgents #12022](https://github.com/OpenAgentsInc/openagents/issues/12022):
  The root CLI reads account quota through `openagents-rpc`. Its sync
  dependency disables the optional SQLite store; app consumers still enable
  that store through their direct sync dependency. This lets both workspaces
  use the existing RPC without changing their SQLite versions.

- [OpenAgents #11836](https://github.com/OpenAgentsInc/openagents/issues/11836):
  Add Coder Light and Coder Noir to `openagents-theme`, make them the default light
  and dark selections, and use them when a selected theme is missing. Map the
  public `oa-tokens` palette snapshot in `crates/theme/src/builtins.rs`; no
  sibling checkout is needed. Preserve upstream themes and saved preferences.
  See [the theme system](theme-system.md) for the source revision.
- [OpenAgents D0 (Coder Desktop identity)](../FORK.md#identity-and-branding-seams-patch-log):
  Coder name and icon, `com.openagents.coder.desktop` bundle IDs, own data
  directory, IPC port and `coder-desktop://` scheme, and OpenAgents's update feed,
  sign-in and edge turned off. Values live in `crates/brand`; the full seam
  list and the platform matrix are in [FORK.md](../FORK.md).
- [OpenAgents #11871](https://github.com/OpenAgentsInc/openagents/issues/11871)
  and [#11883](https://github.com/OpenAgentsInc/openagents/issues/11883): Video
  Lab.
  - Our module `crates/mcp/src/video_lab.rs` offers the running Lab's `lab_*`
    tools to every harness and publishes each result's card through the
    `show_visual` path. Hooks: one `video_lab::prime()` in `crates/mcp/src/lib.rs`,
    the list and call arms in `crates/mcp/src/tools.rs`, and one paragraph in
    `crates/mcp/src/jsonrpc.rs` `INSTRUCTIONS`.
  - Our module `crates/ui/src/lab_card.rs` (with `lab_card/player.rs`,
    AVFoundation) draws a visual whose script starts with
    `-- coder:video_lab {card}` as a native card. The card follows the render
    and plays the video inline. Hooks: the `lab` field and the branch in
    `crates/ui/src/visuals.rs`, and `set_link_ui` taking `cx` in
    `crates/ui/src/transcript.rs`.
  - This card is an interim GPUI view. It moves to a Rust Native Lab card once
    the GPUI adapter exists (AGENTS.md: cards are Rust Native).
  - Saved chats' Lab cards (`/v1/history` `cards`) draw `lab-ui`'s Rust
    Native card through our module `crates/ui/src/lab_view.rs`: image and
    strip, compare, video (poster, and playback on macOS through the player
    above), Model3d and playable posters, and reports. Non-UI parts live in
    `openagents-account`: `lab_cards` (row references and the pictures each
    view draws) and `Reader::lab_card`, `lab_page` and `lab_media`, which read
    only a card's own files. `chat_entries::entries_with_lab_rows` turns each
    card into a `lab-card:` visual part, and the phone's layout draws it too
    (`crates/mobile/src/layout/lab.rs`). While a row exists it follows its
    document's `/v1/labs` events through `Reader::follow_lab`, and adds notes
    at the playing time through `Reader::lab_add_note`. Hooks: one early return to `render_lab_row` in
    `Transcript::render_visual` (`crates/ui/src/transcript.rs`); the row
    itself is in `transcript/account_history.rs`.

- [OpenAgents #11892](https://github.com/OpenAgentsInc/openagents/issues/11892):
  saved chats use the session transcript on desktop and phone. The shared
  `openagents-account::chat_entries` adapter supplies message parts; the existing
  account reader loads the latest page and older messages on upward scroll.
  Desktop attachment and paging hooks live in `transcript/account_history.rs`.
  `SavedTranscriptController` mounts the phone's Rust layout and normal cells;
  `MainTabController` treats it as a chat when hiding the global new-session bar.
  Opening a saved chat starts no harness; replies are added by #11893–#11894.

- [OpenAgents #11910](https://github.com/OpenAgentsInc/openagents/issues/11910):
  the iPhone Home list shows shared account projects without a connected
  computer. `SessionListController` mounts the Rust account reader's project
  rows; `SavedHistoryController` and `AppModel` open the existing website's
  invitations. The Rust mobile reader refreshes chats and projects together
  on pull-to-refresh and when returning to the app.

## Custom agents (#11903–#11908)

The SDK owns private definitions and assignments. `doc_host::custom_agents`
admits pinned assignments into the existing session command ledger. Claude and
Codex receive verified custom instructions through their native instruction
parameters. The SDK runner harness resolves names only through the operator's
`sdk-runners.json`; no definition supplies a command. `HarnessId::Sdk` is an
additive wire value; older app hosts cannot run it.

- [OpenAgents #12018](https://github.com/OpenAgentsInc/openagents/issues/12018):
  SDK runner turns receive the current runtime and model settings selected by
  the host, while instructions and access stay bound to the assigned identity.

`client_ffi::custom_agents` owns account isolation, saved retry requests, agent
selection, and the Rust Native form. UIKit's `AgentFormController` renders that
form and forwards versioned field and activation events. The new-session menu
adds personal agents beside existing models. Rust Native's optional syntax
feature keeps forms independent of the app's separate syntax-engine version.

- [OpenAgents #11966](https://github.com/OpenAgentsInc/openagents/issues/11966):
  a saved agent's chat shows the agent, and its runtime is a setting with an
  ordered fallback. `doc_host::custom_agents::runtime` chooses the first
  ready runtime each turn (installed, turned on, signed in by a file/key
  check that starts no login, model offered, access kept), then this
  computer's own. Our module `sessions/saved_agent.rs` forgets the old
  provider session and gives the fresh one the conversation. Hooks in
  `sessions.rs`: the module line, `dispatch_inner` taking the runtime
  `prepare_custom_agent` returns, and one term in `fork_history_prompt`'s
  condition. `openagents-proto::saved_agent` reads the chat marker; our
  module `crates/ui/src/openagents_saved_agent.rs` draws the OpenAgents mark
  and name, with one hook in `shell.rs` (`render_chat_row`'s mark) and one in
  `shell/tabs.rs` (the session header). `openagents-client` `SessionRow`
  carries `agent_name`; the phone's `SessionCell` and `BrandMarks` draw the
  mark. The agent form offers the runtime, OpenCode, "This computer's
  default", and the fallback order.

- [OpenAgents #11912](https://github.com/OpenAgentsInc/openagents/issues/11912):
  customizable Home dashboards. `openagents-account::dashboard` owns layout edits,
  summary reads, import/export, account saves, and the layout directory. GPUI
  `home` mounts its Rust Native cards. Hooks add the Home destination, account
  lifecycle callbacks, and one read-only session-report accessor; no separate
  chat path or engine starts. The phone's Home screen and its mobile FFI were
  removed in #12009; the phone opens on a chat.

- [OpenAgents #11922](https://github.com/OpenAgentsInc/openagents/issues/11922):
  iPhone logins use a stable Keychain profile across app-container changes.
  `openagents-account::PlatformStore::mobile` carries over existing credentials and
  keeps explicit sign-out effective; `CoreAccount` selects it on iOS.

- Account cloud composer (#11893): `openagents_composer.rs` uses the normal
  `ComposerInput` with the shared Rust `openagents-account::turns` controller.
  `openagents_reader.rs` mounts it. `SavedHistoryController.swift` mounts the
  normal `ComposerBar`; its small cloud-controls hook hides attachments and
  replaces local queue behavior with Stop. `client_ffi/saved.rs` exports Rust
  send/retry/stop state and actions. Neither surface starts a local harness.

- [OpenAgents #11923](https://github.com/OpenAgentsInc/openagents/issues/11923):
  Home uses the Coder Noir and Coder Light palettes, compact current-work rows,
  and a separate dashboard management screen. UIKit mounts shared actions in
  the navigation bar and keeps Sessions visible while scrolling.

- [OpenAgents #11924](https://github.com/OpenAgentsInc/openagents/issues/11924):
  custom-agent forms use the selected computer's cached and live model choices.
  Rust rejects late replies after switching computers or accounts and preserves
  saved model selections. The bundled Claude list includes Opus 5.5.

- [OpenAgents #11960](https://github.com/OpenAgentsInc/openagents/issues/11960):
  one chat list on the desktop. Our module `shell/account_chats.rs` merges the
  account's web, terminal and phone chats into Sessions by last activity, puts
  archived ones in Archived, applies the project filter, keeps one row per
  linked continuation, and loads the next page near the list end.
  `openagents_history.rs` no longer draws a section; it refreshes on focus and
  every minute through `openagents-account`'s `History::refresh_recent`. Hooks:
  the module line and the merge call in `render_chat_sidebar` (`shell.rs`), and
  the merged shelf in `render_archived_section` (`shell/spaces.rs`).
  `examples/sidebar-capture.rs` draws the sidebar offscreen.

- Our cloud engine build (#11852): `apps/openagents` makes `openagents-ui` an optional
  default feature, `ui`. `cargo zigbuild --release -p openagents --no-default-features
  --target x86_64-unknown-linux-gnu.2.36` builds the headless engine alone,
  linking only libc and libm, for the cloud computers that run Our cloud.

- Our cloud phone controls (#11852): the mobile Rust facade calls the shared
  SDK with the current account's cancellation token and rejects replies after
  sign-out. UIKit displays its status in Settings and adds Start Our cloud to
  the new-session computer menu. The live phone check uses a dedicated account.
  `scripts/ios/check-cloud-controls.py` checks status, start, stop, and sign-out
  against a loopback service with the simulator credential fixture enabled.

- [OpenAgents #11894](https://github.com/OpenAgentsInc/openagents/issues/11894):
  `openagents_continuation.rs` carries a saved-chat reference through the normal
  folder, harness, and composer flow. `doc_host::continuations` links the new
  session before execution; `doc::continuation` stores bounded context and feeds
  provider starts. Registry links hide the old part. Stable first-send IDs and
  metadata recovery preserve one destination across retries. The account fixture
  attaches an isolated engine through the existing RPC and app-state watchers.

- [OpenAgents #11958](https://github.com/OpenAgentsInc/openagents/issues/11958):
  the web-link preference writes the OpenAgents key and reads both earlier
  spellings. Pane fixtures enter Chat explicitly because startup opens Home.

- [OpenAgents #11898](https://github.com/OpenAgentsInc/openagents/issues/11898):
  `chat-home::view` defines **Runs on** and part links for GPUI and UIKit.
  `doc_host::transfer` saves the old part as read-only before calling the SDK;
  unfinished moves retry through the account loop. The destination uses the
  normal folder, harness, and command flow. `client_ffi::chat_homes` keeps
  account checks and transfers in Rust. Cloud computers use their registered
  device rows. Pending destinations stay out of lists until their source link
  is confirmed.

- [OpenAgents #11959](https://github.com/OpenAgentsInc/openagents/issues/11959):
  the chat's Runs on view is a small "Runs on This Mac" control in the
  composer footer. Its menu draws the shared `chat-home` view: the current
  home, the reason the chat can't move (only when one applies), the places it
  can move to, and links to its other parts. `openagents_continuation::runs_on`
  draws it for the composer and the saved-chat reader. The signed-in account
  is one sidebar row (`openagents_account.rs`) whose menu holds Agent Lab and
  Sign out. Hook: `crates/ui/src/composer.rs` calls `runs_on_control` in the
  session footer row instead of below the input. `examples/chrome-fixture.rs`
  captures both offscreen.

- [OpenAgents #11897](https://github.com/OpenAgentsInc/openagents/issues/11897):
  `doc_host::session_index` publishes this computer's top-level chats to the
  account's chat list on openagents.com through `openagents::sessions`, as
  summary rows only (title, folder, status, last activity). It runs in the
  signed-in account loop in `doc_host::custom_agents` and republishes every
  30 seconds. It uploads a read-only transcript copy only for the chats the
  service reports as opted in.

## Complete OpenAgents names (#11899)

Renamed crates, the desktop binary, iOS targets and UniFFI modules, packaging,
MCP instructions, and developer settings. Existing bundle IDs, Keychain
identities, storage locations, and the UIKit scene delegate stay stable. Old
environment names, saved theme choices, project imports, and conversation
links remain readable. Upstream fixes now require manual ports.

- [OpenAgents #11918](https://github.com/OpenAgentsInc/openagents/issues/11918):
  agent forms provide a Done button above the keyboard. The Agent Lab phone
  check dismisses the keyboard and scrolls within the form before choosing a
  runner, saving, running, and using an agent. Composer buttons keep their
  identity when model choices change, so updates do not close another menu.

- Shared harness launch policy (#11894): the native Claude Code, Codex, and
  OpenCode run paths accept `process::LaunchBroker`. The host applies its
  borrowed command wrapper before each launch, including OpenCode's MCP version
  probe. A refused or cancelled request cannot start an unwrapped process.
  `default-features = false` builds text drivers without the app voice transport
  and SQLite, so root workspace callers can link them.

- Shared harness supervision (#11894): the optional `supervised` feature keeps
  Coder's `supervise` process ownership around the native protocol drivers.
  `LaunchBroker::supervised` carries the host's bounds and output observer.
  Protocol I/O stops on missing output, and cleanup finishes before the host
  receives the process result.

- [OpenAgents #11878](https://github.com/OpenAgentsInc/openagents/issues/11878):
  Agent Lab refuses legacy Lab tool references before a tool call or model text.
  The shared workbench hides the add control and keeps saved references removable.
  Enabling reads requires a typed source-policy contract; local binding flags
  and arbitrary tool responses cannot authorize evaluation.
- [OpenAgents #11894](https://github.com/OpenAgentsInc/openagents/issues/11894):
  terminal callers pass prompt settings and command hooks through `host::NativeOptions`.
  The launch broker reports bounded provider accounting and process cleanup to the
  terminal host. Codex hook trust travels in thread configuration, including resume.
  The host also receives native exit codes, complete patch paths, and retry metadata.
  A host stop ends supervised processes and their child groups before acknowledgement;
  terminal resume recovery can select its own native history directory.
  Terminal sessions now call these native drivers through the borrowed host
  launch broker. The sync client's default-on `store` feature lets terminal
  callers reuse the Loro room client without linking a second SQLite version.

- [OpenAgents #11930](https://github.com/OpenAgentsInc/openagents/issues/11930):
  run keepers, so a running agent outlives the engine that started it. Our
  modules are `crates/harness/src/keeper/` (protocol version 1, the keeper
  process, the engine-side client, and `LineHarness`, the line-protocol driver
  that always runs under a keeper) and `crates/engine/src/sessions/keepers.rs`
  (boot pickup, the keeper scope for each run, and the transcript entry a
  picked-up run continues). Hooks:
  - `crates/harness/src/lib.rs`: `pub mod keeper` and the default
    `Harness::attach`, which returns `None`.
  - `crates/engine/src/sessions.rs`: the `keepers` module and `Inner` field;
    `drive_run` starts the harness through `keepers::start` and seeds the
    segment from `keepers::continued_segment`; `recover_stale` and `shutdown`
    skip chats a keeper holds.
  - `crates/engine/src/lib.rs`: `start_keepers` before `recover_stale`, and the
    `keeper` re-export.
  - `apps/openagents/src/main.rs`: the hidden `keeper` subcommand.
  Claude Code and Codex start under keepers by default; `OPENAGENTS_KEEPERS=0`
  (or `with_keepers(false)`) starts them directly. Our transport is
  `crates/harness/src/keeper/kept.rs`. Hooks:
  - `crates/harness/src/process/runtime.rs`: the `kept` field of
    `RuntimeChild`, its constructor, `events` and `keeper`, and one branch in
    `id`, `try_wait`, `wait`, `signal_target`, and `shutdown`.
  - `crates/harness/src/claude/mod.rs`: the `keepers` setting, `attach`, and in
    `run_with_mode` the keeper child, no opening lines for a picked-up run, and
    the event stream from `RuntimeChild::events`. `first_command` is optional.
  - `crates/harness/src/codex/mod.rs`: the same, plus the session loop's relay
    and `skip_ids_through` after the replayed handshake.
  - `crates/harness/src/jsonrpc.rs`: `RpcClient::skip_ids_through`.
  - `crates/harness/tests/fixtures/fake-claude.sh` and `fake-codex.sh`: the
    `scenario:kept` scenarios.
  The other native drivers don't start under keepers yet.
  #11951 added terminal keepers: `openagents keeper` can hold a shell's PTY
  (`crates/harness/src/keeper/pty.rs`, behind the default `pty` feature), and
  `crates/engine/src/terminals/kept.rs` opens terminals under one and picks
  them up on boot under the same id. Kept terminals don't hold back an update.
  Hooks:
  - `crates/engine/src/terminals.rs`: the `kept` field and `open_kept` call,
    resize and close through the keeper, shutdown and drop leave kept shells
    running, `any_open` counts only terminals without a keeper (`any_live`
    counts all), and live subscribers skip events at or before their `afterSeq`.
  - `crates/engine/src/lib.rs`: `terminals.start_keepers` after `Terminals::new`.
  - `crates/engine/src/sessions.rs`: re-exports `keepers::keeper_launch`.
  - `crates/engine/src/handoff.rs`: `quiescent_check` uses `any_live` when the
    service doesn't keep keepers alive across a restart.

- [OpenAgents #11932](https://github.com/OpenAgentsInc/openagents/issues/11932):
  move a chat to a cloud computer at a turn boundary. Our module is
  `crates/engine/src/sessions/move_cloud.rs`: a durable record per chat, the
  hidden-ref capture (`refs/openagents/move/<chat>`, pushed or bundled), the
  session file upload in pieces of 4 MiB or less, the same-path or seeded
  restore, and the resume with the login by credential reference. Hooks:
  - `crates/engine/src/sessions.rs`: `pub mod move_cloud`.
  - `crates/engine/Cargo.toml`: the `openagents-sdk` dependency, and `oa-copy`
    for tests.

- [OpenAgents #11931](https://github.com/OpenAgentsInc/openagents/issues/11931):
  the background engine by default. On launch the app starts this data
  folder's service (a LaunchAgent on macOS, a systemd user unit on Linux, each
  named after the data folder) and attaches over IPC instead of embedding an
  engine. Source builds and translocated copies still embed;
  `OPENAGENTS_ENGINE_MODE=background|embedded` or the data folder's
  `engine-mode` file overrides. Our crate is `crates/background` (service
  files, the attach-or-start decision, the login check before a handoff, and
  the hidden `openagents background ensure|status|stop` command); our module
  is `crates/ui/src/background.rs` (the launch step, and the macOS menu-bar
  item with Open, Move to background, and Stop all and quit). Hooks:
  - `crates/ui/src/state.rs`: `EngineHandle::bootstrap` calls
    `background::ensure_engine` before it embeds.
  - `crates/ui/src/lib.rs`: the `background` module and `background::init`.
  - `apps/openagents/src/main.rs`: the hidden `background` subcommand.
  - `crates/account/src/desktop.rs`: `has_saved_sign_in`.
  - `crates/brand/src/lib.rs`: the service label and unit prefixes.
  #11933 added Move all to a cloud computer to the menu-bar item.

- [OpenAgents #11933](https://github.com/OpenAgentsInc/openagents/issues/11933):
  move plans, Move all, Bring back, and agent tools. Our modules are
  `crates/engine/src/sessions/move_cloud/plan.rs` (the durable plan, its
  states, and Bring back), `crates/engine/src/rpc/moves.rs` (the `PlanMove`
  family of methods, background runs, and the hooks into the engine's chats),
  `crates/rpc/src/moves.rs` (the shared method names and views), and
  `crates/ui/src/shell/move_sheet.rs` (the Move sheet, its progress, and
  approval of agent requests). Cloud logins are read by reference from
  `<data>/moves/logins.json`. Hooks:
  - `crates/engine/src/rpc.rs`: the `moves` module and its dispatch.
  - `crates/engine/src/doc_host/transfer.rs`: `move_seed`, `move_lease`,
    `part_revision`.
  - `crates/rpc/src/lib.rs`: `pub mod moves`.
  - `crates/mcp/src/tools.rs`: `plan_move`, `start_move`, `move_status`,
    `bring_back`.
  - `crates/ui/src/shell.rs`: the `move_sheet` module and `moves` field, its
    overlay and keyboard ownership, and the chat menu's Move to a cloud
    computer row.
  - `crates/ui/src/background.rs`: Move all to a cloud computer.

- [OpenAgents #11946](https://github.com/OpenAgentsInc/openagents/issues/11946):
  the engine update handoff. An installed engine service restarts into a newer
  binary while runs under keepers continue; runs without a keeper, terminals,
  and voice still hold the restart. Our module is
  `crates/engine/src/handoff.rs` (the updater's quiet check, and the background
  engine's restart, which exits with code 75 so its service starts the new
  binary); the drain and its admission are in
  `crates/engine/src/sessions/keepers.rs`. Hooks:
  - `crates/engine/src/lib.rs`: the `handoff` module, the quiet check passed to
    `Updater::spawn`, and in `Engine::run` the `handoff::superseded` branch and
    its exit.
  - `crates/engine/src/sessions.rs`: `keepers::admit` at the top of
    `dispatch_inner`, and the `HANDOFF_*` re-exports.
  - `crates/harness/src/keeper/client.rs`: a welcome outside
    `MIN_PROTOCOL..=PROTOCOL` is refused with nothing else sent.

- [OpenAgents #11911](https://github.com/OpenAgentsInc/openagents/issues/11911):
  project-sharing controls use `coder-sharing` for account-bound device keys,
  owner-approved invitations, shared files, and commands through NIP-HOST.
  `client_ffi::sharing` binds callbacks and polling to the current account lease.
  Settings opens the shared Rust Native form in `AgentFormController`; UIKit
  forwards versioned actions and copies codes only after an explicit action.

- [OpenAgents #11940](https://github.com/OpenAgentsInc/openagents/issues/11940):
  Saved agents accept an enabled OpenCode runtime and its advertised model.
  Admission and assignment checks share the runtime/access rule. OpenCode keeps
  the host's permission settings; read-only and workspace-write agent settings
  are refused. OpenCode receives instructions through a per-run inline native
  agent (`prompt` on v1, `system` under `agents` on v2), selected explicitly on
  the session or prompt. Shared configuration files stay unchanged. The existing
  harness owns launch and event handling.

- [OpenAgents #11854](https://github.com/OpenAgentsInc/openagents/issues/11854):
  the optional app device connection embeds the existing Coder host and connector,
  with account-scoped host keys, explicit session-only phone pairing, admitted
  unary RPCs, original command IDs, and exact-request NIP-98 proofs for sync.
  Transcripts continue through the existing Loro rooms. The default remains
  WebSocket; refusals and uncertain commands do not retry on another transport.

- [OpenAgents #11886](https://github.com/OpenAgentsInc/openagents/issues/11886):
  `account::video_editor` previews the shared Video Lab intents, saves against
  the loaded version, and follows preview jobs through the Labs SDK.
  `ui::video_timeline` draws native tracks, drag previews, caption controls,
  links, speed, and dissolves. Saved cut cards open it through an additive
  `LabView` hook. Account changes and access refusals clear the editor.
  Scrubbing reuses the card's loaded frames; playback uses its hosted player.

- Coder Noir sidebar (#11964): uses `#050505`, darker than the main area's
  `#0a0a0a` background.

- [OpenAgents #11962](https://github.com/OpenAgentsInc/openagents/issues/11962):
  removed upstream's Pull requests screen. Gone: `crates/ui/src/pull_requests.rs`
  (the board, its filters and stars), `crates/ui/src/shell/pull_request_pane.rs`
  (its tabbed side pane), `crates/ui/examples/pull-request-fixture.rs`,
  `Route::PullRequests`, `NavEntry::PullRequests`/`PullRequest`, the
  `OPENAGENTS_OPEN_ROUTE=pull-requests` knob, and the sidebar footer button.
  A pull request link inside a chat still opens beside it
  (`shell/chat_pull_requests.rs`); outside a chat it opens in the browser. The
  saved `pull_request_stars` and `pull_request_pane_width` settings stay
  readable and unused.

- [OpenAgents #11961](https://github.com/OpenAgentsInc/openagents/issues/11961):
  removed upstream's side chats. Gone: `crates/ui/src/shell/side_chats.rs`,
  `RightSurface::SideChat`, the session card's "New side chat", the file
  explorer's **Chats** footer section and its "+"/fork buttons, side-chat state
  in `AppState`, `Composer` and `Pickers` (`side_chat_state`, unsaved side chats),
  the explorer rename surface, and `ForkSideChat`. MCP `create_chat` and
  `create_chats` always make standalone chats (no `kind`/`parent` in their
  schemas; the result reports `createdFrom`), and the engine's `createChat`
  ignores `parentChatId`. `Chat::is_top_level` no longer checks
  `parent_chat_id`, so side chats saved by older builds list and open as
  ordinary chats; the stored field is kept.
- [OpenAgents #11963](https://github.com/OpenAgentsInc/openagents/issues/11963):
  Fork chat makes a full standalone fork. Our modules are
  `crates/engine/src/chat_fork.rs` (the `ForkChat` RPC: copied history, the
  "Forked from" seam, the native-fork source), `crates/doc/src/native_fork.rs`,
  and `crates/ui/src/shell/fork_chat.rs` (the card action, opening the fork,
  and the seam's link back). Hooks:
  - `crates/engine/src/rpc.rs`: the `FORK_CHAT` arm and its `forwardable` entry.
  - `crates/engine/src/sessions.rs`: in `dispatch_inner`, strip
    `FORK_SESSION_OPTION` and inject the native fork; in `drive_run`, skip the
    history bootstrap for a native fork and clear it on `SessionStarted`;
    `fork_history_prompt` keys on the transcript's fork seam instead of
    `parent_chat_id`.
  - `crates/proto/src/agent.rs`: `FORK_SESSION_OPTION`.
  - `crates/harness/src/claude/mod.rs`: `--fork-session` beside `--resume`.
  - `crates/harness/src/codex/mod.rs`: `thread/fork` instead of
    `thread/resume`; a failure is returned so the host retries with seeded
    history.
  - `crates/ui/src/transcript.rs`: `fork_marker` takes the source id and
    dispatches `OpenForkSource`; `crates/ui/src/state.rs`: `add_pending_chat`
    (the former pending side-chat row); `crates/ui/src/shell/session_info.rs`:
    the Fork row.

- [OpenAgents #11969](https://github.com/OpenAgentsInc/openagents/issues/11969):
  blocking MCP calls wait at most 120 seconds and end early when the calling
  chat gets a new message. Our module `crates/mcp/src/inbox.rs` watches that
  chat's transcript and queue; hooks are the wait race and the cap in
  `crates/mcp/src/tools.rs`, the `Interrupted` outcome in
  `crates/mcp/src/openagents.rs`, and one sentence in `jsonrpc.rs`
  `INSTRUCTIONS`. `crates/doc/src/wait_target.rs` keeps the chat a waiting
  tool names (hook: the `Mcp` arm of `sanitize_tool_call` in `parts.rs`), and
  `crates/ui/src/tool_progress.rs` turns the running tool into the working
  row's words and timer (hook: `render_working_trailer` in
  `crates/ui/src/transcript.rs`). `examples/tool-progress-capture.rs` draws
  it headless.

- [OpenAgents #11965](https://github.com/OpenAgentsInc/openagents/issues/11965):
  Foreman check-ins. `engine::foreman` and `doc_host/foreman.rs` run from the
  custom-agent host loop. They mirror one account schedule through the SDK's
  `schedules` module and queue each check-in into Foreman's chat as an
  ordinary message. They also coalesce chat, issue and stale-claim events,
  enforce the standing authority on the app tools' `QUEUE_COMMAND` and
  `QUEUE_MESSAGE` (which now carry `originChatId`), and append
  `foreman/decisions.jsonl`. MCP adds `record_decision`. Settings → General
  shows a Foreman card.

- [OpenAgents #11971](https://github.com/OpenAgentsInc/openagents/issues/11971):
  agent chats build through a gate. `main` calls
  `openagents_engine::build_gate::enable()` once at start, which writes the
  `cargo` shim from `coder_lease::agent_builds` (root `crates/coder-lease`)
  into `~/.openagents/bin/agent-build-shims`. Our module
  `crates/harness/src/build_gate.rs` then gives every harness child that shim
  first on `PATH`, the shared target folder root (`~/work/agent-targets`) and
  the `openagents` command that takes the build lease. Hook: the last lines of
  `compose_path` in `crates/harness/src/lib.rs`. Heavy `cargo` commands build
  in one target folder per workspace, refuse below 30 GiB free, and wait for
  one of the coordinator's build slots (5 by default). Our module
  `crates/mcp/src/build_status.rs` is the `build_status` tool that reports
  and sets them; hooks are one list entry and one call arm in
  `crates/mcp/src/tools.rs` and one sentence in `crates/mcp/src/jsonrpc.rs`
  `INSTRUCTIONS`. `OPENAGENTS_AGENT_BUILD_GATE=0` turns it off;
  `docs/coder/runtime/leases.md` in the root has the details.
- #12033 machine actions: our module `crates/mcp/src/machine_action.rs` is
  the `machine_action` tool, which prunes a build cache or stops a process
  only under the owner's grant to the calling chat's ID and logs every
  action and refusal (`coder_lease::machine`). Hooks are one list entry and
  one call arm in `crates/mcp/src/tools.rs` and one sentence in
  `crates/mcp/src/jsonrpc.rs` `INSTRUCTIONS`.

## Phone wallet (#11999)

- The Spark wallet is back on the phone. `crates/mobile/src/wallet/` links
  the root `crates/spark-wallet` (Breez SDK, mainnet) and turns on Breez's
  SQLite store here only; the workspace patches `boltz-client` from the
  root's `vendor/` as the root workspace does. `wallet::seed` reads the
  earlier app's Keychain item (`com.openagents.app.spark` / `seed-v1`)
  through the `KeyStore` foreign trait, which can only read and add, and
  `wallet_seed_words` gives the identity the same words. The screen is a
  Rust Native view; the iOS shell is `apps/ios/OpenAgents/Wallet/`
  (Keychain glue, the page, Face ID for the words). Hooks: `showWallet` on
  `AppRouter`, a drawer toolbar button, a Settings row, the `-route wallet`
  launch argument, and `WalletModel.shared.open()` at the start of
  `AppModel.init` and in `willEnterForeground`.
- Delete account (#12002). Our code: `Controller::delete_account` and the
  deletion strings in `crates/account/src/lib.rs`, the
  `crates/mobile/src/client_ffi/account_delete.rs` bindings, and the iOS
  Settings row in `apps/ios/OpenAgents/Shell/MoreViewController.swift`.
  Hook: the Delete account row and its steps in the desktop account menu,
  `crates/ui/src/openagents_account.rs`.

- Wallet presentation (#12013). Our Rust wallet now separates the balance,
  receive methods, send entry, payment review, activity, and wallet information
  into focused Rust Native views. The iOS adapter uses the Bitcoin Design and
  WalletUI hierarchy, typography, and button patterns described in
  `docs/design/` at the repository root. Wallet operations, seed storage, and
  backup authentication are unchanged.

- Voice selection display (#12014). The iOS Talk with Codex voice row shows
  the saved voice name, or Codex default, beside Voice. Its existing next-call
  explanation and call behavior stay the same.

- Active voice host names (#12017). The iOS call header reads the latest
  name for the device that started the call and refreshes on device updates
  and screen return. Missing metadata keeps the last available name; choosing
  another host for a later call does not change the active call’s label.

- OpenAgents Cloud wording (#12015): Settings and the new-chat computer menu
  use **Start OpenAgents Cloud** and **Stop OpenAgents Cloud**, with matching
  status, help, and error text. The cloud device's display name follows the
  same wording; its service name and cloud-host API stay unchanged.
## Connect your phone (#11998)

- The device connection (NIP-HOST host) runs by default with an iroh
  endpoint, signed in or not; `OPENAGENTS_NIP_HOST=0` turns it off. A start
  or device-key registration failure is logged and the app keeps running.
  Hooks: `assemble_runtime` in `crates/engine/src/lib.rs` and
  `host_rpc::configuration`.
- Our module `crates/engine/src/connect_phone.rs` serves `CreateConnectCode`
  and `CancelConnectCodes` (`crates/rpc/src/connect_phone.rs`) only on the
  in-process and localhost transports, through
  `EngineRuntime::local_rpc_service`. Hooks: the headless IPC service in
  `Engine::run` and the embedded service in `crates/ui/src/state.rs`.
- Our module `crates/ui/src/shell/connect_phone.rs` is the sheet. Hooks: the
  `ConnectPhone` action, field, overlay, keyboard check, and window
  deactivation pause in `crates/ui/src/shell.rs`; a menu row and a signed-out
  button in `crates/ui/src/openagents_account.rs`; the Phone row in
  `crates/ui/src/settings/devices.rs`.

## Desktop identity (#11997)

- Our module `crates/account/src/desktop/identity.rs` holds the desktop's
  side of the identity: the `DesktopIdentity` trait the shared identity
  (#11995) answers, `KeyIdentity` (that trait over `Identities` and the
  controller's key sign-in), the first-launch welcome rules, and their
  words. A fresh computer makes no key and signs in with none until the
  person picks Start here or Restore from 12 words.
  Connect your phone makes the key without signing in and offers it in
  `desktop::identity::link` (a waiting marker on disk keeps a relaunch from
  signing in with it); the key signs in once the device connection reports
  the phone's link proof signed (`link::signed`).
- Our module `crates/ui/src/openagents_account/identity.rs` draws the
  welcome (Connect your phone, Start here, Restore from 12 words), the
  identity row's menu, and Back up your account. Hooks: the `id` field,
  `attach_identity` in `init`, `identity_changed` in the snapshot watch, and
  the identity branches in `signed_in_row` and `render` in
  `crates/ui/src/openagents_account.rs`, which no longer has the browser
  sign-in (its `sign_in` opens the welcome); the overlay and keyboard check in
  `crates/ui/src/shell.rs`. `examples/identity-capture.rs` draws it offscreen.

## Connect a computer by scanning its code (#12000)

- Our modules `crates/sync/src/account_link.rs` (the account-link proof,
  phone and computer halves) and `crates/client/src/pairing.rs` (pair with no
  registry entry, keep the grant in a `PairingStore`, reconnect on launch,
  link the computer to the account). `crates/sync/src/paired_device.rs` gains
  `pair_scanned`, `open`, and the Keychain store; `pair` keeps its old
  behavior.
- Hooks: one `pairings` field on `ClientInner` and one `forget_saved_pairing`
  call in `forget_device_channel` (`crates/client/src/client.rs`); paired
  computers merged into the registry's devices in
  `crates/client/src/workspace/mod.rs`; one `LinkAccount` branch and
  `Hosted::set_account_signer` in `crates/engine/src/host_rpc.rs`.
- Phone: `Shell/CodeScannerViewController.swift` (AVFoundation scanner and
  `CodeAction`), a Settings row, the tutorial's connect page, and the
  universal-link handler in `App/SceneDelegate.swift`. The scanner also takes
  website sign-in codes (#12001) through `client_ffi/web_login.rs`.
- [OpenAgents #12010](https://github.com/OpenAgentsInc/openagents/issues/12010):
  the Design Lab desktop tour. Our module `crates/ui/src/shell/design_lab_tour.rs`
  (behind `account-fixture`) opens the whole shell offscreen with fixture data
  and saves every screen in light and dark mode;
  `examples/design-lab-tour.rs` and `scripts/design-lab-capture-desktop.sh`
  run it. Hook: one `pub mod design_lab_tour` in `crates/ui/src/shell.rs`.
- [OpenAgents #12019](https://github.com/OpenAgentsInc/openagents/issues/12019):
  The phone's saved-account transcript uses the existing turn activity state
  for its Working row, including before answer text arrives. The saved
  message adapter, Lab cards, Stop, and computer sessions keep their paths.
- [OpenAgents #12020](https://github.com/OpenAgentsInc/openagents/issues/12020):
  The new-session model picker checks its draft against the selected computer's
  live catalog in Rust. A supported choice stays selected; failed discovery,
  a different host, later edits, and custom-agent bindings keep their choices.
  The iOS hook is limited to `NewSessionViewController.loadModels`.
  With a project picked, **Run on** also lists OpenAgents Cloud (or Start
  OpenAgents Cloud): it has no checkout, so the only cloud choice there was
  the account chat, which has no model chip or computer progress. Picking it
  starts a project-less session on that computer
  (`HostOption.cloudBesides`); the account choice shows the OpenAgents mark.
- [OpenAgents #12016](https://github.com/OpenAgentsInc/openagents/issues/12016):
  the iOS Talk with Codex list reads the execution host's Codex account
  through the existing `ListAgentAccounts` RPC and its cache. Our module
  `crates/mobile/src/client_ffi/voice_usage.rs` owns host selection, refreshes,
  and usage state; hooks are `Client::agent_accounts`, `VoiceCall`'s host id,
  and the usage section in `VoiceViewController.swift`.
- [OpenAgents #12056](https://github.com/OpenAgentsInc/openagents/issues/12056):
  upgrading the development app from `main` with no owner steps. Our crate
  `crates/host-upgrade` holds the upgrader (fetch, build in the shared
  target, check, hand over), the quiet-moment wait, the bundle swap, and the
  log; `apps/openagents/src/host_upgrade_cli.rs` is `openagents
  host-upgrade`; `crates/mcp/src/upgrade_host.rs` is the `upgrade_host`
  tool; `crates/ui/src/host_upgrade.rs` is the app's watcher, notice,
  relaunch, and reopened line. Hooks: one `note_input` call in
  `ComposerInput::record_edit` (`crates/ui/src/composer.rs`),
  `Shell::voice_live` and `Shell::has_unsaved_files`
  (`crates/ui/src/shell.rs`), one `host_upgrade::init` in `run_app`
  (`crates/ui/src/lib.rs`), `DocHost::resume_queues_after_restart` and its
  check where a chat handle opens (`crates/engine/src/doc_host.rs`, called
  from `crates/engine/src/lib.rs`), and `OPENAGENTS_DEV_APP_OUT` plus the
  commit stamp in `scripts/run-macos-dev.sh`.
- [OpenAgents #12039](https://github.com/OpenAgentsInc/openagents/issues/12039):
  Add the read-only `machine_status` app tool for any agent ID, using shared
  machine, capacity, landing, and pool readers. Expose
  `machine_status::collect`, `Status::prompt_block`, and resource alerts for
  check-in hosts.

- [OpenAgents #12040](https://github.com/OpenAgentsInc/openagents/issues/12040):
  saved agents can notify their owner through the public SDK and the existing
  push gateway. The engine checks the calling assignment; the mobile core
  fetches private owner notes, and the iOS shell shows the note and opens its
  chat, including on a cold launch.
- 2026-10-18 (#12062): Add `work_issues` through the public SDK Work/Pool
  connection and the shared `coder-cloud` host service. Use saved agent IDs
  for budget checks and preserve owner commands in refusals.
- [OpenAgents #12068](https://github.com/OpenAgentsInc/openagents/issues/12068):
  desktop account messages describe connecting, retrying, and chats that belong
  to a different account instead of asking people to sign in. The guard in
  `crates/account/tests/desktop_copy.rs` checks account, background, client,
  engine, sync, UI, and SDK strings, including startup pickup errors. Agent-provider
  logins and the optional upstream WorkOS flow keep their own instructions.
- [OpenAgents #12054](https://github.com/OpenAgentsInc/openagents/issues/12054):
  Route public run commands through NIP-HOST app RPC and the existing session
  command ledger. Our `engine/src/doc_host/run_commands.rs` owns execution;
  `engine/src/rpc/run_commands.rs` maps the RPC body. `host_rpc` checks account,
  native rights, and definition-based command grants; the device-channel adapter
  preserves the request ID when delivery is uncertain.
- [OpenAgents #12088](https://github.com/OpenAgentsInc/openagents/issues/12088):
  add the shared Android semantic widget adapter in
  `apps/openagents/apps/android/platform/SemanticViewAdapter.kt`. Rust's `coder-ui::android`
  bridge validates callback identity and distinguishes prepared and displayed
  revisions. Cards and domain actions remain Rust views. The app shell must
  connect the bridge and provide transcript, composer, Markdown, and local
  surface adapters. This baseline has no Gradle app shell; Android compilation,
  device input, TalkBack, deployment, and store checks have not run.
- [OpenAgents #12082](https://github.com/OpenAgentsInc/openagents/issues/12082):
  Android stores opaque encrypted values through Android Keystore and an
  atomic private-file adapter. Rust owns the account, identity, wallet-seed,
  and paired-grant records and decides when they may be created or replaced;
  cloud backup and device transfer exclude the encrypted store.
- [OpenAgents #12093](https://github.com/OpenAgentsInc/openagents/issues/12093):
  Android push delivery uses the existing Rust push registration and private
  owner-note APIs. `CoreClient::handle_push_wake` receives a typed wake; Rust
  returns the newest checked owner note for attention and redials sync for a
  reconnect without changing app lifecycle state. The shared platform
  coordinator validates data-only wake messages, and the `push` source set
  contains the Firebase transport adapter; `nopush` has no Firebase symbols.
  The app owns client creation, foreground suppression, notification routing,
  and tap handling. Android channel setup and permission checks live in
  `platform/AndroidPushNotifications.kt`.
- [OpenAgents #12082](https://github.com/OpenAgentsInc/openagents/issues/12082):
  Android stores opaque encrypted values through Android Keystore and an
  atomic private-file adapter. Rust owns the account, identity, wallet-seed,
  and paired-grant records and decides when they may be created or replaced;
  cloud backup and device transfer exclude the encrypted store.

- [OpenAgents #12096](https://github.com/OpenAgentsInc/openagents/issues/12096):
  classify only bounded HTTPS links on `openagents.com` for computer pairing
  and project invitations in the Rust mobile FFI. The existing connect-code
  parser checks shape and expiry; the server still authorizes invitation use.
  Synthetic fixtures cover the connect fragment, project path, and rejected
  URL forms. `AndroidScannerPresentationBoundary.kt` adds bounded raw-text
  handoff, permission denial/cancel/retry states, and duplicate/late callback
  fencing. `CameraXQrScannerDriver.kt` uses `ImageAnalysis` with
  `KEEP_ONLY_LATEST`, a 1280×720 target, and QR-only ML Kit decoding; decoded
  raw text goes to the Rust classifier callback. Synthetic fake analyzer and
  decoder cases cover duplicates and late results. It does not perform link
  approval. B1 owns the Gradle and manifest changes. The official CameraX
  release page lists 1.6.2 as stable, and the official ML Kit page documents
  bundled `com.google.mlkit:barcode-scanning:17.3.0` ([CameraX](https://developer.android.com/jetpack/androidx/releases/camera),
  [ML Kit](https://developers.google.com/ml-kit/vision/barcode-scanning/android)).
  Both production files now live in the app source set under
  `apps/android/app/src/main/java/com/openagents/app/scanner/`, with synthetic
  instrumentation coverage under `apps/android/app/src/androidTest/java/com/openagents/app/scanner/`.
  B1 owns `MainActivity.startQrScanner(previewView, classifyWithRust, onFailure)`,
  permission-result handling, the Activity shell wiring, and pins CameraX 1.6.2
  with bundled ML Kit 17.3.0. The app manifest requests camera permission.
  Scanning sends bounded raw text only to the Rust classifier callback; Kotlin
  does not classify or authorize links. Neither dependency has been resolved in
  an Android build.
  `scripts/android/app.sh compile` runs the debug and release Kotlin compile,
  compiles scanner instrumentation sources, and runs debug JVM tests; it remains
  unrun below the 100 GiB build floor. The scanner instrumentation test still
  needs a device run through `:app:connectedDebugAndroidTest`. The release check
  (`scripts/android/app.sh check`) and API 26 install/run check remain pending. The Rust classifier
  fixture check (`cargo test --locked -p openagents-mobile app_links`) and
  `scripts/android/build-core.sh` also remain unrun; the latter generates
  bindings and builds arm64 then x86_64. #12082 and #12091 remain native blockers.
  Release-certificate
  verification is an owner-only After-close check in `NEEDS_OWNER.md`.

- [OpenAgents #12090](https://github.com/OpenAgentsInc/openagents/issues/12090):
  The Android composer presentation keeps draft text, selection, and IME marked
  ranges in sync through a narrow Rust editor port. Context chips, attachment
  labels, mention suggestions, and composer actions are native Views; question,
  approval, and queued-message cards mount through `SemanticViewAdapter`.
  The composer and transcript share a weighted chat layout, so Rust receives
  the transcript's new viewport size when the keyboard changes available space.
  The app build includes transcript main, unit-test, and instrumentation source
  roots, and routes Android platform tests to instrumentation tests. It pins
  ZXing core 3.5.4 for A3's scanner replacement while retaining CameraX 1.5.3.
  A bounded fixture core covers send choices and attachment intake outcomes.
  The mobile FFI now has a versioned draft store keyed by hashed account ID and
  Rust session ID, plus a closed `SessionViewIntent` registry. It accepts only
  acknowledged `ValidatedView` activations and field changes, rejects stale,
  disabled, or unknown actions, and routes send/interrupt/input answers through
  existing session operations. Draft save, restore, clear, account switch, and
  unsupported action tests are authored but have not run. The B1 app shell
  provides `AndroidAttachmentActivityResultOwner`; D2 listens through its
  `MainActivity` forwarding methods for Photo Picker, document picker, and
  camera capture results. D2 bounds each stream read by Rust's
  `max_attachment_bytes()` value before it copies bytes into
  `ComposerAttachmentPayload`. The Rust FFI has
  `SessionHandle::submit_native_composer`, which passes `OutgoingAttachment`
  bytes through the existing send call. The production Kotlin
  `ComposerCorePort` adapter and regenerated UniFFI binding are still pending,
  so this path is not yet connected end to end. D2 limits presentation intake
  to 10 files and 48 MiB total, but the Rust FFI does not yet validate those
  limits or MIME content; MIME types remain untrusted metadata. No image
  conversion or paste API exists in this boundary. Photo/document cancellation
  is an explicit no-op, camera cancellation uses `saved=false`, and stale
  owner callbacks are fenced. Approval actions remain outside the mobile intent
  set. Fixtures cover denial, cancellation, oversize, stream failure, stale
  callbacks, and failed-send retry. Rust tests, UniFFI binding regeneration,
  Kotlin/JUnit, and device input checks remain pending. Integrate A3's ZXing
  scanner source and F1's regenerated bindings before the next Gradle check.
  The C1 viewport snapshot commit
  `6668ece8130ecadac82168f9a7c27f8e04f5acc6` is coordinated but not yet in
  `origin/main`; D2 will integrate it after it lands.
