# Windows notifications verification

Issue: [#10062](https://github.com/OpenAgentsInc/openagents/issues/10062).
Follows macOS ([#10061](https://github.com/OpenAgentsInc/openagents/issues/10061))
and Linux ([#10026](https://github.com/OpenAgentsInc/openagents/issues/10026));
the content and switch rules are the INVARIANTS notification row.

## What changed

- **Toasts.** `crates/openagents-desktop/src/win_notify.rs` is the Windows
  `notices::Center`: `ToastNotificationManager::CreateToastNotifierWithId`
  for the AppUserModelID `OpenAgents.Desktop`, a toast from
  `notices::windows_toast_xml` (the chat's title and the status, nothing
  else; `launch` is the notice's ID, `coder-<chat>`), tagged
  `notices::windows_tag` (the notice's ID, hashed only past Windows's
  64-character limit) in the group `coder`, so a newer toast for a chat
  replaces the older one. The `windows` crate 0.62.2 was already in the
  graph (through `accesskit_windows`); only features were added.
- **Permission.** Windows never asks. `authorize` reads the notifier's
  `NotificationSetting`: on is granted, off for the app, the user, group
  policy, or the manifest is denied, and nothing is posted.
- **Clicks.** Each toast's `Activated` handler reads
  `ToastActivatedEventArgs::Arguments` and opens that chat through
  `native::open_chat`, the path Linux and macOS clicks take. Shown toasts
  are kept (by tag) so their handler lives. There is no COM activator
  (`ToastActivatorCLSID`), so a click in Action Center after the app quit
  opens nothing, like macOS's quit-then-click limit.
- **AppUserModelID.** The app calls `SetCurrentProcessExplicitAppUserModelID`
  at startup. Both MSIs put `System.AppUserModel.ID` on the Start menu
  shortcut (the `.ps1` with `<ShortcutProperty>`; the `.sh` adds an
  `MsiShortcutProperty` row with msitools' `msibuild`, since wixl has no
  `<ShortcutProperty>`) and register the ID under
  `HKCU\Software\Classes\AppUserModelId\OpenAgents.Desktop` (`DisplayName`)
  in a new per-user component removed on uninstall.
- **Unpackaged.** Without that registration (a `.zip` copy, `cargo run`)
  nothing is shown and nothing crashes; `--notify-test` says so.
- `--notify-test` on Windows reports "Windows toast notifications
  (ToastNotificationManager)" or why nothing showed.

## Checks (M5 Max Mac only)

No Windows machine was used; coderos-4080 was not touched.

- `cargo clippy -p openagents-desktop --all-targets --target
  x86_64-pc-windows-gnu -- -D warnings`: clean.
- `cargo clippy -p openagents-desktop --all-targets -- -D warnings`
  (macOS): clean.
- macOS tests: `openagents-desktop` lib 137 passed, bin 94 passed and 5
  ignored. The new lib tests through the `notices` seam:
  `a_windows_toast_says_only_the_title_and_status_and_carries_the_notice_id`,
  `a_newer_windows_toast_for_a_chat_replaces_the_older_one`,
  `a_click_on_a_windows_toast_opens_its_chat`,
  `windows_notifications_turned_off_show_nothing` (the mock center with
  Windows's setting off, and with no registration), and
  `both_installers_give_the_shortcut_the_app_id_the_app_claims`. The
  Settings switch is still the shell's gate
  (`each_setting_persists_across_a_reopen_and_applies_at_once`).
- `win_notify`'s own tests (`the_registration_key_names_the_app_id`,
  `the_toast_xml_parses_as_windows_reads_it`) compile for Windows (clippy
  above) but were not run: no Windows or Wine run this time.
- The release Windows binary links (`cargo build --release --target
  x86_64-pc-windows-gnu -p openagents-desktop`).
- **MSI tables.** `scripts/desktop/package-windows.sh --bin-dir` with stub
  MinGW executables, then `msiinfo` (msitools 0.106 from Homebrew):
  `MsiShortcutProperty` holds `ShortcutAppId | StartMenuShortcut |
  System.AppUserModel.ID | OpenAgents.Desktop`, the `Registry` table holds
  `Software\Classes\AppUserModelId\OpenAgents.Desktop` `DisplayName =
  OpenAgents` (HKCU) in component `Notifications`. The `.ps1` (WiX v4+) was
  not built here.

## Not verified (owner, Windows 11)

A toast appearing, a click opening the chat and bringing the window
forward, replacement of an older toast for the same chat, and the Settings
switch turning toasts off, with the installed MSI on real Windows 11: in
the workspace `NEEDS_OWNER.md`, next to the Windows entry. Windows may
flash the taskbar button instead of raising the window after a click
(foreground rules for a process without a COM activator).
