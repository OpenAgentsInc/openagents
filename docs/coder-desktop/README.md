# Coder Desktop

Coder Desktop is Coder's desktop app: a fork of
[Zeron](https://github.com/zeronsh/zeron) (MIT, Rust and GPUI) at
[OpenAgentsInc/coder-desktop](https://github.com/OpenAgentsInc/coder-desktop),
wired to our Computers and payments SDKs. It's also the first app built on
the [OpenAgents SDK](../sdk/README.md) from outside this repository.

- [plan.md](plan.md): the integration plan. Ownership rules, the public
  contracts it uses, stages D0–D6 and their exit criteria.
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

## Local setup

The clone sits next to this repository:

```sh
cd ~/work
git clone https://github.com/OpenAgentsInc/coder-desktop.git
cd coder-desktop
git remote add upstream https://github.com/zeronsh/zeron.git
```

Build and open an isolated development bundle on macOS:

```sh
scripts/run-macos-dev.sh
```

The script comes from upstream. It builds `cargo build -p zeron` (debug) and
wraps it in `target/macos-dev/Zeron Dev.app`, with its own bundle ID
(`sh.zeron.app.dev`), data directory (`target/macos-dev/data`) and engine
IPC port (49777). A real Zeron install can stay open beside it. Set
`ZERON_DEV_BUILD_ONLY=1` to build without opening it.

Until stage D0 renames the app (its own bundle ID, data root, URL scheme and
update channel), the dev bundle is still named "Zeron Dev".

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
- **Desktop app work:** issues in
  [OpenAgentsInc/coder-desktop](https://github.com/OpenAgentsInc/coder-desktop).
- **The Coder 1.0 freeze** ([1.0-freeze](../coder/1.0-freeze.md)) covers the
  `coder`, `openagents`, `microcoder` and `coder-boundary` programs. Coder
  Desktop is a separate app and isn't frozen.
