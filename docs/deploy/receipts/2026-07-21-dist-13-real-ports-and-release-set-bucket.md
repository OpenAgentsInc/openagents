# DIST-13 real release ports + release-set GCS bucket

- Date: 2026-07-21
- Program: [#8913](https://github.com/OpenAgentsInc/openagents/issues/8913)
- Issues: [DIST-04 #8917](https://github.com/OpenAgentsInc/openagents/issues/8917),
  [DIST-09 #8922](https://github.com/OpenAgentsInc/openagents/issues/8922),
  [DIST-13 #8926](https://github.com/OpenAgentsInc/openagents/issues/8926)
- Landed on `main`: release ports `4dce465a97`, STE baseline `3b001cfa30`,
  audit refresh in the same push range `b222fdc8fb..0e51113025`.
- Related runbook:
  [`../2026-07-20-owned-infra-cross-platform-desktop-build-runbook.md`](../2026-07-20-owned-infra-cross-platform-desktop-build-runbook.md)

## Status

This record documents two things a later agent or the owner must be able to
find: the real `pnpm run release` integration ports (which replaced the
dry-run-only fixture skeleton), and a new production Google Cloud Storage
bucket that backs the public ReleaseSet v2 feed. No release was promoted. The
owner-gated clean-machine acceptance and channel-pointer promotion stay open by
design (see the boundary below).

## Release command ports (real)

Before this change, `scripts/release.ts` `main()` selected fixture ports, so a
real (non `--dry-run`) run was refused fail-closed. `main()` now calls
`createReleasePorts(plan, io)` from `scripts/release-ports-real.ts`:

- `--dry-run` keeps the fixture ports; no infrastructure is touched.
- A real run builds the real ports:
  - `createRealCoordinatorPort` assembles the already-landed
    `createOwnedReleaseCoordinator` with concrete adapters: an ed25519 request
    signer loaded from `.secrets`/environment (never printed), a worker
    keyring, a Google Cloud Storage immutable-object head over
    `gcloud storage`, a `FileCoordinatorStateStore`, and the owned four-target
    build-host inventory from the cross-platform build runbook.
  - `createRealFeedPort` deploys the candidate feed through
    `apps/oa-updates/scripts/deploy-cloudrun.sh` with the release-set bucket
    and pins environment, probes mobile over-the-air preservation, and verifies
    the served, ed25519-signed ReleaseSet v2 plus the `/download` resolver.

All effects (shell, Google Cloud Storage, HTTP) are injectable, and the wiring
is unit-tested in `scripts/release-ports-real.test.ts` (port selection, feed
port against injected effects, coordinator owner-gate refusal, and the happy
four-target inventory bind once every host is attested). A `--dry-run` run
completes clean through all nine transaction steps.

## Release-set Google Cloud Storage bucket (new production infrastructure)

The public ReleaseSet v2 feed reads candidates and the channel pointer from a
dedicated bucket. That bucket did not exist and the `oa-updates` Cloud Run
service had no `OA_RELEASE_SET_BUCKET` value. Provisioned with the
`oa-mvp-automation` service account (holds `storage.admin`):

| Property | Value |
| --- | --- |
| Bucket | `openagentsgemini-oa-updates-release-set` |
| Project | `openagentsgemini` |
| Location | `us-central1` |
| Uniform bucket-level access | enabled |
| Public access prevention | enforced |

The pinned public verification key the `oa-updates` image mounts at
`OA_RELEASE_SET_PINS_PATH` is now committed at
`apps/oa-updates/openagents-desktop-dist/release-set-pins.json` (ed25519, key
id `2dbe811d19f67528`, matching `apps/oa-updates/keys/release-pubkey.json`).

Object layout the feed and the coordinator use in this bucket:

- Candidate: `desktop/release-set-v2/<channel>/candidates/<generation>.json`
  (immutable, create-if-absent).
- Channel pointer: `desktop/release-set-v2/<channel>/pointer.json` (mutable,
  compare-and-swap on the object generation).

The `oa-updates` Cloud Run service is not yet reading the bucket. Wiring
`OA_RELEASE_SET_BUCKET` and `OA_RELEASE_SET_PINS_PATH` into the live service
happens through the feed port deploy step (`deployCandidateFeed`) during a real
release run. Until a candidate is written and the pointer is promoted, the v2
paths return not-found and existing packaged Desktops keep resolving the v1
manifest paths, so wiring the bucket does not change current client behavior.

## Boundary — what stays owner-gated

A valid worker receipt must carry the nine native-proof references (clean
install, launch, agent runtime, shutdown, update, interruption resume, rollback
or no-rollback, reinstall, uninstall). Those proofs are the DIST-12
([#8925](https://github.com/OpenAgentsInc/openagents/issues/8925)) clean-machine
acceptance. Until the owner attests a target's native-acceptance host, its
inventory row keeps an `unavailable:` reference and the coordinator fails closed
at inventory bind with `worker_inventory_unavailable`. So a real
`pnpm run release` surfaces the owner gate through the tool itself; it never
promotes a channel pointer without owner acceptance.

The remaining owner-gated steps are therefore: complete DIST-12 clean-machine
install / update / rollback acceptance so the native-acceptance hosts can be
attested, then run a real release candidate that drives the attested build path
and the atomic channel-pointer promotion. The first stable promotion also
requires an explicit `--approve first_stable_promotion` owner gate; release
candidate promotion is safe for an unattended `--yes` run once acceptance
exists.
