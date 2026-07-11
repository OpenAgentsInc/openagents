# D1-F: typed Effect Native Desktop session controls

- Issue: #8665
- Parent track: #8574
- Depends on: closed #8661–#8664
- Status: closed after the main receipt recorded on the live issue
- Authority:
  [`../2026-07-10-r1-r2-identity-sync-contract.md`](../2026-07-10-r1-r2-identity-sync-contract.md)

## Landed boundary

Runtime Gateway bootstrap now carries one explicit session phase:
`signed_out`, `unverified`, `session_ready`, `denied`, or `unavailable`. This is
the only session state the renderer receives. The gateway still carries no
callback/authorize URL, state, code, verifier, server owner, credential, or
storage field.

Effect Native Settings now renders honest phase copy and one typed action.
Signed-out/denied/unavailable offer `Sign in with GitHub`; session-ready offers
`Sign out`; loading/unverified/authenticating disable the action. Typed intents
send argument-free `session.sign_in` or `session.sign_out` commands, schema-
decode the bounded outcome, and update view state. Host commands remain
single-flight.

The enforced behavior contract is
`openagents_desktop.session.effect_native_controls.v1`.

## Explicit residual

This leaf does not claim a live browser/device acceptance run, authenticated
Khala Sync, `device_session`, package identity, GUI smoke, or physical
acceptance. Tests drive the real Effect Native view and intent registry with a
tokenless fake bridge only.
