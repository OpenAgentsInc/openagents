# Sarah Nostr identity contract

- Date: 2026-07-24
- Class: contract
- Packet: `SARAH-NR-04`
- OpenAgents issue: [OpenAgentsInc/openagents#9218](https://github.com/OpenAgentsInc/openagents/issues/9218)
- Master tracker: [OpenAgentsInc/omega#31](https://github.com/OpenAgentsInc/omega/issues/31)
- Spec home: [2026-07-24-sarah-workroom-mvp-spec.md](./2026-07-24-sarah-workroom-mvp-spec.md) §22.1 and §24.5
- Implementation: `packages/sarah/src/nostr-identity/`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: admitted for the Sarah Nostr runtime

## 1. Purpose

This contract freezes how `principal.sarah` holds one Nostr identity.

It defines:

1. custody in Google Secret Manager
2. a signing boundary that returns signatures, not keys
3. owner attestation (NIP-OA) and agent authentication (NIP-AA)
4. rotation, revocation, and archival (NIP-IA)

It does not deploy a relay.
It does not replace the Khala Sync turn consumer (`SARAH-NR-05`).
It does not mint a production secret. An operator creates the Secret Manager
entry out of band.

Relay acceptance is never an OpenAgents admission.

## 2. Identity

| Field | Value |
| --- | --- |
| Principal | `principal.sarah` |
| Schema | `openagents.sarah.nostr_identity.v1` |
| Key type | BIP-340 x-only secp256k1 |
| Public form | 64 lowercase hex digits |
| Conversation role | Sarah half of the owner conversation triple in the record contract |

Sarah authors durable turn records (kind `44300`) and authority receipts
(kind `44301`). The owner authors kind `14` messages. Sarah never authors an
owner message.

## 3. Custody

### 3.1 Admitted path

Google Secret Manager in project `openagentsgemini` is the only admitted
custody path for the Sarah service key.

The Cloud Run monolith mounts the secret through the existing
`apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh` `--set-secrets`
map.

| Cloud Run env name | Secret Manager secret id |
| --- | --- |
| `SARAH_NOSTR_IDENTITY_SECRET` | `sarah-nostr-identity-secret` |

The secret value is a 64-character lowercase hex private key, or a
`nsec1…` NIP-19 encoding of that key. The runtime accepts either form and
never re-emits either form.

### 3.2 Forbidden paths

- a plain-text key in a repository file
- a key in an operator-edited Cloud Run env file or `.env` commit
- a build-time bake of the key into an image layer
- a second secret mechanism (Vault, raw GCS object, local file on the VM)

### 3.3 Process rule after mount

Cloud Run injects Secret Manager material as a process environment variable.
That mount is the admitted delivery path. The sealed signer loads the key
into a non-exportable closure. The runtime then clears
`process.env.SARAH_NOSTR_IDENTITY_SECRET`. Later logs, dumps, and child
processes must not re-read the raw material from the environment.

## 4. Signing boundary

### 4.1 Port

Callers reach `SarahNostrSigner` only:

| Method | Returns |
| --- | --- |
| `getPublicKey()` | 64 hex public key |
| `signEvent(template)` | fully signed NIP-01 event |
| `getPublicIdentity()` | public identity record (no secrets) |

There is no `exportPrivateKey`, `exportNsec`, or raw-byte escape hatch on this
port.

### 4.2 Protocol form

NIP-46 remote signing is the protocol form of the same boundary. A future
bunker adapter may implement `SarahNostrSigner` without changing callers.
The process-local sealed signer is the admitted Cloud Run form for this
packet. After Secret Manager mount, the key stays inside the process. It
never crosses a log, event, tag, crash record, or receipt.

### 4.3 Redaction

Any public-safe projection, log line, receipt, or error must pass through
`assertSarahNostrPublicSafe`. That guard rejects objects that name forbidden
secret fields (`privateKey`, `nsec`, `seckey`, mnemonic, seed, and related
aliases).

## 5. Attestation

### 5.1 NIP-OA

The owner binds Sarah's public key with a NIP-OA `auth` tag:

```text
["auth", <owner-pubkey-hex>, <conditions>, <sig-hex>]
```

The owner signs:

```text
SHA256("nostr:agent-auth:" || agent_pubkey || ":" || conditions)
```

The preimage mixes the **agent** pubkey (Sarah), not the owner pubkey.

Conditions are a `&`-joined clause string. For the Sarah runtime the default
condition set is empty (full owner attestation) unless a later packet freezes
kind windows.

### 5.2 NIP-AA

Sarah authenticates to the owned relay with a NIP-42 kind `22242` AUTH event
that carries exactly one NIP-OA `auth` tag. A relay that admits Sarah admits
an attested agent key, never an anonymous pubkey alone.

The pre-signed owner `auth` tag is public material. It may live in config or
a non-secret Cloud Run env var (`SARAH_NOSTR_OWNER_AUTH_TAG_JSON`). The owner
secret key never mounts into the Sarah process for day-to-day AUTH.

## 6. Lifecycle

| State | Meaning |
| --- | --- |
| `active` | current signing identity |
| `rotating` | new key active. Old key waits for archive |
| `revoked` | the key does not sign. Request archive |
| `archived` | NIP-IA archive path completed for this pubkey |

Rotation:

1. Mint a new key in Secret Manager (new secret version).
2. Load the new key into a new sealed signer.
3. Publish a NIP-IA archive request for the old pubkey with reason `rotated`.
4. Re-issue the owner NIP-OA tag for the new pubkey.
5. Mark the old identity `archived` after the relay projects the archive.

Revocation follows the same archival path with reason `retired` and does not
mint a replacement until the operator creates a new secret version.

An operator cannot repair encrypted history. Key loss causes history loss for
material encrypted to that key. Rotation and archival are therefore
first-class in this contract, not later cleanup.

## 7. Package surface

`@openagentsinc/sarah` exports the identity module under
`@openagentsinc/sarah/nostr-identity`.

Tests prove:

1. a sealed signer signs and verifies events without exporting the key
2. Secret Manager load clears the env slot after bind
3. public-safe guards reject secret-shaped fields
4. NIP-OA tags verify for the Sarah agent pubkey
5. NIP-42 AUTH templates carry exactly one `auth` tag
6. lifecycle transitions reject illegal jumps

## 8. Exit for `SARAH-NR-04`

- Sarah can authenticate to an owned relay as an attested agent (local Node
  relay from `nostr-effect` is enough for development).
- No raw key reaches an event, a tag, a log, a crash record, or a receipt.
- The deploy script mounts `SARAH_NOSTR_IDENTITY_SECRET` from Secret Manager
  only. No second mechanism lands.

## 9. Non-goals

- Creating the production Secret Manager value (operator out of band)
- The turn service consumer (`SARAH-NR-05`)
- The Omega conversation client (`SARAH-NR-06`)
- Metering, ledger, or admission changes
