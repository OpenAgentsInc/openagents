# OpenClaw Drift Report

Date: 2026-02-19
Generated: 2026-02-19T09:34:54Z
Upstream: https://github.com/openclaw/openclaw.git
Upstream HEAD: f38e1a8d8260ea9c60568f5cac087144931be46c

## Classification

- `in_sync`: pinned SHA matches upstream HEAD
- `upstream_head_mismatch`: pinned SHA differs from upstream HEAD
- `missing_pin`: intake/fixture has no exact pinned SHA
- `invalid_sha`: value is not a valid 40-char SHA

## Drift Summary

- In sync: 0
- Upstream head mismatch: 1
- Missing pin: 1
- Invalid SHA: 0
- Actionable rows: 2

## Capability Drift Table

| Capability | Pinned SHA | Upstream HEAD | Drift Type | Recommended Action |
|---|---|---|---|---|
| comms-tool-pack-resend-v1 | pending | f38e1a8d8260ea9c60568f5cac087144931be46c | missing_pin | Pin exact upstream SHA in intake record and add/refresh parity fixture coverage. |
| openclaw-tool-policy-fixtures | 8e1f25631b220f139e79003caecabd11b7e1e748 | f38e1a8d8260ea9c60568f5cac087144931be46c | upstream_head_mismatch | Review upstream diff vs pinned SHA, refresh fixtures/parity tests, and open re-ingestion issue if behavior changed. |

## Next Step Rule

For any `upstream_head_mismatch` or `missing_pin` row, open/update an ingestion issue that includes:
1. Diff scope summary (upstream vs pinned SHA)
2. Fixture/parity impact
3. Port/adapt/adopt decision
4. Rollout risk and test updates

## Actionable Follow-ups

- Capability: `comms-tool-pack-resend-v1`
  - Drift type: `missing_pin`
  - Pinned SHA: `pending`
  - Action: Pin exact upstream SHA in intake record and add/refresh parity fixture coverage.
  - Suggested issue command:
    `gh issue create --title "[OpenClaw Drift] comms-tool-pack-resend-v1 (missing_pin)" --label planning --body "Drift detected by scripts/openclaw-drift-report.sh on 2026-02-19.\n\nPinned SHA: pending\nUpstream HEAD: f38e1a8d8260ea9c60568f5cac087144931be46c\n\nAction: Pin exact upstream SHA in intake record and add/refresh parity fixture coverage." `
- Capability: `openclaw-tool-policy-fixtures`
  - Drift type: `upstream_head_mismatch`
  - Pinned SHA: `8e1f25631b220f139e79003caecabd11b7e1e748`
  - Action: Review upstream diff vs pinned SHA, refresh fixtures/parity tests, and open re-ingestion issue if behavior changed.
  - Suggested issue command:
    `gh issue create --title "[OpenClaw Drift] openclaw-tool-policy-fixtures (upstream_head_mismatch)" --label planning --body "Drift detected by scripts/openclaw-drift-report.sh on 2026-02-19.\n\nPinned SHA: 8e1f25631b220f139e79003caecabd11b7e1e748\nUpstream HEAD: f38e1a8d8260ea9c60568f5cac087144931be46c\n\nAction: Review upstream diff vs pinned SHA, refresh fixtures/parity tests, and open re-ingestion issue if behavior changed." `
