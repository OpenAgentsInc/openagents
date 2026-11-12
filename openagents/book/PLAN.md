# Plan

Written 2026-11-10. This is the plan for writing and building The OpenAgents
Book. It follows
[the port audit](../../docs/audits/2026-11-10-episode-289-port-audit.md),
which lists what [Episode 289](../../docs/transcripts/289.md) promised,
compares it with the old tree and with today's, and proposes this book.

Everything here is a plan. It gives an order and no dates.

## Where the book sits

The audit proposes a new top-level directory, `openagents/`, for the general
agent, in the way `coder/`, `nostr/`, `bitcoin/` and `psionic/` each own a
domain. This book is the first thing in it. The code the chapters plan goes
beside it:

| Planned path | What goes there | Chapters |
| --- | --- | --- |
| `openagents/agent/` | The runtime, the front, members, plans, grants, the journal, surfaces | 1–9, 14–18, 21, 57, 58, 61 |
| `openagents/knowledge/` | Prepared answers, the knowledge store, the index, memory | 10–13 |
| `openagents/wallet/` | The wallet and payment approval | 19, 20 |
| `openagents/plugins/` | The plugin interface, sandbox, locks and examples | 22–25 |
| `openagents/eval/` | The authoring interview, test sets, runs, claims, adoption, credit, the Gym | 26–36 |
| `openagents/registry/` | Releases, installs, defaults, identity, reputation, realms | 33, 38, 39, 43, 44, 59 |
| `openagents/pay/` | Prices, receipts, the meter, payouts, jobs | 40–42, 45 |
| `openagents/api/`, `openagents/selfhost/` | The API and the self-host bundle | 48, 53 |
| `openagents/release/` | The release pipeline | 63 |

None of these paths exists yet. The name and place of the directory is the
audit's [decision 1](../../docs/audits/2026-11-10-episode-289-port-audit.md#8-decisions-for-the-owner).

The wire shapes do not go beside the book. They live in
[`proto/`](../../proto/README.md), which exists, and
[the OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md)
describes them. Chapters 46 to 52 tie the book to both.

## How the outline differs from the audit's

The audit proposes six parts: one agent, members, plugins, evidence, the
market, everywhere. This book keeps those six in that order and adds four,
because the premise is wider than Episode 289 alone:

- **Knowledge and conversation** (Part II) comes out of "one agent". Promise
  P2 and the non-work routes of P6 need five chapters of their own.
- **The protocol is the agent** (Part VII) comes out of "everywhere". It is
  the essay's Part IV, and the API and self-hosting rest on it.
- **Compute** (Part VIII) is new. The audit's port map says to port Pylons
  and computers "as members and as a compute market", and Episode 290
  relaunches Pylons.
- **The Verse** (Part IX) is new as a part. It is the audit's step 10,
  "agents in the Verse", and it is where the foundation meets the agent.

"Everywhere" (Part X) keeps the surfaces, the Windows host, the release
pipeline and the meaning of 1.0.

## The protocol in the book

Added 2026-11-11, after [the protocol
review](../../docs/protocol/2026-11-11-protocol-review.md) and [the compliance
plan](../../docs/protocol/2026-11-11-compliance-plan.md).

- **Four outline chapters in Part VII.** 49 says a record is one schema
  carried three ways. 50 builds accounts and agent grants. 51 builds sync
  across devices. 52 states the compliance rule. The chapters after them moved
  up by four: Host it yourself is 53, and What 1.0 means is 64.
- **A table in every chapter that sends, signs, stores or publishes a
  record.** It names the proto package and message, the carriage, and what is
  true today. 56 chapters have one.
- **Status comes from `proto/registry.json` and the compliance plan,** not
  from the spec's "Implemented" labels, which describe code from before the
  reset.
- **Links back.** The spec's [book
  table](../../docs/protocol/openagents-protocol-spec.md#in-the-book) lists
  the chapters for each section.
- **Each compliance step updates its chapter.** When a record moves from
  "spec sketch" to "in use", the step that moves it edits the chapter's table.

What the tables add up to today: three of this book's records are in use.
Agent grants and the served decision go through generated codecs, and the
realm session goes through a pinned profile. A handful have a schema file. Most are sketches whose NIP JSON is
written by hand.

## Promise coverage

Every promise from the audit's
[section 1](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)
is covered by at least one chapter. The gap for each is in the audit's
[section 4](../../docs/audits/2026-11-10-episode-289-port-audit.md#4-promise-by-promise-the-gap-and-the-shape).

| Promise | Chapters |
| --- | --- |
| P1 | 1–2, 51, 57, 61 |
| P2 | 10–13 |
| P3 | 3, 14–16, 21 |
| P4 | 3, 8, 16, 22, 58 |
| P5 | 17, 22–25, 28–29, 36 |
| P6 | 9–10, 17, 36 |
| P7 | 4–6, 54–55 |
| P8 | 7, 18–20 |
| P9 | 26 |
| P10 | 33, 37–39, 59 |
| P11 | 19, 37, 40–42, 45, 56, 59–60 |
| P12 | 8, 37–38, 45–47, 49, 52–54, 56, 60 |
| P13 | 21, 48, 53, 57, 61 |
| P14 | 7, 13, 19, 34, 43–44, 50 |
| P15 | 24, 27–36 |
| P16 | 61–64 |

## Order of writing

The owner edits before anything is built. A useful order:

1. **The open questions**, [below](#open-questions-for-the-owner). Several
   decide what later chapters say: the sandbox (chapter 23), the wallet's
   base (19), the author's share (42) and the Windows graphics interface
   (62).
2. **The seven spine chapters**: 1, 3, 5, 27, 37, 47 and 64. If these read
   right, the parts hang on them.
3. **Part by part**, in the order of building below. A chapter's status
   moves from "design draft" to "approved" in [chapters.json](chapters.json)
   and [SUMMARY.md](SUMMARY.md) when the owner has edited it.

Then each approved chapter is finished the way the other books' chapters
are: its code is written in Coax with checks, its examples run, and its
prose is rewritten to describe what runs.

## Order of building

This follows the audit's
[section 7](../../docs/audits/2026-11-10-episode-289-port-audit.md#7-order-of-work).
Each step ships something a person can use and is the base for the next.
Steps on different layers can run at the same time.

| Audit step | What ships | Chapters | Promises |
| --- | --- | --- | --- |
| 1. Velocity first | A faster compiler loop, a shared build cache, a green main. No chapters; every later step depends on it | none | none |
| 2. The book's skeleton and the port map | This draft. Next: `PORT.md` with a line for each of the 229 old crates | all, as drafts | none |
| 3. One agent, in chat | The loop, the runtime, members, the typed front, grants, the record, conversation, answers, knowledge, memory, and Coder as a member, on the web chat and the command line | 1–15 | P1, P2, P3, P6, P7 |
| 4. The wallet and CLI members | Every command callable from chat; send and receive sats with approval | 18–20 | P8 |
| 5. Plugins and evals | Visible subagents, programs, the plugin interface, the sandbox, the nine plugins, two-arm runs, claims, adoption, credit, the Gym | 16, 17, 22–25, 27–36 | P4, P5, P15 |
| 6. The authoring interview | Build a missing plugin from chat | 26 | P9 |
| 7. The registry and payouts | Publish, install, adopt, paid calls, receipts, payouts, identity, reputation, jobs | 37–45 | P10, P11, P14 |
| 8. The API and self-host | The protocol table, the schema rule and its check, the API, a partner sample, the self-host bundle | 46–49, 52, 53 | P12, P13 |
| 9. 1.0 everywhere | Thin clients, the Windows host, the release pipeline, the 1.0 checklist | 61–64 | P16 |
| 10. Agents in the Verse | The Verse as a surface, agents you can meet, makers paid, a paid decision in the world | 57–60 | P1, P4, P11 |

Four groups have no step of their own in the audit:

- **Where work runs** (chapter 21) ships with step 3 for your own computer
  and with step 8 for hosted ones.
- **Compute** (chapters 54–56) can start as soon as step 3 exists, because
  the front is the first buyer of decisions. Pylon payouts need step 7.
- **Accounts and agent grants** (chapter 50) are built ahead of every step, in
  `coax/web/api`. Step 3 uses them, and step 4's allowances are their spend
  caps.
- **Sync** (chapter 51) follows the [sync spec's](../../docs/sync/README.md)
  own order. It needs sessions, which exist, and step 3 needs it for one
  conversation on every device.

## What runs at the end of each part

| Part | Chapters | What it adds | What runs at the end |
| --- | --- | --- | --- |
| I. One agent | 1–8 | Start from one loop and turn it into a front that decides what each message needs. | A chat on the web and in the terminal that answers through one runtime, routes each message with a typed decision, asks before any effect, and keeps a record you can read. |
| II. Knowledge and conversation | 9–13 | Give the agent something true to say: conversation, reviewed answers, a knowledge base, retrieval and memory. | Ask the agent about OpenAgents and get an answer that cites the page, book chapter or episode it came from. |
| III. Members that do work | 14–21 | Put the working members behind the front: Coder, other engines, subagents, programs, the command line, a wallet and the computers work runs on. | From one chat: have Coder fix an issue, ask for it to be done with another engine, open a deck, and send sats after you approve the payment. |
| IV. Plugins | 22–26 | Let other people add members: a plugin interface, a sandbox, grants, and a way to build a plugin by talking to the agent. | Ask for something the agent can't do, answer its questions, and watch it draft a plugin and its tests. |
| V. Evidence | 27–36 | Make the rule that lets the composition grow without getting worse: a component is a candidate until evidence says what admitting it changed. | Re-run the repository map's adoption from 2026-09-29 in Coax, with a second key reproducing it, and get the same verdict. |
| VI. The market | 37–45 | Open the member list to everyone and pay the people who fill it: a registry, pinned installs, paid calls, receipts, payouts, identity and reputation. | Publish a plugin under your own key, have another person's agent use it in a paid call, and receive your share over Lightning. |
| VII. The protocol is the agent | 46–53 | Show that every step so far is a record with one schema, carried as a signed event, as an API body or as binary frames. Then open the same system to partner apps and to people who host it themselves. | A second implementation that reads only the public events rebuilds an agent's member list and the evidence behind it; a partner app answers through the API; an agent acts under a grant its owner approved; a self-hosted instance installs a plugin from the public registry. |
| VIII. Compute | 54–56 | Say where the models run: behind one gateway, on our own engine, and on machines that people lend for Bitcoin. | One request to the front is answered by a decision model running on a Pylon, and the Pylon's owner is paid for it. |
| IX. The Verse | 57–60 | Put all of it in a world you can walk through: the same agent, your working agents, makers who are paid, and characters that decide. | Walk into a realm, see your agents working, buy something a maker listed, and watch a guard make a decision that a Pylon served and was paid for. |
| X. Everywhere | 61–64 | Ship it: every surface as a thin client, a host for the one platform we lack, one release pipeline, and a plain statement of what 1.0 means. | The same version, built by one pipeline, installed from one download page on every platform Episode 289 named. |

## The port map

The audit's
[section 6](../../docs/audits/2026-11-10-episode-289-port-audit.md#6-port-map-by-group-from-coder-v100)
sorts the 229 old crates into groups. This table says where each group
lands in the book. The chapters cite 137 of the crates by name so
far. A full list with one line per crate, `PORT.md`, is the next thing to
write.

| Old group | Disposition in the audit | Chapters |
| --- | --- | --- |
| Chat and front | Port as the agent front | 1–10 |
| Coder | Mostly done in `coder/`; port what remains as the Coder book reaches it | 14–17 |
| Plugins | Port the interface, then the nine plugins | 22–26 |
| Evals and Gym | Port as the evidence part and the hosted runner | 27–36 |
| Knowledge | Port the store and the index | 11–13 |
| Payments | Port on `coax/lib/lightning` and `coax/lib/x402`; Bitcoin and Lightning only | 19, 20, 40–42 |
| Protocol | The Nostr carriage is done in `coax/lib/nostr`; keep the NIPs current. The schemas are in `proto/`, most of them still sketches | 46, 47, 49, 52 |
| Compute | Psionic and the gateway are done; port Pylons and computers | 21, 54–56 |
| Apps | Rebuild as thin clients in `apps/` | 2, 61–63 |
| Verse and game | Done and ahead in Aethyr and Everglade | 57–60 |
| Labs | Fold into Aethyr labs where wanted; the owner picks | none |
| Small web toys | Drop unless the owner names one | none |
| Infra | Auth is rebuilt as our own login in `coax/web/api` (chapter 50), not ported. Port keys, vault and push | 13, 43, 48, 50, 53 |

## Not written yet

- `PORT.md`: one line for each of the 229 crates at `coder-v1.0.0`.
- `CONTRACT.md`: what "implemented" will mean for this book, as
  [Coder's](../../coder/book/CONTRACT.md) does for that one.
- `PROTOCOL.md` (chapter 47) and `ONE-POINT-ZERO.md` (chapter 64).
- The drafts of chapters 49 to 52, which are outlines.
- This book's entry in [the books' index](../../docs/books/README.md), in the
  root [README](../../README.md) and in the book registry check. Those wait
  for the owner's first edit.
- The capstones "A maker is paid" and "Coder takes a job from the queue",
  which chapters 59 and 45 point to.

## Open questions for the owner

The audit lists five decisions in its
[section 8](../../docs/audits/2026-11-10-episode-289-port-audit.md#8-decisions-for-the-owner):
the name and place of `openagents/`, the plugin sandbox, the revenue split,
which labs and toys come back, and Windows graphics. Chapters 23, 42 and 62
repeat three of them. The chapters raise 80 questions in all:

| Chapter | Question |
| --- | --- |
| [2](ch002-one-runtime-every-surface.md) | Which surfaces run the runtime on the device, and which always call the hosted service? |
| [2](ch002-one-runtime-every-surface.md) | The [sync spec](../../docs/sync/README.md) recommends syncing a conversation through our service, readable by the server at first (chapter 51). Confirm that, and say which collections get end-to-end encryption first. |
| [3](ch003-members.md) | What does the product call these? The essays say "members"; Episode 289 says tools, subagents and plugins. |
| [4](ch004-a-typed-plan.md) | The audit's plan carries a budget. Our rule is that we impose no limits on a run and show any limit the user sets. Should a plan carry only limits the user set? |
| [5](ch005-decide-before-you-generate.md) | Which decision models serve the front for 1.0, and in what order: our own on our hardware, a hosted one, and which backup? |
| [5](ch005-decide-before-you-generate.md) | What does the front do when no decision route answers: plain conversation with no routing, as the old tree did, or a visible error? |
| [6](ch006-measure-the-front.md) | Where do the labeled messages come from now? The old set is at the tag. New ones would come from real conversations, which needs a consent rule and must leave out our own testing. |
| [7](ch007-grants-and-approvals.md) | Which grants can a person give once and keep, and which ask every time? |
| [8](ch008-on-the-record.md) | Confirm the default: records are private, and publishing one is a separate, explicit act. |
| [9](ch009-conversation.md) | Which model answers general conversation by default, and may a person choose another? |
| [9](ch009-conversation.md) | Where does the written policy live, and who can change it? |
| [10](ch010-prepared-answers.md) | Who reviews a prepared answer before it is served: only us, or trusted contributors too? |
| [11](ch011-a-knowledge-base.md) | Which of the old tree's notes come back? Many were learned from benchmark tasks and need new evidence. |
| [11](ch011-a-knowledge-base.md) | Is every entry about OpenAgents public, or are some for logged-in people only? |
| [12](ch012-retrieval-by-meaning.md) | Which model makes the embeddings: a hosted one through the gateway, or one we serve with Psionic? |
| [13](ch013-memory-and-consent.md) | Where does memory live: on the device only, encrypted on a relay, or on our service? |
| [13](ch013-memory-and-consent.md) | Which sharing model do we offer first: none, opt-in publishing, or opt-in sale? |
| [15](ch015-other-engines.md) | Which engines does 1.0 name? The Coax tree drives Claude Code, Codex and Cursor. The old tree also drove Grok Build, OpenCode and Devin. |
| [17](ch017-programs.md) | Which of the twenty recipes come back, and are they members of the general agent or examples in this book? |
| [18](ch018-the-command-line-behind-the-agent.md) | Which commands can chat never call? Proposed: key export, admin commands, and anything that deletes data. |
| [19](ch019-a-wallet.md) | What is the wallet built on: the Spark path the plan in `docs/lightning/` describes, our own Lightning node from `coax/lib/lightning`, or Spark first and our own later? |
| [19](ch019-a-wallet.md) | Do we offer a way to buy Bitcoin with a card, as Episode 290 shows, and through whom? |
| [20](ch020-spending-approval.md) | May a person give an agent a standing allowance in 1.0, or does every payment ask? |
| [21](ch021-where-work-runs.md) | Does 1.0 offer hosted computers to everyone, and at what price? Episode 276 named one dollar an hour at the time. |
| [21](ch021-where-work-runs.md) | How does hosted compute fit inside the monthly cloud budget as use grows? |
| [22](ch022-the-plugin-interface.md) | Do we read Claude Code's plugin and skill formats directly, or only our own package format? |
| [23](ch023-the-sandbox.md) | Wasm with grants, as the audit recommends, or native processes with an operating-system sandbox? This is the audit's decision 2. |
| [23](ch023-the-sandbox.md) | Which Wasm runtime runs plugins on native hosts: one we write in Coax, or an existing one we link? |
| [25](ch025-write-a-plugin.md) | Which of the nine do we port first after the repository map? |
| [26](ch026-the-authoring-interview.md) | Who pays for the compute an authoring run uses: the person, us for a first run, or the first paid use of the plugin? |
| [26](ch026-the-authoring-interview.md) | A plugin that files tickets needs the person's login to another service. Does 1.0 include a vault for such logins, or do first plugins avoid them? |
| [29](ch029-a-two-arm-run.md) | How many cases does a test set need before its result may be published? The first adoption used six. |
| [32](ch032-external-validation.md) | How do we get the first validation written by someone outside the company? |
| [33](ch033-adoption-is-policy.md) | What does the policy require before something becomes a default for everyone: how many confirmations, how many validations, and from whom? |
| [33](ch033-adoption-is-policy.md) | Is there one default set for the whole agent, or one per member, as Coder had? |
| [34](ch034-credit.md) | Should verification also earn sats, for example a bounty for a check or a validation, or does it stay points only? |
| [35](ch035-revalidation.md) | How much compute do we set aside each month for re-running old claims, inside the cloud budget? |
| [36](ch036-the-gym.md) | Who pays for a run on the hosted runner: the trainer in sats, us up to a free amount, or both? |
| [38](ch038-releases-as-signed-events.md) | Do we mirror releases from any relay by default, or only from a list we choose? An open mirror needs a plan for spam. |
| [39](ch039-everyone-gets-it.md) | Does a defaults update install on its own, or does each person confirm it? |
| [40](ch040-paid-calls.md) | What can an author price: a call, a unit of work, a subscription? |
| [40](ch040-paid-calls.md) | Do we offer card-funded credits as Episode 290 describes, with a fee and a Bitcoin discount, and what are the numbers? |
| [40](ch040-paid-calls.md) | Is there a free amount of paid calls for new people, and who funds it? |
| [41](ch041-metering-and-receipts.md) | How public is the ledger: every receipt, or totals per release with receipts shown only to the author and the payer? |
| [42](ch042-payouts-to-authors.md) | What is the author's share of each paid call? This is the audit's decision 3. |
| [42](ch042-payouts-to-authors.md) | What is the payout schedule and the minimum: for example, daily above a set number of sats? |
| [42](ch042-payouts-to-authors.md) | When a plugin depends on another paid plugin, how is the price split between the two authors? |
| [42](ch042-payouts-to-authors.md) | A tip goes straight from the payer to the maker's own wallet (owner decision D6 in [docs/auth](../../docs/auth/decisions.md)). A paid call with a split can't always do that. Who holds the money between such a call and the payout, and under what custody? |
| [43](ch043-one-identity.md) | When does a person who logged in by email get a Nostr key for signing public records, where is it made, and how do they take it with them? |
| [44](ch044-reputation-from-reproduced-claims.md) | What makes two keys distinct enough to count as independent confirmations? |
| [45](ch045-markets-for-work.md) | Who accepts a result: the buyer alone, or an independent checker when the buyer asks for one? |
| [45](ch045-markets-for-work.md) | Do the terms of the subscriptions people would lend allow this use? We need an answer per provider before we ship worker mode. |
| [47](ch047-every-step-is-an-event.md) | Which gaps do we close before 1.0, and which do we ship with and name? |
| [48](ch048-the-openagents-api.md) | Does the API include an endpoint in the shape other vendors' clients already speak, so existing tools can point at it? |
| [48](ch048-the-openagents-api.md) | What does API use cost a partner, and how does that sit with the author's share of paid calls? |
| [49](ch049-one-schema-three-carriages.md) | Which record is the worked example: a release, which is public, or a grant, which is private? |
| [49](ch049-one-schema-three-carriages.md) | Do chapters show schema text, or only link to `proto/`? |
| [50](ch050-accounts-and-agent-grants.md) | Do a member's grant (chapter 7) and an agent's grant become one record, or stay two? |
| [50](ch050-accounts-and-agent-grants.md) | Which scopes come after `profile:read` and `wallet:spend`? |
| [51](ch051-sync-across-devices.md) | Confirm the sync spec's recommendations S1 to S6, including the 1 GB limit for each account. |
| [51](ch051-sync-across-devices.md) | Which collections get end-to-end encryption first? |
| [52](ch052-the-compliance-rule.md) | Does a chapter reach "implemented" only when its records are compliant? |
| [52](ch052-the-compliance-rule.md) | Do we publish the fixtures as a kit that a second implementation can run? |
| [53](ch053-host-it-yourself.md) | How are authors paid when a self-hosted instance runs a paid plugin locally: by license terms, by plugins that call home for payment, or not at all? |
| [54](ch054-one-gateway.md) | What is the default order of providers for each job: decisions, conversation and long work? |
| [55](ch055-models-we-serve.md) | Which models does 1.0 serve on our own hardware, and which stay hosted? |
| [56](ch056-pylons.md) | What does a Pylon earn per decision, and who sets it: us, the Pylon's owner, or a market? |
| [56](ch056-pylons.md) | How large a sample of results do we recompute, and what happens to a Pylon whose answers don't match? |
| [56](ch056-pylons.md) | Beyond our own agent and the Verse, who is the first outside buyer? |
| [58](ch058-agents-you-can-meet.md) | Do agents appear in shared public realms by default, or only in your own space until you choose to show them? |
| [59](ch059-realms-and-makers.md) | Is the maker's share the same as a plugin author's, and what does a fork's maker owe the original's? |
| [59](ch059-realms-and-makers.md) | Who curates the featured list, and by what written rule? |
| [60](ch060-a-decision-in-the-world.md) | Who pays for a guard's decisions: the realm's maker, the player, or us while the network starts? |
| [61](ch061-thin-clients.md) | Does Android get the full OpenAgents app for 1.0, or does the Android game app come first and the agent follow? |
| [62](ch062-a-windows-host.md) | Which graphics interface does the Windows host use: the platform's own, or Vulkan as on Linux? This is the audit's decision 5. |
| [62](ch062-a-windows-host.md) | Do we need a Windows machine in the build fleet, or do we build from another platform and test on a rented one? |
| [63](ch063-one-release-pipeline.md) | Which stores do we publish to at 1.0, and which platforms get a direct download only? |
| [63](ch063-one-release-pipeline.md) | Do apps update themselves, or does the person choose each update? |
| [64](ch064-what-one-point-zero-means.md) | Is 1.0 one release of everything, or does each surface reach 1.0 when its own chapters run? |
| [64](ch064-what-one-point-zero-means.md) | Which of the sixteen promises may ship marked "gated" at 1.0, if any? |
