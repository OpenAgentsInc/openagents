# Guest plugins

The Cargo workspace for `openagents coder` WASM plugins and the owned PDK
that authors them. The host lives in
`packages/openagents-cli/src/coder-plugins.ts` with the engine seam in
`coder-plugin-engine.ts`; the contract is issue
OpenAgentsInc/openagents#26 and
`docs/plugins/2026-08-24-coder-plugin-demo-shape.md`.

## Layout

- `pdk/` — `openagents-pdk`, the library every guest builds on. It owns the
  `packet-v0` ABI (`packet_alloc`, `handle_packet`, the JSON envelope, the
  return-word packing), the typed `Refusal` enum, and the host capability
  imports (`read_mounted_file`, `list_mounted_dir`). A plugin author
  writes one `fn handle(input) -> Result<Output, Refusal>` over serde
  types and invokes `plugin_entry!(handle)`.
- `word-stats/` — pure computation: no imports, text statistics.
- `file-stats/` — the read-only-mount proof: imports exactly
  `openagents.read_file`, which the host exposes only because its manifest
  declares a mount.
- `dir-stats/` — the listing proof: imports exactly `openagents.list_dir`,
  the second and last capability a mount declaration grants.
- `foreign-sessions/` — the first working plugin over both capabilities:
  discovers recent Claude Code and Codex CLI sessions from `~/.claude` and
  `~/.codex` mounted read-only, metadata only. The scanner half of
  OpenAgentsInc/openagents.com#198.

Each plugin's built `.wasm` artifact and its `sha256:` digest pin are
checked in beside the source, so the CLI runs them without a Rust
toolchain.

## Rebuilding artifacts

From this directory:

```sh
cargo build --release --target wasm32-unknown-unknown
for name in word_stats file_stats dir_stats foreign_sessions; do
  crate=$(printf '%s' "$name" | tr _ -)
  cp "target/wasm32-unknown-unknown/release/$name.wasm" "$crate/$name.wasm"
done
shasum -a 256 */*.wasm
```

Then update each manifest's `artifact.digest` — the host refuses a stale
pin — and rerun the plugin tests in `packages/openagents-cli`. A change
to the PDK reshuffles every artifact's bytes, so rebuild and repin all of
them together, never one alone.

The checked-in artifacts were built with rustc 1.94.1 targeting
`wasm32-unknown-unknown` (`rustup target add wasm32-unknown-unknown`),
release profile as declared in `Cargo.toml` (`opt-level = "z"`, `lto`,
`panic = "abort"`, `strip`). Dependencies are `serde` and `serde_json`
only, pinned by the checked-in `Cargo.lock`; with a warm cargo cache the
build needs no network.

This workspace is deliberately separate from the repo-root Cloud Rust
workspace: guest crates are cross-compiled artifacts, not native services.
