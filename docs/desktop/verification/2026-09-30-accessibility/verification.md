# Desktop accessibility verification

Issue: [#10024](https://github.com/OpenAgentsInc/openagents/issues/10024)
(part of [#10003](https://github.com/OpenAgentsInc/openagents/issues/10003),
audit CDP-15).

Every Rust Native desktop window now serves screen readers an
[AccessKit](https://accesskit.dev) tree: NSAccessibility for VoiceOver on
the Mac, AT-SPI for Orca on Linux, UI Automation for Narrator on Windows.

## What it does

- `rust_native_desktop::access` builds the tree from the validated view as
  the window laid it out. Buttons are buttons (checkbox glyphs are
  checkboxes with their state), lists are lists named by their label,
  headings are headings, text is static text, cards are groups, composers
  are multi-line text fields named by their placeholder with the typed text
  as their value, a working row is a polite status, and each node carries
  its bounds in window pixels. Disabled buttons say so.
- A surface the application paints itself is an image named by its label,
  unless the application describes it (`App::access_content`). The desktop
  transcript does (`Transcript::access`): the conversation is a log of
  articles named **You**, **Reply**, or **Notice**, holding the message's
  text (Markdown as plain text, code blocks included), tool rows, cards,
  and their buttons.
- The chat panel gives the typed text of its composer, search, command
  query, and rename fields (`access_value`) and which one has the text
  cursor (`access_focus`), so focus follows the caret as well as Tab.
- The window attaches `accesskit_winit` before it first shows, builds no
  tree until a screen reader asks, and rebuilds only when the layout,
  focus, scroll, or a surface's drawing revision changes.
- A screen reader's request runs as input the window already handles
  (`access::answer`): press on a button is the activation a click makes,
  resolved against the current view; focus is a Tab focus move the
  application allows; press on a transcript control is a press and release
  at its on-screen center; focusing a composer is a click at the end of its
  text; setting its value is select-all then the text typed, and inserting
  text is typing at the caret. See the **Desktop accessibility** invariant.

No visuals, menus, palette, sidebar layout, or painting changed. The one
visible difference: the window is created hidden and shown right after the
adapter attaches, as AccessKit requires.

## Checks

Rust on an M-series Mac, 2026-09-30:

- `cargo test -p rust-native-desktop` (81 passed) and
  `--no-default-features` (64 passed), including `access::tests`:
  `a_screen_reader_reads_the_sidebar_the_replies_and_the_composer` (the
  whole tree as `accesskit_consumer`, the crate the platform adapters serve
  from, reads it), `focus_follows_the_windows_focus_and_the_applications_text_cursor`,
  `requests_become_the_input_the_window_already_handles`,
  `bounds_are_the_laid_out_rectangles_in_window_pixels`,
  `ids_stay_the_same_from_one_tree_to_the_next`.
- `cargo test -p openagents-desktop` (lib 126, bin 82 passed, after rebasing on the new Settings page and theme), including
  `access_tests` on the real chat window with the in-process fake host:
  `a_screen_reader_navigates_chats_reads_replies_and_sends_a_message`,
  `a_screen_reader_writes_and_sends_a_message` (New chat from the sidebar,
  focus lands in the composer, set value, press Send; the log then reads
  **You** "Is the build green?" and the **OpenAgents is replying…** status,
  the chat appears as a sidebar row, and pressing that row after a new chat
  brings the message back),
  `a_screen_reader_opens_another_chat_from_the_sidebar`,
  `a_screen_reader_opens_settings_and_reads_its_controls`.
- `cargo clippy -p rust-native-desktop -p openagents-desktop --all-targets
  -- -D warnings` clean; `cargo fmt` clean.
- `cargo check -p rust-native-desktop --target x86_64-pc-windows-gnu`
  passes. A Linux cross-check from this Mac stops in `tree-sitter`'s C
  build (no Linux C toolchain here), before this crate; the Linux build
  runs on a Linux machine.

### Live, through the Mac's accessibility API

[`macos-ax-probe.txt`](macos-ax-probe.txt) is the window as VoiceOver
reads it: `cargo run -p rust-native-desktop --example access_probe` (a
chat-shaped window with no disk or network) queried and driven with
[`ax-probe.swift`](ax-probe.swift) through `AXUIElement`, the API
VoiceOver uses. It shows the sidebar list of chat buttons, the heading, the
conversation's **You** and **Reply** groups with their text, the text area
named **Message OpenAgents**, and **Send**. Setting `AXFocused` on the text
area put the caret there; setting `AXValue` typed "Is the build green?";
`AXPress` on **Send** sent it (the new messages appear in the conversation
and the field empties); `AXPress` on **Plan the launch** opened that chat.

## Not verified here

A person running VoiceOver on the Mac and Orca on Linux over the real app:
listed in the workspace `NEEDS_OWNER.md`.
