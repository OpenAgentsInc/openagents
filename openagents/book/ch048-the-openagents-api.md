# 48. The OpenAgents API

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

The protocol is open to anyone who speaks it. This chapter adds the easier
door for products that only want the result: one API, the same one our own
apps use, that a partner app can build on. It is a second carriage of the same
schema, not a second protocol.

## Draft

Episode 289 says the open ecosystem should result in very simple interfaces.
You go to openagents.com, you load the mobile app, or "you use a partner app
consuming the OpenAgents API". This is promise P13.

A partner app is someone else's product with our agent inside it. Its
developer does not want to run relays or read event kinds. They want to send a
message and get back an answer, a plan to approve, or a stream of work.

So there is one API, over HTTP and WebSocket. A request carries a message and
its context. The reply streams what the runtime produces: the route, the plan,
any approval to ask the person for, each step and the result. A session, an
agent's API key or a signed request identifies the caller, and an agent's key
carries its grant (chapter 50). Limits are published. The API is described in
a machine-readable file, so a partner's coding agent can write the client.

The bodies come from the same schema as the events. A request and each item in
the reply stream are the JSON form of the messages chapter 47 lists, inside
one envelope for errors, pages and repeat-safe requests. Where a record is
also a NIP body, the API's JSON and the event's content are the same JSON. The
plan is to generate the machine-readable file from the schema, so the two
can't drift.

There is a third carriage, and part of the API already uses it. Agent grants
travel as binary protobuf, written by a codec that Coax generates from the
schema. Binary suits frames, files and calls where both ends are ours. Chapter
49 explains the three together.

Today most of the site's routes write their own JSON. The [compliance
plan](../../docs/protocol/2026-11-11-compliance-plan.md) orders the move onto
the envelope.

The rule that keeps the API honest is that our own apps use it. Chapter 2's
hosted runtime is behind it. There is no private API with more in it.

Money flows through as it does everywhere else. A partner's users can make
paid calls, plugin authors are paid from them, and the partner can add members
of its own to the same registry.

A partner need not take everything. Episode 289 says "or a subset of it they
care about". The API is grouped so that a product can use answers alone, or
Coder alone, or the registry alone.

[Episode 85](../../docs/transcripts/085.md) designs the first version of this,
joined to the store and to builder payments.

## What we build

A plan for what this chapter ships once the draft is approved:

- The API over HTTP and WebSocket: a message in, a typed stream of route,
  plan, approvals, steps and result out.
- Keys, published limits and a machine-readable description.
- A sample partner app, small enough to read in one sitting.

It builds on what exists today:

- [`apps/openagents-cli/api.coax`](../../apps/openagents-cli/api.coax)
- [`proto/README.md`](../../proto/README.md)
- [`docs/protocol/openagents-protocol-spec.md`](../../docs/protocol/openagents-protocol-spec.md)
- [`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax)
- [`coax/web/api/nip98.coax`](../../coax/web/api/nip98.coax)
- [`coax/lib/net`](../../coax/lib/net)
- [`coax/lib/gateway/sse.coax`](../../coax/lib/gateway/sse.coax)
- [`coax/lib/access`](../../coax/lib/access)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| The envelope: errors, pages, repeat-safe requests | `openagents.api.v1` ([spec 10](../../docs/protocol/openagents-protocol-spec.md#10-openagentsapiv1-the-http-envelope)) | HTTP JSON | Spec sketch; no `.proto` file. Each route writes its own JSON |
| A message in and the stream out | `openagents.chat.v1`; for hosted runs, planned `openagents.agent.managed.v1alpha1` events ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1), [spec 18.2](../../docs/protocol/openagents-protocol-spec.md#182-openagentsagentmanagedv1); [compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), step C5) | HTTP JSON and a stream | Hand-written JSON |
| Who is calling | A NIP-98 signed event, an `oa_key_` API key or a session ([sessions and tokens](../../docs/auth/sessions-and-tokens.md)) | HTTP headers | Built. NIP-98 is an external format |
| Agent grants | `openagents.account.v1alpha1` ([`agents.proto`](../../proto/openagents/account/v1alpha1/agents.proto)) | HTTP, as binary protobuf | In use: the generated codec `coax/web/api/agent-wire.coax` |
| The machine-readable description | `openagents.discovery.v1` `ApiDocument` ([spec 30.3](../../docs/protocol/openagents-protocol-spec.md#303-openagentsdiscoveryv1)), generated from the schema | HTTP | Planned |

Planned paths: `openagents/api/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 3:00, partner apps on the API
  - [Episode 85, *API Design*](../../docs/transcripts/085.md)
  - [Episode 169, *Agent Payments API*](../../docs/transcripts/169.md): an agent payments API
  - [Episode 242, *Khala, Collective Intelligence*](../../docs/transcripts/242.md): an API other tools can call
- **Old code at `coder-v1.0.0`, crates:** `gateway`, `oa-auth`, `oa-tokens`, `push-gateway`, `tenancy`, `coder-sdk`, `coder-service`
- **NIPs:** [NIP-CJ](../../nostr/nips/openagents/NIP-CJ.md), [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md), [NIP-KEYS](../../nostr/nips/openagents/NIP-KEYS.md)
- **Also:**
  - [The OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md), sections 8, 10 and 31
- **Schemas:** [`proto/`](../../proto/README.md) defines every wire shape, by the [protocol specification](../../docs/protocol/openagents-protocol-spec.md); [`proto/registry.json`](../../proto/registry.json) lists each shape and its one codec
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P13

## Open questions for the owner

1. Does the API include an endpoint in the shape other vendors' clients
   already speak, so existing tools can point at it?
2. What does API use cost a partner, and how does that sit with the author's
   share of paid calls?
