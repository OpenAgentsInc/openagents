# Probe Fleet Telemetry And Failover

Date: 2026-06-07

Status: implemented contract slice for Probe issue #162.

## Contract

OpenAgents product surface remains the cross-account selection and lease authority. Probe reports
runtime account signals and asks OpenAgents product surface for failover when the selected account
cannot continue.

Probe does not locally iterate across raw ChatGPT tokens.

## Implemented Probe Surface

`packages/runtime/src/fleet/telemetry.ts` exports:

- `ProbeAuthHealthSignal`
- `makeProbeAuthHealthSignal`
- `deriveOpenAgents product surfaceAccountHealthPatch`
- `recordProbeAuthHealthSignal`
- `shouldRequestOpenAgents product surfaceFailover`
- `makeOpenAgents product surfaceFleetTelemetryClient`
- `makeStaticProbeFleetTelemetryClient`

`packages/runtime/src/contracts/provider-account.ts` also exports
`canSelectProviderAccountForLease`, which mirrors the Probe-side lease
eligibility guard: connected, healthy, has a public secret ref, not low-credit,
not cooling down, and lease limit greater than zero.

## Signals

Probe can report:

- success
- scrubbed
- `access_token_failed`
- `refresh_failed`
- `requires_reauth`
- `low_credit`
- `rate_limited`
- `provider_unavailable`
- `non_auth_failure`

Auth/account failures with a lease ref request OpenAgents product surface failover through the
operator lease failover route. Successful and non-auth failures do not request
account failover.

## Backend Capabilities

`packages/runtime/src/fleet/backend-capability.ts` adds the first backend
capability report for Pylon/SHC-style provider routing:

- capability: `probe.backend.apple_fm_bridge`
- backend kind: `apple_fm_bridge`
- profile: `apple-fm-local`
- readiness source: live Apple FM `/health`

Probe advertises the capability only when live Apple FM health is ready.
Unavailable and unsupported states are still reported as typed capability
status, with redacted availability receipts, so OpenAgents product surface/OpenAgents can distinguish
Apple FM from Qwen, Codex, MLX, or other future backends without leaking local
paths, callback secrets, or transcript payloads.

## Tests

`packages/runtime/tests/fleet-telemetry.test.ts` covers:

- rate-limit failover
- reauth-required mapping
- low-credit mapping
- no failover for success/non-auth failures
- skipping low-credit, cooldown, unhealthy, and exhausted accounts
- raw credential rejection in telemetry metadata
- ready and unsupported Apple FM backend capability reports
