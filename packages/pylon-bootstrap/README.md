# `@openagentsinc/pylon`

Bootstrap the latest tagged standalone `Pylon` release asset from GitHub
Releases, fall back to a deterministic source build when no matching asset
exists for the local platform, stream first-run status updates in the terminal,
and open the Pylon terminal UI without Cargo when prebuilt binaries are
available.

## Usage

```bash
npx @openagentsinc/pylon
bunx @openagentsinc/pylon
npm install -g @openagentsinc/pylon && pylon
bun install -g @openagentsinc/pylon && pylon
npx @openagentsinc/pylon --version 0.0.1-rc5
npx @openagentsinc/pylon --no-launch
npx @openagentsinc/pylon --download-curated-cache --model gemma-4-e2b --diagnostic-repeats 2
npx @openagentsinc/pylon --verbose
```

The launcher:

- supports direct `npx` / `bunx` execution plus global `npm install -g` /
  `bun install -g` installs with the same `pylon` command
- checks GitHub for the latest tagged `pylon-v...` release on each default run,
  or resolves a specific tagged `Pylon` version when `--version` is provided
- resolves the correct `pylon-v<version>-<os>-<arch>.tar.gz` asset for the
  current machine
- falls back to the exact tagged source checkout and builds `pylon` plus
  `pylon-tui` locally when no matching release asset exists for the machine
- prompts before installing the Rust toolchain via `rustup` if a source build
  is needed and `cargo` / `rustc` are missing
- downloads the archive and published SHA-256 checksum
- verifies the checksum before extracting
- caches the unpacked binaries under `~/.openagents/pylon/bootstrap/`
- never links or copies those cached standalone binaries into a shared global
  bin directory, so the package-managed `pylon` launcher remains the command on
  `PATH`
- prints status lines such as release resolution, runtime checks, and local
  model scanning while it runs
- ends first run with an explicit verdict such as `fully online`, `runtime
  ready`, or `installed but runtime missing`, plus exact next-step guidance
- runs `pylon --help`, `init`, `status --json`, and `inventory --json`
- runs `pylon gemma diagnose <model> --json`
- only runs `pylon gemma download <model>` when `--download-curated-cache` is
  set, because the optional GGUF cache does not satisfy the sellable runtime by
  itself
- falls back to `curl` for release metadata and asset downloads when the Node
  fetch path fails in constrained network contexts
- opens `pylon-tui` by default after the smoke path unless `--no-launch` is set
- does not try to install or register a local runtime automatically; the
  bootstrap stays honest about the separate Ollama-compatible runtime
  prerequisite instead of mutating the host behind the user's back

## Publish

Publish directly from this package directory:

```bash
cd packages/pylon-bootstrap
npm pack --dry-run
npm publish
```
