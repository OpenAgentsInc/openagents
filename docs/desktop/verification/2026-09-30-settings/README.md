# Desktop Settings verification

Issue: [#10021](https://github.com/OpenAgentsInc/openagents/issues/10021)
(part of [#10003](https://github.com/OpenAgentsInc/openagents/issues/10003),
audit CDP-22).

## What landed

Settings has six pages, chosen from a row of chips at the top:

- **Appearance**: the app is dark only (no light theme, owner direction), so
  the page says so and offers **Reduce motion**, which keeps the Grid behind
  the window still. The Watch backdrop follows the switch at once, not only
  at the next showing, alongside the system's own setting.
- **Text size**: Smaller, Default, Larger, Largest (90, 100, 115, 130
  percent). It scales the window theme's text and the chat transcript's
  metrics (`TextSize::transcript`, shared with the phones), and the
  transcript is laid out again at once.
- **Keyboard shortcuts**: a read-only list from the shared command registry
  (`openagents_chat_app::commands::bindings`, each checked against
  `shortcut`), plus the window's Cmd/Ctrl+B for the sidebar.
- **Notifications**: a switch for Coder's notifications. Linux delivers them
  (#10026). On macOS and Windows the page says this computer doesn't show them
  yet.
- **Phones and computers**: the existing pairing screens (`screens::root`),
  unchanged. Their buttons keep the person on Settings. Leaving the page for
  another Settings page, a chat, or the Grid cancels a shown code, as leaving
  the standalone screen does (`DSK-01`).
- **Archived chats**: the chats the shared list marks archived
  (`chat_list::archived`), each with **Restore**, which sends the shared
  `Command::Restore` without opening the chat.

The preferences are shared Rust (`openagents_chat_app::preferences`) and live
in the `app` section of `~/.openagents/settings.json`, beside Coder's
settings. They're saved through `coder::task::settings`, which keeps the
`coder` section as it is and leaves alone a file it refuses. The window reads
them at launch. A capture or test keeps them only in memory.

## Checks

Rust 1.97.1 on an M5 Max Mac, in a clean worktree from `origin/main`:

- `openagents-chat-app`: 121 passed, including
  `a_missing_or_broken_section_is_the_defaults`,
  `a_section_reads_back_what_was_written`,
  `every_size_scales_the_transcript_and_the_default_is_unchanged`, and
  `every_listed_shortcut_is_one_the_window_admits`.
- `openagents-desktop` library: 117 passed, including
  `every_page_validates_and_uses_plain_words` (no banned word on any page)
  and `the_apps_reduce_motion_applies_while_the_window_shows`.
- `openagents-desktop` binary: 68 passed, 5 opt-in ignored, including:
  - `each_setting_persists_across_a_reopen_and_applies_at_once`: text size,
    reduce motion, and notifications apply at once (theme sizes, a taller
    transcript, the backdrop's flag, and no notifications), survive a fresh
    window reading the same file, and leave `coder.start` as it was. Setting
    them back restores the default transcript height.
  - `a_setting_that_cannot_be_saved_still_applies_and_says_so`
  - `leaving_phones_and_computers_in_settings_cancels_the_code`: a code shown
    on the Settings page is cancelled at the fake computer when the person
    moves to another page, a chat, or the Grid.
  - `an_archived_chat_is_restored_from_settings`
  - `every_settings_page_mounts_and_paints` at 1200 × 840 and 760 × 540.
- Version lockstep: 3 passed. Clippy (`-D warnings`, all targets) is clean on
  both crates, and `cargo fmt` is clean.

## Captures

Headless `rust_native_desktop::capture` at 1200 × 840, 1×, written by the
tests above with `OPENAGENTS_SETTINGS_CAPTURE_DIR`:

- [Appearance](settings-appearance.png) and
  [with Reduce motion on](settings-appearance-reduced.png)
- [Text size](settings-textsize.png) and [at Largest](settings-textsize-largest.png)
- [Keyboard shortcuts](settings-shortcuts.png)
- [Notifications](settings-notifications.png)
- [Phones and computers](settings-computers.png)
- [Archived chats with one to restore](settings-archived-list.png) and
  [empty](settings-archived.png)

## Not verified here

No owner home, real settings file, keychain, host, or installed app was used.
Owner-only checks are listed in `NEEDS_OWNER.md`: that a change survives quitting
and reopening the installed app, and that on the Linux desktop turning
notifications off stops them.
