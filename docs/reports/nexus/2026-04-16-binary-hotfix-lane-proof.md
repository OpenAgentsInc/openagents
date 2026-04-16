## Nexus Binary Hotfix Lane Proof

Date: 2026-04-16

This document records the first retained bounded proof that the warm-builder
binary-first Nexus hotfix lane works end to end.

Scope:

- builder: `nexus-builder-1`
- bounded target VM: `nexus-hotfix-lane-1`
- public checks: intentionally skipped on the bounded lane with
  `VERIFY_PUBLIC_CHECKS_ENABLED=false`
- websocket probe target for local verification:
  `wss://nexus-hotfix-lane.internal/`

The hotfix lane VM started the run on the older Docker/image unit. The proof
therefore exercised both:

- first binary activation from image mode with image-unit backup retained
- normal binary-to-binary rollback through `current` and `previous`

## Retained Receipts

Build receipts:

- `docs/reports/nexus/20260416-025517-warm-builder-build-6cc56613486f.json`
- `docs/reports/nexus/20260416-031226-warm-builder-build-6cc56613486f.json`
- `docs/reports/nexus/20260416-032403-warm-builder-build-509300ad0d8a.json`

Upload / activate / rollback receipts:

- `docs/reports/nexus/20260416-033135-binary-release-upload-6cc56613486f.json`
- `docs/reports/nexus/20260416-033312-binary-release-activate-6cc56613486f.json`
- `docs/reports/nexus/20260416-033542-binary-release-upload-509300ad0d8a.json`
- `docs/reports/nexus/20260416-033715-binary-release-activate-509300ad0d8a.json`
- `docs/reports/nexus/20260416-033800-binary-release-rollback-6cc56613486f.json`

Verification receipts:

- `docs/reports/nexus/20260416-033506-deploy-receipt.json`
- `docs/reports/nexus/20260416-033728-deploy-receipt.json`
- `docs/reports/nexus/20260416-033906-deploy-receipt.json`

## Sequence

1. Reused the retained builder artifact for `6cc56613486f79ae87608ea61ea763a699547cd8`.
2. Built `509300ad0d8ab8fee310d97c1dbeeae8e11ecc40` on the warm builder to get a
   second release ID.
3. Uploaded `6cc566...` to `/opt/nexus-relay/releases/6cc566...`.
4. Activated `6cc566...`, which converted the VM from image mode to binary mode
   and preserved `/etc/systemd/system/nexus-relay.service.image-backup`.
5. Verified the bounded lane with the expected release SHA pinned.
6. Uploaded `509300...` and activated it.
7. Verified `509300...` with the expected release SHA pinned.
8. Rolled back to `6cc566...` through the `previous` symlink.
9. Re-verified `6cc566...` after rollback.

## Timing Summary

Builder:

- cold `6cc566...` build: `1011166 ms` (`16m 51.166s`)
- same-SHA warm `6cc566...` repeat: `30158 ms` (`30.158s`)
- next-SHA warm `509300...` build: `140291 ms` (`2m 20.291s`)

Upload / activate / rollback:

- upload `6cc566...`: `89377 ms` total
- activate `6cc566...`: `55506 ms` total
- upload `509300...`: `88407 ms` total
- activate `509300...`: `8076 ms` total
- rollback to `6cc566...`: `7701 ms` total

Verification:

- first `6cc566...` verification passed with endpoint latencies:
  - `/healthz`: `4 ms`
  - `/api/stats`: `5 ms`
  - `/api/training/rollout`: `5 ms`
- `509300...` verification passed with endpoint latencies:
  - `/healthz`: `4 ms`
  - `/api/stats`: `4 ms`
  - `/api/training/rollout`: `4 ms`
- final post-rollback `6cc566...` verification passed in `27897 ms`

Archive compression mattered materially:

- raw `509300...` binary artifact on the builder: `766 MB`
- uploaded builder archive for `6cc566...`: `152093339 bytes`

Inference:

Even with the still-suboptimal cross-SHA warm build, the bounded binary lane
completed the build plus upload plus activate plus verify loop in well under
five minutes. That is materially better than the April 15 Cloud Build-driven
loop, which stalled past `4085 s` before the image tag even existed.

## Proof Result

The bounded proof passed.

Observed release behavior:

- `current` switched cleanly between two immutable release dirs
- `previous` was populated on the second activation and used by rollback
- the binary service stayed healthy after each restart
- the verification receipts identified the exact activated git SHA
- bounded-lane public checks were honestly marked skipped instead of faked

## Problems Found During The Proof

The proof run surfaced three operator-lane defects that were fixed immediately:

- the first upload draft moved the raw release directory over IAP twice; it now
  archives the builder artifact before transfer
- the verifier had a jq nested-array bug in its public-gate branch; it now
  emits a flat gate list again
- local temp-script creation used an unsafe `mktemp` pattern; it now uses a
  portable unique template

## What Still Needs Improvement

The hotfix lane is operationally usable now, but the proof exposed one clear
next optimization:

- cross-SHA warm builds are still too slow relative to same-SHA repeats because
  the builder stages each revision under a git-SHA-specific source root, which
  prevents Cargo from reusing as much work as it should

The medium-term structural fix remains unchanged:

- split relay-core from fast-changing training and demo-control work so routine
  relay hotfixes stop recompiling the larger graph

## End State After Proof

The bounded VM was left healthy on the rolled-back binary release:

- release dir:
  `/opt/nexus-relay/releases/6cc56613486f79ae87608ea61ea763a699547cd8`
- current symlink:
  `/opt/nexus-relay/current`
- deploy mode:
  `binary`
