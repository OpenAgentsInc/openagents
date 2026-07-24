# Sarah Nostr record contract

- Date: 2026-07-24
- Class: contract freeze
- Packet: `SARAH-NR-00`
- OpenAgents issue: [OpenAgentsInc/openagents#9217](https://github.com/OpenAgentsInc/openagents/issues/9217)
- Master tracker: [OpenAgentsInc/omega#31](https://github.com/OpenAgentsInc/omega/issues/31)
- Spec home: [2026-07-24-sarah-workroom-mvp-spec.md](./2026-07-24-sarah-workroom-mvp-spec.md) §22 and §24.1
- Fixtures: `fixtures/sarah-nostr-record/`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: frozen for later Sarah Nostr record packets

## 1. Purpose

This freeze locks the Sarah conversation record on Nostr.

A second implementation must interoperate from this document and the fixtures
alone.

This freeze does not deploy a relay.
This freeze does not mint a Sarah signing key.
This freeze does not build a Khala Sync client.
This freeze does not change `nostr-effect`.

Relay acceptance is never an OpenAgents admission.

## 2. Authority chain

| Role | Artifact | Note |
| --- | --- | --- |
| Product intent | `specs/openagents/sarah-owner-orchestrator.product-spec.md` | revision 6, SARAH-AC-28..31 |
| Sarah authority | `docs/authority/SARAH_AUTHORITY.md` | revision 7, `program.sarah_nostr_runtime` |
| Workroom specification | `docs/omega/2026-07-24-sarah-workroom-mvp-spec.md` | revision 4, §22 and §24.1 |
| Runtime receipt schema | `packages/authority` | `openagents.authority_decision_receipt.v1` |
| Protocol home | `OpenAgentsInc/nostr-effect` | executable NIP library. This packet does not edit it |

## 3. Laws

1. NIP-AO kind `24200` is ephemeral. A compliant relay broadcasts it and does
   not store it. It is not the durable turn record.
2. The durable turn record and the authority receipt use stored kinds. They are
   regular, append-only events.
3. Conversation content and receipt payloads are NIP-44 encrypted to the owner.
   The relay operator must not read plaintext.
4. A signed event is a record. It never decides admission. Admission stays in
   `resolveAuthorityDecision`.
5. Exact metering, the credit ledger, target broker execution, secrets, and Git
   object safety stay outside the relay. See §10.
6. No opaque custom kind without this specification, canonical fixtures,
   negative vectors, and a NIP-31 `alt` value.

## 4. Conversation identifier

### 4.1 Form

The Sarah owner conversation identity is the triple:

1. the owner Nostr public key
2. the Sarah Nostr public key for `principal.sarah`
3. one conversation identifier tag

The conversation identifier tag is:

```text
["conversation", "sarah.<24 lowercase hex>"]
```

The `<24 lowercase hex>` value is the same digest that the current Khala Sync
reference uses after `thread.sarah.`.

### 4.2 Legacy mapping

| Legacy reference | Nostr conversation tag value |
| --- | --- |
| `thread.sarah.<digest>` | `sarah.<digest>` |

A migration packet must keep both forms resolvable.
A client that sees only the legacy form must derive the Nostr tag value by
removing the `thread.` prefix.
A client that sees only the Nostr tag value must derive the legacy form by
adding the `thread.` prefix.

The raw owner id must never enter a tag, an event, or a fixture.

### 4.3 Scope tags

Every durable Sarah record event in this contract must carry:

| Tag | Count | Rule |
| --- | --- | --- |
| `conversation` | exactly one | value matches §4.1 |
| `p` | exactly one | owner public key |
| `agent` | exactly one | Sarah public key, equal to `pubkey` for Sarah-authored events |
| `alt` | exactly one | NIP-31 fallback. See §8 |

## 5. Durable turn-record kind

### 5.1 Kind and extension

| Field | Value |
| --- | --- |
| Kind | `44300` |
| NIP-01 class | regular (stored, append-only) |
| Extension id | `openagents.sarah.turn_record` |
| Extension version | `1` |
| Schema id inside payload | `openagents.sarah.turn_record.v1` |

Advertise the extension through relay `supported_extensions`.
Do not put this extension in `supported_nips`.

Kind `44300` is outside the ephemeral range `20000` to `29999`.
A relay that stores ordinary regular events must store kind `44300`.

### 5.2 Outer event

```json
{
  "kind": 44300,
  "pubkey": "<sarah_pubkey>",
  "created_at": 1753387200,
  "content": "<NIP-44 v2 ciphertext>",
  "tags": [
    ["p", "<owner_pubkey>"],
    ["agent", "<sarah_pubkey>"],
    ["conversation", "sarah.<24 hex>"],
    ["entry", "<entry_kind>"],
    ["turn", "<turn_ref>"],
    ["alt", "OpenAgents Sarah turn record (encrypted)"],
    ["e", "<parent_event_id>", "", "<marker>"]
  ],
  "sig": "<signature>"
}
```

Required tags: `p`, `agent`, `conversation`, `entry`, `turn`, `alt`.
Causal `e` tags follow §7.

`entry` must be one of:

| `entry` value | Maps from current runtime event |
| --- | --- |
| `turn.started` | `turn.started` |
| `tool.call` | `tool.call` |
| `tool.result` | `tool.result` |
| `tool.error` | `tool.error` |
| `turn.finished` | `turn.finished` |
| `turn.interrupted` | `turn.interrupted` |

Live `text.delta`, `text.completed`, and interrupt control stay on NIP-17 and
NIP-AO. They are not kind `44300` entries.

### 5.3 Encryption

`content` must be NIP-44 v2 ciphertext with key material
`(sarah_privkey, owner_pubkey)`.

Plaintext must not appear in `content`, tags, logs, or fixtures that claim to
be wire events.
Decrypted payload size must not exceed `65535` bytes.

### 5.4 Decrypted payload

```json
{
  "schema": "openagents.sarah.turn_record.v1",
  "entry": "turn.started",
  "conversation": "sarah.<24 hex>",
  "turnRef": "turn.<opaque>",
  "seq": 1,
  "timestamp": "2026-07-24T20:00:00.000Z",
  "parents": [
    {
      "eventId": "<64 hex>",
      "marker": "prompt"
    }
  ],
  "payload": {}
}
```

Rules:

- The writer must include `schema`, `entry`, `conversation`, `turnRef`,
  `seq`, `timestamp`, and `parents`.
- `seq` is a positive integer. Increase it for each durable entry in one turn.
- `timestamp` is ISO 8601 UTC.
- The writer must include `payload`. The value may be `{}`.
- Keep `payload` public-safe. Do not put raw prompts, raw tool output,
  credentials, private paths, or wallet material in it.
- Readers must ignore unknown `entry` values. Writers must refuse them in
  this revision.

### 5.5 Authorship, deletion, replacement, relay derivation

| Rule | Requirement |
| --- | --- |
| Authorship | Sarah signs kind `44300`. The owner does not author this kind. |
| Deletion | Author may publish NIP-09 kind `5`. Deletion does not rewrite history. Dependents keep their `e` links. |
| Replacement | Forbidden. No `d` tag. No parameterized replaceable address. |
| Relay derivation | Forbidden. A relay must not synthesize kind `44300`. |

## 6. Authority-receipt kind

### 6.1 Kind and extension

| Field | Value |
| --- | --- |
| Kind | `44301` |
| NIP-01 class | regular (stored, append-only) |
| Extension id | `openagents.sarah.authority_receipt` |
| Extension version | `1` |
| Payload schema | `openagents.authority_decision_receipt.v1` |

Advertise through `supported_extensions`, never through `supported_nips`.

### 6.2 Outer event

```json
{
  "kind": 44301,
  "pubkey": "<sarah_pubkey>",
  "created_at": 1753387201,
  "content": "<NIP-44 v2 ciphertext>",
  "tags": [
    ["p", "<owner_pubkey>"],
    ["agent", "<sarah_pubkey>"],
    ["conversation", "sarah.<24 hex>"],
    ["receipt", "<receipt_ref>"],
    ["alt", "OpenAgents Sarah authority receipt (encrypted)"],
    ["e", "<parent_event_id>", "", "authority-of"]
  ],
  "sig": "<signature>"
}
```

Required tags: `p`, `agent`, `conversation`, `receipt`, `alt`.
At least one causal `e` tag with marker `authority-of` or `parent` is
required.

### 6.3 Encryption

Same NIP-44 rule as §5.3: encrypt to the owner with
`(sarah_privkey, owner_pubkey)`.

### 6.4 Decrypted payload

The payload must carry the exact fields of
`openagents.authority_decision_receipt.v1` as emitted by
`packages/authority`:

| Field | Type |
| --- | --- |
| `schema` | literal `openagents.authority_decision_receipt.v1` |
| `receiptRef` | non-empty ref |
| `profileRef` | non-empty ref |
| `profileRevision` | integer >= 1 |
| `programRef` | non-empty ref |
| `grantRef` | ref or `null` |
| `actorRef` | non-empty ref |
| `actorRole` | non-empty ref |
| `action` | non-empty ref |
| `targetRef` | non-empty ref |
| `triggerRef` | non-empty ref |
| `conditionResults` | array of `{ conditionRef, passed, evidenceRefs }` |
| `startedAt` | ISO 8601 UTC |
| `settledAt` | ISO 8601 UTC |
| `outcome` | `succeeded` or `refused` |
| `evidenceRefs` | array of refs |

Two honesty rules stay absolute:

1. Publishing a receipt never makes a decision.
2. A missing receipt never permits an action.

### 6.5 Authorship, deletion, replacement, relay derivation

| Rule | Requirement |
| --- | --- |
| Authorship | Sarah publishes after the authority service settles. The receipt records a prior decision. |
| Deletion | Author may publish NIP-09 kind `5`. The decision record in OpenAgents systems is not deleted by relay tombstones alone. |
| Replacement | Forbidden. Append-only. |
| Relay derivation | Forbidden. |

## 7. Causal link rules

### 7.1 Markers

Use NIP-01 `e` tags with a fourth-position marker:

| Marker | Meaning |
| --- | --- |
| `prompt` | owner NIP-17 message that started the turn |
| `root` | first durable entry of the turn (`turn.started`) |
| `parent` | immediate prior durable entry in the same turn |
| `authority-of` | authority receipt that covers this entry, or the entry that requested the receipt |
| `reply` | reserved for conversation-thread use. Do not use it for turn ladder edges in this revision |

### 7.2 Required parents

| Entry | Required parents |
| --- | --- |
| `turn.started` | one `prompt` parent: the owner message event id |
| `tool.call` | one `parent` or `root`: prior durable entry in the turn |
| `tool.result` / `tool.error` | one `parent`: the matching `tool.call` entry |
| `turn.finished` / `turn.interrupted` | one `parent` or `root`: last durable step before terminal |
| authority receipt `44301` | one `authority-of` or `parent`: the turn or tool entry that needed the decision |

Do not publish a durable kind `44300` or `44301` entry with zero parents.
This revision admits no exception.

### 7.3 Reader rule

An owner must be able to ask why an effect happened and walk signed parents to
the prompt and the authority receipt.
A missing parent link is a contract defect, not a soft gap.

## 8. NIP-31 `alt` values

| Kind | Required `alt` text |
| --- | --- |
| `44300` | `OpenAgents Sarah turn record (encrypted)` |
| `44301` | `OpenAgents Sarah authority receipt (encrypted)` |

The `alt` text must stay free of secrets, prompts, tool names, receipt refs,
and owner identifiers.

## 9. Projection map for workroom §7

Each pane projection has one carrier and one writable authority.

**Room.** Carrier: the conversation triple in §4, plus NIP-OA and NIP-AA
attestation for Sarah. Wire fields: `conversation`, owner `p`, Sarah
`agent`, and the principal display name from the client profile. Writable
authority: OpenAgents Sarah bootstrap and Omega identity binding. Not the
relay.

**Transcript.** Carrier: NIP-17 kind `14` rumors in NIP-59 wraps. Wire
fields: standard NIP-17 message fields. Writable authority: the owner for
owner messages, Sarah for Sarah answers.

**Activity.** Carrier: kind `44300` durable entries. NIP-AO kind `24200` is
live only. Wire fields: `entry`, `turn`, `seq`, and public-safe `payload`.
Writable authority: Sarah turn service.

**Receipts.** Carrier: kind `44301`. Wire fields: the
`openagents.authority_decision_receipt.v1` fields. Writable authority: the
authority service decides, and Sarah publishes the receipt event.

**Run state.** Carrier: kind `44300` terminal `entry` values, plus NIP-AO
`cancel_turn` control. Wire fields: `turn.started`, `turn.finished`,
`turn.interrupted`, and live control. Writable authority: Sarah turn service
for settlement, and the owner for interrupt intent.

Every projected row still carries a source label, a freshness label, and a gap
label at the client seam. Those labels are client projection fields. They are
not Nostr tags in this freeze.

## 10. Boundary contract from workroom §21

This section is normative.

| Surface | Must stay outside the relay | Contract statement |
| --- | --- | --- |
| Exact `token_usage_events` | yes | A kind `44300` or NIP-AM event may hold a public-safe usage reference. It must not become the metering row. |
| Public served-token counter | yes | The counter reconciles only to exact Cloud SQL rows. |
| Admission and authority resolution | yes | `resolveAuthorityDecision` decides. Kind `44301` only records. |
| Target broker execution and its receipts | yes | Full Auto, coding capacity, releases, and sandboxes keep their systems. |
| Raw secrets, credentials, mnemonics | yes | No secret enters an event, a tag, a fixture plaintext field, or a log. |
| Git objects and refs | yes | Git keeps object safety. NIP-34 may carry coordination later. |
| Provider credentials and model access | yes | Runtime only. |

Relay acceptance of an event is not OpenAgents admission.
It does not admit a turn, a tool call, a payment, a release, or a public claim.

## 11. Relation to live NIP-AO and NIP-17

| Concern | Live carrier | Durable carrier |
| --- | --- | --- |
| Tool telemetry during a turn | NIP-AO `24200` | kind `44300` entry |
| Cancel turn | NIP-AO control `cancel_turn` | kind `44300` `turn.interrupted` after settlement |
| Owner text and Sarah answer text | NIP-17 kind `14` | NIP-17 stored wraps |
| Usage metric | optional NIP-AM `44200` | not a substitute for Cloud SQL rows |

## 12. Fixtures and verification

Canonical and negative fixtures live under
`fixtures/sarah-nostr-record/`.

Verify with:

```sh
node fixtures/sarah-nostr-record/validate.mjs
```

Exit criteria for this packet:

1. This document specifies kinds `44300` and `44301` with authorship,
   encryption, deletion, replacement, and relay-derivation rules.
2. This document specifies conversation identity and the legacy mapping.
3. This document specifies causal link markers and required parents.
4. This document fixes the NIP-31 `alt` values.
5. §9 maps every §7 projection field to one carrier and one writable
   authority.
6. §10 records the §21 boundary as a contract.
7. Canonical fixtures and negative vectors pass `validate.mjs`.

## 13. Non-goals

This freeze does not:

- implement builders or parsers in `nostr-effect`
- deploy `relay.openagents.com`
- cut over the live Khala Sync Sarah thread
- admit NIP-AE memory event shapes beyond the already recorded memory audit
- invent a second authority schema

## 14. Falsifier

A later packet breaks this freeze when it does any of these:

- writes a durable Sarah turn or receipt as kind `24200`
- writes plaintext conversation or receipt content on the relay
- omits the conversation tag
- omits causal parents
- omits `alt`
- treats relay acceptance as admission
