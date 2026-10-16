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
destination also links x86_64 and fails):

```sh
cd apps/ios
xcodebuild -project Zeron.xcodeproj -scheme Zeron -destination 'id=<simulator id>' ARCHS=arm64 CODE_SIGNING_ALLOWED=NO build
```

A persistent `CARGO_TARGET_DIR` outside the checkout saves a cold build of
about 900 crates. `coder-account` uses the SDK from this repository by path
(`../../crates/openagents-sdk`), so SDK changes and app changes land in
one commit.

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
