# Windows desktop verification

Issue: [#10027](https://github.com/OpenAgentsInc/openagents/issues/10027).
Code: `d5e8c59e46` (updater, packaging, clipboard, folder chooser, chat
cache, local-run sandbox). Release steps: [release.md](../../release.md),
**Releasing for Windows**.

Chat, Coder runs from chat (#10032, #10033), and pairing are the same
shared Rust on Windows as on the Mac. The Verse backdrop stays off
(`backdrop` returns `None` on Windows; Verse does not build there). No
desktop visual, menu, or painter change.

## What changed for Windows

- **Chat cache.** `openagents-chat`'s device cache fsynced its directory
  after every write by opening it with `File::open`, which Windows refuses:
  every cache write failed there ("device cache I/O failed"), so kept chats,
  transcripts, and Gym cards did not persist. The directory sync is now
  Unix-only, as is the chat migration's. Under Wine this took
  `openagents-chat-app` from 15 failures to 112 of 112 passing.
- **Clipboard.** Copy and paste use the Windows clipboard API (`arboard`)
  as Unicode text instead of a PowerShell pipe (console code page, a
  flashing console window).
- **IME and AltGr.** IME composition is winit's (`set_ime_allowed`,
  `set_ime_cursor_area`, preedit and commit); unchanged. A character typed
  with AltGr, which Windows reports as Ctrl+Alt, is now inserted as text
  rather than dropped as a shortcut.
- **Folder chooser.** The common item dialog (`IFileOpenDialog` with
  `FOS_PICKFOLDERS`, through `rfd`), replacing a PowerShell
  `FolderBrowserDialog` whose answer passed through the console code page.
- **Notifications.** Coder's desktop notifications were Linux's (#10026)
  when this landed; macOS followed in #10061 and Windows toasts in #10062
  (see [../2026-09-30-windows-notifications/verification.md](../2026-09-30-windows-notifications/verification.md)).
- **Local runs.** The window sets `HOME` from `USERPROFILE` as `coder.exe`
  does, so the in-process runner finds `~/.openagents`. The `toolchains`
  sandbox (#10045) on Windows is the AppContainer with the network and no
  extra DACL reads (the system folders on `PATH` refuse an entry, which
  refused every run, and a profile toolchain tree is too large to grant per
  run); where no AppContainer can be made it refuses before any command.
- **Updater.** `update::windows`: a manifest for `"platform": "windows"`,
  formats `msi` and `zip`; the per-user MSI install stages a verified MSI
  and hands it to Windows Installer after exit; a `.zip` copy is offered
  the download; a development build never checks.
- **Packaging.** `scripts/desktop/package-windows.sh` (cross-build, strip,
  wixl, osslsigncode when given a certificate) and
  `scripts/desktop/sign-manifest-windows.sh` (refuses unsigned packages
  outside a test prefix). Both MSIs install per user in
  `%LOCALAPPDATA%\Programs\OpenAgents`.
- Also fixed: the Linux updater's `PermissionsExt` use, which stopped the
  desktop crate compiling for Windows after #10025.

## Checks

On an M5 Max Mac (Rust 1.97.1) and coderos-4080 (NixOS, Wine 11.0,
PortableGit 2.56.0, msitools 0.106), in scratch folders only:

- `cargo clippy --all-targets -D warnings` for `openagents-desktop`,
  `coder`, `microcoder`, `coder-boundary`, `rust-native-desktop`,
  `openagents-chat`, `openagents-chat-app`: clean on
  `x86_64-pc-windows-gnu`, macOS, and Linux.
- macOS tests: `openagents-chat`, `openagents-desktop`, `coder-boundary`,
  and `rust-native-desktop` all pass. Linux (coderos-4080): the desktop
  crate's `update::` tests, 27 passed.
- Under Wine (Windows test executables cross-built in release):
  - `openagents-desktop` lib: 86 passed, including
    `a_windows_manifest_is_only_for_windows`,
    `a_windows_install_is_recognized_by_where_it_runs`,
    `only_an_msi_header_is_accepted`,
    `an_msi_install_stages_the_signed_msi_and_a_zip_is_offered_the_download`,
    `the_release_key_signs_a_windows_manifest_the_windows_app_accepts`,
    `the_windows_install_script_takes_its_paths_from_variables`.
  - `openagents-desktop` bin: 62 passed, 5 ignored, including
    `a_chat_run_starts_follows_stops_and_continues_through_the_local_runner`,
    `a_chat_run_follows_the_local_capability_settings`, and the four
    `platform::windows` tests (Run entry, start flag, pipe name,
    login-agent round trip). Ignored on Windows: the saved Codex and
    Claude Code session browser (`coder-history` reads those on Linux and
    macOS only).
  - `openagents-chat-app` (the chat service): 112 passed.
    `openagents-chat`: 46 passed, 1 ignored.
  - `rust-native-desktop`: 60 passed. `coder-boundary`: 18 passed,
    including `windows_toolchains_grant_nothing`.
  - `coder` `task::local`: 10 passed and `task::settings`: 3 passed, with
    Git for Windows making the checkouts.
  - `microcoder` `repository::windows_tests`: 3 passed, including the new
    `a_toolchain_turn_runs_in_its_appcontainer_or_refuses_before_any_command`
    (Wine makes no AppContainer, so it refused before admission with the
    task still queued and no command run).
  - Snapshot tests read their fixtures from the source tree, so the tree was
    copied to the same path inside the Wine prefix for these runs.
- **MSI under Wine.** `msiexec /i OpenAgents-1.0.0-x64.msi /qn` installed
  `OpenAgents.exe`, `coder.exe`, `microcoder.exe`, and `coder-boundary.exe`
  in `C:\users\<user>\AppData\Local\Programs\OpenAgents` with no elevation
  (Word Count 10: compressed, no elevated privileges), wrote the `Run`
  value `"…\OpenAgents.exe" --start-host`, `HKCU\Software\OpenAgents\Desktop`,
  and a Start menu shortcut. `msiexec /x` removed the `Run` value. A 1.0.1
  fixture MSI from the same binaries upgraded it in place:
  `FindRelatedProducts` set `WIX_UPGRADE_DETECTED` to the 1.0.0 product,
  `RemoveExistingProducts` removed it, and only 1.0.1 stayed registered.
  The launch action is custom action type 210 (the installed
  `OpenAgents.exe`, asynchronous), sequenced after `InstallFinalize` when
  `UILevel > 2`.
- **The installed app's updater under Wine.** The installed
  `OpenAgents.exe --check-update` read `desktop/windows/manifest.json`
  (404: no release is published); a copy named `openagents-desktop.exe`
  said it is a development build. With
  `OPENAGENTS_UPDATE_MANIFEST_URL` set to the test manifest below, the
  installed app verified its Ed25519 signature with the compiled key and
  answered `OpenAgents 1.0.0 is up to date.`
- `sign-manifest-windows.sh` without a test prefix refused the unsigned
  packages ("not Authenticode-signed").

## Artifacts (unsigned, test prefix only)

No Authenticode certificate exists yet (workspace `NEEDS_OWNER.md`), and
unsigned packages are not a release, so nothing is published to
`desktop/windows/` and `/install` still says Coming soon for Windows. The
build is published to the test prefix for the owner's Windows 11 check:

| File | SHA-256 |
| --- | --- |
| [OpenAgents-1.0.0-x64.msi](https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/windows-test/1.0.0/OpenAgents-1.0.0-x64.msi) (136,078,336 bytes) | `46b125461de766235508f23cd66856d038cc55596847458d6f4a980d0bce7158` |
| [OpenAgents-1.0.0-windows-x64.zip](https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/windows-test/1.0.0/OpenAgents-1.0.0-windows-x64.zip) (128,437,421 bytes) | `7e6207a45eceb9a6296f64a6290d276cdb8a59d255753379d057c02dcddc1f17` |

Beside them: `SHA256SUMS`, `SHA256SUMS.sig`, the public key, `BUILDINFO`
(commit `d5e8c59e46576d1b53a5803e52ac796959affe72`, rustc 1.97.1,
`x86_64-pc-windows-gnu`, signed no), and the signed manifest at
`desktop/windows-test/manifest.json` (key `desktop-update-2026-09`).

## Owner checks

On a real Windows 11 PC (Wine makes no AppContainer and no GPU window):
install, pair, chat, a Coder run from chat, a sign-out and sign-in, and,
once a signed release exists, **Restart to update**. The steps are in the
workspace `NEEDS_OWNER.md`.
