# The OpenAgents desktop and mobile apps

This directory is the Coder Desktop fork of [Zeron](https://github.com/zeronsh/zeron),
imported with its history on 2026-10-16. It becomes the OpenAgents desktop
app (GPUI) and the OpenAgents mobile app (UIKit on iOS, Kotlin on Android,
over the Rust core in `crates/mobile`). The binding rules are in the root
[AGENTS.md](../../AGENTS.md), under "One OpenAgents desktop app and one mobile
app". The plan and its steps are in the
[merge audit](../../docs/audits/2026-10-16-coder-desktop-monorepo-merge.md).
`FORK.md` and `docs/downstream-patches.md` still describe each OpenAgents
change on top of upstream.

## Build and test

This is its own Cargo workspace (its own `Cargo.lock`, profiles and
`rust-toolchain.toml`, Rust 1.98.1). The root workspace does not include it.
Run commands from this directory.

On NixOS, first run `nix-shell shell-linux.nix` to make the Linux desktop's
native development libraries available to `pkg-config`, including OpenSSL,
Fontconfig, and ALSA. Keep the build and launch commands in that shell.

```sh
cargo build -p openagents
cargo test -p openagents-account -p openagents-brand -p openagents-update -p openagents-mcp
scripts/run-macos-dev.sh            # build and open the macOS development bundle
scripts/ios/build-core.sh           # the phone core for the iOS simulator
```

Android implementation is tracked in the
[Android parity sprint](../../docs/audits/2026-10-18-android-parity-sprint.md)
(epic #12077). A1 cross-built this workspace's shared Rust core for Android
and generated the UniFFI Kotlin source at
`apps/android/app/src/main/java/uniffi/openagents_core/openagents_core.kt`.
`scripts/android/build-core.sh` syncs that source and builds both ABIs. B1
adds the Kotlin shell and release path. Do not use the retired Android app as
the current build target.

Enter the pinned Android shell with `nix develop ./os#android` from the
repository root. The shell pins JDK 17, Gradle 8.14.4, Android API 35, and
NDK 27.1.12297006. From `apps/openagents/`, use `scripts/android/app.sh` for
the local Android commands:

```sh
scripts/android/app.sh core     # build the Rust core and both Android ABIs
scripts/android/app.sh apk      # build the debug APK
scripts/android/app.sh install  # install the debug APK on the selected device
scripts/android/app.sh run      # launch the debug app
scripts/android/app.sh bundle   # build the release Android App Bundle
scripts/android/app.sh check    # compile debug and release variants
```

The script runs Gradle with a clean environment and two workers. Gradle can
download its pinned public dependencies; the demo itself makes no network
requests. Set `CARGO_TARGET_DIR` to the shared app target before building the
core.
The debug application uses an isolated `com.openagents.app.debug` ID; release
uses the mobile ID from `crates/brand` and version metadata shared with iOS.
No GitHub workflow builds or publishes the Android app.

The iOS core build is arm64 only. For an Xcode simulator build, name an
Apple-silicon simulator and pass `ARCHS=arm64` (a generic simulator
destination also links x86_64 and fails). Sign locally (`CODE_SIGN_IDENTITY=-`):
an unsigned build can't reach the Keychain and shows "Couldn't save or restore
your login":

```sh
cd apps/ios
xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents -destination 'id=<simulator id>' ARCHS=arm64 CODE_SIGN_IDENTITY=- build
```

Tests never build the real macOS platform: AppKit aborts off the main
thread and opens a crash dialog. Use `openagents-gpui-test` (`crates/gpui-test`):
`headless_app` and `save_capture` for offscreen screenshots, or
`#[gpui::test]`. Fixture binaries that show a window go through
`real_application` and run only with `OPENAGENTS_GPUI_REAL_PLATFORM=1`. A guard
test in that crate fails on any other use of the platform.

Before pushing a UI change, also check that the fixture examples still
compile. This only builds them; it never runs one or opens a window:

```sh
cargo check -p openagents-ui --examples --features account-fixture,project-palette-fixture,compact-picker-fixture,browser-fixture,windows-render-fixture,appshots-fixture,pull-request-fixture,fork-chat-fixture,resource-profile
```

`--all-features` also turns on `upstream-workos`, which OpenAgents builds never
use. Add a new fixture feature to this list.

A persistent `CARGO_TARGET_DIR` outside the checkout saves a cold build of
about 900 crates. `openagents-account` uses the SDK from this repository by path
(`../../crates/openagents-sdk`), so SDK changes and app changes land in
one commit.

## The owner's everyday app

The owner's daily desktop app is the development bundle built from a clean
worktree of `main` at `~/work/openagents-app`. Its data (chats, settings, the
device identity on `coder-sync`) lives outside any checkout, in
`~/Library/Application Support/OpenAgents Dev/data`. It was moved there on
2026-10-16 from the archived fork's `~/work/coder-desktop/target/macos-dev/data`,
which is kept as a backup. Never run the old bundle again: it has the same
device identity.

Since the OpenAgents rename (2026-10-16) the development bundle is
`OpenAgents Dev.app` (`com.openagents.desktop.dev`), and packaged builds are
`OpenAgents.app` (`com.openagents.desktop`). The OpenAgents sign-in is carried
over once from the old `Coder Dev` Keychain entry. Earlier data-directory
paths are listed in `openagents-account/previous-profiles` inside the data
directory, one per line. macOS may ask once to let the renamed app read that
entry. A packaged install's `Coder Desktop` data folder is renamed to
`OpenAgents Desktop` on first start.

Upgrade to the newest `main` without quitting (#12056):

```sh
"$HOME/work/openagents-app/apps/openagents/target/macos-dev/OpenAgents Dev.app/Contents/MacOS/openagents" host-upgrade --reason "newest main"
```

The bundle is built with the `dev-opt` profile (our crates at `opt-level 2`,
debug info and incremental kept); plain `cargo build` stays unoptimized.
It fetches `main` into `~/work/openagents-app`, builds the bundle beside the
running one in `~/work/openagents-target-app` (waiting for a build slot like
an agent's build), and checks it: the signature, the data folder and port
written into it, and its headless engine answering on a scratch port. If the
build or a check fails, the checkout goes back, the running app is left
alone, and the command says why. Otherwise the app takes over: it waits until
you haven't typed for 30 seconds, aren't on a voice call, and have no unsaved
files, shows a notice for 10 seconds, swaps the bundle in, and reopens on the
chat you had open. Agents under keepers keep running through the restart,
and the queues of chats that were running keep draining. The reopened app
says "Reopened after an upgrade". `host-upgrade --status` shows the upgrade
waiting in the app and the last results; every upgrade is a line in
`host-upgrade/upgrades.jsonl` in the data folder (from and to commit,
Chicago time, result). If the new app doesn't start within three minutes,
the previous bundle is put back and opened. Agents use the `upgrade_host`
tool, which needs the `host.upgrade` scope.

Rebuilds and upgrades never ask for the Keychain password (#12069). The
Keychain trusts the app's designated requirement, so every build is signed
with the same identity: `scripts/run-macos-dev.sh` picks the Apple
Development identity on the OpenAgents team (`OPENAGENTS_DEV_TEAM_ID`,
default `HQWSG26L43`), never just the first one installed, and the host
upgrade refuses a build that doesn't meet the running app's requirement.
Without an Apple Development identity the bundle is signed ad hoc and macOS
asks again after every rebuild. Processes that outlive the app, keepers and
`openagents mcp`, must not read the Keychain: once a rebuild or the next
upgrade replaces or deletes their binary, macOS no longer trusts them.

Update and relaunch by hand (quit the app first):

```sh
git -C ~/work/openagents-app fetch origin main
git -C ~/work/openagents-app checkout --detach origin/main
cd ~/work/openagents-app/apps/openagents
OPENAGENTS_DEV_DATA_DIR="$HOME/Library/Application Support/OpenAgents Dev/data" scripts/run-macos-dev.sh
```

The data directory is written into the bundle, so opening
`~/work/openagents-app/apps/openagents/target/macos-dev/OpenAgents Dev.app` from the
Dock or Finder uses the same data. Agents testing their own builds keep the
default checkout-local data and a different `OPENAGENTS_DEV_IPC_PORT`, never this
directory.

## Port upstream fixes

The apps retain Zeron's MIT attribution. Since the complete OpenAgents rename
(#11899), import upstream fixes by manual port from
[Zeron](https://github.com/zeronsh/zeron), then build and test the affected
crates. Record the upstream commit and integration in `FORK.md` and
`docs/downstream-patches.md`. Don't use `git subtree pull` over the renamed
source tree.

Crates and the desktop binary use `openagents` names. Set `OPENAGENTS_*`
variables for new scripts; previous `ZERON_*` settings remain fallbacks for
one release. Bundle IDs, Keychain identities, data directories, and the UIKit
scene delegate keep their existing values. Existing project files, theme
selections, and conversation links remain readable.

## Not here

- Zeron's Cloudflare `edge/` server: replaced by
  [`crates/coder-sync`](../../crates/coder-sync) at `sync.openagents.com`.
- The landing sites and GitHub workflows. Release steps move to local
  scripts.
