# OpenAgents

OpenAgents is a desktop and mobile app for coding agents, chats, files, Git,
pull requests, terminals, and browser previews. It is based on
[Zeron](https://github.com/zeronsh/zeron) by Wing, MIT. The original copyright
notices remain in `LICENSE` and `THIRD_PARTY_NOTICES.md`.

Desktop and phone share OpenAgents sign-in and the sync service. Build and
release instructions are in [OPENAGENTS.md](OPENAGENTS.md). Upstream fixes are
ported manually; [FORK.md](FORK.md) records the upstream revision and identity
changes.

## Build and run (macOS)

You need a recent stable Rust (1.98.1 worked on 2026-10-15; the GPUI fork uses
`std::hint::cold_path`, which older toolchains reject).

```sh
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents/apps/openagents
scripts/run-macos-dev.sh
```

The script builds `cargo build -p openagents` and opens
`target/macos-dev/OpenAgents Dev.app`, bundle ID `com.openagents.desktop.dev`, with its data in `target/macos-dev/data`
and the engine on port 49777. Set `OPENAGENTS_DEV_BUILD_ONLY=1` to build without opening it.
`scripts/package-macos.sh` builds `OpenAgents.app` and a `.dmg`.

Only macOS has been checked so far; see the platform matrix in
[FORK.md](FORK.md).

## How it works

[ARCHITECTURE.md](ARCHITECTURE.md) describes the engine, UI and protocol, as
inherited from OpenAgents.

## License

Licensed under the [MIT License](LICENSE). OpenAgents's copyright notice stays in
[LICENSE](LICENSE), and third-party notices are in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
