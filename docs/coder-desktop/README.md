# Coder Desktop

Coder Desktop is Coder's desktop app: a fork of
[Zeron](https://github.com/zeronsh/zeron) (MIT, Rust and GPUI) at
[OpenAgentsInc/coder-desktop](https://github.com/OpenAgentsInc/coder-desktop),
wired to our Computers and payments SDKs. It's also the first app built on
the [OpenAgents SDK](../sdk/README.md) from outside this repository.

- [plan.md](plan.md): the integration plan. Ownership rules, the public
  contracts it uses, stages D0–D6 and their exit criteria.
- [Coder on the phone](../mobile/2026-10-15-coder-mobile-from-zeron.md): porting the iOS app to the fork's app, with `coder-sync` (Rust, Google Cloud) replacing Zeron's Cloudflare server.
- [Zeron research](../research/zeron/README.md): the earlier gap analysis,
  kept as a workflow and test checklist.

## Decisions (owner, 2026-10-15)

- **Fork it.** The fork is
  [OpenAgentsInc/coder-desktop](https://github.com/OpenAgentsInc/coder-desktop),
  with Zeron as the `upstream` remote.
- **Upstream moves fast, and that's fine.** Zeron ships nearly every day
  (100 commits and three releases in the first half of October 2026). Agents
  pull upstream often and resolve conflicts. Zeron keeps doing the harness
  glue and the UI. Our work is the wiring: computers, payments and the rest
  of the SDK.
- **Contribute back where it helps.** SDK-related pieces that make sense in
  Zeron itself, such as an OpenAgents provider, can go upstream as pull
  requests.
- **Coder is about coding agents.** Coder Desktop focuses on coding. Coding
  is the first part of the broader OpenAgents picture (see the
  [README](../../README.md#the-bigger-picture)), not the whole of it.
- **A dedicated OpenAgents desktop may come later.** It would carry the
  Verse, the plugin marketplace, the wallet and other OpenAgents-wide
  surfaces. It's separate from Coder Desktop, and nothing here retires the
  current OpenAgents desktop yet.
- **GPUI is welcome.** The fork uses Zeron's GPUI stack. Our monorepo's
  Rust Native renderer rule doesn't apply to it.
- **Other languages are allowed in the fork.** It's a separate repository;
  this monorepo stays Rust-only.

## First SDK milestone

Sign in to an existing OpenAgents account, show previous chats in the sidebar,
and open any saved chat. This comes before computer provisioning and agent
execution. Reuse website sign-in and app sessions through the Rust SDK;
Zeron's WorkOS/edge login is not the OpenAgents account system. Include
complete message paging, saved files/tool details, archived-chat access,
and authorized shared-project chats. Local chats not synced to the account
stay local. See [#11824](https://github.com/OpenAgentsInc/openagents/issues/11824)
and the [SDK delivery plan](../sdk/delivery-plan.md#first-desktop-delivery-authentication-and-history).

## Local setup

The clone sits next to this repository:

```sh
cd ~/work
git clone https://github.com/OpenAgentsInc/coder-desktop.git
cd coder-desktop
git remote add upstream https://github.com/zeronsh/zeron.git
```

Build and open the development bundle on macOS:

```sh
RUSTUP_TOOLCHAIN=1.98.1 scripts/run-macos-dev.sh
```

The fork pins the `stable` channel. Its GPUI uses `std::hint::cold_path`,
which Rust 1.94 rejects as unstable, so build with a recent stable (1.98.1
worked on 2026-10-15) or run `rustup update stable`. The first debug build
compiles about 900 crates.

The app is called **Coder** everywhere users see it (bundle, menus, dock,
window title, notifications); "Coder Desktop" is its formal name. Stage D0
gave it its own identity, separate from Zeron and from the OpenAgents
desktop:

| | Packaged app | Development bundle |
| --- | --- | --- |
| Bundle | `Coder.app` | `target/macos-dev/Coder Dev.app` |
| Bundle ID | `com.openagents.coder.desktop` | `com.openagents.coder.desktop.dev` |
| Data directory | `~/Library/Application Support/Coder Desktop` | `target/macos-dev/data` in the checkout |
| Engine IPC port | 27854 | 49777 |
| URL scheme | `coder-desktop://` | not registered |

The bundle ID isn't `com.openagents.coder`, which is the iPhone app's App
Store record and the Android package. Zeron's update feed, sign-in and hosted sync are off, so the app
runs local and account-free and never contacts Zeron's services. An installed Coder,
Zeron and the OpenAgents desktop can run side by side. Set
`ZERON_DEV_BUILD_ONLY=1` to build without opening the bundle. The binary
crate keeps its upstream name (`cargo build -p zeron`).

The fork's [FORK.md](https://github.com/OpenAgentsInc/coder-desktop/blob/main/FORK.md)
lists every identity seam, what's still named Zeron, and the platform matrix
(only macOS checked so far). The values live in one crate, `crates/brand`.

## Appearance

The fork includes **Coder Light** and **Coder Noir** as its default light and
dark themes ([#11836](https://github.com/OpenAgentsInc/openagents/issues/11836)).
They use a pinned snapshot of the public `oa-tokens` palette, with opaque
surfaces and neutral primary controls. Terminal panels use Coder Noir in both
appearances. Saved theme choices and accent/glass preferences still apply.

The fork's [theme documentation](https://github.com/OpenAgentsInc/coder-desktop/blob/main/docs/theme-system.md)
records the palette revision. Its [patch log](https://github.com/OpenAgentsInc/coder-desktop/blob/main/docs/downstream-patches.md)
tracks the changes to review when updating from Zeron.

## Keeping up with upstream

```sh
git fetch upstream
git merge upstream/main     # or rebase our branch; resolve, build, run
```

Then rebuild and open the dev bundle. Keep our changes in clearly separated
places so upstream merges stay easy:
- new crates or modules for the SDK adapters;
- small edits at the integration seams;
- a short patch log in the fork.

## Where the work is tracked

- **Shared SDK and Computers work:** this repository, under
  [#11800](https://github.com/OpenAgentsInc/openagents/issues/11800) (the
  [delivery plan](../sdk/delivery-plan.md)).
- **Desktop app work:** tracked in this repository, beginning with
  [#11824](https://github.com/OpenAgentsInc/openagents/issues/11824).
  Implementation stays in `OpenAgentsInc/coder-desktop`; all issues stay
  in `OpenAgentsInc/openagents` (owner, 2026-10-15).
- **The Coder 1.0 freeze** ([1.0-freeze](../coder/1.0-freeze.md)) covers the
  `coder`, `openagents`, `microcoder` and `coder-boundary` programs. Coder
  Desktop is a separate app and isn't frozen.
