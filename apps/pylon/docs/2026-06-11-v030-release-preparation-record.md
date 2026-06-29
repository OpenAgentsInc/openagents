# Pylon 0.3.0 Release Preparation Record

Date: 2026-06-11 (~03:20 UTC)
Issue: #4662 (stable 0.3.0 release)

## State

- Current package version: `0.3.0-rc2` (tagged, agent-economy sprint).
- Full release gate run on macOS at rc2 on 2026-06-11 ~02:40 UTC:
  **passed** — unit/runtime tests, bootstrap/status/inventory/operator JSON
  smokes, dashboard startup smoke, `bun pm pack --dry-run`, and the local
  package install smoke (which installs the packed tarball with
  `@openagentsinc/nip90` and `@openagentsinc/tassadar-executor` tarball overrides).
  Output: `release gate passed`.
- Gate policy: manual, script-based, no hosted CI — owner decision recorded
  2026-06-10 (#4654, `release-install-smokes.md`). Run the gate on macOS and
  Linux before tagging.
- Platforms: macOS and Linux. Windows/WSL strongly deprioritized by owner
  decision (registry `2026-06-10.26`).

## What blocks the actual release

One owner action: a valid npm credential with publish rights to the
`@openagentsinc` and `@openagents` scopes. Both locally available tokens
returned 401 on `GET /-/whoami` as of 2026-06-11 02:30 UTC.

## Release runbook (when the credential exists)

1. `npm login` (or place a granular automation token in `~/.npmrc`).
2. Publish the workspace deps first (consumers of the published package
   otherwise 404 on `workspace:*` resolution):
   - `packages/nip90` → `@openagentsinc/nip90` (publishConfig public, ready)
   - `packages/tassadar-executor` → `@openagentsinc/tassadar-executor`
     (publishConfig public, ready)
3. In `apps/pylon/package.json`: version `0.3.0-rc2` → `0.3.0`, and replace
   the `workspace:*` dep specifiers with the published versions at publish
   time (or use a publish-time rewrite).
4. Re-run `bash scripts/release-gate.sh`; retain output.
5. `npm publish` from `apps/pylon`.
6. Verify `npm install -g @openagentsinc/pylon@0.3.0` on a clean machine
   (macOS and Linux).
7. Write the release record (version, platforms, gate evidence) and update
   `launch-gates-no-overclaim.md` allowed copy — the blocked-copy list stays
   intact for everything still ungated (assignment-ready network, paid
   settlement broadly, training, marketplace claims).
8. Record the `pylon_v03_stable_release_not_green` clear transition
   receipt-first, then bump the registry (one bump per promise flip,
   serialized).

## What this release will NOT claim

- No automatic Bitcoin earning, no self-serve paid work, no
  assignment-ready-network claims (earning gate copy governs).
- No training, local-inference, or Qwen claims — local-inference and Qwen
  products are out of scope by owner decision (2026-06-10); the training
  focus is Tassadar executor training.
- Tassadar executor-trace copy stays scoped to the promise's safeCopy (one
  workload family, digest-pinned, receipts cited).
