# 46. Signed JSON over WebSockets

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Earlier chapters publish events without saying much about what carries them.
This chapter states the one protocol under everything: signed JSON, sent over
WebSockets, through relays that anyone can run.

## Draft

[Episode 289](../../docs/transcripts/289.md): "No blockchain garbage, just
data, signed JSON over WebSockets. Very easy."

That sentence is the protocol. An event is a small JSON object: who wrote it,
when, what kind it is, some tags, the content, and a signature over all of it.
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

Nostr From Scratch builds all of this in Coax: the event model, a client, a
relay and the NIPs. This book uses that library and does not rebuild it. What
this chapter adds is the short version, and the relay we run for the agent's
own records.

## What we build

A plan for what this chapter ships once the draft is approved:

- A short tour that publishes one event and reads it back from two relays,
  using the existing library.
- The relay we run for agent records, with its limits published.
- A list of which records are public events and which are private, encrypted
  ones.

It builds on what exists today:

- [`coax/lib/nostr/event.coax`](../../coax/lib/nostr/event.coax)
- [`coax/lib/nostr/relay.coax`](../../coax/lib/nostr/relay.coax)
- [`coax/lib/nostr/relay_server.coax`](../../coax/lib/nostr/relay_server.coax)
- [`coax/lib/nostr/nip59.coax`](../../coax/lib/nostr/nip59.coax)
- [`nostr/book/README.md`](../../nostr/book/README.md)
- [`apps/openagents-cli/relay.coax`](../../apps/openagents-cli/relay.coax)

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
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P12
