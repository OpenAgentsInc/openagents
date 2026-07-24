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
- `get_sync_status` / `publish_projection` — honest Sync stub
  (`omega_khala_sync_session_unavailable`). Publish failure never blocks
  local dispatch (`publishBlocksDispatch: false`).

Mobile never writes durable run state directly. Omega/Desktop remains the
sole executor.

## Verification

- `pnpm --filter @openagentsinc/omega-effectd test` — 183 passed

## Next

- Omega supervisor/GPUI consumption of report and receipt
- FA-06 native project join
