# 9. Conversation

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Part I ends with a front that can route but has almost nothing to route to.
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
one to overuse. Chapter 6's measurement is what keeps the front from sending
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
