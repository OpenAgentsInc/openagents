# D1-E: Desktop loopback PKCE entry and fail-closed sign-out host

- Issue: #8664
- Parent track: #8574
- Depends on: closed #8661–#8663
- Status: closed after the main receipt recorded on the live issue
- Authority:
  [`../2026-07-10-r1-r2-identity-sync-contract.md`](../2026-07-10-r1-r2-identity-sync-contract.md)

## Landed boundary

Electron main now composes the frozen Desktop public-client policy. It binds a
temporary server to literal `127.0.0.1` on an OS-assigned non-privileged port,
generates cryptographic state and PKCE verifier/challenge, opens the exact
`openagents-desktop` GitHub code + S256 authorize URL, and accepts only GET on
the exact `/auth/callback` path with matching state and a non-empty code.
Invalid callbacks do not terminate the valid listener. The listener closes
after one terminal callback or a bounded timeout.

The no-store callback page reflects no request parameter. Main exchanges the
code and verifier, calls the native-session GET, derives owner only from the
server, applies immediate rotation, and then writes encrypted custody. OAuth
denial returns bounded cancellation and no credential is saved.

Sign-out sends both credential classes to the native-session DELETE and clears
only after the response proves both revocations. Network, status, or incomplete
proof retains custody. The closed Runtime Gateway accepts only argument-free
`session.sign_in` / `session.sign_out` commands and returns bounded phase
outcomes; callback URLs and credential fields cannot enter its schema.

The enforced behavior contract is
`openagents_desktop.session.loopback_pkce_entry_exit.v1`.

## Explicit residual

This leaf does not add a visible renderer button/intent, live Khala Sync,
`device_session`, package identity, or GUI/physical acceptance. Tests use a
fake browser opener and local loopback requests only.
