# The OpenAgents Book

Building the OpenAgents apps, and the general agent behind them, in
[Coax](../../coax/book/README.md) on [Coax Native](../../coax/native.md),
one small step at a time.

> **Status: 31 chapters partly implemented on CPU (`1–9`, `11–15`, `17`, `21–22`, `24`, `26`, `28`, `32–33`, `42–45`, `54–55`, `58`, `69–70`); 44 design drafts and 3 planned outlines (`35`, `65`, `66`).**
> Chapters 1–9, 14, 21–22, 24, 42–45, 54–55, 58, and 69–70 each ship a runnable Coax example in [`openagents/examples/`](../examples/opening-case-runner.coax)
> with verified debug/release transcripts grounded in
> [`apps/shared/chat/screen.coax`](../../apps/shared/chat/screen.coax),
> [`apps/shared/app/history/index.coax`](../../apps/shared/app/history/index.coax),
> [`apps/shared/app/environment/select.coax`](../../apps/shared/app/environment/select.coax),
> [`apps/shared/app/settings/identity.coax`](../../apps/shared/app/settings/identity.coax),
> [`coax/lib/agent/router/decide.coax`](../../coax/lib/agent/router/decide.coax),
> [`openagents/knowledge/nipkb.coax`](../../openagents/knowledge/nipkb.coax),
> [`openagents/knowledge/embed.coax`](../../openagents/knowledge/embed.coax),
> [`openagents/core/members.coax`](../../openagents/core/members.coax),
> [`coax/lib/agent/desktop/capability-author.coax`](../../coax/lib/agent/desktop/capability-author.coax),
> [`coax/lib/agent/desktop/capability-gate.coax`](../../coax/lib/agent/desktop/capability-gate.coax),
> [`coax/lib/nostr/eval/report.coax`](../../coax/lib/nostr/eval/report.coax),
> [`psionic/lib/peer/module-registry.coax`](../../psionic/lib/peer/module-registry.coax),
> [`psionic/lib/peer/release-promote.coax`](../../psionic/lib/peer/release-promote.coax),
> [`psionic/lib/hybrid-exec.coax`](../../psionic/lib/hybrid-exec.coax),
> [`psionic/lib/pylon/onboard.coax`](../../psionic/lib/pylon/onboard.coax), and
> [`psionic/lib/peer/pay.coax`](../../psionic/lib/peer/pay.coax).
> Chapters 11–13, 15, 17, 26, 28, and 32–33 are additionally backed by
> shipped, CPU-tested Coax modules across **Coax Web / Coax UI**
> ([`components/ui/`](../../components/ui), [`apps/openagents-web/demos/docs-starter/`](../../apps/openagents-web/demos/docs-starter)),
> the unified 24-layer pure-Coax agent core ([`openagents/core/`](../../openagents/core),
> [`coder/tui/oa-worker.coax`](../../openagents/runtime/oa-worker.coax),
> [`coder/tui/oa-subagent.coax`](../../openagents/runtime/oa-subagent.coax)),
> the typed router ([`coax/lib/agent/router/`](../../coax/lib/agent/router)),
> the shared knowledge base ([`openagents/knowledge/`](../../openagents/knowledge)),
> the Desktop + Apple Breez Spark wallet and `#authority` money rules
> ([`pay/`](../../pay), [`apps/openagents-desktop/page/walletpane.coax`](../../apps/openagents-desktop/page/walletpane.coax)),
> and 3-leg Executor Network author-royalty settlement ([`pay/executor-settlement.coax`](../../pay/executor-settlement.coax)).
> Native host accessibility UI tests (`macos-arm64`, `ios-simulator`, `ios-device`),
> mobile feature parity (`#14068`), multi-relay `NIP-KB` sync and neural gateway
> embeddings, general sandboxed third-party plugin SDKs, and public plugin
> economy scale (`#14060`) remain open per [CONTRACT.md](CONTRACT.md).
> Where a piece already runs in the repository, the chapter cites its real paths
> and separates what runs today from what remains. [CONTRACT.md](CONTRACT.md)
> says what moves a chapter forward. The book gives no dates.

OpenAgents builds open agents that people can own, inspect, improve and earn
from, on open protocols and Bitcoin. Since November 2023 we have worked on
that in public, across [290 episodes](../../docs/transcripts/README.md).
[Episode 289](../../docs/transcripts/289.md) says what the work adds up to:
**a composable general agent**. One agent sits behind every surface. It
answers from a shared knowledge base, hands work to Coder and to other
engines, grows by plugins that anyone can write, and pays their authors in
Bitcoin.

This book builds that, and it builds the app you use it through first. You
start with the desktop app ([`apps/openagents-desktop`](../../apps/openagents-desktop))
as the primary host, backed by shared Coax logic in
[`apps/shared/chat/`](../../apps/shared/chat) that mobile
([`apps/openagents-ios`](../../apps/openagents-ios)) consumes from the same
codebase. Each chapter adds one visible thing to the chat page: a GPU text
input, Send and serialized actions, a scrollable markdown transcript, a reply
from our service, and the login card when a send needs an account. Chapter 6
runs one acceptance case through every step on the primary desktop host and
on mobile. Chapters 7 to 11 add history, the environment and repository
chips, settings, full mobile and other native hosts (iOS and Android), and
the web. Chapter 12 puts the first agent of our own behind the chat page,
and every later chapter adds one capability to an app that already runs.

Both the desktop and mobile apps are compiled Coax linked into native
binaries: no JavaScript, no JavaScriptCore, no Wasm, and no over-the-air
updates. The shared display-list kit in [`components/`](../../components)
draws every component on the GPU
([GPU components on native](../../components/gpu-components.md)); each host
platform supplies only the focused text field's native editor, accessibility,
and system sheets. Desktop (macOS and Linux) is the primary host from
chapter 1 onward, with mobile (iOS and Android) consuming the same shared
screens, state, actions, and service. The web keeps DOM for text surfaces
(chapter 11).

Each chapter cites the episodes where an idea was first tried, the old
crates that implemented it in the Rust tree that ended at `coder-v1.0.0`,
and the NIPs that carry it. Then it says what we build now, what it builds
on, and how a run proves it.

## Book-driven development

The book specifies, verifies, and reconciles the product on `main`:

- **Every capability has a chapter.** Planned features start as design drafts
  naming their schema, refusal cases, and check; when engineering ships a
  subsystem on `main` (such as [`openagents/core/`](../../openagents/core),
  the unified [`apps/openagents-desktop/`](../../apps/openagents-desktop)
  panes, or Coax UI SSR), the corresponding chapter is reconciled to cite the
  real source paths and list what remains before full qualification.
- **No silent drift between book and code.** A chapter never claims a target
  without recorded evidence ([CONTRACT.md](CONTRACT.md)), and never cites a
  deleted path or pretends shipped code does not exist.
- **One small increment per check.** Each chapter names a four-part check
  ([CONTRACT.md](CONTRACT.md#the-chapters-check))—source, trace, refusal, and
  independent check—so progress from `design draft` to `partly implemented`
  to `implemented` is audited mechanically.
- **The chapter changes with the code.** When building teaches something
  the draft got wrong, the chapter is corrected alongside the code.

## How to read it

- **To build the app,** read Parts I and II in order. Chapter 6 is the
  first milestone: the primary desktop app (and the mobile consumer sharing
  the same logic) types a message, logs in when the send needs it, receives
  a reply, and stops a pending one, checked by one end-to-end case.
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
| The app framework: state, components, hosts, `coax drive` | [Coax Native](../../coax/native.md), [Coax Web](../../coax/web.md), [the components spec](../../components/spec.md), [GPU components on native](../../components/gpu-components.md) |
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
| I. The chat page, one step at a time | 1–6 | The primary desktop app's chat page (with mobile consuming the same shared logic) on the GPU kit: a text input, Send, a transcript, a streamed reply with Stop, and the login card, with one acceptance case passing end to end. |
| II. What the app adds next | 7–11 | History and the sidebar, the environment and repository chips, settings, full mobile and other native host parity (iOS and Android), and the web's DOM presentation. |
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
| [1. A text input on the GPU](ch001-a-text-input-on-the-gpu.md) | Partly implemented: cpu |
| [2. State and actions](ch002-state-and-actions.md) | Partly implemented: cpu |
| [3. A transcript](ch003-a-transcript.md) | Partly implemented: cpu |
| [4. A reply from the service](ch004-a-reply-from-the-service.md) | Partly implemented: cpu |
| [5. Logging in when a send needs it](ch005-logging-in-when-a-send-needs-it.md) | Partly implemented: cpu |
| [6. One case, every step](ch006-one-case-every-step.md) | Partly implemented: cpu |

### II. What the app adds next

| Chapter | Status |
| --- | --- |
| [7. History and the sidebar](ch007-history-and-the-sidebar.md) | Partly implemented: cpu |
| [8. Environment and repository chips](ch008-environment-and-repository-chips.md) | Partly implemented: cpu |
| [9. Settings](ch009-settings.md) | Partly implemented: cpu |
| [10. Mobile and other native hosts](ch010-mobile-and-other-native-hosts.md) | Unimplemented: design draft |
| [11. The web](ch011-the-web.md) | Partly implemented: cpu |

### III. One general agent

| Chapter | Status |
| --- | --- |
| [12. The simplest agent](ch012-the-simplest-agent.md) | Partly implemented: cpu |
| [13. Members](ch013-members.md) | Partly implemented: cpu |
| [14. A typed plan](ch014-a-typed-plan.md) | Partly implemented: cpu |
| [15. Decide before you generate](ch015-decide-before-you-generate.md) | Partly implemented: cpu |
| [16. Measure the front](ch016-measure-the-front.md) | Unimplemented: design draft |
| [17. Grants and approvals](ch017-grants-and-approvals.md) | Partly implemented: cpu |
| [18. On the record](ch018-on-the-record.md) | Unimplemented: design draft |
| [19. Conversation](ch019-conversation.md) | Unimplemented: design draft |
| [20. Prepared answers](ch020-prepared-answers.md) | Unimplemented: design draft |
| [21. A knowledge base](ch021-a-knowledge-base.md) | Partly implemented: cpu |
| [22. Retrieval by meaning](ch022-retrieval-by-meaning.md) | Partly implemented: cpu |
| [23. Memory and consent](ch023-memory-and-consent.md) | Unimplemented: design draft |

### IV. Capabilities on your devices

| Chapter | Status |
| --- | --- |
| [24. Coder as a member](ch024-coder-as-a-member.md) | Partly implemented: cpu |
| [25. Other engines](ch025-other-engines.md) | Unimplemented: design draft |
| [26. Subagents you can see](ch026-subagents-you-can-see.md) | Partly implemented: cpu |
| [27. Programs](ch027-programs.md) | Unimplemented: design draft |
| [28. The command line behind the agent](ch028-the-command-line-behind-the-agent.md) | Partly implemented: cpu |
| [29. Where work runs](ch029-where-work-runs.md) | Unimplemented: design draft |
| [30. Files and device services](ch030-files-and-device-services.md) | Unimplemented: design draft |
| [31. Voice](ch031-voice.md) | Unimplemented: design draft |
| [32. A wallet](ch032-a-wallet.md) | Partly implemented: cpu |
| [33. Spending approval](ch033-spending-approval.md) | Partly implemented: cpu |

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
| [42. The authoring interview](ch042-the-authoring-interview.md) | Partly implemented: cpu |
| [43. Test-time capabilities](ch043-test-time-capabilities.md) | Partly implemented: cpu |
| [44. Cases and graders](ch044-cases-and-graders.md) | Partly implemented: cpu |
| [45. A two-arm run](ch045-a-two-arm-run.md) | Partly implemented: cpu |
| [46. A claim's key](ch046-a-claims-key.md) | Unimplemented: design draft |
| [47. Reproduction by another key](ch047-reproduction-by-another-key.md) | Unimplemented: design draft |
| [48. External validation](ch048-external-validation.md) | Unimplemented: design draft |
| [49. Adoption is policy](ch049-adoption-is-policy.md) | Unimplemented: design draft |
| [50. Credit](ch050-credit.md) | Unimplemented: design draft |
| [51. Revalidation](ch051-revalidation.md) | Unimplemented: design draft |
| [52. The Gym](ch052-the-gym.md) | Unimplemented: design draft |
| [53. Why an open market](ch053-why-an-open-market.md) | Unimplemented: design draft |
| [54. Releases as signed events](ch054-releases-as-signed-events.md) | Partly implemented: cpu |
| [55. Everyone gets it](ch055-everyone-gets-it.md) | Partly implemented: cpu |
| [56. Paid calls](ch056-paid-calls.md) | Unimplemented: design draft |
| [57. Metering and receipts](ch057-metering-and-receipts.md) | Unimplemented: design draft |
| [58. Payouts to authors](ch058-payouts-to-authors.md) | Partly implemented: cpu |
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
| [69. Models we serve](ch069-models-we-serve.md) | Partly implemented: cpu |
| [70. Pylons](ch070-pylons.md) | Partly implemented: cpu |

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
