# Coder Desktop patches

Keep fork-specific changes here so upstream updates can review each integration.

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

`client_ffi::custom_agents` owns account isolation, saved retry requests, agent
selection, and the Rust Native form. UIKit's `AgentFormController` renders that
form and forwards versioned field and activation events. The new-session menu
adds personal agents beside existing models. Rust Native's optional syntax
feature keeps forms independent of the app's separate syntax-engine version.

- [OpenAgents #11912](https://github.com/OpenAgentsInc/openagents/issues/11912):
  customizable Home dashboards. `openagents-account::dashboard` owns layout edits,
  summary reads, import/export, account saves, and the layout directory. UIKit
  `HomeViewController` and GPUI `home` mount the same Rust Native cards. Hooks
  add the Home destination, account lifecycle callbacks, mobile FFI, and one
  read-only session-report accessor; no separate chat path or engine starts.

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
  Move all to a cloud computer is commented out until #11933.

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
