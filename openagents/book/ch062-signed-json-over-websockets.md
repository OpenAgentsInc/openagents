# 62. Signed JSON over WebSockets

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Earlier chapters publish events without saying much about what carries them.
This chapter states the open carriage under everything public: signed JSON,
sent over WebSockets, through relays that anyone can run. It is one of three
ways a record travels, and all three carry the same schema.

## Draft

[Episode 289](../../docs/transcripts/289.md): "No blockchain garbage, just
data, signed JSON over WebSockets. Very easy."

That sentence is the public face of the protocol. An event is a small JSON
object: who wrote it, when, what kind it is, some tags, the content, and a
signature over all of it.
A relay is a server that accepts events and hands them to whoever asks. A
client connects to relays over a WebSocket, publishes events and subscribes to
the ones it wants. This is Nostr. It has no chain, no token and no global
agreement to wait for.

Three properties matter for an agent. An event is signed, so you know who said
it without trusting the relay that gave it to you. An event is plain data, so
any program in any language can read it. And relays can be replaced: the same
event can sit on many, and losing one loses nothing.

The archive comes back to this protocol many times: a plugin registry in
[Episode 66](../../docs/transcripts/066.md), login in [Episode
82](../../docs/transcripts/082.md), a client in [Episode
177](../../docs/transcripts/177.md), and negotiated markets in [Episode
266](../../docs/transcripts/266.md). Each time the reason is the same. A
platform can change its rules or disappear, and work built on it goes with it.

Where does an event's content come from? Each record in this book has one
definition: a protobuf message in [`proto/`](../../proto/README.md). Most
are still sketches in the spec, and chapter 65 lists which exist. The
message's JSON form goes in the event's content, and that JSON is what gets
signed. The same message is the JSON body of an API call (chapter 64). It can
also travel as binary frames where both ends are ours and size matters, as in
a realm session or sync. [Section 8 of the protocol
spec](../../docs/protocol/openagents-protocol-spec.md#8-one-message-three-carriages-nostr-http-and-rpc)
calls this one message, three carriages, and chapter 65 walks through it.

Signed JSON over WebSockets is the carriage for anything public or signed. Any
program can read it, and anyone can check who said it.

Today most of that JSON is written by hand, one Coax library for each NIP.
Coax generates binary codecs from a schema, and JSON ones are planned. Chapter
52 says when a hand-written codec is allowed.

Nostr From Scratch builds the carriage in Coax: the event model, a client, a
relay and the NIPs. This book uses that library and does not rebuild it. What
this chapter adds is the short version, and the relay we run for the agent's
own records.

## What we build

A plan for what this chapter ships once the draft is approved:

- A short tour that publishes one event and reads it back from two relays,
  using the existing library.
- The relay we run for agent records, with its limits published.
- A list of which records are public events and which are private, encrypted
  ones, taken from each schema's data class
  ([spec 6](../../docs/protocol/openagents-protocol-spec.md#6-data-classes-descriptor-options-and-authority)).

It builds on what exists today:

- [`coax/lib/nostr/event.coax`](../../coax/lib/nostr/event.coax)
- [`coax/lib/nostr/relay.coax`](../../coax/lib/nostr/relay.coax)
- [`nostr/relay/relay_server.coax`](../../nostr/relay/relay_server.coax)
- [`coax/lib/nostr/nip59.coax`](../../coax/lib/nostr/nip59.coax)
- [`nostr/book/README.md`](../../nostr/book/README.md)
- [`apps/openagents-cli/relay.coax`](../../apps/openagents-cli/relay.coax)
- [`proto/nostr/README.md`](../../proto/nostr/README.md)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| The event, a filter and the relay messages | `nostr.event.v1alpha2` `SignedEvent`, `Filter`, `ClientFrame`, `RelayFrame` ([`event.proto`](../../proto/nostr/event/v1alpha2/event.proto)). NIP-01 is the definition | Nostr: JSON over a WebSocket | Schema file with a generated Coax codec, tested against fixture bytes. It is a typed mirror: the signed JSON stays the authority, and `coax/lib/nostr` checks a signature over the bytes it received |
| A private record | `envelope.v1alpha1` `ArtifactEnvelope` ([spec 8.3](../../docs/protocol/openagents-protocol-spec.md#83-encrypted-carriages)) | Nostr kind 3188, encrypted with NIP-44 | Spec sketch; no `.proto` file. `coax/lib/nostr/envelope.coax` writes the JSON |
| Any record in this book | One message in `proto/` ([spec 8.1](../../docs/protocol/openagents-protocol-spec.md#81-the-rule)) | Nostr, HTTP or binary frames | See each chapter's table and [chapter 65](ch065-one-schema-three-carriages.md) |

Planned paths: none. The carriage is [`coax/lib/nostr`](../../coax/lib/nostr)
and the relay in [`nostr/relay`](../../nostr/relay).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 2:30, signed JSON over WebSockets
  - [Episode 288, *Three DevDays Later*](../../docs/transcripts/288.md): decentralized, permissionless protocols
  - [Episode 66, *Nostr Plugin Registry*](../../docs/transcripts/066.md): a registry on Nostr
  - [Episode 82, *Nostr Login*](../../docs/transcripts/082.md)
  - [Episode 177, *Commander as Nostr Client*](../../docs/transcripts/177.md): a Nostr client
  - [Episode 266, *Single Points of Failure and Nostr Markets*](../../docs/transcripts/266.md): single points of failure
  - [Episode 267, *Immortal Infrastructure*](../../docs/transcripts/267.md): separate relay, provider and client roles
- **Old code at `coder-v1.0.0`, crates:** `nostr`, `nostr-relay`, `nostr-transport`
- **NIPs:** [NIP-REG](../../nostr/nips/openagents/NIP-REG.md), [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md)
- **Also:**
  - [Nostr From Scratch](../../nostr/book/README.md)
  - [The OpenAgents NIPs](../../nostr/nips/openagents/README.md)
  - [The OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md), section 8, and [`proto/`](../../proto/README.md)
- **Schemas:** [`proto/`](../../proto/README.md) defines every wire shape, by the [protocol specification](../../docs/protocol/openagents-protocol-spec.md); [`proto/registry.json`](../../proto/registry.json) lists each shape and its one codec
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P12
