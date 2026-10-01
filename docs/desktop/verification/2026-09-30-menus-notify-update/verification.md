# Desktop native menu bar, notification clicks, and update strip (2026-09-30)

[#10023](https://github.com/OpenAgentsInc/openagents/issues/10023), part of
[#10003](https://github.com/OpenAgentsInc/openagents/issues/10003) (audit
CDP-20). The in-window menus, palette, sidebar, chat layout, and painter are
unchanged. The new code has its own modules: `appmenu`, `native`, `strip`,
and `native_tests`. The shell and `rust-native-desktop`'s window only call
into them.

## What changed

- **Mac menu bar** (`crates/openagents-desktop/src/appmenu.rs`). It replaces
  winit's default main menu with **OpenAgents** (Settings, Hide, Hide
  Others, Show All, Quit), **File** (New chat, Search chats), **Chat** (Stop
  receiving reply, Rename, Pin/Unpin, Archive, Restore, and up to ten
  "Switch to" rows), and **Window** (Minimize, Zoom, Bring All to Front).
  Every chat row comes from `openagents_chat_app::commands::registry` and
  carries its enabled state. Choosing a row runs
  `chat_action::Action::Command { key }` with the same key the palette
  uses, after dismissing any open palette or menu. The menu redraws only
  when the registry changes. Chat rows carry no key equivalents, so the
  window's own shortcuts (`commands::shortcut`) keep their composing and
  overlay scopes. Linux and Windows have no native menu bar under winit, so
  the in-window menus stay.
- **Notification clicks.** A chat's notice now carries a click that opens
  the chat.
  - Linux portal: the notice sets `default-action` = `open` and
    `default-action-target` = the chat ID. `open` is not an `app.` action,
    so the portal returns the click as `ActionInvoked`.
  - Linux notification server: the notice sends `actions` =
    `["default", "Open"]`, and the click comes back as `ActionInvoked` with
    the server's ID, which maps back to the chat.
  - Both paths now use one long-lived session-bus connection, which a
    listener thread watches. The listener hands the chat ID to
    `native::open_chat`. The window then runs the registry's
    `switch-{chat}` command (`notices::open_command`) and comes forward
    through the new `App::focus_request` hook, which calls winit
    `focus_window`.
  - macOS and Windows still drop notices (see "Not verified").
    Since then macOS notices go through the notification center (#10061)
    and Windows notices are toasts (#10062), and their clicks take the same
    `native::open_chat` path; see
    [Windows notifications](../2026-09-30-windows-notifications/verification.md)
    and [release](../../release.md#releasing-for-windows).
- **Update strip** (`strip.rs`). When the updater holds a downloaded,
  verified build, a small strip at the top right says **Update ready** and
  shows **Restart to update**.
  - Linux and Windows read the build from `updates::ready`. The Mac reads it
    from the menu-bar item's updater (`menubar::update_ready`).
  - Its button carries Settings' update intent (`chrome::Action::Update`).
    On Linux and Windows that runs `updates::act`, which installs and
    relaunches. On the Mac, `updates::act` now calls
    `menubar::install_update`, the same install as the menu's **Restart to
    Update**.
  - The strip uses the window's one floating layer, so it steps aside while
    the palette, a chat menu, or "Scroll to bottom" is up. It is hidden on
    Settings, which shows the update itself.

## Headless checks

`cargo test -p openagents-desktop -p rust-native-desktop` on the Mac: all
pass (lib 114, bin 70 + 5 ignored, others). New tests:

- `appmenu`: `the_menu_bar_is_the_command_registry`,
  `tags_resolve_to_registry_keys_and_system_rows_run_nothing_here`,
  `the_chat_menu_lists_at_most_ten_chats_and_nothing_without_chats`
- `native_tests`: `menu_bar_items_run_the_registry_commands` (Switch with
  the palette open, New chat, Settings),
  `a_notification_click_opens_its_chat` (from Grid with the palette open;
  a chat that no longer exists changes nothing),
  `a_downloaded_update_shows_the_strip_and_its_button_runs_the_update`
- `strip`: `a_downloaded_update_shows_the_strip_and_its_button_restarts`
- `notices`: `a_notice_opens_its_chat_through_the_shared_switch_command`
- `platform::linux`: `a_click_on_a_notice_opens_its_chat` (portal and
  server signals parsed; other apps' clicks ignored).
  `notifications_over_a_private_bus` now also checks `default-action` and
  its target.

`cargo clippy --all-targets -D warnings` is clean for
`openagents-desktop` and `rust-native-desktop` on macOS,
`x86_64-unknown-linux-gnu`, and `x86_64-pc-windows-gnu`. The Linux and
Windows checks were cross-compiled with cargo-zigbuild's zig `cc`; their
tests compiled but did not run here. `cargo fmt` is clean.

Frames (`OPENAGENTS_NATIVE_CAPTURE_DIR`):

- `update-strip.png`
- `update-strip-under-palette.png`
- `update-settings.png`
- `notification-opened-chat.png`
- `menu-settings.png`

## Live on this Mac (dev build, isolated HOME, `--fake-host --no-login-agent`)

System Events read the dev process's menu bar as `Apple, openagents-desktop,
File, Chat, Window`, with these menus:

- File: `New chat, Search chats`
- Chat: `Stop receiving reply, —, Rename chat, Pin chat, Archive chat,
  Restore chat, —, Switch to New chat`. Enabled: false, false, true, true,
  true, false, false, true. That matches the registry: Stop is off while
  idle, and Restore is off for a chat that is not archived.

Clicking **File > New chat** added a second "Switch to" row. Clicking
**Settings** opened Settings (`live-menu-settings.png`). **Quit
OpenAgents** quit the process.

## Not verified here

- A Linux click on a real notification (GNOME portal, mako/dunst, Plasma)
  was not tried; coderos-4080 was busy. Only the signal parsing and the
  private-bus test cover it. On Wayland, bringing the window forward may
  need an activation token that this change does not pass.
- macOS notifications (UNUserNotificationCenter) need a signed bundle and a
  run of it, so they are left for later. Windows toasts need an AppUserModelID.
- The strip's restart on a real downloaded update. The install and relaunch
  are the existing, tested updater paths.
