# D1-D: Desktop OpenAuth loopback PKCE client policy

- Issue: #8663
- Parent track: #8574
- Depends on: closed #8661 and #8662
- Status: closed after the main receipt recorded on the live issue
- Authority:
  [`../2026-07-10-r1-r2-identity-sync-contract.md`](../2026-07-10-r1-r2-identity-sync-contract.md)

## Landed boundary

The OpenAuth issuer now recognizes a distinct public native client
`openagents-desktop`. It accepts only GitHub authorization code with a bounded
S256 PKCE challenge and a redirect shaped exactly as:

`http://127.0.0.1:{ephemeral-port}/auth/callback`

The host must be the literal IPv4 loopback address, the port must be explicit
and non-privileged, and the path must match exactly. HTTPS, localhost, IPv6,
custom schemes, missing/privileged ports, alternate paths, userinfo, query,
fragment, non-GitHub providers, non-code response, plain/malformed PKCE, and
mobile-client reuse are rejected. Existing web and mobile redirect policy is
unchanged.

This follows [RFC 8252 §7.3 and §8.3](https://www.rfc-editor.org/rfc/rfc8252.html):
desktop native apps use a loopback IP literal with a dynamically assigned port,
bind only loopback, close the listener after the response, and rely on PKCE to
make an intercepted code unusable. It also avoids contending for the mobile
`openagents://auth` handler.

The enforced behavior contract is
`openagents_desktop.session.loopback_pkce_policy.v1`.

## Explicit residual

This policy leaf does not start a listener, launch the system browser, exchange
a code, write the Desktop vault, sign out, make Sync live, or freeze package/
update identity. The next leaf composes those already-frozen boundaries.
