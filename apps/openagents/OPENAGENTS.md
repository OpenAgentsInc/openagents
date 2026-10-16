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
cargo build -p zeron
cargo test -p coder-account -p coder-brand -p zeron-update -p zeron-mcp
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
xcodebuild -project Zeron.xcodeproj -scheme Zeron -destination 'id=<simulator id>' ARCHS=arm64 CODE_SIGN_IDENTITY=- build
```

A persistent `CARGO_TARGET_DIR` outside the checkout saves a cold build of
about 900 crates. `coder-account` uses the SDK from this repository by path
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
ZERON_DEV_DATA_DIR="$HOME/Library/Application Support/OpenAgents Dev/data" scripts/run-macos-dev.sh
```

The data directory is written into the bundle, so opening
`~/work/openagents-app/apps/openagents/target/macos-dev/OpenAgents Dev.app` from the
Dock or Finder uses the same data. Agents testing their own builds keep the
default checkout-local data and a different `ZERON_DEV_IPC_PORT`, never this
directory.

## Pull upstream Zeron

From the repository root, on a clean `main`:

```sh
git subtree pull --prefix=apps/openagents https://github.com/zeronsh/zeron.git main
```

Resolve conflicts, keeping upstream structure and names (see the root
AGENTS.md rules). Then build and run the tests above. Record the merged
upstream commit in `FORK.md`. Upstream changes to the parts we removed
(`edge/` other than `edge/src/install.sh`, `apps/landing`,
`apps/www-redirect`, `.github/`) show as modify/delete conflicts: delete them
again.

## Not here

- Zeron's Cloudflare `edge/` server: replaced by
  [`crates/coder-sync`](../../crates/coder-sync) at `sync.openagents.com`.
- The landing sites and GitHub workflows. Release steps move to local
  scripts.
