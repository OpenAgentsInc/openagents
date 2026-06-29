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

## Release Gate (manual, script-based — no hosted CI by owner decision)

Owner decision (2026-06-10): OpenAgents does not connect this repo to GitHub
Actions or any hosted CI. The release gate is custom and script-based, run
manually (or by an agent) before any release-bearing change:

```sh
# from apps/pylon, on macOS and on Linux
bun install --frozen-lockfile
bun run test
bun pm pack --dry-run
bun run smoke:install:local
```

Run the sequence on both platforms before tagging a release; keep the
captured output as gate evidence in the release record. The smoke is
intentionally package-install based. It does not rely on the old v0.2
launcher or deprecated OpenAgents Rust Pylon implementation homes.

The fuller local release gate is still:

```sh
bun run release:gate
```

That local gate runs unit/runtime tests, bootstrap/status/inventory/operator
JSON smokes, dashboard startup smoke, package dry-run, and the local package
install smoke before a release candidate is treated as launchable.

## npm-without-bun Consumer Smoke (REQUIRED before publish)

The local smoke above installs with **bun**, which by default **blocks**
dependency lifecycle scripts (`postinstall`/`prepare`). Larry's clean Ubuntu
x64 box hit a launch-blocking bug the bun smoke could never catch: a plain
`npx @openagentsinc/pylon` crashed during install with

```
sh: bun: command not found
npm error code 127   (git dep preparation failed)
```

Root cause: `@openagentsinc/nip90` depends on `nostr-effect` as a **git
dependency** (`github:OpenAgentsInc/nostr-effect#<sha>`). npm runs the
`prepare` lifecycle script for git deps on consumer install (it does NOT for
registry tarballs). `nostr-effect`'s old `prepare` ran `bun run setup:hooks`,
which hard-required bun. The working path the tester found was: install bun,
then `bunx @openagentsinc/pylon` (bun blocks the offending script).

Fixed 2026-06-18 by guarding `nostr-effect`'s `prepare`
(`scripts/prepare.mjs`, Node-only) so it no-ops on consumer/git-dep installs
and when bun is absent, and repinning `nip90` to the fixed
`nostr-effect#4c52847`. **This ships only after `@openagentsinc/nip90` and
`@openagentsinc/pylon` are republished** (see npm-publishing-runbook.md);
the registry copies of nip90 0.1.0 / pylon 1.0.3 still carry the old pin.

To prevent recurrence, run this **npm + no-bun** smoke before any publish, on
a box (or PATH) without bun:

```sh
# bun must NOT be on PATH for this smoke to be meaningful
command -v bun && echo "REMOVE bun from PATH first" && exit 1
mkdir -p /tmp/pylon-npm-smoke && cd /tmp/pylon-npm-smoke
npm init -y >/dev/null
npm install @openagentsinc/pylon@<version>   # must exit 0, no code 127
npx --no-install @openagentsinc/pylon --version   # or bootstrap --json
```

A non-zero exit, `code 127`, or `git dep preparation failed` means a
transitive dependency is running a bun-requiring (or otherwise failing)
lifecycle script on consumer install. Never publish until this passes.

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
