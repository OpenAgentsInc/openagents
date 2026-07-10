# D1-C: recovered Desktop native-session validation and rotation

- Issue: #8662
- Parent track: #8574
- Depends on: closed #8661 and server boundary #8659
- Status: closed after the main receipt recorded on the live issue
- Authority:
  [`../2026-07-10-r1-r2-identity-sync-contract.md`](../2026-07-10-r1-r2-identity-sync-contract.md)

## Landed boundary

Electron main now loads a recovered encrypted credential and calls the exact
native-session GET with its bearer and bounded refresh header. The response is
schema-decoded for authenticated server owner and optional replacement tokens
only. A matching owner produces bounded verified state; any valid OpenAuth
rotation is rewritten to the safeStorage vault before readiness is projected.

HTTP 401/403 and server-derived owner mismatch purge custody fail-closed.
Network, other server status, malformed response, invalid rotation, and vault
rewrite failure retain the existing record where possible but return only
unavailable. Raw owner and credential values stay in Electron main.

The Runtime Gateway maps verified recovery to one available
`openagents-session` capability and maps signed-out, denied, unverified, or
unavailable states to bounded public-safe copy. Verified session readiness does
not make Khala Sync live.

The enforced behavior contract is
`openagents_desktop.session.recovered_validation_rotation.v1`.

## Explicit residual

This leaf does not implement Desktop browser PKCE, explicit sign-out, live
Khala Sync, `device_session`, packaging, or physical acceptance.
