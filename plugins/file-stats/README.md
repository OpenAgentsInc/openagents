# file-stats

The read-only-mount proof plugin. Its manifest declares one mount
(`data/`, relative to the manifest, `readonly: true`), so the host exposes
exactly one capability import — `openagents.read_file` — and confines every
path the guest asks for: relative paths only, no `..` escape, no symlinks,
a per-file size bound. The guest measures one mounted file: byte count,
UTF-8 or not, line count.

The escape-attempt tests live in
`crates/openagents-cli/tests/plugin_host_test.rs`.

Load it into a coder session:

```sh
printf 'Use the file_stats capability to measure sample.txt\n' \
  | openagents coder --plain
```

Rebuild from `plugins/` (then update `artifact.digest` in `manifest.json`):
see `../README.md`. Built with rustc 1.94.1 targeting
`wasm32-unknown-unknown`.
