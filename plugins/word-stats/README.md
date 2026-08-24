# word-stats

Pure-computation guest plugin: text statistics in, JSON out, no imports.
Built on `openagents-pdk` (see `../pdk/`), which owns the whole `packet-v0`
ABI — this crate is one typed `handle` function and a `plugin_entry!`
invocation. The built artifact `word_stats.wasm` and its digest pin in
`manifest.json` are checked in so the plugin runs without a Rust toolchain.

Load it into a coder session:

```sh
# from packages/openagents-cli, after pnpm build
printf '/plugin load ../../plugins/word-stats/manifest.json\ncount the words in: hello hello world\n' \
  | node dist/main.js coder --plain
```

Rebuild from `plugins/` (then update `artifact.digest` in `manifest.json`):
see `../README.md`. Built with rustc 1.94.1 targeting
`wasm32-unknown-unknown`.
