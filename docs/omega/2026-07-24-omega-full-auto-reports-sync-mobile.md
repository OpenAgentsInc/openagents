# Omega Full Auto reports, Sync, and mobile control (FA-05)

- Date: 2026-07-24
- Packet: `OMEGA-FA-05`
- Omega issue: [OpenAgentsInc/omega#24](https://github.com/OpenAgentsInc/omega/issues/24)
- Package: `@openagentsinc/omega-effectd` `0.1.0`
- Pack SHA-256: `2dec2474e2cb64acb88291beb3d5efdeef4cbd8004dfe26c0492d1f3757174a9`
- Protocol: `openagents.omega.effectd.v1` methods `get_report`, `get_receipt`,
  `apply_control_intent`, `get_sync_status`, `publish_projection`
- Freeze: [2026-07-24-full-auto-contract-freeze.md](./2026-07-24-full-auto-contract-freeze.md)

## Result

Framed protocol now exposes private reports and public-safe receipts:

- `get_report` — owner-local `FullAutoRunReport`
- `get_receipt` — digests and counts only (no objective/transcript text)
- `apply_control_intent` — mobile Pause/Resume/Stop through the same action
  API with `actor: "mobile"` and typed `applied` / `rejected` outcomes
- `get_sync_status` / `publish_projection` — the real Khala Sync transport.
  A publish failure never blocks local dispatch (`publishBlocksDispatch:
  false`).
- The Sync heartbeat publishes the active public-safe run projection. It also
  gets pending mobile Pause, Resume, and Stop intents.
- The service writes each typed intent outcome to `sync-outcomes.json` before
  it reports the outcome. If the report response is lost, or if the service
  restarts, the service reports the same outcome again. It does not apply the
  control action again.

Mobile never writes durable run state directly. Omega/Desktop remains the
sole executor.

## Installed Omega session boundary

The service requests `resolve_sync_session` through the private framed host
bridge. The request has an empty parameter object. An admitted host response
has this result:

```json
{
  "available": true,
  "baseUrl": "https://openagents.com",
  "accessToken": "<runtime-only bearer>"
}
```

The host can instead return `{ "available": false }`, or it can return the
typed `unavailable` host error. In that state, `get_sync_status` returns
`omega_khala_sync_session_unavailable`. Full Auto continues to dispatch
locally.

The host must get the bearer from an admitted OpenAgents session authority.
It must not use a Codex credential, a Pylon credential, an environment token,
or a NIP-98 signature as a substitute. The host and service must keep the
bearer in memory. They must not write it to a run, report, projection, intent
outcome, diagnostic, or log. The base URL must use HTTPS. The service limits
the returned bearer to 16 KiB and rejects an empty value.

The current Omega Rust host does not implement this request. Therefore, an
installed Omega build reports Sync as unavailable until the host connects an
admitted OpenAgents session source. This missing host connection is the only
remaining authentication seam. The effect service does not add a legacy
Pylon dependency or invent a credential fallback.

## Verification

- `pnpm --filter @openagentsinc/omega-effectd test`
- `pnpm --filter @openagentsinc/omega-effectd typecheck`

## Next

- Omega host implementation of `resolve_sync_session`
- FA-06 native project join
