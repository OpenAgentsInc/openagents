# M1-B: fail-closed mobile native-session custody

- Issue: #8658
- Parent track: #8597
- Depends on: closed #8657
- Status: closed after the main receipt recorded on the live issue
- Authority:
  [`../2026-07-10-r1-r2-identity-sync-contract.md`](../2026-07-10-r1-r2-identity-sync-contract.md)

## Landed boundary

OpenAgents mobile persists one schema-decoded record in Expo SecureStore under
the OpenAgents keychain service. The record contains the OpenAuth access and
refresh tokens plus the owner ref returned by the server. It is versioned with
an explicit credential epoch and uses after-first-unlock, this-device-only
accessibility where supported.

Malformed, partial, empty, or retired-epoch records are deleted. Reads, writes,
and clears fail with a public-safe typed storage error that never includes
credential content. Effect Native receives only `signed_out` or
`credential_present_unverified`; cached work stays hidden until server
verification lands.

The enforced behavior contract is
`openagents_mobile.session.secure_store_custody.v1`.

## Explicit residual

This leaf does not open a browser, perform PKCE exchange, validate or refresh
the credential, revoke it server-side, subscribe to Sync, create a
`device_session`, or claim physical-device proof. Those remain the next R1/F1
leaves.
