# Sales and revenue

Status: proposal, updated October 7, 2026. This page records the sales strategy
for OpenAgents, with Coder as the first adoption path. The
[unified revenue roadmap](revenue-roadmap.md) owns the delivery order, missing
pieces, dependencies, and evidence needed to earn revenue. The
[agent sales floor](agent-sales-floor.md) plans how our own agents do the
sales work, led by Paul from a building in Everglade. Product and payment
documents retain their implementation contracts. These are public plans;
customer records, compensation agreements, and negotiations stay private.

## Contents

- [Summary](#summary)
- [What businesses want](#what-businesses-want)
- [What we sell](#what-we-sell)
- [First workflow offer v1](#first-workflow-offer-v1)
- [Principles](#principles)
- [How customers arrive](#how-customers-arrive)
- [Pricing and billing](#pricing-and-billing)
- [Affiliate and referral program](#affiliate-and-referral-program)
- [Partners and fulfillment](#partners-and-fulfillment)
- [Sell in public](#sell-in-public)
- [Sell through our agents in Everglade](#sell-through-our-agents-in-everglade)
- [Business accounts](#business-accounts)
- [Hands-on onboarding](#hands-on-onboarding)
- [Agents as products](#agents-as-products)
- [What exists today](#what-exists-today)
- [What we need to build](#what-we-need-to-build)
- [Roadmap](#roadmap)
- [Measures](#measures)
- [Considerations and risks](#considerations-and-risks)
- [Open questions](#open-questions)

## Summary

OpenAgents is the composable agent and ecosystem; Coder is the coding workflow
that gives developers a concrete reason to adopt it. Coder combines installed
agents, existing logins, and cheap typed decisions (Jev). The first promise to
prove is accepted work with less expense or more capacity from the accounts a
customer already pays for. Plugins extend that workflow and let authors earn
from paid use.

The revenue plan follows product-led growth: individual developers adopt Coder
because it makes their existing usage go further, they bring it into their
teams, and teams become paying business accounts. Assisted business pilots run
in parallel: choose a recurring workflow, prove the result, and convert it
into paid usage and reusable capabilities. We plan to sell usage and bounded
services, without seat bundles or multi-year commitments. Affiliates and
partners expand demand after payment, attribution, and margins are proven.

Free local use creates adoption; it does not itself generate OpenAgents
revenue. The commercial task is to connect that adoption to a useful paid
addition, collect payment, deliver it, and earn repeat use. Existing billing,
wallet, and cloud modules are foundations for that path, not evidence that the
whole purchase flow is available today.

## What businesses want

These needs come from conversations with operators who run teams and sell
software to businesses. They recur often enough to shape the product.

- **Spend that goes further.** Most teams pay for frontier models on every
  request, including requests a much smaller model could answer. They want the
  same results for less, without changing tools.
- **No lock-in.** Models change every few weeks, so nobody can price the value
  of a tool 12 to 18 months out. Long contracts are a liability for the buyer.
  Pay-as-you-go that scales up and down is easier to say yes to.
- **One place instead of many.** People juggle several agent accounts and
  interfaces. Businesses already think in onboarding bundles (a new employee
  gets the company's email, CRM, phone, and so on), and they want agents to be
  part of that bundle, not another island.
- **Visibility.** Leaders can't see which tasks agents do, which ones they do
  well, how long work waits, or what each team spends. They want an admin view
  of agent work and its cost by person, team, and task type.
- **Proof against goals.** When a board asks where to cut cost or improve
  service, leaders want evidence from real agent work over a quarter: which
  workflows agents already handle reliably, where service levels slip, and
  where help is needed.
- **Department knowledge in an agent.** A department head knows the process.
  They want to turn their documents and know-how into a department agent that
  their team directs, which can be measured, improved, and benchmarked.
- **Control of data.** Companies don't know what employees paste into general
  chat tools. They want governance: what leaves the company, under which
  policy, with a record.
- **Reliability over demos.** Agents are good at prototypes. Production work
  needs repeatable processes, checks, and evidence, not just confident output.
- **Help, not homework.** Most businesses can't keep up with how fast models
  change. They want someone to fit the tools to their work, then leave them
  with something that runs.

## What we sell

| Offering | Who buys | How it earns |
| --- | --- | --- |
| Coder | Developers, then teams | Free to use with the person's own subscriptions; paid usage for anything we run (cloud work, the OpenAgents gateway, paid plugins) |
| Cloud computers | Developers and teams who outgrow one machine | Usage-based compute, quoted and metered in sats ([retail cloud](../cloud/retail-contract.md)) |
| Decision access | Callers who need supported System One judgments | Funded usage through the decision gateway; hosted agent/model execution is a separate proposed paid resource |
| Plugins | Plugin authors sell; users buy | The author receives the declared per-call fee; OpenAgents earns the separately priced endpoint charge, under the current [split contract](../payments/2026-10-02-central-receive-and-splits.md#3-who-gets-paid) |
| Business accounts | Teams and companies | Usage plus optional paid onboarding and support |
| Department agents | Businesses | Built during onboarding, run on usage, measured in the Gym |

## First workflow offer v1

Offer `openagents.sales.coder-pilot.v1`, defined October 7, 2026, selects
**assisted delivery of one checked public-repository change** as the first
service offer. The workflow and proposed commercial defaults below are frozen
for implementation ([REV-01](https://github.com/OpenAgentsInc/openagents/issues/10808)).
Selling waits for [O1 owner activation](../../NEEDS_OWNER.md#first-workflow-offer-o1-rev-01-10808)
and a buyer's private agreement; no customer has accepted this specification.
Paid plugins remain the first proposed small product purchase in the
[roadmap](revenue-roadmap.md#first-offers-and-their-limits).

| Part | Version 1 definition and source |
| --- | --- |
| Buyer and problem | A developer or small team's workflow owner has a recurring repository maintenance task and wants help making it repeatable with Coder. The buyer names the acceptance decision maker before work starts. |
| Client and resource | Installed `coder` and companion `openagents` on the buyer's macOS arm64 computer, built from a recorded clean commit through the [installer](../../scripts/install-coder.sh). The [local terminal/headless turn](../coder/guides/headless.md) uses a buyer-owned supported login. REV-02 qualifies that exact installed path before the pilot; a demo or another client does not qualify it. |
| Input | One buyer-authorized public HTTPS GitHub repository, exact 40-character commit, clean isolated worktree, task text at most 16 KiB, and 1–8 frozen acceptance commands of at most 1,024 bytes each. No submodules. These deliberately match the source/request/check envelope of [retail v1](../cloud/retail-contract.md#task-class-retail-repo-change-v1), without purchasing retail compute. |
| Output | A patch against the pinned commit, candidate digest, each declared check's status and last 64 KiB of output, run summary, private trace reference, and a short setup/repeat-workflow runbook. The buyer applies or publishes it. A reply, executor exit, or unchecked patch does not establish acceptance. |
| Acceptance | Before generation, the buyer freezes the intended behavior and checks. After generation, a person other than the executor runs those checks on the exact candidate in a clean worktree and records every result; the buyer then accepts the behavior and deliverables explicitly. REV-03 joins baseline, attempts, repairs, elapsed time, and known/unknown costs; no savings claim precedes that evidence. |
| Delivery cap | One buyer, one repository, one change, at most one repair attempt, and a seven-calendar-day pilot with a dated review. At most three operator hours cover discovery, setup, delivery, and support. Each attempt stops at 30 minutes; each check stops at 15 minutes. These are operator-run engagement limits, not new automatic Coder controls. |
| Service price | Proposed fixed fee **USD 250**, invoice after the buyer accepts the checked patch and runbook, due in seven calendar days. This is a new design default, not a historical price, demonstrated margin, or active rate. O1 must approve it or create a new offer version before quoting. No fee is earned for an unaccepted result. |
| Payment and trial | A separately issued service invoice and owner-selected external payment route; retain the invoice, confirmed payment, and buyer acceptance privately. No product balance is credited. Default free discovery is one 30-minute conversation, with **zero** promotional credits and no provider subsidy. The owner approves any exception before spending or quoting. |
| Resource payers | The buyer pays their model provider and uses their own computer. OpenAgents earns only the service fee. Approved Codex/Claude routing and any separately configured Jev connection name their recipients and payer before use; disable [sponsored cloud fallback](../coder/runtime/cloud-fallback.md) with `CODER_CLOUD=off`. No operator credential or unapproved paid provider is used. Subscription capacity, list-price estimates, and actual API charges remain separate; the buyer accepts a provider budget privately. |
| Delivery and support owner | The OpenAgents owner supplies delivery and one named private support contact until a collaborator accepts that role. Acknowledge requests within one business day during the agreed business hours; support lasts through the pilot review and stays inside the three-hour cap. There is no availability SLA or continuing maintenance promise. |

**Data policy.** Public source still needs disclosure permission. The buyer
approves sending the task and relevant source to the named model/decision
providers, and sharing the patch, checks, and necessary redacted diagnostics
with the named OpenAgents delivery person. Keys remain on the buyer's machine;
do not copy credentials, unrelated files, private repositories, or personal
data into the pilot. [Coder traces](../coder/runtime/traces.md) stay local
unless the buyer separately approves an exact redacted export. The operator
deletes shared task content and diagnostic copies within 30 days after review
and records deletion; this is a manual service obligation, not an implemented
automatic retention feature. O1 fixes the separate invoice/consent retention
period and provider terms before activation. The buyer controls local copies.
Training, reusable customer examples, public benchmarks, and marketing use
require separate permission; none is included in this offer.

**Cancellation and recovery.** The buyer may stop before acceptance without a
service invoice; provider charges already incurred remain theirs. Retain the
stop, partial result, and unknown execution/cost states before any new attempt;
never replay uncertain writes automatically. An accepted delivery creates the
invoice obligation. A later defect uses the included repair allowance; any
refund, credit, extension, or replacement scope needs a recorded private
agreement. No automatic product refund or Lightning payout is promised.

**Accept, extend, or stop.** At review, accept only the independently checked
patch, agreed behavior, and runbook. Stop on a missing ready provider, revoked
disclosure, unsafe input, unconfirmed stop, failed checks after the repair, or
any cap reached. Extend only with a new versioned scope, price, budget, and
review date accepted by both parties; silence never extends a pilot.
Publication, private repositories, new integrations, department-agent training,
remote shells, guaranteed savings, and autonomous selling are excluded.

A warm buyer can agree through a private introduction and written scope now:
record the offer version, task/check digests, workflow owner, delivery person,
data recipients, provider budget, price, support contact, and review date;
then qualify the install and start only after O1 and buyer acceptance. REV-04
and REV-06 make that manual record and pilot kit durable. No public offer page,
shared balance, referral program, or agent floor is required.

If a later pilot buys cloud compute, it needs a separate retail admission and
O3/O4: [retail v1](../cloud/retail-contract.md) uses Boat `large`, the buyer's
OpenAI key, and no publication, with [price book `retail-2026-10-06.1`](../cloud/retail-prices.md)
at 40 millisatoshis/second plus 100 sats per started task, at most 244 sats
for 60 minutes. A paid plugin likewise needs its own exact release, quote,
payment, and O2: the current [paid endpoint](../payments/2026-10-02-central-receive-and-splits.md#6-paid-endpoints-on-the-central-receiver)
admits one guest step requiring no capabilities, with an empty read snapshot.
Neither product charge is included in the service fee, and a service invoice
does not qualify product settlement or author payout.

## Principles

- **A strict upgrade as the goal.** Prove quality, cost, and time on a team's
  own work before making a comparative claim. Extra capacity from a fixed
  subscription is different from a lower bill; report each accurately.
- **Usage, not contracts.** We don't sell multi-year commitments or seat
  bundles. Customers connect a payment method, use what they need, and stop
  when they want. Our incentive is to make usage worth more, not to lock it in.
- **Bring your own subscription.** Coder uses the accounts customers already
  pay for. We earn on paid resources and services we add. Routing local work
  through a customer's login does not automatically create a charge for us.
- **Prove it on their work.** Every claim we make to a business should come
  with evidence from their own tasks: receipts, the Gym, and before-and-after
  costs.
- **Open by default.** The protocols are open (Nostr NIPs in
  [`nips/openagents/`](../../nips/openagents/)), so customers and partners can
  build on them, and identity and reputation travel with the agent, not with
  our database.

## How customers arrive

1. **A developer installs Coder.** It detects supported agents and logins,
   reports what is ready, and helps them complete a first useful task.
2. **They use more of it.** Cloud computers when their machine is busy, paid
   plugins, and supported paid capabilities their current tools do not supply.
3. **They bring their team.** A champion shares sessions, plugins, and
   department agents with colleagues.
4. **The team becomes a business account.** Shared billing, an admin view, and
   policies.
5. **The business expands.** Measured workflows become department agents and
   reusable plugins, with policies and evidence for the team.

Businesses can also arrive through a direct introduction, a partner, or a
public demonstration. Hands-on onboarding starts with one scoped pilot; it
does not wait for organic team conversion or the full admin product. The
[roadmap's funnel](revenue-roadmap.md#sales-and-pilot-operations) defines the
handoff from lead to accepted pilot, paid use, and expansion.

## Pricing and billing

- **First service offer.** Use [Coder pilot v1](#first-workflow-offer-v1):
  one checked public-repository change, a proposed USD 250 service fee,
  one 30-minute free discovery call, and zero promotional credits. O1 and
  buyer agreement activate it. Broader business credit remains a decision.
- **Pay as you go.** Usage is metered per resource (compute, gateway calls,
  plugin calls) and charged against a prepaid balance or a card. The retail
  cloud already defines quotes, holds, metering, and settlement
  ([compute balance](../cloud/compute-balance.md),
  [price book](../cloud/retail-prices.md)).
- **One balance across products is the goal.** The purchased compute balance,
  paid-call funding, and decision gateway currently have different accounting
  paths. Connecting them requires account mapping and settlement adapters,
  with their existing units and rights preserved.
- **Payment rails.** Plan cards for businesses; the existing decision billing
  provider is sandbox-only, so live card collection needs an adapter.
  Lightning serves individuals, plugin authors, and machine-to-machine payments
  ([NIP-X402](../../nips/openagents/NIP-X402.md)).
- **Margins come from what we add.** Price against full delivery costs,
  including failed work, checks, payments, support, and incentives. Provider
  grants can fund a trial; they do not establish sustainable margins.

## Affiliate and referral program

Affiliate programs built large businesses on the early web, and the agent space
has no real equivalent yet. Ours pays people, and their agents, for the usage
they bring. Episode 239 set the rule: refer once, earn forever.

- **Referral links.** Anyone, person or agent, can create a referral link. New
  accounts that arrive through it are attributed to the referrer permanently.
- **Earn on usage.** The referrer earns a small share of everything their
  referrals buy anywhere in the OpenAgents ecosystem, for as long as those
  accounts stay active, under a published rule for eligible settled usage.
  Free credits and unused top-ups do not create commission. Business accounts
  they bring in count under the same attribution rules.
- **Agents sell too.** An agent with a key pair can hold a referral link,
  bring buyers, and earn. A network of selling agents is a sales force that
  scales with the number of agents, not with hiring.
- **Paid in sats or credit.** Earnings accrue to the referrer's balance and pay
  out over Lightning or as credit.
- **Transparent.** Referrers see their attributed accounts, usage, and earnings
  in their account. Attribution and payouts post to the same ledger as other
  billing.
- **Abuse resistant.** Self-referral, fake accounts, and recycled credits are
  the obvious attacks. Attribution needs real paid usage, holds before payout,
  and reputation signals.
- **Plugin authors and agent builders are affiliates too.** A plugin that
  brings new users earns twice: its own fees and the referral share.

## Partners and fulfillment

We are the front door, not the whole house. Some buyers need work we don't do
ourselves, such as performance marketing or brand design, and some partners
already serve businesses that need agents.

- **Partner organizations** (agencies, startups, and agent groups) fulfill
  parts of a buyer's order. We refer them business, build their offering into
  ours, or both, with referral commissions in either direction.
- **The coding agent pool.** Plan to pay contributors for accepted work that
  customers need, with explicit checks and receipts. Paid labor and dispute
  handling need their own qualification; a free labor host is not paid
  fulfillment.
- **Both sides at once.** Sellers (compute, data, labor, verification) join
  through the Pylon network; buyers come through the sales funnel. Supply has
  always been the easy side; demand closes the revenue loop.

## Sell in public

We built in public; now we sell in public (episode 247). We share what works
and what doesn't: launches, revenue graphs, conversion, and lessons, in the
same channels where we show the product. It keeps us honest, it recruits
affiliates and partners, and it shows buyers real results instead of claims.

## Sell through our agents in Everglade

Episode 239 called agents a sales force that scales with the number of
agents. We start with our own. The [agent sales floor](agent-sales-floor.md)
plans a small sales organization of OpenAgents agents, built on the
[crew](../verse/crew.md)'s shared machinery and run inside
[Everglade](../verse/everglade.md):

- **Paul leads it.** In the cryptography cast's search games, Paul asks the
  questions, which is how discovery-led selling starts. He plans the work,
  reviews drafts, runs training, and proposes a few hires: a researcher, a
  prospector, a demo agent, a partner-channel agent, and an
  affiliate-program agent.
- **The owner holds every external decision.** Each hire, each message at
  first, and every price, term, and agreement waits for the owner's CONFIRM.
  Batched approvals come only as the record earns them.
- **Agents train before they sell.** A playbook built from this page, a
  claims register backed by evidence, role-play against simulated buyers,
  and Gym suites that grade claims, compliance, and tone.
- **The work is visible.** The Agora building stands north of the market
  hall, facing Fountain Plaza, after the west-of-Lantern site failed its
  survey. Its standing desks and bell hook are built; agents, private
  record-backed boards, and earned-sale triggers remain to be integrated.
  The bell requires an earned, attributed, settled sale and delivery
  evidence. Live amounts and bell events are private by default.
- **Selling stays honest.** Agents disclose that they are AI, make only
  verified claims, honor every opt-out, and follow anti-spam law. The
  boiler-room energy stays in the world, never in how we treat a real
  person.

The floor serves the roadmap's sales operations (G7) and R0 lead work. It
doesn't gate the first payment: Paul alone, with a verified claims register
and drafts the owner sends, is the first useful slice.

The [October 7 decisions](agent-sales-floor.md#initial-operating-decisions)
keep Paul and the proposed names, start with permissioned US email, cap
floor-wide model use at $5/day, ramp from five to twenty daily messages,
and keep launch conversations written with human closing. Batching needs
an implemented gate, measured evidence, and an explicit grant; four weeks
alone doesn't qualify it.

## Business accounts

What a team needs to adopt Coder as a company:

- **Workspaces and members** with owner, admin, and member roles, invitations,
  and recovery ([`tenancy::accounts`](../../crates/tenancy/src/accounts.rs)).
- **An admin view** of agent work: tasks by person, team, and type; cost; wait
  times against service levels; which models and plugins ran; and outcomes.
- **Spend controls:** budgets per team and person, alerts, and routing policy
  (for example, routine tasks always go to the smallest capable model).
- **Policies:** which data may leave, which plugins and models are allowed, and
  where work runs (local, our cloud, or the customer's own computers).
- **Evidence:** receipts for each task and periodic reports that show which
  workflows agents handle reliably and where they don't.
- **Single sign-on and audit export,** when larger customers ask for them.

## Hands-on onboarding

Most businesses want someone to fit the tools to their work. We offer
forward-deployed help: an engineer works with the customer's team for a short
period, connects Coder to their existing tools and accounts, writes the plugins
and department agents they need, and leaves them with measured, repeatable
workflows.

- **Start where they are.** If they already use an agent, Coder routes to it.
  The first deliverable is a measured improvement in accepted work, cost,
  completion time, or capacity from their existing subscription.
- **Build department agents.** Turn a department's documents and procedures
  into an agent the team directs, with a test set in the Gym so its
  performance is tracked over time.
- **Measure.** Before-and-after cost, time to completion, and service-level
  adherence on the customer's own tasks.
- **Make it repeatable.** Every engagement should produce reusable plugins,
  templates, and runbooks, so the next one is faster.

## Agents as products

Agents and plugins become things people can find, trust, and pay for.

- **Identity.** The [identity and engrams epic #10807](../verse/agent-identity-and-engrams.md)
  plans each agent's key, private memory, and steering loop; portable
  lifecycle and owner-device sync remain open phases. Plugin releases bind the
  publisher's identity and exact version
  ([NIP-SOV](../../nips/openagents/NIP-SOV.md),
  [NIP-HOST](../../nips/openagents/NIP-HOST.md)).
- **Reputation.** Verified work earns experience
  ([NIP-XP](../../nips/openagents/NIP-XP.md)); reviews and track records attach
  to the agent's identity, not to our database.
- **Registries.** Planned curated plugin registries
  ([NIP-REG](../../nips/openagents/NIP-REG.md)) and a marketplace
  ([NIP-MKT](../../nips/openagents/NIP-MKT.md)) let buyers find agents and
  plugins by skill, price, and reputation.
- **Discovery partners.** Because identity and reputation live on Nostr,
  third-party search and web-of-trust indexes can help people find publishers.
  Verify each index's coverage; a social score does not establish capability
  availability or work quality. A first partner plugin and separate public
  discoverability pilot are planned in
  [`docs/plugins/brainstorm-v1-integration.md`](../plugins/brainstorm-v1-integration.md).

## What exists today

The October 6 documentation and code review separates foundations from a
customer-ready offer. It includes no new live payment or deployment check.

| Need | Foundation and remaining boundary |
| --- | --- |
| Coder | [Live bundled plugins and installed-login paths](../../crates/coder-new/README.md); customer account, top-up, and paid-call purchase controls still need integration |
| Accounts | [`tenancy::accounts`](../../crates/tenancy/src/accounts.rs) and sessions; product-wide identity and billing mapping remain work |
| Card billing | [Plans and checkout](../decision-models/service/billing.md) are implemented with a sandbox provider only |
| Cloud | [Balance](../cloud/compute-balance.md), retail lifecycle, and [fake/simulated qualification](../cloud/retail-qualification.md); customer service integration and funded launch remain gates |
| Paid plugins | Signed releases and a deployed pay front; the [October 3 receipt](../payments/2026-10-03-end-to-end-demo.md) proves the challenge path, not funded execution and author payout |
| Shared money | [Payment ledger and splits](../payments/README.md); compute funding, paid calls, and gateway accounting still require a unified customer path |
| Evidence | Receipts and the Gym; customer baselines and independent workflow acceptance must connect to them |
| Discovery | Nostr identities and protocol contracts; supported indexing, usable listings, and referral conversion require qualification |

## What we need to build

The [missing-pieces table](revenue-roadmap.md#missing-pieces-and-existing-owners)
covers product integration, payment, sales operations, and launch evidence.
The first priorities are a supported offer, cost and quality proof, a funded
purchase path in the client, and a bounded business pilot. Shared billing,
attribution, team controls, and reusable department agents build on that path.
The [agent sales floor's build order](agent-sales-floor.md#build-order) adds
the agent-run sales operation: Paul and the sales records, training and
certification, outbound under the owner's approval, hiring, and the Agora in
Everglade, about 66 incremental agent-hours alongside applicable shared
agent and world dependencies. #10807 estimates about 33 shared agent-hours
separately; neither estimate is a launch schedule.

## Roadmap

Use the [unified revenue roadmap](revenue-roadmap.md#delivery-order) for the
order and exit criteria: prove an offer, collect the first payment, earn repeat
use, qualify referrals and partners, then expand teams and workflows. Assisted
pilots and acquisition preparation run alongside engineering from the start.
Existing terminal, router, cloud, and payment backlogs keep their technical
ownership; this roadmap orders their contribution to revenue.

Its [build issue list](revenue-roadmap.md#build-issue-list) is the consolidated
inventory: 76 detailed GitHub issues with stable IDs, responsibility areas,
dependencies, and acceptance, including the sales floor. At filing, 74 remain
open and the two Agora survey/art items are complete. It links completed
foundations and existing open world issues, identifies the smallest revenue
path for each offer, and separates owner activation from code completion.
The [Revenue and Sales project](https://github.com/orgs/OpenAgentsInc/projects/21)
organizes all filed work and shared prerequisites by delivery, workstream,
status, and readiness, with native dependencies and current blocker mirrors.
All issues also remain on the required OpenAgents project. Conditional
integrations and alternative lanes stay explicit in issue scope.
Its [shared-agent phase map](revenue-roadmap.md#shared-agent-foundation-10807)
reuses #10807: Alice steering Coder first, then generic crew machinery for
Paul. Sales adds current-record adapters, narrowing charters, aggregate
budgets, and owner-approved outreach; first revenue can proceed through humans
while the epic remains open.

The agent sales floor runs alongside from R0: Paul and a verified claims
register first, then certified hires sending under per-message approval, then
batched approvals, referrals, and partners as R3 terms land. Its phases and
dependencies are in the unified inventory; its
[build order](agent-sales-floor.md#build-order) retains the detailed S0–S6
scope and rough effort.

## Measures

- Weekly active developers, and how many of them pay for anything.
- Savings per active developer, measured on their own work.
- Paid usage per account, and how it grows month over month.
- Earned usage and service revenue, separately from cash top-ups and unused
  purchased balances; contribution margin after delivery and incentives.
- First accepted task, first settled purchase, and repeat paid use by cohort.
- Share of new accounts that arrive through referrals.
- Teams per champion, and time from first install to first team account.
- Onboarding engagements, time to first measured result, and retention after
  the engagement ends.
- Plugin author earnings, a health measure for the marketplace.

## Considerations and risks

- **Subscription terms.** Coder routes through customers' own subscriptions.
  We must respect each provider's terms and make it clear which account pays
  for which work.
- **Price changes upstream.** Model prices and subscription limits change
  often. Usage pricing and routing let us adjust quickly, but cost claims must
  be measured, not assumed.
- **Affiliate fraud.** Attribution and payouts need holds and reputation
  checks from the first day.
- **Data governance.** Businesses will ask where their code and documents go.
  Policies, local execution, and clear disclosure come before scale.
- **Services that don't scale.** Hands-on onboarding is valuable early but must
  turn into reusable templates, or it caps growth.
- **Claims about replacing people.** Businesses will ask whether agents can
  take over work. We should frame it as evidence about which workflows agents
  handle reliably, and leave decisions about people to the people who run the
  business.

## Background

The revenue loop has been part of the plan for a while. These episodes in the
retained [transcript archive](../transcripts/README.md) describe it:

- [Episode 239, Let's Make Money](../transcripts/239.md): the turn from supply
  to buyer demand, persistent referrals ("refer once, earn forever"), and
  agents as a sales force.
- [Episode 247, Sell in Public](../transcripts/247.md): lead generation with
  affiliates, partner fulfillment, the paid coding agent pool, and sharing the
  results in public.

The [Coder-era review](revenue-roadmap.md#what-the-background-changes) adds
episodes 275–289: dependable daily use, cloud placement, trusted devices,
measured System One work, and the composable general agent. Coder supplies
the first concrete buyer workflow; successful plugins extend it into other
work without requiring every surface or market to launch together.

## Open questions

- Should later business offers add a starting credit, and on what eligibility
  and spending rules? Coder pilot v1 defaults to zero credit.
- What referral share is sustainable for permanent attribution, and should
  business referrals earn differently from individual ones?
- Does the proposed USD 250 Coder pilot fee cover measured delivery and
  support costs? Confirm it under O1 before selling; later offers may differ.
- Which admin features does the first business customer need before they pay?
- Which department agents should we build first as reusable templates?

The sales floor's former questions about names, hiring, outreach, trust,
the building, visibility, voice, and human handoff now have
[provisional operating decisions](agent-sales-floor.md#initial-operating-decisions).
