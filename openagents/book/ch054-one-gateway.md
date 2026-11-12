# 54. One gateway

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Every chapter has called "a model" without saying whose. This chapter builds
the answer: one gateway in front of every provider, so that members name what
they need and policy picks who serves it.

## Draft

The agent uses models for three jobs: decisions, conversation and long work.
No single provider is best at all three, and none is certain to be there next
year under the same terms.

The gateway is one interface over many providers. A member asks for a kind of
model and the gateway picks a route. The Coax tree already has routes to the
large hosted providers, to model routers, to Psionic and to Pylons. Each call
records which route answered, how long it took and what it cost.

The archive argues for this from experience. [Episode
67](../../docs/transcripts/067.md) replaces one provider's chat product in
daily use, and [Episode 150](../../docs/transcripts/150.md) argues that
neutral protocols and a choice of model can support an open market. [Episode
204](../../docs/transcripts/204.md) is the cost of the alternative: a provider
restricts access, and working setups break.

Being neutral does not make providers the same. They differ in tools, price,
limits and how they fail. So the choice is a written policy: which provider is
tried first for which job, and what happens when it is down or over its limit.
The owner's standing rule for hosted models is Google first: Vertex before
any other provider, with the others as fallbacks. Decisions go through the
Decisions API, which tries our own decision model first (chapter 5).

Chapter 6's lesson applies to every model call. One account running out of
credits should never stop the agent.

The gateway is also where cost becomes visible. Each step in a record carries
what it cost, so a person can see what a request spent and on what.

Rates and usage live here for the same reason. They are checked against what
providers report, not assumed.

## What we build

A plan for what this chapter ships once the draft is approved:

- The gateway as the only path to any model, for every member.
- A routing policy per job: order of providers and failover, with the route
  recorded on each call.
- Cost per step, checked against what the provider reports.

It builds on what exists today:

- [`coax/lib/gateway/gateway.coax`](../../coax/lib/gateway/gateway.coax)
- [`coax/lib/gateway/plan.coax`](../../coax/lib/gateway/plan.coax)
- [`coax/lib/gateway/rates.coax`](../../coax/lib/gateway/rates.coax)
- [`coax/lib/gateway/anthropic.coax`](../../coax/lib/gateway/anthropic.coax)
- [`coax/lib/gateway/openai.coax`](../../coax/lib/gateway/openai.coax)
- [`coax/lib/gateway/vertex.coax`](../../coax/lib/gateway/vertex.coax)
- [`coax/lib/gateway/openrouter.coax`](../../coax/lib/gateway/openrouter.coax)
- [`coax/lib/gateway/psionic.coax`](../../coax/lib/gateway/psionic.coax)
- [`coax/lib/gateway/pylon.coax`](../../coax/lib/gateway/pylon.coax)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A model request and its stream | `openagents.psionic.responses.v1alpha1` `CreateRequest`, `Response`, `StreamEvent` ([`responses.proto`](../../proto/openagents/psionic/responses/v1alpha1/responses.proto)); the wider package is `openagents.inference.v1` ([spec 16.2](../../docs/protocol/openagents-protocol-spec.md#162-openagentsinferencev1)) | HTTP, with a server-sent event stream | Schema file for the Psionic profile; the rest is a spec sketch. Each provider's request body is an external format |
| Which route answered and what it cost | `openagents.decision.v1` `DecisionCall` ([spec 16.1](../../docs/protocol/openagents-protocol-spec.md#161-openagentsdecisionv1)); usage is in `openagents.common.v1` ([spec 9.9](../../docs/protocol/openagents-protocol-spec.md#99-usage)) | In the run journal (chapter 8) | Spec sketch; no `.proto` file |

## Sources

- **Episodes:**
  - [Episode 67, *Replacing ChatGPT*](../../docs/transcripts/067.md): replacing one provider's product
  - [Episode 89, *Goodbye ChatGPT*](../../docs/transcripts/089.md): leaving it for good
  - [Episode 150, *Neutrality Wins*](../../docs/transcripts/150.md)
  - [Episode 204, *DO NOT BREAK USERSPACE*](../../docs/transcripts/204.md)
  - [Episode 278, *Coder Commands Codex & Claude*](../../docs/transcripts/278.md): a provider-neutral agent
  - [Episode 181, *American DeepSeek?*](../../docs/transcripts/181.md): studying an open model
- **Old code at `coder-v1.0.0`, crates:** `model-access`, `openrouter`, `inference`, `gateway`
- **NIPs:** [NIP-POL](../../nostr/nips/openagents/NIP-POL.md), [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md)
- **Also:**
  - [Coder book, chapter 13: Choose an engine](../../coder/book/ch013-choose-an-engine.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P7, P12

## Open questions for the owner

1. What is the default order of providers for each job: decisions,
   conversation and long work?
