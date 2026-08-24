# file-stats

The read-only-mount proof plugin. Its manifest declares one mount
(`data/`, relative to the manifest, `readonly: true`), so the host exposes
exactly one capability import — `openagents.read_file` — and confines every
path the guest asks for: relative paths only, no `..` escape, no symlinks,
a per-file size bound. The guest measures one mounted file: byte count,
UTF-8 or not, line count.

The escape-attempt tests live in
`packages/openagents-cli/test/coder-plugin-mounts.test.ts`.

Load it into a coder session:

```sh
# from packages/openagents-cli, after pnpm build
printf '/plugin load ../../plugins/file-stats/manifest.json\nmeasure sample.txt with file_stats\n' \
  | node dist/main.js coder --plain
```

Rebuild from `plugins/` (then update `artifact.digest` in `manifest.json`):
see `../README.md`. Built with rustc 1.94.1 targeting
`wasm32-unknown-unknown`.
