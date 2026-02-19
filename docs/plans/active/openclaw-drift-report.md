# OpenClaw Drift Report

Date: 2026-02-19
Generated: 2026-02-19T03:41:19Z
Upstream: https://github.com/openclaw/openclaw.git
Upstream HEAD: 6b05916c1445d5afb348de3c54d28bb46ccabda1

## Classification

- `in_sync`: pinned SHA matches upstream HEAD
- `upstream_head_mismatch`: pinned SHA differs from upstream HEAD
- `missing_pin`: intake/fixture has no exact pinned SHA
- `invalid_sha`: value is not a valid 40-char SHA

## Capability Drift Table

| Capability | Pinned SHA | Upstream HEAD | Drift Type | Recommended Action |
|---|---|---|---|---|
| comms-tool-pack-resend-v1 | pending | 6b05916c1445d5afb348de3c54d28bb46ccabda1 | missing_pin | Pin exact upstream SHA in intake record and add/refresh parity fixture coverage. |
| openclaw-tool-policy-fixtures | 8e1f25631b220f139e79003caecabd11b7e1e748 | 6b05916c1445d5afb348de3c54d28bb46ccabda1 | upstream_head_mismatch | Review upstream diff vs pinned SHA, refresh fixtures/parity tests, and open re-ingestion issue if behavior changed. |

## Next Step Rule

For any `upstream_head_mismatch` or `missing_pin` row, open/update an ingestion issue that includes:
1. Diff scope summary (upstream vs pinned SHA)
2. Fixture/parity impact
3. Port/adapt/adopt decision
4. Rollout risk and test updates
