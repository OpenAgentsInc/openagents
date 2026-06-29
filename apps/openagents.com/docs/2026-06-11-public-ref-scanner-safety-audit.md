# Public Ref Scanner-Safety Audit

Issue: OpenAgents #4734

Date: 2026-06-11

## Scope

Some coding agents run client-side secret scanners over every fetched JSON
payload. Public OpenAgents refs must therefore avoid shapes that look like raw
credentials even when the ref is intentionally public. This audit covers the
public endpoints named in the issue:

- `GET /api/pylons`
- `GET /api/public/pylon-stats`
- `GET /api/public/artanis/report`

The scanner-safety rule treats these public string shapes as unsafe for public
JSON rendering:

- JWT-like values with three base64url segments.
- Long unbroken base64url-like refs of 40 or more characters.

Short dotted public refs such as `cap.gepa.retained.v1`,
`receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221`, and
`route:/api/public/pylon-stats` remain readable.

## Live Audit

The live audit used cache-busted reads of the three endpoints and recursively
checked string fields for scanner-shaped values. The audit output records only
paths, reasons, lengths, and short previews.

| Endpoint                     | Finding                                                                                                                                                                                                           | Rendering change                                                                                                                                                                               |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `/api/pylons`                | No scanner-shaped public strings found in the live response.                                                                                                                                                      | `publicPylonApiRegistrationProjection` now aliases scanner-shaped capability, capacity, health, load, and wallet refs before returning list or detail JSON.                                    |
| `/api/public/pylon-stats`    | No scanner-shaped public strings found in the live response.                                                                                                                                                      | Recent Pylon `products` are now rendered through the scanner-safe public-ref aliaser because they come from registered `capabilityRefs`.                                                       |
| `/api/public/artanis/report` | Two public strings were flagged under `pylonOmegaReleaseGate.evidenceRefs` and `pylonOmegaReleaseGate.multiPylonProofRefs`. Both were long opaque public evidence refs with the same short preview prefix/suffix. | `projectPylonV02OmegaReleaseGate(..., "public", ...)` now aliases scanner-shaped evidence refs to `evidence.public.pylon_v0_2.omega_gate.scanner_safe.<hash>` before Artanis report rendering. |

## Regression Coverage

- `public-ref-scanner-safety.test.ts` covers the shared detector, aliaser, and
  JSON audit walker.
- `pylon-api-routes.test.ts` verifies `/api/pylons` list/detail/registration
  responses hide scanner-shaped capability refs.
- `public-pylon-stats.test.ts` verifies recent Pylon product refs hide
  scanner-shaped capability refs.
- `pylon-v02-omega-release-gate.test.ts` verifies the historical opaque Omega
  evidence ref is emitted as a short dotted alias in public projections.
- `artanis-public-report.test.ts` verifies the aggregate public report does not
  serialize the scanner-shaped Omega evidence ref.

## Boundary

The change is a public rendering migration, not a storage migration. Internal
records may retain historical evidence ids for continuity. Public endpoints now
render scanner-safe aliases whenever a public ref's shape would likely be
silently redacted by agent-side secret scanners.
