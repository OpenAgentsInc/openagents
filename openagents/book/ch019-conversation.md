# 19. Conversation

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Chapters 12 to 18 end with a front that can route but has almost nothing to route to.
This chapter adds the member that takes everything no specialist claims:
general conversation, small talk and questions about policy.

## Draft

Most messages are not work. People say hello, ask what the agent can do, ask
whether something is allowed, or think out loud. [Episode
289](../../docs/transcripts/289.md) lists these routes next to coding: policy,
small talk and general. They belong to the agent as much as Coder does, and
this is promise P6.

The conversation member is a general chat model behind the gateway. The front
hands it whatever no specialist claims. It streams its reply, as our chat has
since [Episode 81](../../docs/transcripts/081.md). It knows which surface you
are on and which project you are in, because the turn carries that context. It
says so when you ask, and it doesn't pretend to know nothing.

Policy is its own route. When you ask what the agent will and won't do, the
answer comes from written rules and is the same each time. A model does not
make it up fresh on each turn.

Two rules of voice apply everywhere. OpenAgents speaks as "we". Coder and the
other agents speak as "I". And the agent says what it heard and what it did.
It doesn't fill the screen with status lines or with warnings nobody asked
for.

Conversation is the member the agent falls back to, which makes it the easiest
one to overuse. Chapter 16's measurement is what keeps the front from sending
real work here.

## What we build

A plan for what this chapter ships once the draft is approved:

- A conversation member on the gateway, with streaming and with the turn's
  surface and project as context.
- A policy route that answers from written rules.
- Labeled rows for each of these routes in the front's test set.

It builds on what exists today:

- [`coax/lib/gateway/chat.coax`](../../coax/lib/gateway/chat.coax)
- [`coax/lib/gateway/sse.coax`](../../coax/lib/gateway/sse.coax)
- [`coax/lib/text`](../../coax/lib/text)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Conversation request, result and feedback | `openagents.chat.v1` `ConversationRequest`, `ConversationResult`, `ConversationFeedback` ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1)) | Nostr kinds 25900, 26900 and 27000, encrypted (NIP-CJ); HTTP chat routes | Spec sketch; no `.proto` file. The reply stream to the page is hand-written lines today ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), step C9) |

### The chapter's check

- **Source:** the conversation member in
  `openagents/agent/members/conversation.coax`, behind a fake gateway that
  streams a recorded reply, and the policy route over its written rules.
- **Trace:** `hello` routes to conversation, and the reply arrives in its
  recorded parts as the clock releases them. The fake gateway's record
  shows the turn's surface and project in the request. A policy question
  asked twice prints the same answer both times, and the gateway records
  no model call for it.
- **Refusal:** the fake gateway cuts the stream after two parts. The
  transcript keeps the two parts and ends with one notice. The agent does
  not invent the rest of the reply.
- **Independent check:** the hand-labeled small talk, policy and general
  rows in chapter 16's held-out set, scored by the eval tool, which is
  not this member's code.

Planned paths: `openagents/agent/members/conversation.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 1:00, policy, small talk and general
  - [Episode 77, *Chat UI Buildout*](../../docs/transcripts/077.md): the chat UI
  - [Episode 81, *Streaming LLM Response*](../../docs/transcripts/081.md): streaming replies
  - [Episode 67, *Replacing ChatGPT*](../../docs/transcripts/067.md): replacing ChatGPT for daily use
  - [Episode 83, *Multimodal Chat*](../../docs/transcripts/083.md)
- **Old code at `coder-v1.0.0`, crates:** `openagents-chat`, `markdown-stream`, `model-access`, `openrouter`
- **NIPs:** [NIP-CJ](../../nostr/nips/openagents/NIP-CJ.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P6

## Open questions for the owner

1. Which model answers general conversation by default, and may a person
   choose another?
2. Where does the written policy live, and who can change it?
