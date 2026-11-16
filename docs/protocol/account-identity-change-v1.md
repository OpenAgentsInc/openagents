# Changing an account-managed identity

`POST /api/auth/identity/change/v1` is an explicit, fresh L2 human-session
operation. It does not change login authenticators, wallet keys, grants or
Project ownership. Email login still ensures a key through identity-v1.

The registered `openagents.account.identity.change.v1` request has `schema`,
`command` (32 lowercase hex), `expected` (the current 64-hex public key),
`expected_revision` (0..2147483646), `mode` (`rotate` or `restore`) and `secret` (empty for rotate, 64 lowercase
hex for restore). Restore transfers custody of that private key to the
account's encrypted store; the phone accepts hex or nsec and converts through
the shared NIP-19 codec. Zero/out-of-range scalars refuse. No private key is
ever an app-core event, semantic row, diagnostic or preference.

Under the account lock, check the operation journal before the expected-key
comparison. A retry with the same command and exact request digest returns
the original public identity-v1 answer. Reusing a command with different bytes
or changing a stale expected key or revision returns 409.
`GET /api/auth/identity/state/v1` returns the separately versioned
identity-state schema with account, pubkey, source and revision. The revision
increases on every change, even restoring the same key; it never wraps. An unknown network outcome keeps
the same command/body in device-only protected storage for an explicit retry;
it must not silently generate another key.

`POST /api/auth/identity/cancel/v1` accepts the closed identity-cancel schema
and command only, under the same fresh L2 admission. It takes the account
lock and inserts a terminal cancellation tombstone if the command has not
finished. A delayed request with that command then refuses. If the change
already finished, it stays finished. The response is current public
identity-v1 metadata in both cases. Only this successful reconciliation or
a successful exact replay clears the protected pending request on the
phone; dismissing the screen does not. Cancellation can itself be retried. A different account's key refuses.

The new immutable key uses the existing sealing service and public-key-bound
associated data. `auth.account_identity_selection_v1` selects the current key;
`auth.account_identity_changes_v1` stores only public operation metadata and
the request digest. Old sealed keys remain account aliases for historical
messages and stable ownership. Rotating an identity does not rewrite signed
events or promise that earlier correspondents know the new key. Restoring an
older key selects it again. The existing identity-v1 response stays unchanged.

Selection and journal participate in export/merge/purge. A surviving account's
selection wins a merge collision. Account deletion purges all selections,
receipts and sealed keys; exports never include private keys or ciphertext.
All responses are private/no-store. Audit rows name the operation, not its
private request. Frozen, limited, anonymous, stale-L2 and machine callers
cannot change or export an identity. The host uses the existing system
passkey registration/step-up flow and local biometric authentication for
backup; it never upgrades an email session by merely knowing a managed key.

Settings shows the public identity, Copy, Backup, Rotate and Restore. Rotate
and Restore require a separate confirmation explaining identity replacement.
Private text is confined to the active secure platform editor, is masked,
clears on dismissal/background and is covered during screen capture. Private
clipboard writes expire and are local-only. Cancelling changes no key.
