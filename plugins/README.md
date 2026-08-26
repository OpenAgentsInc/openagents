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
- `read-conversation/` — the content half (OpenAgentsInc/openagents#41):
  locates one session through the scanner's library and reads its
  conversation back as ordered turns, bounded by turn and character
  ceilings. The first plugin over `openagents.read_file_range`, the
  bounded range import that reads the tail of a session file past the
  whole-file bound. `foreign-sessions` gates its packet entry behind the
  default `entry` feature so this crate can depend on the scan logic
  without a duplicate-export link error.
- `knowledge-base/` — the retrieval rail rather than a tool: a corpus of
  reviewed stances and public-doc summaries compiled into the artifact at
  build, which the harness queries on every turn and attaches as a bracketed
  note. It declares no mounts and no hosts, and it is never offered to a model
  as a tool. `knowledge-base/README.md` covers the corpus, the rebuild, and how
  a system memory is promoted into a stance.

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
