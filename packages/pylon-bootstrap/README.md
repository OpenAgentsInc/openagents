# `@openagentsinc/pylon`

Bootstrap the latest tagged standalone `Pylon` release asset from GitHub
Releases and run the first-run smoke path without Cargo.

## Usage

```bash
npx @openagentsinc/pylon
npx @openagentsinc/pylon --version 0.1.0
npx @openagentsinc/pylon --model gemma-4-e2b --diagnostic-repeats 2
```

The launcher:

- resolves the latest tagged `pylon-v...` release by default, or a specific
  tagged `Pylon` version when `--version` is provided
- resolves the correct `pylon-v<version>-<os>-<arch>.tar.gz` asset for the
  current machine
- downloads the archive and published SHA-256 checksum
- verifies the checksum before extracting
- caches the unpacked binaries under `~/.openagents/pylon/bootstrap/`
- runs `pylon --help`, `init`, `status --json`, and `inventory --json`
- runs `pylon gemma download <model>`
- runs `pylon gemma diagnose <model> --json`

## Publish

Publish directly from this package directory:

```bash
cd packages/pylon-bootstrap
npm publish
```
