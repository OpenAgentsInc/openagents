# word-stats

The demo guest plugin for the coder plugin walking demo. Pure computation:
text statistics in, JSON out, no imports. The built artifact
`word_stats.wasm` and its digest pin in `manifest.json` are checked in so
the demo runs without a Rust toolchain.

Load it into a coder session:

```sh
# from packages/openagents-cli, after pnpm build
printf '/plugin load ../../plugins/word-stats/manifest.json\ncount the words in: hello hello world\n' \
  | node dist/main.js coder --plain
```

Rebuild the artifact (then update `artifact.digest` in `manifest.json`):

```sh
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/word_stats.wasm word_stats.wasm
shasum -a 256 word_stats.wasm
```

Built with rustc 1.94.1, no dependencies, so the build needs no network.
The ABI and what the real skeleton replaces are recorded in
`docs/plugins/2026-08-24-coder-plugin-demo-shape.md`.
