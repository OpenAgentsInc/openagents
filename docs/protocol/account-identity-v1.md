# Automatic account identities

Owner-approved 2026-11-14: create Nostr keypairs automatically for keyless
accounts; keep their private keys encrypted on the server. Wallet keys remain
separate. The existing login contracts and authentication strengths keep their
identity.

## Identity and authentication

An account-managed identity is a real secp256k1 keypair, separate from a login
authenticator. It is not inserted into `auth.nostr_keys`, does not increase
`auth.strong_ways`, cannot step up an email session, and does not change grants,
creator access or administration. Private keys never appear in login responses,
the public identity endpoint, logs, notices or data exports. This version adds
no private-key download or arbitrary signing endpoint.

Successful interactive logins ensure an identity. Existing sessions acquire
one when reading their account/identity or first using account-owned Projects.
If an account already has a managed identity, keep it. Otherwise preserve an
existing linked Nostr key, without copying or replacing its private key. Only
an account with neither receives a new managed pair. Frozen/deleted accounts
do not receive new keys. A storage/randomness/sealing failure refuses
provisioning and never substitutes an unrelated identity.

## Storage and concurrency

`auth.account_identity_keys_v1` holds account, public key, sealed secret and
creation milliseconds. Provisioning locks the account row before checking or
creating a key, so concurrent logins cannot mint competing identities. Ordinary
logins never rotate a key. Account merges retain managed key rows as aliases;
the oldest managed row is selected deterministically. Existing linked keys are
selected by their existing row order when there is no managed identity.

The private key is 32 bytes from OS entropy, validated and projected to its
x-only public key by the existing fixed-flow Coax secp256k1 implementation.
The existing auth seal service encrypts its hex encoding under the versioned
`OA_AUTH_SEAL_KEY`, with a fresh nonce and associated data
`auth.account_identity_keys_v1|sealed|<pubkey>`. The immutable public-key row
identity survives a permitted account merge. Opening must also verify that
the plaintext derives that public key. Plaintext scratch is wiped. Deletion
purges the row; exports omit the seal. Backup/key-rotation obligations are the
existing auth encryption obligations.

## Public contract

`GET /api/auth/identity/v1` requires an authenticated account and returns:

```json
{"schema":"openagents.account.identity.v1","account":"acct_example","pubkey":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","source":"account"}
```

`source` is `account` for the server-managed identity or `linked` for an
existing independently held key. No request accepts a caller-supplied account
id or replacement public key. Responses are private/no-store. Anonymous
requests are 401; frozen/deleted accounts cannot provision. Existing v1alpha1
login/account responses are unchanged.

## Project compatibility

New account Project owner mappings use the real identity public key. Existing
`drops.account_owners_v1` mappings remain stable ownership aliases; replacing
them would strand existing drafts, files and projects. A normal human session
may use its own mapped alias in private project membership checks, while
limited/frozen sessions, unrelated accounts and forged signatures retain their
existing refusals. NIP-98 authority and existing linked-key memberships are
unchanged. The alias is never presented as another signing key.

Qualification must cover repeated/concurrent provisioning, real key/seal
round trips, existing-key preservation, unchanged email L1/strong counts,
account isolation, unavailable entropy/storage/sealing, export omission,
merge/purge, and legacy project owner access.
