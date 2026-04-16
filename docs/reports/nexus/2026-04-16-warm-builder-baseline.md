# Nexus Warm Builder Baseline

Date: 2026-04-16

Revision under test:

- git sha: `6cc56613486f79ae87608ea61ea763a699547cd8`
- git short sha: `6cc56613486f`
- build profile: `fast-release`

Builder host:

- VM: `nexus-builder-1`
- zone: `us-central1-a`
- machine type: `c3d-standard-8`
- cache disk: `nexus-builder-cache-1`
- cache mount: `/mnt/disks/nexus-builder-cache`
- builder user: `nexus-builder`

Commands exercised:

```bash
scripts/deploy/nexus/11-provision-warm-builder.sh
NEXUS_BUILDER_CLEAR_CACHES=true scripts/deploy/nexus/12-build-nexus-binary.sh
scripts/deploy/nexus/12-build-nexus-binary.sh
```

Retained receipts:

- `docs/reports/nexus/20260416-025517-warm-builder-build-6cc56613486f.json`
- `docs/reports/nexus/20260416-031226-warm-builder-build-6cc56613486f.json`

## Timing

Cold-cache run:

- total duration: `1,011,166 ms` (`16m 51.166s`)
- dependency fetch: `24,484 ms`
- compile and link: `983,674 ms` (`16m 23.674s`)

Warm-cache run:

- total duration: `30,158 ms` (`30.158s`)
- dependency fetch: `430 ms`
- compile and link: `26,700 ms`

Observed delta:

- total hotfix build loop improved by `33.53x`
- compile and link phase improved by `36.84x`
- binary digest was identical across both runs:
  `02fb6a3c28db3884644279499ea80260134669c87d68b65cd2d01ec4ea2c0538`

## Notes

This is the first retained proof that Nexus binary production no longer needs
Cloud Build to produce a deployable Linux `nexus-relay` artifact.

The build log also confirms the medium-term architectural problem from the April
15 audit: routine `nexus-relay` hotfixes still compile `psionic-train` through
`nexus-control`. The warm builder hides that cost operationally, but it does
not remove the bad boundary.
