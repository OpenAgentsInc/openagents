# Sarah NIP-AE companion profile

- Date: 2026-07-24
- Class: contract freeze
- Packet: `SARAH-NR-07a`
- OpenAgents issue: [OpenAgentsInc/openagents#9232](https://github.com/OpenAgentsInc/openagents/issues/9232)
- Master tracker: [OpenAgentsInc/omega#31](https://github.com/OpenAgentsInc/omega/issues/31)
- Parent packet: `SARAH-NR-07` ([openagents#9221](https://github.com/OpenAgentsInc/openagents/issues/9221))
- Decision audit: [docs/sarah/2026-07-24-sarah-memory-on-nostr-audit.md](../sarah/2026-07-24-sarah-memory-on-nostr-audit.md)
- Spec home: [2026-07-24-sarah-workroom-mvp-spec.md](./2026-07-24-sarah-workroom-mvp-spec.md) §24.8
- Fixtures: `fixtures/sarah-nip-ae/`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: frozen before any Sarah engram write

## 1. Purpose

This freeze locks the OpenAgents companion profile for Sarah memory on NIP-AE.

A second implementation must interoperate from this document and the fixtures
alone.

This freeze does not write an engram.
This freeze does not deploy a relay.
This freeze does not enable graph recall.
This freeze does not fork NIP-AE or invent a new kind.

Relay acceptance is never an OpenAgents admission.

## 2. Authority chain

| Role | Artifact | Note |
| --- | --- | --- |
| Product intent | `specs/openagents/sarah-owner-orchestrator.product-spec.md` | revision 6, `SARAH-AC-31` |
| Sarah authority | `docs/authority/SARAH_AUTHORITY.md` | `program.sarah_nostr_runtime` |
| Decision audit | `docs/sarah/2026-07-24-sarah-memory-on-nostr-audit.md` | owner-accepted 2026-07-24 |
| Workroom specification | `docs/omega/2026-07-24-sarah-workroom-mvp-spec.md` | §24.8 |
| Protocol home | Buzz NIP-AE + `OpenAgentsInc/nostr-effect` | `EngramService` owns the wire |
| Graph SDK | `packages/agent-experience-memory` | projection semantics only |

## 3. Laws

1. The durable memory record is NIP-AE. The kind is addressable `30174`.
2. Sarah signs every engram. The owner is the single `p` tag.
3. `content` is NIP-44 v2 ciphertext under the agent-owner conversation key
   `K_c`. The owner can always decrypt.
4. The addressable `d` tag is HMAC-blinded. Slugs must not leak in tags.
5. The graph and its embeddings are a derived rebuildable index. They are never
   authority.
6. If the projection and the engrams disagree, the engrams win.
7. Ranking must not enter a durable body. Ranking lives only in the projection.
8. Wiki links are human cross-reference only. Typed edges use companion fields.
9. No companion field ships without this specification and fixtures.

## 4. NIP-AE envelope (unchanged)

### 4.1 Kind and class

| Field | Value |
| --- | --- |
| Kind | `30174` |
| NIP-01 class | addressable (parameterized replaceable) |
| NIP | NIP-AE Agent Engrams |
| Wire library | `nostr-effect` `EngramService` |

Do not invent a second memory kind.
Do not put this profile in `supported_nips` as a new NIP number.
Advertise OpenAgents behavior as extension id
`openagents.sarah.nip_ae_companion` version `1` when a relay list needs it.

### 4.2 Outer event

```json
{
  "kind": 30174,
  "pubkey": "<sarah_pubkey>",
  "created_at": 1753387300,
  "content": "<NIP-44 v2 ciphertext>",
  "tags": [
    ["d", "<64 lowercase hex HMAC>"],
    ["p", "<owner_pubkey>"],
    ["alt", "encrypted agent memory record"]
  ],
  "sig": "<signature>"
}
```

Required tags:

| Tag | Count | Rule |
| --- | --- | --- |
| `d` | exactly one | HMAC-blinded slug. See §5 |
| `p` | exactly one | owner public key |
| `alt` | exactly one | NIP-31 fallback. See §4.4 |

Additional tags have no effect on NIP-AE validity.
Do not put the slug, the value, or ranking in a tag.

### 4.3 Encryption

`content` must be NIP-44 v2 ciphertext under:

```text
K_c = nip44_conversation_key(sarah_seckey, owner_pubkey)
    = nip44_conversation_key(owner_seckey, sarah_pubkey)
```

Plaintext must not appear in `content`, tags, logs, or wire fixtures.
Decrypted body size must not exceed `65535` bytes.

### 4.4 NIP-31 `alt`

Required `alt` text:

```text
encrypted agent memory record
```

This matches the NIP-AE default.
The text must stay free of secrets, slugs, digests, and owner identifiers.

### 4.5 Authorship, deletion, replacement

| Rule | Requirement |
| --- | --- |
| Authorship | Sarah signs kind `30174`. The owner does not author this kind. |
| Replacement | Addressable head per `(kind, pubkey, d)`. Newer `created_at` wins. |
| In-band delete | Memory body with `"value": null` is a tombstone. Readers treat the slug as absent. |
| NIP-09 | Sarah may publish kind `5` with `k=30174` and `a=30174:<sarah_pubkey>:<d>`. Do not depend on NIP-09 alone. |
| Tombstone retention | Keep tombstones on the owned relay. Do not garbage-collect them. |
| Relay derivation | Forbidden. A relay must not synthesize kind `30174`. |

## 5. HMAC-blinded `d` tag

### 5.1 Formula

```text
K_c = nip44_conversation_key(...)
d   = lower_hex(
        HMAC-SHA256(
          K_c,
          utf8("agent-memory/v1/d-tag") || 0x00 || utf8(slug)
        )
      )
```

Domain separator: `agent-memory/v1/d-tag`.
Separator byte: `0x00`.
Output: 64 lowercase hex characters.

### 5.2 Slug grammar

A valid slug is either `core` or:

```text
^mem/[a-z0-9][a-z0-9_-]{0,63}(/[a-z0-9][a-z0-9_-]{0,63})*$
```

Total length is at most 255 bytes.

### 5.3 Reference vectors

Use the NIP-AE pinned test keys only in fixtures:

```text
seckey_a = 0000...0001
seckey_o = 0000...0002
K_c      = c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d
```

| Slug | `d` |
| --- | --- |
| `core` | `bdc233238ffe52e272b44cc233c8f33a2bc510b08be04495b225964283be4a90` |
| `mem/example` | `72d4f9629106451505d7d341ea85bb3ebad4f654fcfd2aad100d5a35f8a85cba` |
| `mem/fact/owner-prefers-codex` | `2a93fa200395921923724765c52989fa4a2b3b81095940f8e9cb4e002664af25` |

Production code must not use the test keys.

## 6. Companion body fields

NIP-AE bodies are `{ slug, value }` or `{ slug, profile }` for core.
Readers must ignore unknown fields.
OpenAgents adds fields under that rule.

### 6.1 Schema id

| Field | Value |
| --- | --- |
| Extension id | `openagents.sarah.nip_ae_companion` |
| Extension version | `1` |
| Body schema | `openagents.sarah.nip_ae_companion.v1` |

### 6.2 Memory body shape

```json
{
  "slug": "mem/fact/owner-prefers-codex",
  "value": "Owner prefers Codex for coding tasks.",
  "openagents": {
    "schema": "openagents.sarah.nip_ae_companion.v1",
    "admission": "admitted",
    "entityId": "entity.aaaaaaaaaaaaaaaaaaaaaaaa",
    "contentDigest": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "sourceEventRefs": [
      {
        "eventId": "1111111111111111111111111111111111111111111111111111111111111111",
        "role": "turn_record"
      }
    ],
    "relations": [
      {
        "type": "about",
        "targetSlug": "mem/entity/owner",
        "direction": "out"
      }
    ],
    "derivedFromSlugs": []
  }
}
```

### 6.3 Field rules

| Field | Required | Rule |
| --- | --- | --- |
| `slug` | yes | NIP-AE slug grammar |
| `value` | yes | UTF-8 string, or `null` for a tombstone |
| `openagents` | yes for Sarah writes under this profile | object |
| `openagents.schema` | yes | literal `openagents.sarah.nip_ae_companion.v1` |
| `openagents.admission` | yes when `value` is a string | `admitted`, `candidate`, or `rejected` |
| `openagents.entityId` | yes when `value` is a string | `entity.<24 lowercase hex>` |
| `openagents.contentDigest` | yes when `value` is a string | `sha256:` + 64 lowercase hex of redacted `value` UTF-8 bytes |
| `openagents.sourceEventRefs` | yes when `admission` is `admitted` | array length >= 1 |
| `openagents.sourceEventRefs[].eventId` | yes | 64 lowercase hex |
| `openagents.sourceEventRefs[].role` | yes | `turn_record`, `tool_result`, `owner_message`, or `import` |
| `openagents.relations` | yes when `value` is a string | array, may be empty |
| `openagents.relations[].type` | yes | non-empty token `^[a-z][a-z0-9_]{0,63}$` |
| `openagents.relations[].targetSlug` | yes | valid NIP-AE slug |
| `openagents.relations[].direction` | yes | `out`, `in`, or `both` |
| `openagents.derivedFromSlugs` | yes when `value` is a string | array of valid slugs, may be empty |

### 6.4 Tombstone body

```json
{
  "slug": "mem/fact/owner-prefers-codex",
  "value": null
}
```

A tombstone may omit `openagents`.
If `openagents` is present, `schema` must still match.
Readers treat the slug as absent after head selection.

### 6.5 Core body

Core stays NIP-AE:

```json
{
  "slug": "core",
  "profile": "<agent identity, rules, goals>"
}
```

This profile does not require `openagents` on core.
Do not put ranking on core.

### 6.6 Forbidden durable fields

These names must not appear on a durable body or inside `openagents`:

- `ranking`
- `feedback_weight`
- `score`
- `weight`
- `rank`
- `embedding`
- `vector`

A body with any of these names is invalid for this profile.

### 6.7 Redaction

Every `value` must pass the same public-safe redaction gate used by the current
Sarah graph path (`guardMemoryText`).
Drop a candidate that carries secret, credential, token, private-path, or
wallet material.
Do not scrub a secret into storage.

### 6.8 Deterministic identity

`entityId` is a derived identity for an extracted fact or entity.
It is not a user-chosen display name.
The derivation function is implementation-local if it is stable for one owner
scope and one redacted value.
The wire form is always `entity.<24 lowercase hex>`.

## 7. Derived graph rule

Derive the graph from the events. Never treat the graph as authority.

### 7.1 Two stores

```text
Durable record (authority)          Derived index (rebuildable)
+---------------------------+      +------------------------------+
| NIP-AE kind 30174 engrams | ---> | graph elements and relations |
| NIP-44 to the owner       |      | embeddings and vector plane  |
| blinded d tags            |      | ranking and feedback weights |
| companion openagents body |      | deletion derivation index    |
+---------------------------+      +------------------------------+
```

### 7.2 Rebuild rule

A projection rebuild reads engrams and produces the same graph.
If the rebuild fails, discard the projection.
A memory that no engram produced is a defect.

### 7.3 Host for the index

The owner decided that the default index lives in OpenAgents Google Cloud.
It is a cache with a rebuild path.
It is not a second authority.

Mitigations that stay mandatory:

1. Index only redacted, bounded derivatives.
2. Scope the index by the existing one-way owner digest.
3. Destroy and rebuild rather than hand-repair a corrupt index.
4. Let the owner list and delete the index for their scope without deleting
   engrams.
5. Do not claim the index is unreadable by OpenAgents. Claim only that the
   relay operator cannot read the ciphertext.

Client-side search over locally decrypted engrams remains valid.

### 7.4 Recall bounds

Preserve the current product bounds:

- default off behind `SARAH_GRAPH_MEMORY_RECALL_ENABLED`
- at most four recall items
- each summary at most 320 characters
- fail-soft: recall failure yields an empty slice
- no model spend in the write-back path

### 7.5 Listing truncation

NIP-AE listing is best-effort.
A truncated listing is a gap, not an empty set.
Surface a limit-reached signal when a relay caps results.

## 8. Deletion planning

1. Publish an in-band tombstone for the slug.
2. Optionally publish NIP-09 for honoring relays.
3. Rebuild the projection. Drop the tombstoned slug.
4. Walk reverse edges of `derivedFromSlugs` to plan dependent drops.
5. Do not retain a shadow copy of a tombstoned slug in the index.

Tombstones stay on the owned relay so a deletion is auditable.

## 9. Export

Export schema: `openagents.sarah_memory_export.v1`.

| Part | Content |
| --- | --- |
| Events | newline-delimited JSON of raw signed engram events, byte for byte |
| Manifest | agent pubkey, owner pubkey, event count, ordered event ids, export time, SHA-256 over the event lines |

Rules:

- Include tombstones.
- Exclude the derived index.
- Keep plaintext engrams encrypted inside the archive.
- Verification must work offline with any NIP-01 verifier.

## 10. Fixtures and verification

Canonical and negative fixtures live under `fixtures/sarah-nip-ae/`.

Verify with:

```sh
node fixtures/sarah-nip-ae/validate.mjs
```

Exit criteria for this packet:

1. This document freezes kind `30174` with NIP-44 to the owner and HMAC-blinded
   `d` tags.
2. This document freezes companion fields for provenance, identity, edges,
   admission, and derivation.
3. This document forbids ranking in the durable record.
4. This document states the derived-graph rule: engrams win.
5. Canonical fixtures and negative vectors pass `validate.mjs`.

## 11. Non-goals

This freeze does not:

- enable `SARAH_GRAPH_MEMORY_RECALL_ENABLED`
- publish a production engram
- change `nostr-effect` APIs
- migrate Cloud SQL graph envelopes
- define NIP-RS read state or NIP-ER reminders (`SARAH-NR-07` remainder)
- invent a second durable memory kind

## 12. Falsifier

A later packet breaks this freeze when it does any of these:

- writes Sarah memory as a kind other than `30174`
- writes plaintext memory on the wire
- puts a slug in a public tag
- puts ranking, score, weight, or embeddings in a durable body
- treats the graph projection as authority over engrams
- invents a second memory kind for fields this profile already carries
- ships a companion field without fixtures
- treats a truncated listing as a complete empty set
- treats relay acceptance as OpenAgents admission
