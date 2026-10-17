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

## Complete OpenAgents names (#11899)

Renamed crates, the desktop binary, iOS targets and UniFFI modules, packaging,
MCP instructions, and developer settings. Existing bundle IDs, Keychain
identities, storage locations, and the UIKit scene delegate stay stable. Old
environment names, saved theme choices, project imports, and conversation
links remain readable. Upstream fixes now require manual ports.
