# 49. One schema, three carriages

> **Status: outline.** This chapter lists what it will say and build. Its
> draft is not written, and paths marked planned do not exist.

## Premise

Chapters 46 and 48 describe two ways a record travels. This chapter says what
the record is: one protobuf message in `proto/`, carried three ways. It then
follows one record from its schema to a signed event and back.

## Outline

- **The rule.** Each record is defined once, as a protobuf message under
  [`proto/`](../../proto/README.md). Nothing else defines a wire shape
  ([spec 8](../../docs/protocol/openagents-protocol-spec.md#8-one-message-three-carriages-nostr-http-and-rpc)).
- **Carriage one: a signed event.** The message's JSON form is the content of
  a Nostr event or of an encrypted envelope. That JSON is what gets signed
  (chapter 46).
- **Carriage two: the API.** The same message is an HTTP JSON body. When a
  record is both an event and an API body, the JSON is the same (chapter 48).
- **Carriage three: binary.** The same message as protobuf bytes: frames on a
  socket, files, and calls across a native boundary. We use it where both ends
  are ours, or where size and speed matter. A binary form never replaces a
  NIP's wire without a new version that both sides agree on.
- **How a record becomes an event.** Build the message. Map it to the NIP's
  JSON: its keys, enum spellings, amounts as decimal strings and its rule for
  null. Put the JSON in the content with the kind and tags. Hash and sign.
  Send it to relays.
- **Reading goes the other way.** Check the signature over the bytes you
  received, then parse. Never check a signature over bytes you wrote again
  ([spec 35](../../docs/protocol/openagents-protocol-spec.md#35-conformance-and-verification)).
- **When a hash covers a record,** the bytes are canonical, and the rule is
  written down ([spec 7.4](../../docs/protocol/openagents-protocol-spec.md#74-canonical-bytes-and-hashing)).
- **Public and private.** Each message has a data class. A private record
  rides in a kind 3188 envelope, encrypted with NIP-44 ([spec 6](../../docs/protocol/openagents-protocol-spec.md#6-data-classes-descriptor-options-and-authority),
  [spec 8.3](../../docs/protocol/openagents-protocol-spec.md#83-encrypted-carriages)).
- **Names and versions.** A package starts as `v1alpha1` and reaches `v1`
  when it passes the conformance list. A published name, number or JSON
  spelling stays frozen ([spec 5](../../docs/protocol/openagents-protocol-spec.md#5-naming-versioning-and-stability)).
- **What Coax generates today.** `#import_proto` gives typed views and bounded
  binary codecs. JSON mappers are planned
  ([#13237](https://github.com/OpenAgentsInc/openagents/issues/13237)). Until
  then each NIP's JSON is written by hand, which chapter 52 allows when the
  bytes are pinned.
- **What exists.** The table below. The spec's "Implemented" labels describe
  code from before the repository was reset, so this book reads status from
  [`proto/registry.json`](../../proto/registry.json) and the compliance plan.
- **The money rule is in the schema.** An amount is millisats or a Taproot
  asset amount, and the list of rails is closed ([spec 26.1](../../docs/protocol/openagents-protocol-spec.md#261-the-money-rule)).

## What we build

A plan for what this chapter ships once the outline is approved:

- One worked example with a fixture: a release, from message to signed event
  and back, byte for byte.
- The status column of chapter 47's `PROTOCOL.md`: for each record, its
  schema, its carriages and its codec.

It builds on what exists today:

- [`proto/README.md`](../../proto/README.md)
- [`proto/registry.json`](../../proto/registry.json)
- [`docs/protocol/openagents-protocol-spec.md`](../../docs/protocol/openagents-protocol-spec.md)
- [`coax/proto.md`](../../coax/proto.md)
- [`coax/lib/proto/importer.coax`](../../coax/lib/proto/importer.coax)
- [`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax)
- [`coax/lib/nostr/event.coax`](../../coax/lib/nostr/event.coax)
- [`coax/lib/nostr/jcs.coax`](../../coax/lib/nostr/jcs.coax)

Where the schemas stand today:

| Status | Meaning | Packages |
| --- | --- | --- |
| In use | A schema file, one generated codec, a fixture, and a product path that calls it | `openagents.account.v1alpha1` (agent grants), `openagents.decision.v1alpha1` (on the server), `openagents.metareal.home.v1alpha1`, `openagents.metareal.haunt.v1alpha1`, `openagents.voice.v1alpha1` |
| In use, hand-packed | One shared hand-written codec, pinned by a profile with exact bytes | The Everglade realm session ([`everglade-session-v6.json`](../../proto/verse/everglade-session-v6.json)), its latency record, and the Orbit League frames |
| Schema file | A `.proto` in `proto/`. It has no product consumer, or the code still writes other bytes by hand | `nostr.*` mirrors, `openagents.pylon.v1alpha1`, `openagents.psionic.*`, `openagents.host.v1`, `openagents.reach.v1`, `openagents.engine.v1`, `openagents.common.v1`, the Coder and CLI records |
| Spec sketch | Drafted in the spec only. The NIP's JSON, written by a Coax library, is the definition today | Most of this book: `chat`, `capability`, `policy`, `run`, `trace`, `knowledge`, `extension`, `eval`, `xp`, `market`, `labor`, `payment`, `receipt` |
| Planned | Named, not drafted | `openagents.sync.v1alpha1`, `openagents.session.log.v1alpha1`, `openagents.verse.doc.v1alpha1` |

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 2:30, signed JSON over WebSockets
- **NIPs:** [the OpenAgents NIPs](../../nostr/nips/openagents/README.md)
- **Also:**
  - [The OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md), sections 3 to 8 and 12
  - [The protocol review](../../docs/protocol/2026-11-11-protocol-review.md) and [the compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md)
  - [`#import_proto`](../../coax/proto.md)
  - [Nostr From Scratch](../../nostr/book/README.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P12

## Open questions for the owner

1. Which record is the worked example: a release, which is public, or a grant,
   which is private?
2. Do chapters show schema text, or only link to `proto/`?
