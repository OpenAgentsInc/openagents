# M1-C: recovered native-session validation and rotation

- Issue: #8659
- Parent track: #8597
- Depends on: closed #8658
- Status: closed after the main receipt recorded on the live issue
- Authority:
  [`../2026-07-10-r1-r2-identity-sync-contract.md`](../2026-07-10-r1-r2-identity-sync-contract.md)

## Landed boundary

The existing native user-bearer boundary accepts a bounded
`X-OpenAgents-Refresh-Token` only on `GET /api/mobile/auth/session`. It passes
that token to the existing OpenAuth verifier; if verification rotates the
credential, the endpoint returns the replacement access/refresh pair in its
no-store response. Other mobile bearer routes cannot trigger rotation and lose
the replacements.

On startup, the mobile host loads the SecureStore record, calls that exact
endpoint, verifies the returned server-derived owner ref, and rewrites the
single vault record when tokens rotate. A 401/403 or owner mismatch purges the
credential. Network, server, or response-schema failure retains the record for
later recovery but projects only unavailable. Effect Native receives no token
or owner field and distinguishes `session_ready` from `live` Sync.

The enforced behavior contract is
`openagents_mobile.session.recovered_validation_rotation.v1`.

## Explicit residual

This leaf does not implement the browser authorization-code/PKCE prompt,
explicit sign-out UI, Sync subscription, `device_session`, or physical-device
proof. Those remain later R1/R2/F1 leaves.
