# M1-D: mobile OpenAuth PKCE entry and fail-closed sign-out

- Issue: #8660
- Parent track: #8597
- Depends on: closed #8658 and #8659
- Status: closed after the main receipt recorded on the live issue
- Authority:
  [`../2026-07-10-r1-r2-identity-sync-contract.md`](../2026-07-10-r1-r2-identity-sync-contract.md)

## Landed boundary

The greenfield mobile app now owns its native OpenAuth entry and exit path. One
imperative Expo AuthRequest creates and validates callback state and owns its
S256 verifier. It uses exact client `openagents-khala-mobile`, provider GitHub,
authorization code, and canonical redirect `openagents://auth`; the rollback
`khala://auth` remains issuer compatibility only. The browser prompt prefers an
ephemeral session so a later sign-in does not silently reuse the prior account.

After code exchange, the host validates access and refresh credentials through
the existing native-session GET. It derives the owner only from the server,
applies any immediate rotation, then writes the existing one-record SecureStore
vault. Error results are never exchanged and all public outcomes are bounded.

Explicit sign-out calls the existing native-session DELETE with both credential
classes. The local record remains present on network, status, or response-schema
failure and is deleted only when the response proves `accessRevoked` and
`refreshRevoked`. Effect Native renders typed sign-in/sign-out intents and sees
only session phases.

The enforced behavior contract is
`openagents_mobile.session.pkce_sign_in_sign_out.v1`.

## Explicit residual

This leaf does not implement live Khala Sync, the draft `device_session`
projection, repository selection, remote workrooms, or physical iOS/Android
acceptance. Those remain separate R1/R2/R6 gates.
