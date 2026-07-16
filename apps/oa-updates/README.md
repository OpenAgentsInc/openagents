# OpenAgents Updates

Desktop ReleaseSet v2 candidate publication, atomic promotion, retention, key
rotation, and recovery are documented in
[`docs/release-set-v2-feed-runbook.md`](docs/release-set-v2-feed-runbook.md).

`oa-updates` is the owned Expo Updates-compatible OTA and signed-release
service behind `updates.openagents.com`. OpenAgents Mobile consumes its manifest
endpoint and publish script; OpenAgents Desktop and Pylon consume its separate
signed release feeds. It is an active release boundary, not archival code.

## Verification

From this directory:

```sh
pnpm run typecheck
pnpm run test
```

The strict, no-emit test project covers production source and mechanically
proves that every TypeScript test/spec file is a project root. The canonical
typecheck also compiles a valid update fixture, removes its required
`id`, and proves that TypeScript rejects the broken fixture before the native
Expo Updates client could receive an invalid manifest. The
project currently has no diagnostic baseline: all surfaced errors are fixed.

OpenAgents Mobile owns its client-side update polling and identity tests under
`apps/openagents-mobile`; this service owns manifest resolution, signing,
assets, release seeding, and protocol responses.
