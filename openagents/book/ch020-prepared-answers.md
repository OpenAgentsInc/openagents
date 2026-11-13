# 20. Prepared answers

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Conversation generates every reply. This chapter adds a member that doesn't: a
bank of reviewed answers, served only when the front is sure one fits.

## Draft

Many questions about a product have one right answer. How do I install it?
What does it cost? Where is my wallet? Generating a fresh answer each time is
slow, costs more, and sometimes gets it wrong.

A prepared answer is written once, reviewed, and stored with the questions it
answers. When the front's decision says a stored answer fits, and says so
above a high threshold, the agent serves it. The turn costs one judgment and
no generation. In the old tree a prepared answer reached the phone in 0.6 to
0.7 seconds, where a generated one took 3 to 5.

The threshold matters more than the bank. A wrong prepared answer is worse
than a slow generated one, so the bank is tuned for precision. If the front
isn't sure, the question goes to the knowledge base (chapter 21) or to
conversation.

A prepared answer names its source: the page, chapter or episode it rests on.
When that source changes, the answer is checked again. A stale answer is a
defect, and we fix it when we find it.

Every answer that gives advice also gives the next step: a link, or the one
command to run.

## What we build

A plan for what this chapter ships once the draft is approved:

- An answer bank: entries with the answer, the questions it serves, its source
  and its review date.
- A serving rule tied to chapter 16's calibrated threshold, with the bank entry
  named in the log.
- A check that flags an answer when its source changes.

It builds on what exists today:

- [`coax/lib/nostr/kb`](../../coax/lib/nostr/kb)
- [`coax/lib/systemone`](../../coax/lib/systemone)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Answer-bank entry | `openagents.chat.v1` `AnswerBankEntry` ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1)) | A file the runtime loads | Spec sketch; no `.proto` file |

Planned paths: `openagents/knowledge/answers/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, asking about the knowledge base
  - [Episode 287, *Building a System One Coding Agent*](../../docs/transcripts/287.md): cheap judgments before generation
- **Old code at `coder-v1.0.0`, crates:** `openagents-chat`, `knowledge`, `oa-copy`
- **NIPs:** [NIP-KB](../../nostr/nips/openagents/NIP-KB.md), [NIP-CJ](../../nostr/nips/openagents/NIP-CJ.md)
- **Also:**
  - `programs/answer-question.json` at `coder-v1.0.0`
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "Our numbers: judgments before thinking"
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P2, P6

## Open questions for the owner

1. Who reviews a prepared answer before it is served: only us, or trusted
   contributors too?
