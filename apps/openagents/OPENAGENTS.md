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
Run commands from this directory:

```sh
cargo build -p openagents
cargo test -p openagents-account -p openagents-brand -p openagents-update -p openagents-mcp
scripts/run-macos-dev.sh            # build and open the macOS development bundle
scripts/ios/build-core.sh           # the phone core for the iOS simulator
```

The phone core is arm64 only. For an Xcode simulator build, name an
Apple-silicon simulator and pass `ARCHS=arm64` (a generic simulator
destination also links x86_64 and fails). Sign locally (`CODE_SIGN_IDENTITY=-`):
an unsigned build can't reach the Keychain and shows "Couldn't save or restore
your login":

```sh
cd apps/ios
xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents -destination 'id=<simulator id>' ARCHS=arm64 CODE_SIGN_IDENTITY=- build
```

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

Update and relaunch (quit the app first):

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
