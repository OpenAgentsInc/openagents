# Pylon v0.3 Release And Install Smokes

Pylon v0.3 release candidates are published as `@openagentsinc/pylon` with the
`pylon` binary. The first candidate version is `0.3.0-rc1`; do not publish or
document `0.3.0` as stable until the launch gates pass.
The first operator platforms are macOS and Linux only.

## Local macOS Smoke

Run from the repo root:

```sh
bun run smoke:install:local
```

This packs the current repo, installs the tarball into a clean temporary Bun
project, and verifies:

- the package installs as `@openagentsinc/pylon`;
- the `pylon` binary resolves;
- `pylon bootstrap --json` emits a supported macOS/Linux summary;
- the summary points at the v0.3 home/config/cache/release layout.

## Linux CI Smoke

The same smoke is ready for an `ubuntu-latest` CI runner. The current GitHub
token cannot create workflow files, so the reproducible command sequence is
documented here until a workflow-scope token wires it:

```sh
bun install
bun run test
bun pm pack --dry-run
bun run smoke:install:local
```

The smoke is intentionally package-install based. It does not rely on the old
v0.2 launcher or deprecated OpenAgents Rust Pylon implementation homes.

## Bootstrap Surface

The launch-safe automation entry points are:

```sh
pylon bootstrap --json
pylon bootstrap --register-openagents --setup-mdk-wallet --pylon-ref <ref> --display-name <name> --resource-mode background_20 --capability-ref <ref> --json
pylon status --json
```

`bootstrap` creates the local v0.3 home/cache/release directories and writes a
minimal public-safe config summary. Registration, MDK wallet execution, and
live endpoint mutation remain separately gated by later launch issues.
`status --json` emits the persisted local identity and runtime state through
the public projection guard.

Source-build fallback is disabled for v0.3 launch. Release/update discovery is
defined as GitHub Releases polling while the dashboard is open.
