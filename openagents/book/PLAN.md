# Plan

This is the plan for writing and building The OpenAgents Book. It gives an
order and no dates.

The first draft (2026-11-10) followed
[the port audit](../../docs/audits/2026-11-10-episode-289-port-audit.md) and
started from the agent loop; the apps waited for chapter 61. On 2026-11-13
([#13512](https://github.com/OpenAgentsInc/openagents/issues/13512)) the book
was reordered so that working web and iPhone apps come first, on
[Coax Native](../../coax/native.md), and every later part adds to apps that
already run. The [chapter map](#chapter-map) gives each chapter's number in
the first draft, so links and notes against the old numbers can be traced.

## Where the code goes

A chapter adds a path only where no current owner exists. Everything else
extends the owner.

| What | Owner | Chapters |
| --- | --- | --- |
| App state, actions, routes and the driver | Coax Native ([`coax/native.md`](../../coax/native.md)); generic pieces in the proposed `coax/lib/app/` | 1–6 |
| Components, text, Markdown, the edit buffer | [`components/`](../../components) | 3, 7, 10, 11 |
| The apps | [`apps/openagents-web`](../../apps/openagents-web), [`apps/openagents-ios`](../../apps/openagents-ios), [`apps/openagents-desktop`](../../apps/openagents-desktop/README.md), shared app code in [`apps/shared`](../../apps/shared) | 1–11, 30, 31, 34, 36, 75 |
| Login, accounts, agent grants and the HTTP API | [`coax/web/api/`](../../coax/web/api), [`apps/shared/app-login-flow.coax`](../../apps/shared/app-login-flow.coax) | 8, 9, 59, 64 |
| Engines, sessions, the journal, observation | [`coax/lib/agent/`](../../coax/lib/agent) | 11, 12, 18, 25, 26, 37 |
| Money rules | The Payments context, [`pay/`](../../pay) | 32, 33, 56–58, 61 |
| The wallet | The iPhone app's Breez Spark wallet ([`apps/openagents-ios/OpenAgents/Wallet`](../../apps/openagents-ios/OpenAgents/Wallet)) over `pay/` | 32, 33 |
| Voice | [`metaverse/voice.coax`](../../metaverse/voice.coax), [`apps/shared/dictation.coax`](../../apps/shared/dictation.coax) | 31 |
| Worlds | The Metaverse Engine | 71–74 |
| Releases and over-the-air updates | [`coax/remote/`](../../coax/remote), each app's release script, [Coax Native Updates](../../coax/native-updates.md) | 76 |
| The front, members, plans, knowledge, plugins, evidence and the registry | `openagents/` (planned), the product's own behavior | 12–23, 27, 28, 38–55 |

The wire shapes live in [`proto/`](../../proto/README.md), and
[the OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md)
describes them. Chapter 9 gives the apps the minimum; chapters 62 to 67 give
the rest.

The [environment application profile](../../docs/protocol/environments-claude-and-world-editing.md)
owns the proto-first desktop/phone connection to Claude on this Mac and
CoderOS, and subsequent realm editing/preview. It composes HOST, ENV, WS and
SESS rather than defining another app-specific agent wire. The desktop app
and optional headless CLI attach to the same resident execution owner.

The checker refuses a planned path that already exists or that sits where a
current owner lives ([CONTRACT.md](CONTRACT.md#what-the-checker-enforces)).

## Chapter map

Each chapter, its part, its number in the first draft ("new" for a chapter
added in the reorder) and the earlier chapters it depends on.

<!-- map:begin -->
| Chapter | Part | First draft | Depends on |
| --- | --- | --- | --- |
| [1. One app, every surface](ch001-one-app-every-surface.md) | I | 2 | none |
| [2. State and actions](ch002-state-and-actions.md) | I | new | 1 |
| [3. Components](ch003-components.md) | I | new | 2 |
| [4. Routes and navigation](ch004-routes-and-navigation.md) | I | new | 2, 3 |
| [5. Effects, services and time](ch005-effects-services-and-time.md) | I | new | 2 |
| [6. One case on the web and the iPhone](ch006-one-case-on-the-web-and-the-iphone.md) | I | new | 3, 4, 5 |
| [7. The public website](ch007-the-public-website.md) | II | new | 3, 4 |
| [8. Accounts and login](ch008-accounts-and-login.md) | II | 50 | 5, 6 |
| [9. The API the apps call](ch009-the-api-the-apps-call.md) | II | new | 8 |
| [10. The logged-in shell](ch010-the-logged-in-shell.md) | II | new | 8, 9 |
| [11. A conversation screen](ch011-a-conversation-screen.md) | II | new | 6, 9, 10 |
| [12. The simplest agent](ch012-the-simplest-agent.md) | III | 1 | 11 |
| [13. Members](ch013-members.md) | III | 3 | 12 |
| [14. A typed plan](ch014-a-typed-plan.md) | III | 4 | 13 |
| [15. Decide before you generate](ch015-decide-before-you-generate.md) | III | 5 | 14 |
| [16. Measure the front](ch016-measure-the-front.md) | III | 6 | 15 |
| [17. Grants and approvals](ch017-grants-and-approvals.md) | III | 7 | 14 |
| [18. On the record](ch018-on-the-record.md) | III | 8 | 14, 17 |
| [19. Conversation](ch019-conversation.md) | III | 9 | 15 |
| [20. Prepared answers](ch020-prepared-answers.md) | III | 10 | 16, 19 |
| [21. A knowledge base](ch021-a-knowledge-base.md) | III | 11 | 18, 20 |
| [22. Retrieval by meaning](ch022-retrieval-by-meaning.md) | III | 12 | 21 |
| [23. Memory and consent](ch023-memory-and-consent.md) | III | 13 | 17, 21 |
| [24. Coder as a member](ch024-coder-as-a-member.md) | IV | 14 | 13, 17 |
| [25. Other engines](ch025-other-engines.md) | IV | 15 | 24 |
| [26. Subagents you can see](ch026-subagents-you-can-see.md) | IV | 16 | 18, 25 |
| [27. Programs](ch027-programs.md) | IV | 17 | 14, 24 |
| [28. The command line behind the agent](ch028-the-command-line-behind-the-agent.md) | IV | 18 | 15, 17 |
| [29. Where work runs](ch029-where-work-runs.md) | IV | 21 | 17, 24 |
| [30. Files and device services](ch030-files-and-device-services.md) | IV | new | 5, 11 |
| [31. Voice](ch031-voice.md) | IV | new | 11, 30 |
| [32. A wallet](ch032-a-wallet.md) | IV | 19 | 17, 28 |
| [33. Spending approval](ch033-spending-approval.md) | IV | 20 | 17, 32 |
| [34. State that survives](ch034-state-that-survives.md) | V | new | 9, 11 |
| [35. Sync across devices](ch035-sync-across-devices.md) | V | 51 | 1, 8, 34 |
| [36. Background, resume and cancel](ch036-background-resume-and-cancel.md) | V | new | 5, 34 |
| [37. One owner per task](ch037-one-owner-per-task.md) | V | new | 29, 35, 36 |
| [38. The plugin interface](ch038-the-plugin-interface.md) | VI | 22 | 13, 17 |
| [39. The sandbox](ch039-the-sandbox.md) | VI | 23 | 38 |
| [40. Installing grants nothing](ch040-installing-grants-nothing.md) | VI | 24 | 17, 39 |
| [41. Write a plugin](ch041-write-a-plugin.md) | VI | 25 | 40 |
| [42. The authoring interview](ch042-the-authoring-interview.md) | VI | 26 | 27, 41 |
| [43. Test-time capabilities](ch043-test-time-capabilities.md) | VI | 27 | 40 |
| [44. Cases and graders](ch044-cases-and-graders.md) | VI | 28 | 43 |
| [45. A two-arm run](ch045-a-two-arm-run.md) | VI | 29 | 18, 44 |
| [46. A claim's key](ch046-a-claims-key.md) | VI | 30 | 45 |
| [47. Reproduction by another key](ch047-reproduction-by-another-key.md) | VI | 31 | 46 |
| [48. External validation](ch048-external-validation.md) | VI | 32 | 47 |
| [49. Adoption is policy](ch049-adoption-is-policy.md) | VI | 33 | 48 |
| [50. Credit](ch050-credit.md) | VI | 34 | 47, 49 |
| [51. Revalidation](ch051-revalidation.md) | VI | 35 | 46, 49 |
| [52. The Gym](ch052-the-gym.md) | VI | 36 | 41, 49, 50, 51 |
| [53. Why an open market](ch053-why-an-open-market.md) | VI | 37 | 52 |
| [54. Releases as signed events](ch054-releases-as-signed-events.md) | VI | 38 | 38, 53 |
| [55. Everyone gets it](ch055-everyone-gets-it.md) | VI | 39 | 49, 54 |
| [56. Paid calls](ch056-paid-calls.md) | VI | 40 | 32, 33, 54 |
| [57. Metering and receipts](ch057-metering-and-receipts.md) | VI | 41 | 56 |
| [58. Payouts to authors](ch058-payouts-to-authors.md) | VI | 42 | 32, 57 |
| [59. One identity](ch059-one-identity.md) | VI | 43 | 17, 32 |
| [60. Reputation from reproduced claims](ch060-reputation-from-reproduced-claims.md) | VI | 44 | 50, 57, 59 |
| [61. Markets for work](ch061-markets-for-work.md) | VI | 45 | 26, 58, 60 |
| [62. Signed JSON over WebSockets](ch062-signed-json-over-websockets.md) | VI | 46 | 18 |
| [63. Every step is an event](ch063-every-step-is-an-event.md) | VI | 47 | 52, 61, 62 |
| [64. The OpenAgents API](ch064-the-openagents-api.md) | VI | 48 | 1, 9, 63 |
| [65. One schema, three carriages](ch065-one-schema-three-carriages.md) | VI | 49 | 62, 64 |
| [66. The compliance rule](ch066-the-compliance-rule.md) | VI | 52 | 65 |
| [67. Host it yourself](ch067-host-it-yourself.md) | VI | 53 | 55, 64 |
| [68. One gateway](ch068-one-gateway.md) | VI | 54 | 15 |
| [69. Models we serve](ch069-models-we-serve.md) | VI | 55 | 16, 68 |
| [70. Pylons](ch070-pylons.md) | VI | 56 | 58, 69 |
| [71. The Metaverse is a surface](ch071-the-verse-is-a-surface.md) | VII | 57 | 1, 12 |
| [72. Agents you can meet](ch072-agents-you-can-meet.md) | VII | 58 | 26, 71 |
| [73. Realms and makers](ch073-realms-and-makers.md) | VII | 59 | 58, 59, 71 |
| [74. A decision in the world](ch074-a-decision-in-the-world.md) | VII | 60 | 57, 70, 73 |
| [75. Cross-host conformance](ch075-cross-host-conformance.md) | VII | 61 | 1, 11, 37, 64, 71 |
| [76. Releases from one source](ch076-releases-from-one-source.md) | VII | 63 | 75 |
| [77. A Windows host](ch077-a-windows-host.md) | VII | 62 | 70, 75 |
| [78. What 1.0 means](ch078-what-one-point-zero-means.md) | VII | 64 | 52, 61, 67, 70, 74, 76 |
<!-- map:end -->

## Promise coverage

Every promise from the audit's
[section 1](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)
is covered by at least one chapter. The gap for each is in the audit's
[section 4](../../docs/audits/2026-11-10-episode-289-port-audit.md#4-promise-by-promise-the-gap-and-the-shape).
A chapter that covers a promise plans for it; it does not show that the
promise is met.

<!-- coverage:begin -->
| Promise | Chapters |
| --- | --- |
| P1 | 1–7, 10–12, 30–31, 34–37, 71, 75 |
| P2 | 20–23 |
| P3 | 13, 24–26, 29–30, 37 |
| P4 | 13, 18, 26, 38, 72 |
| P5 | 27, 38–41, 44–45, 52 |
| P6 | 11, 19–20, 27, 52 |
| P7 | 14–16, 68–69 |
| P8 | 17, 28, 32–33 |
| P9 | 42 |
| P10 | 49, 53–55, 73 |
| P11 | 32, 53, 56–58, 61, 70, 73–74 |
| P12 | 9, 18, 53–54, 61–63, 65–68, 70, 74 |
| P13 | 9, 29, 37, 64, 67, 71, 75 |
| P14 | 8, 17, 23, 32, 50, 59–60 |
| P15 | 40, 43–52 |
| P16 | 6–7, 75–78 |
<!-- coverage:end -->

## Order of building

Each step ships something a person can use and is the base for the next.

| Step | What ships | Chapters | Targets |
| --- | --- | --- | --- |
| Step 1: the first milestone | The website and the iPhone app use shared actions to open login, log in, send a message, receive a fixture reply and cancel a pending turn. The same operations assert both. Logout and cancel fence a held reply; a reconnect can't send twice | 1–11 | `web`, `ios-simulator` |
| Step 2: Android | The same cases on the existing Android adapters, without claiming a finished Android app | 6, 11, 75 | `android-emulator` |
| Step 3: live and device checks | Live-service checks with their scope stated; device runs for keyboard, VoiceOver, lifecycle and SDKs; payment refusal and duplicate-result cases | 6, 8, 33, 36 | `ios-device`, `android-device` |
| Step 4: one agent | The loop, members, the typed front, grants, the record, conversation, knowledge and memory behind the conversation screen | 12–23 | as step 1 |
| Step 5: capabilities | Coder, other engines, remote work, files, voice, the wallet and payment approval | 24–33 | as step 3 |
| Step 6: continuity | Persistence, sync, background and resume, one owner per task | 34–37 | as step 3 |
| Step 7: plugins, evidence and the market | The plugin interface, the evidence rule, the registry, paid calls and payouts | 38–61 | as step 1 |
| Step 8: the protocol and compute | The protocol chapters, the API for partners, self-hosting, the gateway and Pylons | 62–70 | as step 1 |
| Step 9: worlds and delivery | The agent in realms, the conformance run, releases from one source, Windows and 1.0 | 71–78 | every target |

**Parallel desktop/environment track (owner direction, 2026-11-13).** After
the shared first milestone, replace the existing desktop shell on Mac and
Linux with Coax Native, reusing its hosts, components and engine libraries.
Start with setup/attachment to an independent local service, authorized chat
history from both computers, then managed Claude chat/control from either
desktop and the phone. A CLI-only service may land first for phone access;
the normal setup experience is the desktop app. Closing the GUI must not stop
Claude. See the [canonical stages and gates](../../docs/protocol/environments-claude-and-world-editing.md#10-implementation-sequence-and-completion-gates).

This track can proceed alongside the numbered steps; Mac/Linux delivery does
not wait for Android, the full general agent, realm visualization or chapter
77. It applies the existing app, execution-owner and protocol chapters without
renumbering or claiming their drafts implemented. Realm editing/preview adds
to the same conversation afterward. Windows follows Mac/Linux qualification,
using the same app/protocol with its platform backend still to be selected.

Steps 4 to 8 can overlap once step 1 is done. Windows and the other
additional platforms (chapter 77) are qualified on their own evidence and
never hold up the web or the phone. Name an implementer and a start date for
a step before it begins.

## Not written yet

- `PORT.md`: one line for each of the 229 crates at `coder-v1.0.0`.
- `PROTOCOL.md` (chapter 63) and `ONE-POINT-ZERO.md` (chapter 78).
- The drafts of chapters 35, 65 and 66, which are planned outlines.
- The capstones "A maker is paid" and "Coder takes a job from the queue",
  which chapters 73 and 61 point to.

## Open questions for the owner

The audit lists five decisions in its
[section 8](../../docs/audits/2026-11-10-episode-289-port-audit.md#8-decisions-for-the-owner).
The first, the name and place of `openagents/`, is settled: the directory
holds this book, and product code goes there only where no current owner
exists. Two of the first draft's questions are settled by what now runs: the
first wallet is Breez Spark, in the iPhone app (chapter 32), and every agent
task has one execution owner instead of a fixed split between devices and
the hosted service (chapter 37). The chapters still ask:

<!-- questions:begin -->
There are 81 of them.

| Chapter | Question |
| --- | --- |
| [1](ch001-one-app-every-surface.md) | The [sync spec](../../docs/sync/README.md) recommends syncing a conversation through our service, readable by the server at first (chapter 35). Confirm that, and say which collections get end-to-end encryption first. |
| [3](ch003-components.md) | Which screens of the iPhone app move to the native control backend first, and which stay UIKit controllers until it is qualified? |
| [6](ch006-one-case-on-the-web-and-the-iphone.md) | Which transport does the driver use to reach an app on a physical iPhone, and who may attach to it? |
| [8](ch008-accounts-and-login.md) | Do a member's grant (chapter 17) and an agent's grant become one record, or stay two? |
| [8](ch008-accounts-and-login.md) | Which scopes come after `profile:read` and `wallet:spend`? |
| [13](ch013-members.md) | What does the product call these? The essays say "members"; Episode 289 says tools, subagents and plugins. |
| [14](ch014-a-typed-plan.md) | The audit's plan carries a budget. Our rule is that we impose no limits on a run and show any limit the user sets. Should a plan carry only limits the user set? |
| [15](ch015-decide-before-you-generate.md) | Which of our own decision models backs up Gemini Decision 1 for the front at 1.0, and what result would move our own model to the first door? |
| [15](ch015-decide-before-you-generate.md) | What does the front do when no decision route answers: plain conversation with no routing, as the old tree did, or a visible error? |
| [16](ch016-measure-the-front.md) | Where do the labeled messages come from now? The old set is at the tag. New ones would come from real conversations, which needs a consent rule and must leave out our own testing. |
| [17](ch017-grants-and-approvals.md) | Which grants can a person give once and keep, and which ask every time? |
| [18](ch018-on-the-record.md) | Confirm the default: records are private, and publishing one is a separate, explicit act. |
| [19](ch019-conversation.md) | Which model answers general conversation by default, and may a person choose another? |
| [19](ch019-conversation.md) | Where does the written policy live, and who can change it? |
| [20](ch020-prepared-answers.md) | Who reviews a prepared answer before it is served: only us, or trusted contributors too? |
| [21](ch021-a-knowledge-base.md) | Which of the old tree's notes come back? Many were learned from benchmark tasks and need new evidence. |
| [21](ch021-a-knowledge-base.md) | Is every entry about OpenAgents public, or are some for logged-in people only? |
| [22](ch022-retrieval-by-meaning.md) | Which model makes the embeddings: a hosted one through the gateway, or one we serve with Psionic? |
| [23](ch023-memory-and-consent.md) | Where does memory live: on the device only, encrypted on a relay, or on our service? |
| [23](ch023-memory-and-consent.md) | Which sharing model do we offer first: none, opt-in publishing, or opt-in sale? |
| [25](ch025-other-engines.md) | Which engines does 1.0 name? The Coax tree drives Claude Code, Codex and Cursor. The old tree also drove Grok Build, OpenCode and Devin. |
| [27](ch027-programs.md) | Which of the twenty recipes come back, and are they members of the general agent or examples in this book? |
| [28](ch028-the-command-line-behind-the-agent.md) | Which commands can chat never call? Proposed: key export, admin commands, and anything that deletes data. |
| [29](ch029-where-work-runs.md) | Does 1.0 offer hosted computers to everyone, and at what price? Episode 276 named one dollar an hour at the time. |
| [29](ch029-where-work-runs.md) | How does hosted compute fit inside the monthly cloud budget as use grows? |
| [31](ch031-voice.md) | Which voice reads replies aloud by default, and may a person choose another? |
| [32](ch032-a-wallet.md) | Do we offer a way to buy Bitcoin with a card, as Episode 290 shows, and through whom? |
| [33](ch033-spending-approval.md) | May a person give an agent a standing allowance in 1.0, or does every payment ask? |
| [35](ch035-sync-across-devices.md) | Confirm the sync spec's recommendations S1 to S6, including the 1 GB limit for each account. |
| [35](ch035-sync-across-devices.md) | Which collections get end-to-end encryption first? |
| [38](ch038-the-plugin-interface.md) | Do we read Claude Code's plugin and skill formats directly, or only our own package format? |
| [39](ch039-the-sandbox.md) | Wasm with grants, as the audit recommends, or native processes with an operating-system sandbox? This is the audit's decision 2. |
| [39](ch039-the-sandbox.md) | Which Wasm runtime runs plugins on native hosts: one we write in Coax, or an existing one we link? |
| [41](ch041-write-a-plugin.md) | Which of the nine do we port first after the repository map? |
| [42](ch042-the-authoring-interview.md) | Who pays for the compute an authoring run uses: the person, us for a first run, or the first paid use of the plugin? |
| [42](ch042-the-authoring-interview.md) | A plugin that files tickets needs the person's login to another service. Does 1.0 include a vault for such logins, or do first plugins avoid them? |
| [45](ch045-a-two-arm-run.md) | How many cases does a test set need before its result may be published? The first adoption used six. |
| [48](ch048-external-validation.md) | How do we get the first validation written by someone outside the company? |
| [49](ch049-adoption-is-policy.md) | What does the policy require before something becomes a default for everyone: how many confirmations, how many validations, and from whom? |
| [49](ch049-adoption-is-policy.md) | Is there one default set for the whole agent, or one per member, as Coder had? |
| [50](ch050-credit.md) | Should verification also earn sats, for example a bounty for a check or a validation, or does it stay points only? |
| [51](ch051-revalidation.md) | How much compute do we set aside each month for re-running old claims, inside the cloud budget? |
| [52](ch052-the-gym.md) | Who pays for a run on the hosted runner: the trainer in sats, us up to a free amount, or both? |
| [54](ch054-releases-as-signed-events.md) | Do we mirror releases from any relay by default, or only from a list we choose? An open mirror needs a plan for spam. |
| [55](ch055-everyone-gets-it.md) | Does a defaults update install on its own, or does each person confirm it? |
| [56](ch056-paid-calls.md) | What can an author price: a call, a unit of work, a subscription? |
| [56](ch056-paid-calls.md) | Do we offer card-funded credits as Episode 290 describes, with a fee and a Bitcoin discount, and what are the numbers? |
| [56](ch056-paid-calls.md) | Is there a free amount of paid calls for new people, and who funds it? |
| [57](ch057-metering-and-receipts.md) | How public is the ledger: every receipt, or totals per release with receipts shown only to the author and the payer? |
| [58](ch058-payouts-to-authors.md) | What is the author's share of each paid call? This is the audit's decision 3. |
| [58](ch058-payouts-to-authors.md) | What is the payout schedule and the minimum: for example, daily above a set number of sats? |
| [58](ch058-payouts-to-authors.md) | When a plugin depends on another paid plugin, how is the price split between the two authors? |
| [58](ch058-payouts-to-authors.md) | A tip goes straight from the payer to the maker's own wallet (owner decision D6 in [docs/auth](../../docs/auth/decisions.md), built in #13210). A paid call with a split can't always do that. Who holds the money between such a call and the payout, and under what custody? |
| [59](ch059-one-identity.md) | When does a person who logged in by email get a Nostr key for signing public records, where is it made, and how do they take it with them? |
| [60](ch060-reputation-from-reproduced-claims.md) | What makes two keys distinct enough to count as independent confirmations? |
| [61](ch061-markets-for-work.md) | Who accepts a result: the buyer alone, or an independent checker when the buyer asks for one? |
| [61](ch061-markets-for-work.md) | Do the terms of the subscriptions people would lend allow this use? We need an answer per provider before we ship worker mode. |
| [63](ch063-every-step-is-an-event.md) | Which gaps do we close before 1.0, and which do we ship with and name? |
| [64](ch064-the-openagents-api.md) | Does the API include an endpoint in the shape other vendors' clients already speak, so existing tools can point at it? |
| [64](ch064-the-openagents-api.md) | What does API use cost a partner, and how does that sit with the author's share of paid calls? |
| [65](ch065-one-schema-three-carriages.md) | Which record is the worked example: a release, which is public, or a grant, which is private? |
| [65](ch065-one-schema-three-carriages.md) | Do chapters show schema text, or only link to `proto/`? |
| [66](ch066-the-compliance-rule.md) | Does a chapter reach "implemented" only when its records are compliant? |
| [66](ch066-the-compliance-rule.md) | Do we publish the fixtures as a kit that a second implementation can run? |
| [67](ch067-host-it-yourself.md) | How are authors paid when a self-hosted instance runs a paid plugin locally: by license terms, by plugins that call home for payment, or not at all? |
| [68](ch068-one-gateway.md) | What is the default order of providers for each job: decisions, conversation and long work? |
| [69](ch069-models-we-serve.md) | Which models does 1.0 serve on our own hardware, and which stay hosted? |
| [70](ch070-pylons.md) | What does a Pylon earn per decision, and who sets it: us, the Pylon's owner, or a market? |
| [70](ch070-pylons.md) | How large a sample of results do we recompute, and what happens to a Pylon whose answers don't match? |
| [70](ch070-pylons.md) | Beyond our own agent and the Metaverse, who is the first outside buyer? |
| [72](ch072-agents-you-can-meet.md) | Do agents appear in shared public realms by default, or only in your own space until you choose to show them? |
| [73](ch073-realms-and-makers.md) | Is the maker's share the same as a plugin author's, and what does a fork's maker owe the original's? |
| [73](ch073-realms-and-makers.md) | Who curates the featured list, and by what written rule? |
| [74](ch074-a-decision-in-the-world.md) | Who pays for a guard's decisions: the realm's maker, the player, or us while the network starts? |
| [75](ch075-cross-host-conformance.md) | Does Android get the full OpenAgents app for 1.0, or does the Android game app come first and the agent follow? |
| [76](ch076-releases-from-one-source.md) | Which stores do we publish to at 1.0, and which platforms get a direct download only? |
| [76](ch076-releases-from-one-source.md) | Which kinds of update may install on their own, and which ask the person first? |
| [77](ch077-a-windows-host.md) | Which graphics interface does the Windows host use: the platform's own, or Vulkan as on Linux? This is the audit's decision 5. |
| [77](ch077-a-windows-host.md) | Do we need a Windows machine in the build fleet, or do we build from another platform and test on a rented one? |
| [78](ch078-what-one-point-zero-means.md) | Is 1.0 one release of everything, or does each surface reach 1.0 when its own chapters run? |
| [78](ch078-what-one-point-zero-means.md) | Which of the sixteen promises may ship marked "gated" at 1.0, if any? |
<!-- questions:end -->
