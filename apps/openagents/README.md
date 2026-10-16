# Coder Desktop

Coder Desktop (the app is called **Coder**) is OpenAgents' desktop app for
coding agents: Claude Code, Codex, Cursor, Devin, Grok, Hermes, Pi and
Antigravity, run locally from one workspace with chats, files, diffs, Git,
pull requests, terminals and browser previews.

Based on Zeron by Wing, MIT: Coder Desktop is a fork of
[Zeron](https://github.com/zeronsh/zeron). We keep Zeron as the `upstream`
remote and merge it often; [FORK.md](FORK.md) lists what we changed and how
to merge.

Coder Desktop works locally without an account. It doesn't use Zeron's hosted
sign-in, sync or update feed. OpenAgents sign-in and Computers come next
([openagents#11824](https://github.com/OpenAgentsInc/openagents/issues/11824)).
Downloads and docs: [openagents.com/download](https://openagents.com/download)
and [openagents.com/docs](https://openagents.com/docs).

## Build and run (macOS)

You need a recent stable Rust (1.98.1 worked on 2026-10-15; the GPUI fork uses
`std::hint::cold_path`, which older toolchains reject).

```sh
git clone https://github.com/OpenAgentsInc/coder-desktop.git
cd coder-desktop
git remote add upstream https://github.com/zeronsh/zeron.git
RUSTUP_TOOLCHAIN=1.98.1 scripts/run-macos-dev.sh
```

The script builds `cargo build -p zeron` (the binary crate keeps its upstream
name) and opens `target/macos-dev/Coder Dev.app`, bundle ID
`com.openagents.coder.desktop.dev`, with its data in `target/macos-dev/data`
and the engine on port 49777. Set `ZERON_DEV_BUILD_ONLY=1` to build without opening it.
`scripts/package-macos.sh` builds `Coder.app` and a `.dmg`.

Only macOS has been checked so far; see the platform matrix in
[FORK.md](FORK.md).

## How it works

[ARCHITECTURE.md](ARCHITECTURE.md) describes the engine, UI and protocol, as
inherited from Zeron.

## License

Licensed under the [MIT License](LICENSE). Zeron's copyright notice stays in
[LICENSE](LICENSE), and third-party notices are in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
