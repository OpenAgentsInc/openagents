# `@openagentsinc/pylon`

Bootstrap the latest tagged standalone `Pylon` release asset from GitHub
Releases, fall back to a deterministic source build when no matching asset
exists for the local platform, stream first-run status updates in the terminal,
and start the Pylon terminal UI without Cargo when prebuilt binaries are
available. The terminal UI manages the long-lived earning worker.

## Usage

```bash
npx @openagentsinc/pylon
bunx @openagentsinc/pylon
npm install -g @openagentsinc/pylon && pylon
bun install -g @openagentsinc/pylon && pylon
npx @openagentsinc/pylon --version 0.1.12
npx @openagentsinc/pylon --no-launch
npx @openagentsinc/pylon --download-curated-cache --model gemma-4-e2b --run-diagnostics
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
- emits best-effort anonymous installer telemetry to `openagents.com` so the
  public stats page can show install starts, completions, source-build fallbacks,
  Rust prompts, and smoke-test outcomes
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
- skips Gemma diagnostics by default because hosted homework training does not
  require local Gemma weights
- only runs `pylon gemma diagnose <model> --json` when `--run-diagnostics` is
  set
- only runs `pylon gemma download <model>` when `--download-curated-cache` is
  set, because the optional GGUF cache does not satisfy the sellable runtime by
  itself
- falls back to `curl` for release metadata and asset downloads when the Node
  fetch path fails in constrained network contexts
- starts the installed `pylon-tui` by default after the smoke path; that TUI
  starts and supervises the earning worker
  unless `--no-launch` is set
- for hosted homework/training work, use `0.1.12` or newer. That release fixes
  Mac training-worker launch by preferring a current
  `target/release/psionic-train` binary and falling back to
  `cargo run --release` instead of debug `cargo run`
- does not try to install or register a local runtime automatically; the
  bootstrap stays honest about the separate Ollama-compatible runtime
  prerequisite instead of mutating the host behind the user's back

Set `OPENAGENTS_DISABLE_TELEMETRY=1` to disable installer telemetry, or
`OPENAGENTS_TELEMETRY_URL=http://127.0.0.1:8000/api/telemetry/events` to point
the launcher at a non-production telemetry endpoint.

## Publish

Publish directly from this package directory:

```bash
cd packages/pylon-bootstrap
npm pack --dry-run
npm publish
```
