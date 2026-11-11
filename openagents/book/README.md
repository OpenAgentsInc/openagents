# The OpenAgents Book

Building a composable general agent from scratch, in
[Coax](../../coax/book/README.md).

> **Status: a first draft of all 60 chapters, written 2026-11-10 for the
> owner to edit.** Every chapter is a design draft. Nothing in this book is
> built yet. Each chapter says what it plans to ship, and plans are marked
> as plans. The book gives no dates.

OpenAgents builds open agents that people can own, inspect, improve and earn
from, on open protocols and Bitcoin. Since November 2023 we have worked on
that in public, across [290 episodes](../../docs/transcripts/README.md).
[Episode 289](../../docs/transcripts/289.md) says what the work adds up to:
**a composable general agent**. One agent sits behind every surface. It
answers from a shared knowledge base, hands work to Coder and to other
engines, grows by plugins that anyone can write, and pays their authors in
Bitcoin.

Most of that was built once, in a Rust tree that ended at the tag
`coder-v1.0.0`. The repository was then reset to a small, checked core:
Coax the language, Aethyr the engine and the Verse, one Nostr protocol,
Bitcoin and Lightning, and Psionic. This book rebuilds the whole of Episode
289 on that core. It starts from nothing, a single loop around one model,
and adds one idea per chapter until the whole thing stands.

It is both the story and the plan. Each chapter cites the episodes where an
idea was first tried, the old crates that implemented it, and the NIPs that
carry it. Then it says what we build now and what it builds on.

## How to read it

- **For the argument,** read chapters 1, 3, 5, 27, 37, 47 and 60. They give
  the loop, the composition, the typed front, the evidence rule, the market,
  the protocol and what 1.0 means.
- **For one promise,** find it in the [plan's coverage table](PLAN.md#promise-coverage)
  and read the chapters listed there.
- **To build,** follow the [order of work](PLAN.md#order-of-building). It
  follows section 7 of
  [the port audit](../../docs/audits/2026-11-10-episode-289-port-audit.md).
- **To edit,** start with the [open questions](PLAN.md#open-questions-for-the-owner).
  There are 73 of them, and several change what later chapters say.

What you need first: the Coax book's Parts I to III. Other books own the
foundations, and this book links to them and does not repeat them:

| Topic | Owner |
| --- | --- |
| The coding agent and its harness | [Coder: Coding Agents From Scratch](../../coder/book/README.md) |
| Events, relays and NIPs | [Nostr From Scratch](../../nostr/book/README.md) |
| Keys, signatures, Lightning | [Bitcoin From Scratch](../../bitcoin/book/README.md) |
| Models, decisions, serving, Pylons | [Psionic: ML From Scratch](../../psionic/book/README.md) |
| The engine and realms | [Building Aethyr, World Engine](../../aethyr/book/README.md) |
| A whole game on the engine | [Building Everglade](../../everglade/book/README.md) |

This book owns what none of them does: the composition (a front and its
members), the evidence that admits a member, and the economy around it.

## The parts

Each part ends with something that runs.

| Part | Chapters | What it adds | What runs at the end |
| --- | --- | --- | --- |
| I. One agent | 1–8 | Start from one loop and turn it into a front that decides what each message needs. | A chat on the web and in the terminal that answers through one runtime, routes each message with a typed decision, asks before any effect, and keeps a record you can read. |
| II. Knowledge and conversation | 9–13 | Give the agent something true to say: conversation, reviewed answers, a knowledge base, retrieval and memory. | Ask the agent about OpenAgents and get an answer that cites the page, book chapter or episode it came from. |
| III. Members that do work | 14–21 | Put the working members behind the front: Coder, other engines, subagents, programs, the command line, a wallet and the computers work runs on. | From one chat: have Coder fix an issue, ask for it to be done with another engine, open a deck, and send sats after you approve the payment. |
| IV. Plugins | 22–26 | Let other people add members: a plugin interface, a sandbox, grants, and a way to build a plugin by talking to the agent. | Ask for something the agent can't do, answer its questions, and watch it draft a plugin and its tests. |
| V. Evidence | 27–36 | Make the rule that lets the composition grow without getting worse: a component is a candidate until evidence says what admitting it changed. | Re-run the repository map's adoption from 2026-09-29 in Coax, with a second key reproducing it, and get the same verdict. |
| VI. The market | 37–45 | Open the member list to everyone and pay the people who fill it: a registry, pinned installs, paid calls, receipts, payouts, identity and reputation. | Publish a plugin under your own key, have another person's agent use it in a paid call, and receive your share over Lightning. |
| VII. The protocol is the agent | 46–49 | Show that every step so far is a signed event on one open protocol, then open the same system to partner apps and to people who host it themselves. | A second implementation that reads only the public events rebuilds an agent's member list and the evidence behind it; a partner app answers through the API; a self-hosted instance installs a plugin from the public registry. |
| VIII. Compute | 50–52 | Say where the models run: behind one gateway, on our own engine, and on machines that people lend for Bitcoin. | One request to the front is answered by a decision model running on a Pylon, and the Pylon's owner is paid for it. |
| IX. The Verse | 53–56 | Put all of it in a world you can walk through: the same agent, your working agents, makers who are paid, and characters that decide. | Walk into a realm, see your agents working, buy something a maker listed, and watch a guard make a decision that a Pylon served and was paid for. |
| X. Everywhere | 57–60 | Ship it: every surface as a thin client, a host for the one platform we lack, one release pipeline, and a plain statement of what 1.0 means. | The same version, built by one pipeline, installed from one download page on every platform Episode 289 named. |

## What a chapter contains

Every chapter file has the same sections:

- **Premise.** One paragraph: what this chapter adds to what came before.
- **Draft.** A few paragraphs of the prose, for the owner to edit.
- **What we build.** What the chapter will ship, the Coax libraries it
  builds on (real paths, linked), and the paths it plans to add.
- **Sources.** The episodes behind it, the old crates at `coder-v1.0.0`, the
  related [OpenAgents NIPs](../../nostr/nips/openagents/README.md), and the
  promises from the audit that it covers.
- **Open questions for the owner,** where there are any.

To read the old code, run `git worktree add ../oa-1.0 coder-v1.0.0`. The two
essays are at that tag too:
`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md` and
`git show coder-v1.0.0:docs/essays/2026-10-01-the-return-of-the-general-agent.md`.

## Conventions

- OpenAgents speaks as "we". Coder and other agents speak as "I".
- A number from the old tree is given as a number from the old tree. It
  describes that system at that time, and the new one has to earn its own.
- An episode shows what was said or shown on that day. It does not show
  that a feature exists now.
- Payments are Bitcoin and Lightning only.
- Routing is by typed decision or by embedding search, never by keyword
  matching.

## Status

Written 2026-11-10. All 60 chapters are design drafts. The
[registry](chapters.json) lists each chapter's part, promises, episodes,
NIPs, the paths it builds on and the paths it plans.

| Status | Meaning |
| --- | --- |
| design draft | The chapter says what will be built. The owner has not approved it yet. |
| approved | The owner has edited the chapter, and work on it can start. |
| partly implemented | Some of its examples run; the chapter says what remains. |
| implemented | Its examples run and its done criteria pass on a named host. |

## Chapters

## I. One agent

- [1. The simplest agent](ch001-the-simplest-agent.md) — design draft
- [2. One runtime, every surface](ch002-one-runtime-every-surface.md) — design draft
- [3. Members](ch003-members.md) — design draft
- [4. A typed plan](ch004-a-typed-plan.md) — design draft
- [5. Decide before you generate](ch005-decide-before-you-generate.md) — design draft
- [6. Measure the front](ch006-measure-the-front.md) — design draft
- [7. Grants and approvals](ch007-grants-and-approvals.md) — design draft
- [8. On the record](ch008-on-the-record.md) — design draft

## II. Knowledge and conversation

- [9. Conversation](ch009-conversation.md) — design draft
- [10. Prepared answers](ch010-prepared-answers.md) — design draft
- [11. A knowledge base](ch011-a-knowledge-base.md) — design draft
- [12. Retrieval by meaning](ch012-retrieval-by-meaning.md) — design draft
- [13. Memory and consent](ch013-memory-and-consent.md) — design draft

## III. Members that do work

- [14. Coder as a member](ch014-coder-as-a-member.md) — design draft
- [15. Other engines](ch015-other-engines.md) — design draft
- [16. Subagents you can see](ch016-subagents-you-can-see.md) — design draft
- [17. Programs](ch017-programs.md) — design draft
- [18. The command line behind the agent](ch018-the-command-line-behind-the-agent.md) — design draft
- [19. A wallet](ch019-a-wallet.md) — design draft
- [20. Spending approval](ch020-spending-approval.md) — design draft
- [21. Where work runs](ch021-where-work-runs.md) — design draft

## IV. Plugins

- [22. The plugin interface](ch022-the-plugin-interface.md) — design draft
- [23. The sandbox](ch023-the-sandbox.md) — design draft
- [24. Installing grants nothing](ch024-installing-grants-nothing.md) — design draft
- [25. Write a plugin](ch025-write-a-plugin.md) — design draft
- [26. The authoring interview](ch026-the-authoring-interview.md) — design draft

## V. Evidence

- [27. Test-time capabilities](ch027-test-time-capabilities.md) — design draft
- [28. Cases and graders](ch028-cases-and-graders.md) — design draft
- [29. A two-arm run](ch029-a-two-arm-run.md) — design draft
- [30. A claim's key](ch030-a-claims-key.md) — design draft
- [31. Reproduction by another key](ch031-reproduction-by-another-key.md) — design draft
- [32. External validation](ch032-external-validation.md) — design draft
- [33. Adoption is policy](ch033-adoption-is-policy.md) — design draft
- [34. Credit](ch034-credit.md) — design draft
- [35. Revalidation](ch035-revalidation.md) — design draft
- [36. The Gym](ch036-the-gym.md) — design draft

## VI. The market

- [37. Why an open market](ch037-why-an-open-market.md) — design draft
- [38. Releases as signed events](ch038-releases-as-signed-events.md) — design draft
- [39. Everyone gets it](ch039-everyone-gets-it.md) — design draft
- [40. Paid calls](ch040-paid-calls.md) — design draft
- [41. Metering and receipts](ch041-metering-and-receipts.md) — design draft
- [42. Payouts to authors](ch042-payouts-to-authors.md) — design draft
- [43. One identity](ch043-one-identity.md) — design draft
- [44. Reputation from reproduced claims](ch044-reputation-from-reproduced-claims.md) — design draft
- [45. Markets for work](ch045-markets-for-work.md) — design draft

## VII. The protocol is the agent

- [46. Signed JSON over WebSockets](ch046-signed-json-over-websockets.md) — design draft
- [47. Every step is an event](ch047-every-step-is-an-event.md) — design draft
- [48. The OpenAgents API](ch048-the-openagents-api.md) — design draft
- [49. Host it yourself](ch049-host-it-yourself.md) — design draft

## VIII. Compute

- [50. One gateway](ch050-one-gateway.md) — design draft
- [51. Models we serve](ch051-models-we-serve.md) — design draft
- [52. Pylons](ch052-pylons.md) — design draft

## IX. The Verse

- [53. The Verse is a surface](ch053-the-verse-is-a-surface.md) — design draft
- [54. Agents you can meet](ch054-agents-you-can-meet.md) — design draft
- [55. Realms and makers](ch055-realms-and-makers.md) — design draft
- [56. A decision in the world](ch056-a-decision-in-the-world.md) — design draft

## X. Everywhere

- [57. Thin clients](ch057-thin-clients.md) — design draft
- [58. A Windows host](ch058-a-windows-host.md) — design draft
- [59. One release pipeline](ch059-one-release-pipeline.md) — design draft
- [60. What 1.0 means](ch060-what-one-point-zero-means.md) — design draft

