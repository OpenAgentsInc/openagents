# Sarah memory runtime on Nostr

- Date: 2026-07-24
- Class: contract pointer
- Packet: `SARAH-NR-07`
- OpenAgents issue: [OpenAgentsInc/openagents#9221](https://github.com/OpenAgentsInc/openagents/issues/9221)
- Master tracker: [OpenAgentsInc/omega#31](https://github.com/OpenAgentsInc/omega/issues/31)
- Spec home: [2026-07-24-sarah-workroom-mvp-spec.md](./2026-07-24-sarah-workroom-mvp-spec.md) §22.5 and §24.8
- Decision audit: [docs/sarah/2026-07-24-sarah-memory-on-nostr-audit.md](../sarah/2026-07-24-sarah-memory-on-nostr-audit.md)
- Implementation: `packages/sarah/src/nostr-memory/`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: admitted for MVP adapters

## 1. Purpose

This document points to the Sarah memory runtime adapters.

The adapters build and parse:

1. NIP-AE engrams (kind `30174`)
2. NIP-RS read state (kind `30078`)
3. NIP-ER reminders (kind `30300`)

This packet does not deploy a relay.
This packet does not enable graph recall in production.
This packet does not require a live `nostr-effect` pin.
Production injects NIP-44 through the cipher port.

Relay acceptance is never an OpenAgents admission.

## 2. Laws

1. The durable memory record is NIP-AE. The kind is addressable `30174`.
2. Sarah signs every engram. The owner is the single `p` tag.
3. Content is ciphertext under the agent-owner conversation key. The owner can
   always decrypt.
4. The addressable `d` tag is HMAC-blinded. Slugs must not leak in tags.
5. The graph is a derived rebuildable index. If it disagrees with the engrams,
   the engrams win.
6. Ranking must not enter a durable body.
7. Read state uses kind `30078` with a max-register merge. Do not invent a new
   read-state protocol.
8. Reminders use addressable kind `30300` with NIP-40 expiration.
9. Do not put decrypted memory in a server full-text index and call it search.
   Client-side indexing over decrypted content is the honest path.

## 3. Wire kinds

| Concern | Kind | NIP | Notes |
| --- | --- | --- | --- |
| Memory engram | `30174` | NIP-AE | Addressable. Blinded `d`. Companion fields under unknown-fields rule |
| Read state | `30078` | NIP-RS / NIP-78 | Encrypt-to-self. Max-register CvRDT merge |
| Reminder | `30300` | NIP-ER | Addressable. Public `not_before` when pending. NIP-40 `expiration` |

## 4. Module map

| Path | Role |
| --- | --- |
| `packages/sarah/src/nostr-memory/engram.ts` | Write and read engram templates. HMAC `d`. Companion body |
| `packages/sarah/src/nostr-memory/read-state.ts` | Read-state templates. Max-register merge helpers |
| `packages/sarah/src/nostr-memory/reminder.ts` | Reminder templates |
| `packages/sarah/src/nostr-memory/cipher.ts` | Test cipher port (NIP-44 in production) |
| `packages/sarah/src/nostr-memory/redaction.ts` | Reject secret-shaped values before encryption |
| `packages/sarah/src/nostr-memory/types.ts` | Kinds, companion schema, cipher interface |

Import path: `@openagentsinc/sarah/nostr-memory`.

## 5. Companion profile

OpenAgents companion fields use schema id
`openagents.sarah.nip_ae_companion.v1`.

They carry provenance, admission state, entity identity, typed relations, and
derivation links. Ranking stays outside the record.

Normative field authority is the freeze packet `SARAH-NR-07a` (#9232):
[2026-07-24-sarah-nip-ae-companion-profile.md](./2026-07-24-sarah-nip-ae-companion-profile.md)
and fixtures under `fixtures/sarah-nip-ae/`.
Types in `packages/sarah/src/nostr-memory/types.ts` implement that freeze for
MVP adapters.

## 6. Cipher port

```text
SarahNostrMemoryCipher
  encryptToOwner(plaintext) → ciphertext
  decryptFromOwner(ciphertext) → plaintext
```

Production binds NIP-44 under conversation key `K_c`.
Tests use `testSarahNostrMemoryCipher`.
Wire content must never contain plaintext JSON body markers.

## 7. Search caveat

Encrypted events remove authorized server full-text search over plaintext.
Client-side indexing over decrypted content is the replacement.
Do not store decrypted memory in a server index and call it search.

## 8. Exit criteria for this packet

- Template builders produce kinds `30174`, `30078`, and `30300`.
- Engram round-trip decrypts to the same body.
- Owner-side decryption of every test engram succeeds under the cipher port.
- Read-state merge is monotonic across two device blobs.
- Secret-shaped values fail the redaction gate before encryption.
- Package exports and package tests are green.

## 9. Out of scope

- Relay publish or subscribe
- Live graph projection in Cloud SQL
- Enabling `SARAH_GRAPH_MEMORY_RECALL_ENABLED`
- Migration of historical Cloud SQL memory rows
- Key custody rotation (`SARAH-NR-04` owns identity)
