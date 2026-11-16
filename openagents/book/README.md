# The OpenAgents Book

Building the OpenAgents apps, and the general agent behind them, in
[Coax](../../coax/book/README.md) on [Coax Native](../../coax/native.md),
one small step at a time.

> **Status: design drafts, for the owner to edit.** 75 chapters are design
> drafts and three are planned outlines. No chapter is implemented on the
> new iOS app yet. The opening was rewritten on 2026-11-15
> ([#13645](https://github.com/OpenAgentsInc/openagents/issues/13645)): the
> iOS app is rebuilt as compiled Coax drawn on the GPU, starting from one
> text input. The first milestone recorded on 2026-11-13
> ([#13516](https://github.com/OpenAgentsInc/openagents/issues/13516)) ran on
> the old app, which is being removed; chapter 6 earns it again.
> Where a piece already runs in the repository, such as the login service
> and the GPU login screen, the chapter says so and builds on it.
> [CONTRACT.md](CONTRACT.md) says what moves a chapter forward. The book
> gives no dates.

OpenAgents builds open agents that people can own, inspect, improve and earn
from, on open protocols and Bitcoin. Since November 2023 we have worked on
that in public, across [290 episodes](../../docs/transcripts/README.md).
[Episode 289](../../docs/transcripts/289.md) says what the work adds up to:
**a composable general agent**. One agent sits behind every surface. It
answers from a shared knowledge base, hands work to Coder and to other
engines, grows by plugins that anyone can write, and pays their authors in
Bitcoin.

This book builds that, and it builds the app you use it through first. You
start with an iPhone app that has one screen, the chat page, and on it one
text input. Each chapter adds one visible thing: Send, a transcript, a reply
from our service, the login card when a send needs an account. Chapter 6
runs one case through every step on the simulator and a physical phone.
Chapters 7 to 11 add history, the environment and repository chips,
settings, the desktop and Android, and the web. Chapter 12 puts the first
agent of our own behind the chat page, and every later chapter adds one
capability to an app that already runs.

The iOS app is compiled Coax linked into the binary: no JavaScript, no
JavaScriptCore, no Wasm and no over-the-air updates. Every change ships in a
TestFlight or store build. The shared display-list kit in
[`components/`](../../components) draws every component on the GPU
([GPU components on native](../../components/gpu-components.md)); the
platform supplies only the focused text field's editor, accessibility and
system sheets. The desktop (Mac and Linux) and Android follow and reuse the
same pieces. The web keeps DOM for text surfaces (chapter 11).

Each chapter cites the episodes where an idea was first tried, the old
crates that implemented it in the Rust tree that ended at `coder-v1.0.0`,
and the NIPs that carry it. Then it says what we build now, what it builds
on, and how a run proves it.

## Book-driven development

The book leads and the app follows:

- **The chapter comes first.** A chapter is written, then the app is built
  to it. Code that no chapter asks for is not written.
- **The app is what the book has built.** The app on `main` contains
  exactly what the book has built through its latest implemented chapter,
  and nothing else. A feature that is not in a chapter is not in the app.
  Code from the old app comes back only when a chapter rebuilds it the new
  way.
- **One small increment per chapter.** Each chapter adds one thing you can
  see or use and leaves the app running. Its check
  ([CONTRACT.md](CONTRACT.md#the-chapters-check)) shows the increment works
  before the next chapter starts.
- **The chapter changes with the code.** When the build teaches something
  the draft got wrong, the chapter is corrected in the same change.

## How to read it

- **To build the app,** read Parts I and II in order. Chapter 6 is the
  first milestone: the iPhone app types a message, logs in when the send
  needs it, receives a reply and stops a pending one, checked by one case on
  the simulator and a physical phone.
- **For the argument,** read chapters 10, 12, 13, 15, 37, 43, 53, 63 and 78.
  They give the one app, the loop, the composition, the typed front,
  execution ownership, the evidence rule, the market, the protocol and what
  1.0 means.
- **For the wire,** read chapter 4, then chapters 62 to 67 beside
  [the OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md)
  and [`proto/`](../../proto/README.md).
- **For one promise,** find it in the [plan's coverage table](PLAN.md#promise-coverage).
- **To edit,** start with the [open questions](PLAN.md#open-questions-for-the-owner).

What you need first: the Coax book's Parts I to III. Other books and specs
own the foundations, and this book links to them and does not repeat them:

| Topic | Owner |
| --- | --- |
| The app framework: state, components, hosts, `coax drive` | [Coax Native](../../coax/native.md), [Coax Native Web](../../coax/web.md), [the components spec](../../components/spec.md), [GPU components on native](../../components/gpu-components.md) |
| The coding agent and its harness | [Coder: Coding Agents From Scratch](../../coder/book/README.md) |
| Events, relays and NIPs | [Nostr From Scratch](../../nostr/book/README.md) |
| Keys, signatures, Lightning | [Bitcoin From Scratch](../../bitcoin/book/README.md) |
| Models, decisions, serving, Pylons | [Psionic: ML From Scratch](../../psionic/book/README.md) |
| The engine and realms | [Building the OpenAgents Metaverse Engine](../../metaverse-engine/book/README.md) |
| A whole game on the engine | [Building Everglade](../../everglade/book/README.md) |

This book owns what none of them does: the OpenAgents apps as a product, the
composition of the agent (a front and its members), the evidence that admits
a member, and the economy around it. It uses the framework and writes no
second component toolkit.

## The parts

Each part ends with something that runs.

| Part | Chapters | What runs at the end |
| --- | --- | --- |
| I. The chat page, one step at a time | 1–6 | The iPhone app's chat page on the GPU kit: a text input, Send, a transcript, a streamed reply with Stop, and the login card, with one acceptance case passing in the simulator and on a phone. |
| II. What the app adds next | 7–11 | History and the sidebar, the environment and repository chips, settings, the same app on the desktop and Android, and the web's DOM presentation. |
| III. One general agent | 12–23 | The loop, members, typed plans, routing, approvals, streaming and the record, with knowledge and memory, answering the chat page. |
| IV. Capabilities on your devices | 24–33 | Coder and other engines, remote work, files, voice, and the wallet with payment review, from one conversation. |
| V. Continuity across devices | 34–37 | State that survives, reconnects, sync, background and resume, and one execution owner per task. |
| VI. An extensible agent and its economy | 38–70 | Plugins, evidence, adoption, the registry, paid calls, receipts, payouts, the protocol, self-hosting and compute. |
| VII. Worlds and delivery | 71–78 | The agent in realms, cross-host conformance, releases from one source, additional platforms and what 1.0 means. |

## What a chapter contains

Every chapter file has the same sections:

- **Premise.** One paragraph: what this chapter adds to what came before.
- **Draft.** The prose, for the owner to edit. A planned chapter has an
  **Outline** here instead: one line for each point the draft will make.
- **What we build.** What the chapter will ship, the code it builds on (real
  paths, linked) and the paths it plans to add. A chapter that teaches code
  has **The chapter's check**: its Coax source, the exact trace a run prints,
  a case it must refuse, and an independent check. A chapter that sends,
  signs, stores or publishes a record has an "On the protocol" table:
  chapter 65 explains its columns.
- **Sources.** The episodes behind it, the old crates at `coder-v1.0.0`, the
  related [OpenAgents NIPs](../../nostr/nips/openagents/README.md) and the
  promises from the audit that it covers.
- **Open questions for the owner,** where there are any.

To read the old code, run `git worktree add ../oa-1.0 coder-v1.0.0`.

## Conventions

- OpenAgents speaks as "we". Coder and other agents speak as "I".
- A number from the old tree is given as a number from the old tree. The new
  one has to earn its own.
- An episode shows what was said or shown on that day. It does not show
  that a feature exists now.
- A planned feature is not a result. A chapter says "runs today" only for
  code in the repository, and names a target only with recorded evidence
  ([CONTRACT.md](CONTRACT.md)).
- Payments use Bitcoin rails only: Lightning, on-chain Bitcoin, Spark and
  Taproot Assets.
- It is "Log in", never "Sign in". We never collect a phone number.
- A record has one schema, in `proto/`.
- Routing is by typed decision or by embedding search, never by keyword
  matching.

## Status

The [registry](chapters.json) holds each chapter's status, dependencies,
implemented source, qualified targets and adoption, in the shared format
that `python3 psionic/tests/book.py --book openagents` checks.
[provenance.json](provenance.json) holds the rest: part, promises, episodes,
NIPs, old crates and protocol packages.

| Status | Meaning |
| --- | --- |
| planned | An outline. The draft is not written. |
| design draft | The chapter says what will be built. Owner edits are recorded in the chapter. |
| partly implemented | Some of the chapter's checks run on a named target; the chapter says what remains. |
| implemented | All of the chapter's checks run on its named targets ([CONTRACT.md](CONTRACT.md)). |

## Chapters

<!-- chapters:begin -->
### I. The chat page, one step at a time

| Chapter | Status |
| --- | --- |
| [1. A text input on the GPU](ch001-a-text-input-on-the-gpu.md) | Unimplemented: design draft |
| [2. State and actions](ch002-state-and-actions.md) | Unimplemented: design draft |
| [3. A transcript](ch003-a-transcript.md) | Unimplemented: design draft |
| [4. A reply from the service](ch004-a-reply-from-the-service.md) | Unimplemented: design draft |
| [5. Logging in when a send needs it](ch005-logging-in-when-a-send-needs-it.md) | Unimplemented: design draft |
| [6. One case, every step](ch006-one-case-every-step.md) | Unimplemented: design draft |

### II. What the app adds next

| Chapter | Status |
| --- | --- |
| [7. History and the sidebar](ch007-history-and-the-sidebar.md) | Unimplemented: design draft |
| [8. Environment and repository chips](ch008-environment-and-repository-chips.md) | Unimplemented: design draft |
| [9. Settings](ch009-settings.md) | Unimplemented: design draft |
| [10. Desktop and Android](ch010-desktop-and-android.md) | Unimplemented: design draft |
| [11. The web](ch011-the-web.md) | Unimplemented: design draft |

### III. One general agent

| Chapter | Status |
| --- | --- |
| [12. The simplest agent](ch012-the-simplest-agent.md) | Unimplemented: design draft |
| [13. Members](ch013-members.md) | Unimplemented: design draft |
| [14. A typed plan](ch014-a-typed-plan.md) | Unimplemented: design draft |
| [15. Decide before you generate](ch015-decide-before-you-generate.md) | Unimplemented: design draft |
| [16. Measure the front](ch016-measure-the-front.md) | Unimplemented: design draft |
| [17. Grants and approvals](ch017-grants-and-approvals.md) | Unimplemented: design draft |
| [18. On the record](ch018-on-the-record.md) | Unimplemented: design draft |
| [19. Conversation](ch019-conversation.md) | Unimplemented: design draft |
| [20. Prepared answers](ch020-prepared-answers.md) | Unimplemented: design draft |
| [21. A knowledge base](ch021-a-knowledge-base.md) | Unimplemented: design draft |
| [22. Retrieval by meaning](ch022-retrieval-by-meaning.md) | Unimplemented: design draft |
| [23. Memory and consent](ch023-memory-and-consent.md) | Unimplemented: design draft |

### IV. Capabilities on your devices

| Chapter | Status |
| --- | --- |
| [24. Coder as a member](ch024-coder-as-a-member.md) | Unimplemented: design draft |
| [25. Other engines](ch025-other-engines.md) | Unimplemented: design draft |
| [26. Subagents you can see](ch026-subagents-you-can-see.md) | Unimplemented: design draft |
| [27. Programs](ch027-programs.md) | Unimplemented: design draft |
| [28. The command line behind the agent](ch028-the-command-line-behind-the-agent.md) | Unimplemented: design draft |
| [29. Where work runs](ch029-where-work-runs.md) | Unimplemented: design draft |
| [30. Files and device services](ch030-files-and-device-services.md) | Unimplemented: design draft |
| [31. Voice](ch031-voice.md) | Unimplemented: design draft |
| [32. A wallet](ch032-a-wallet.md) | Unimplemented: design draft |
| [33. Spending approval](ch033-spending-approval.md) | Unimplemented: design draft |

### V. Continuity across devices

| Chapter | Status |
| --- | --- |
| [34. State that survives](ch034-state-that-survives.md) | Unimplemented: design draft |
| [35. Sync across devices](ch035-sync-across-devices.md) | Planned |
| [36. Background, resume and cancel](ch036-background-resume-and-cancel.md) | Unimplemented: design draft |
| [37. One owner per task](ch037-one-owner-per-task.md) | Unimplemented: design draft |

### VI. An extensible agent and its economy

| Chapter | Status |
| --- | --- |
| [38. The plugin interface](ch038-the-plugin-interface.md) | Unimplemented: design draft |
| [39. The sandbox](ch039-the-sandbox.md) | Unimplemented: design draft |
| [40. Installing grants nothing](ch040-installing-grants-nothing.md) | Unimplemented: design draft |
| [41. Write a plugin](ch041-write-a-plugin.md) | Unimplemented: design draft |
| [42. The authoring interview](ch042-the-authoring-interview.md) | Unimplemented: design draft |
| [43. Test-time capabilities](ch043-test-time-capabilities.md) | Unimplemented: design draft |
| [44. Cases and graders](ch044-cases-and-graders.md) | Unimplemented: design draft |
| [45. A two-arm run](ch045-a-two-arm-run.md) | Unimplemented: design draft |
| [46. A claim's key](ch046-a-claims-key.md) | Unimplemented: design draft |
| [47. Reproduction by another key](ch047-reproduction-by-another-key.md) | Unimplemented: design draft |
| [48. External validation](ch048-external-validation.md) | Unimplemented: design draft |
| [49. Adoption is policy](ch049-adoption-is-policy.md) | Unimplemented: design draft |
| [50. Credit](ch050-credit.md) | Unimplemented: design draft |
| [51. Revalidation](ch051-revalidation.md) | Unimplemented: design draft |
| [52. The Gym](ch052-the-gym.md) | Unimplemented: design draft |
| [53. Why an open market](ch053-why-an-open-market.md) | Unimplemented: design draft |
| [54. Releases as signed events](ch054-releases-as-signed-events.md) | Unimplemented: design draft |
| [55. Everyone gets it](ch055-everyone-gets-it.md) | Unimplemented: design draft |
| [56. Paid calls](ch056-paid-calls.md) | Unimplemented: design draft |
| [57. Metering and receipts](ch057-metering-and-receipts.md) | Unimplemented: design draft |
| [58. Payouts to authors](ch058-payouts-to-authors.md) | Unimplemented: design draft |
| [59. One identity](ch059-one-identity.md) | Unimplemented: design draft |
| [60. Reputation from reproduced claims](ch060-reputation-from-reproduced-claims.md) | Unimplemented: design draft |
| [61. Markets for work](ch061-markets-for-work.md) | Unimplemented: design draft |
| [62. Signed JSON over WebSockets](ch062-signed-json-over-websockets.md) | Unimplemented: design draft |
| [63. Every step is an event](ch063-every-step-is-an-event.md) | Unimplemented: design draft |
| [64. The OpenAgents API](ch064-the-openagents-api.md) | Unimplemented: design draft |
| [65. One schema, three carriages](ch065-one-schema-three-carriages.md) | Planned |
| [66. The compliance rule](ch066-the-compliance-rule.md) | Planned |
| [67. Host it yourself](ch067-host-it-yourself.md) | Unimplemented: design draft |
| [68. One gateway](ch068-one-gateway.md) | Unimplemented: design draft |
| [69. Models we serve](ch069-models-we-serve.md) | Unimplemented: design draft |
| [70. Pylons](ch070-pylons.md) | Unimplemented: design draft |

### VII. Worlds and delivery

| Chapter | Status |
| --- | --- |
| [71. The Metaverse is a surface](ch071-the-verse-is-a-surface.md) | Unimplemented: design draft |
| [72. Agents you can meet](ch072-agents-you-can-meet.md) | Unimplemented: design draft |
| [73. Realms and makers](ch073-realms-and-makers.md) | Unimplemented: design draft |
| [74. A decision in the world](ch074-a-decision-in-the-world.md) | Unimplemented: design draft |
| [75. Cross-host conformance](ch075-cross-host-conformance.md) | Unimplemented: design draft |
| [76. Releases from one source](ch076-releases-from-one-source.md) | Unimplemented: design draft |
| [77. A Windows host](ch077-a-windows-host.md) | Unimplemented: design draft |
| [78. What 1.0 means](ch078-what-one-point-zero-means.md) | Unimplemented: design draft |
<!-- chapters:end -->
