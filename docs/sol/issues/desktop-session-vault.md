# D1-B: Desktop OS-encrypted native-session custody

- Issue: #8661
- Parent track: #8574
- Depends on: closed #8655 and #8656
- Status: closed after the main receipt recorded on the live issue
- Authority:
  [`../2026-07-10-r1-r2-identity-sync-contract.md`](../2026-07-10-r1-r2-identity-sync-contract.md)

## Landed boundary

Electron main now owns one versioned native OpenAgents session record containing
the server-derived owner ref and access/refresh token pair. Electron
`safeStorage` encrypts the record before one opaque envelope is atomically
written beneath the Desktop `userData` root. POSIX directory/file modes are
0700/0600.

The vault refuses custody if OS encryption is unavailable and explicitly
rejects Electron's Linux `basic_text` backend. Malformed, undecryptable,
incomplete, and retired-epoch records purge fail-closed. Storage errors are
public-safe and never include a cause or credential field.

Electron main recovers only `signed_out` or
`credential_present_unverified`; an encryption/storage failure becomes
`unavailable`. The Runtime Gateway exposes those meanings only as bounded
capability copy. Neither preload nor renderer can name the vault file, invoke
`safeStorage`, or receive owner/token fields.

The enforced behavior contract is
`openagents_desktop.session.os_encrypted_custody.v1`.

## Explicit residual

This leaf does not implement Desktop browser PKCE, token exchange,
recovered-session server validation/rotation, explicit sign-out, live Khala
Sync, `device_session`, packaging, or physical acceptance.
