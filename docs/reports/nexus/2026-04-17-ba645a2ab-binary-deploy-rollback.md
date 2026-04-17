## 2026-04-17 Binary Deploy Rollback For `ba645a2abfbb`

Attempted binary-first activation of `ba645a2abfbbd4a12a1bafb3c9350d82e2fc67ed`
on `nexus-mainnet-1`.

Retained receipts:

- build: `docs/reports/nexus/20260417-170609-warm-builder-build-ba645a2abfbb.json`
- upload: `docs/reports/nexus/20260417-170841-binary-release-upload-ba645a2abfbb.json`
- activate: `docs/reports/nexus/20260417-171140-binary-release-activate-ba645a2abfbb.json`

Observed verify-gate failure:

- local verification probes against `/healthz`, `/api/training/rollout`, and
  `/v1/treasury/status` intermittently exceeded the 20 second verifier timeout
  on the activated release
- the same local timeout pattern reproduced again after rollback on the prior
  release, so the stalled probes were not unique to `ba645a2abfbb`
- before rollback, a successful treasury status sample still showed degraded
  continuity health with `confirmations_stalled`

Rollback outcome:

- `current` restored to
  `/opt/nexus-relay/releases/52d133c590719b39eccf3e6b93030fa77b65ecde`
- `previous` now points at
  `/opt/nexus-relay/releases/ba645a2abfbbd4a12a1bafb3c9350d82e2fc67ed`
- `systemctl` returned `active` after the rollback restart

Operator note:

- the first rollback invocation completed remotely but the local receipt writer
  in `scripts/deploy/nexus/15-rollback-binary-release.sh` crashed when it tried
  to embed JSON `null` directly into Python source
- that script bug was fixed immediately afterward, but this rollback does not
  have an auto-generated JSON rollback receipt because of the post-action crash
