# 38. Releases as signed events

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 37 sets the terms. This chapter builds the registry that meets the
first of them: a release is a signed event that anyone can publish, on our
relay or any other.

## Draft

A registry is usually a server that someone owns. You ask it to accept your
package, and it can say no. Ours is a set of signed records.

A release is one event. It names a package, lists its components with the hash
of each, and is signed by the author's key. A release never changes. A fix is
a new release. A listing is a second, replaceable record that says what the
package is and which release is current.

Because a release is a signed event, where it is stored does not matter. We
run a relay that keeps them. Anyone else can run one. A registry reader
gathers releases from the relays it is told about and checks every signature
and hash itself. If our relay went away, every release would still be valid
wherever a copy exists.

Publishing needs a key and nothing else. There is no review before a release
appears. Review happens after, in public, as the evidence of Part V. A release
with no evidence is listed as a candidate, and the agent treats it as one.

We have done this before in small. [Episode 49](../../docs/transcripts/049.md)
sets up a plugin registry, and [Episode 66](../../docs/transcripts/066.md)
tests a community contribution that puts plugin discovery on Nostr. The idea
waited for the rest of the system to catch up.

The same registry holds every kind of member. Part IX adds realms and assets
to it.

## What we build

A plan for what this chapter ships once the draft is approved:

- Release and listing events, with a publish command that signs and sends
  them.
- A registry reader that gathers from several relays and verifies every
  signature and hash.
- A registry page on each surface that shows a package, its releases and its
  evidence.

It builds on what exists today:

- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)
- [`coax/lib/nostr/relay.coax`](../../coax/lib/nostr/relay.coax)
- [`coax/lib/nostr/relay_server.coax`](../../coax/lib/nostr/relay_server.coax)
- [`coax/lib/nostr/event_sign.coax`](../../coax/lib/nostr/event_sign.coax)
- [`apps/openagents-cli/relay.coax`](../../apps/openagents-cli/relay.coax)
- [`apps/openagents-cli/skill.coax`](../../apps/openagents-cli/skill.coax)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Release, listing and revocation | `openagents.extension.v1` `Release`, `Listing`, `Revocation` ([spec 20.2](../../docs/protocol/openagents-protocol-spec.md#202-openagentsextensionv1)) | Nostr kinds 3184, 30184 and 3185, public (NIP-EXT) | Spec sketch; no `.proto` file. `coax/lib/nostr/ext` handles the NIP's JSON |

Planned paths: `openagents/registry/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 2:00, the registry
  - [Episode 49, *Plugin Registry Setup*](../../docs/transcripts/049.md): a plugin registry
  - [Episode 55, *Plugin Registry UI*](../../docs/transcripts/055.md): a registry UI
  - [Episode 66, *Nostr Plugin Registry*](../../docs/transcripts/066.md): plugin discovery on Nostr
  - [Episode 288, *Three DevDays Later*](../../docs/transcripts/288.md): point your agents at the NIPs
- **Old code at `coder-v1.0.0`, crates:** `plugin`, `discovery`, `nostr-relay`, `nostr-transport`
- **NIPs:** [NIP-REG](../../nostr/nips/openagents/NIP-REG.md), [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-SKILL](../../nostr/nips/openagents/NIP-SKILL.md), [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P10, P12

## Open questions for the owner

1. Do we mirror releases from any relay by default, or only from a list we
   choose? An open mirror needs a plan for spam.
