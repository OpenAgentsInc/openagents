# Sales and revenue

Status: proposal, updated October 7, 2026. This page records the sales strategy
for OpenAgents, with Coder as the first adoption path. The
[unified revenue roadmap](revenue-roadmap.md) owns the delivery order, missing
pieces, dependencies, and evidence needed to earn revenue. The
[agent sales floor](agent-sales-floor.md) plans how our own agents do the
sales work, led by Paul from a building in Everglade. Product and payment
documents retain their implementation contracts. These are public plans;
customer records, compensation agreements, and negotiations stay private.

The [implementation handoff](revenue-handoff.md) records the completed seven-issue
scope, retained verification, owner activation, and the remaining build order.

The [Coder Cloud web specification](../cloud/coder-cloud.md) maps these offers,
customer and team controls, private sales operations, and the Agora into the
proposed openagents.com application over the existing Rust owners.

[Self-improving codebases](../product/self-improving-codebases.md) specifies the
product this sales plan grows into: the Coder pilot's single checked change,
made repeatable and improving with every accepted pull request.

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
- [Private sales pipeline](#private-sales-pipeline)
- [Assisted pilot kit](#assisted-pilot-kit)
- [Delivery, offboarding, and support](#delivery-offboarding-and-support)
- [Accepted partner assignments](#accepted-partner-assignments)
- [Invoiced services](#invoiced-services)
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
| Resource payers | The buyer pays their model provider and uses their own computer. OpenAgents earns only the service fee. Approved Codex/Claude routing and any separately configured Jev connection name their recipients and payer before use; disable [sponsored cloud fallback](../coder/runtime/cloud-fallback.md) with `CODER_CLOUD=off` and default hosted decisions with `OPENAGENTS_JEV_HOSTED=off`. Existing decision-provider configuration must be disabled or separately admitted. No operator credential or unapproved paid provider is used. Subscription capacity, list-price estimates, and actual API charges remain separate; the buyer accepts a provider budget privately. |
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
- **Free daily allowance.** A signed-in account gets about $1 of model cost
  a day on Coder's `auto` (`openagents/auto`, `openagents/fast`), funded
  from our Google credit, before its own balance
  ([providers](../inference/providers.md#free-daily-allowance-11264)).
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

The [stable introduction source](../../crates/tenancy/src/accounts/referrals.rs)
records a person, agent, author, or partner under a separate referrer ID.
`openagents customer referral` uses the selected authenticated account to manage
links and capture explicit consent. `/join?ref=TOKEN` contains random lookup
material; it grants no account access. Signup preserves missing, declined,
unknown, disabled, and malformed source outcomes. Exact capture replay preserves
the original record; corrections use a separate attribution decision.
Rotation and accepted management migration disable earlier links while keeping
historical source identity. The private pipeline owner can `record_acquisition`
from the canonical account directory, separately from unverified intake text.
These source records establish no earnings right. The separate
[`referrals::attribution` decisions](../../crates/tenancy/src/accounts/referrals/attribution.rs)
retain customer consent to exact operator-published terms. New signup sources
can establish an accepted relationship; early agreements, existing customers,
and corrections require explicit review by the customer and current referrer
manager. Missing, competing, self-referral, and source-only evidence stays in
review. Legacy records without signup provenance remain unknown. Original
sources, policy versions, reasons, and earlier decisions stay in the canonical
account history. Team creation inherits that relationship; ownership transfer
retains its original customer. Workspace reads require current owner or admin
authority and expose only the workspace's binding. A key change cannot replace
the stable referrer, and management succession creates no earnings eligibility.
The separate
[`referrals::commission` contract](../../crates/tenancy/src/accounts/referrals/commission.rs)
retains operator-declared eligible products, earned OpenAgents base, rational
share, exact native unit, rounding, hold, minimum, qualified destination rules,
reversal obligations, and permanence. There are no commercial defaults. The
selected Spark and Lightning rails require BTC denominations and an exactly
payable whole-satoshi minimum; they retain unpaid msat remainders. Foreign
currency contracts are refused. A terms agreement establishes no current
destination or payout qualification. The local operator checks terms and
publishes only an explicitly approved digest.
`openagents customer referral terms`, `accept-terms`, and `commission` inspect
and record both native parties' exact consent under current credentials. An
accepted version survives later publication and referrer management succession;
both parties must accept replacement terms before the active agreement changes.
Historical reads identify whether the agreement is active for new transactions;
later settlement must use the original transaction's pinned agreement.
Current review or correction does not rewrite those original transaction pins.
Attribution review suspends new qualification while retaining historical proof.
Publication and acceptance enable no accrual or payout. REV-30 must verify each
earned settlement and its full costs before the central ledger records a
commission. Unused funding, free or promotional credit, self-referral, recycled
funding, unknown costs, and unresolved attribution are excluded; signed author
fees remain outside the commission base.
OpenAgents sales-agent sources remain source-only. Review wording and consent
before distributing links, as recorded in [NEEDS_OWNER.md](../../NEEDS_OWNER.md).

Affiliate programs built large businesses on the early web, and the agent space
has no real equivalent yet. The planned program pays people, and their agents,
for the usage they bring. Episode 239 set the rule: refer once, earn forever.
The following paid behavior depends on qualified terms and later settlement contracts.

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

The installed CLI's [team commands](../cli/README.md#commercial-customer-openagents-customer)
connect the existing account roles and invitations to explicit workspace and
payer selection. A champion can issue a private invitation, a colleague can
accept it once, and current membership controls access after role changes or
revocation. Switching workspaces preserves the payer on retained purchases.
These controls establish the team entry path. The
[native Gateway budget profile](../decision-models/service/monetary-accounting.md#hierarchical-budgets)
enforces cumulative workspace, reviewed team, and person caps in its existing
money journal, with scoped threshold and blocked-action alerts. Other product
ledgers, funded product admission, admin reporting, and genuine colleague
qualification remain separate gates.
Record that qualification under [O8](../../NEEDS_OWNER.md#team-adoption-o8-rev-38-10845)
before treating a synthetic team fixture as an adopted business account.

Reviewed commercial bindings now join the selected Gateway and Plugin native
sources to a canonical Accounts customer and workspace. The installed CLI reads
each product reference independently; quotes, funding records, and execution
receipts retain their original revision through rotation and team conversion.
A changed mapping requires new purchase approval, and revocation blocks new
effects. Native payers, balances, membership, and payout rights remain separate.
Original Plugin outcomes remain recoverable under current native read access
after canonical retirement or revocation; recovery never pays or executes again.
Qualify the intended mapping under [O1](../../NEEDS_OWNER.md#commercial-mapping-activation-o1-rev-19-10826)
before treating these isolated fixtures as a funded business account.

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

## Selected installation and first task (REV-02)

The initial service selects source-built `coder` and companion `openagents` on
macOS arm64 from clean commit
`52736d04dde8575b2b62d1e519fc2d25949dde5f`. The
[qualification record](install-qualification.json) pins the installer, release
binaries, setup time, and check artifacts. This path uses the existing
[source installer](../../scripts/install-coder.sh), terminal/headless turn, and
buyer-owned Codex login. Other client, platform, provider, and store combinations
need separate qualification.

1. Check out the pinned commit in a clean clone. Run
   `./scripts/install-coder.sh`, then start a terminal with its installed
   `~/.openagents/bin` on `PATH`. Confirm `coder --version` and
   `openagents --version` identify the same clean commit.
2. Use your own supported Codex login. Set `CODER_CLOUD=off` and
   `OPENAGENTS_JEV_HOSTED=off`, and select `CODER_DELEGATE=always` with
   `CODER_DELEGATE_AGENT=codex` so an unavailable Codex login refuses instead of
   choosing another provider. Existing `TYPESAFE_*`, `CODER_DECISION_*`, or
   `~/.openagents/jev.json` configuration can still enable decision access;
   disable it for this environment without deleting saved configuration, or
   separately approve its exact recipient and payer before work.
3. Run `coder doctor`. Confirm the selected executor is Codex, its login is
   available, and the displayed decision/cloud settings match the agreement.
   Missing login is a setup refusal, not a completed task. Resolve it before
   sending buyer content.
4. Freeze the authorized repository commit and acceptance commands in an
   isolated worktree. Use the [headless guide](../coder/guides/headless.md) for
   the task and private trace. Retain the exact candidate patch and check that
   candidate independently in a separate clean worktree; an answered turn alone
   does not establish delivery acceptance.
5. On failure, retain its typed result and private trace. On interruption, stop
   the caller and its supervised executor through the existing task/terminal
   controls, inspect any partial effects, and leave acceptance unknown. Start a
   fresh attempt only after that inspection; do not automatically replay an
   uncertain effect. A fresh process must recheck the retained candidate.

Keep the previous installed binary pair for recovery. The installer restores
both prior commands if either staged command fails its version probe. Use
`./scripts/install-coder.sh --rollback` to restore the previous pair. Support
receives the source revision, binary/patch/check digests, refusal or failure
status, and agreed redacted diagnostic references, never credentials or private
source copied into an issue.

For reproducible code acceptance, run
[`scripts/qualify-coder-first-task.py`](../../scripts/qualify-coder-first-task.py)
with the absolute installed `--coder`, full `--commit`, and a new `--root` under
`openagents scratch`. Run the checker from current main against the pinned
installed pair; the pinned installation revision predates this acceptance tool.
It exercises the actual installed binaries with an offline
Codex protocol fixture, isolated HOME, disabled hosted decisions/cloud, and
independent candidate, failure, stopped-effect, and restart checks. It uses no
real provider login. This establishes deterministic code behavior;
[O8](../../NEEDS_OWNER.md#selected-install-and-first-task-o8-rev-02-10809)
still requires a genuine buyer installation and accepted task.

## What exists today

The October 6 documentation and code review separates foundations from a
customer-ready offer. It includes no new live payment or deployment check.

| Need | Foundation and remaining boundary |
| --- | --- |
| Coder | [Live bundled plugins and installed-login paths](../../crates/coder-new/README.md); customer account, top-up, and paid-call purchase controls still need integration |
| Accounts | [`tenancy::accounts`](../../crates/tenancy/src/accounts.rs) and sessions; product-wide identity and billing mapping remain work |
| Card billing | [Plans and checkout](../decision-models/service/billing.md) are implemented with a sandbox provider and Stripe subscriptions (test mode until the owner adds live keys) |
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

## Private sales pipeline

`openagents sales` stores one private lead/account pipeline in an explicit
host root. Its records and mutations are owned by
[`coder::task::sales`](../../crates/coder/src/task/sales.rs); later sales-agent
adapters extend these records. Initialize it with a named responsible owner
and a credential file outside the root's `sales` directory:

```sh
openagents sales init --root PRIVATE_HOST_ROOT --owner founder --credential OWNER_FILE
openagents sales issue --root PRIVATE_HOST_ROOT --credential OWNER_FILE --human collaborator --role writer --new-credential COLLABORATOR_FILE
openagents sales apply --root PRIVATE_HOST_ROOT --credential OWNER_FILE --input PRIVATE_COMMAND_FILE --json
openagents sales list --root PRIVATE_HOST_ROOT --credential OWNER_FILE --limit 50 --json
```

The command envelope uses `openagents.sales.pipeline-command.v1`, a stable
`id`, `lead` (null for creation), `expected_revision` (zero for creation), and
an `operation` tagged by `kind`. General pipeline operations are `create`, `update`,
`propose_handoff`, `accept_handoff`, `reject_handoff`, `suppress`, and `delete`.
The Rust `Command`, `Input`, and `Details` types define the bounded JSON shape.
Keep command files private. Exact-byte retries return the original receipt;
reusing an ID with changed bytes or a different actor refuses. Updates and
handoffs require the current record revision.

A record retains contact/source/date, jurisdiction, permission evidence and
expiry, workflow/baseline, recipients/use/retention, stage, accepted human,
next action/date, and customer decision. Human recipients use `human:ID`.
The pipeline owner, responsible human, readers, and proposed handoff target
must fit that boundary. Only the owner can change recipients/use or extend
retention, with fresh permission evidence. Renewing permission or adding a
channel also requires fresh evidence. A writer manages its own records;
a reader sees only expressly granted records. The target accepts a handoff
with its own credential before accountability changes.

Use `show`, `export`, `audit`, and `suppressed` for private inspection. Exports
create exclusive private files outside the store. Revocation is checked on
every operation. Permission expiry closes qualification and cancels handoffs;
retention expiry, deletion, and suppression remove contact-wide content while
keeping salted contact hashes and digest-only references to prevent recontact.
Cleanup runs when the store opens or is used, so an idle host does not purge
on a timer. The store holds at most 512 live records and 4,096 receipts; it
reserves cleanup capacity before ordinary history fills. Version 1 has no
unsuppression or history-compaction operation.

The host stores private files atomically under its existing lock and
filesystem rules. It stores credential digests and never prints secrets.
These records grant no outbound, model/provider, or customer-data disclosure
authority. O1 confirms real humans, consent, and retention privately; O6
separately authorizes any real outreach, as recorded in `NEEDS_OWNER.md`.


REV-60 adds `openagents sales privacy view|apply|check|prune` to this same
store. Before assigning a lead or preparing an agent draft, the owner records an
`openagents.sales-contact-command.v1` `admit` operation. It pins the current
lead revision, original customer/account ID, exact source and permission-reference
digests, known US business scope, and requested contact or accepted introduction.
Supported source kinds are `given_business_role` and `published_business_role`.
Only explicit bounded ASCII email and Nostr identities normalize; the proactive
check supports permissioned business email. Unknown or private source categories,
unknown jurisdiction, stale pins, ambiguous aliases, and missing consent refuse.
These are attributed owner records, not independent contact verification.
`privacy check --lead ID --channel email` confirms current admission and grants
no sending, model, or relay authority.

A current human writer can record an immediate `opt_out`, including ambiguity,
without renewing expired permission. The command suppresses every linked alias
and original customer across hires and channels. Suppression survives source,
policy, credential, and native-key changes, removal, and reimport. Legacy contacts
that fail new commercial validation can still be suppressed or deleted. There is
no unsuppression command. The default inactivity period is 90 days; an owner
`policy` operation records a new version and bounded period. An `engagement`
operation records an actual customer reply or accepted introduction with its
original time and evidence reference. Operator edits and model preferences do
not extend inactivity. Expiry cleanup runs on native store use, not an idle timer.

Private lead, service, partner, funnel, and weekly exports use one native copy
writer. It requires an owned private parent (`0700` on Unix), creates an exclusive
private file (`0600`), and retains exact file identity, digest, recipient, and the
original retention deadline. Each copy is at most 2 MiB; the store tracks at most
1,024. Suppression and expiry remove exact retained copies; replaced, unsealed,
or inaccessible copies remain visibly unavailable for cleanup. Extending a lead
never extends a service or funnel copy's original boundary. Deletion removes
assignments and drafts, minimizes owned memory, journals, caches, and local
encrypted engrams, and preserves native keys and opaque original payment,
fulfillment, and audit references. Signed historical verdicts containing customer
material remain unavailable for projection and require owner repair rather than
rewriting approval evidence. Unmanaged captures and historical relay erasure are
never reported as verified.

Known customer and credential fingerprints also screen native prompts, records,
journals, memory, core heads, snapshots, and role-stripped imports. Screening
checks decoded JSON strings and JSONL, including escaped identifiers. Knowledge
drafts also screen parsed front-matter fields before writing, projection, and
cleanup. Native views and queued reports recheck current canonical identifiers;
legacy leads reconstruct bounded fingerprints from their retained fields. It bounds
each copy at 2 MiB, 131,072 fingerprint checks, 4,096 JSON nodes, and depth 16;
exhaustion returns unavailable. The canonical state has its separate 8 MiB bound.
Customer model disclosure is unavailable for the enabled sales profile before
model factories run, so it creates no customer Coder trace. Sales relay sync
stays off, including owner-key relay reads; active-profile relay reads require an
exact current local non-sales subject and screen decrypted bodies before output.
Missing configured privacy state refuses these consumers. REV-53's fixed opaque
memory projection remains available under its original grant. Future model,
sender, and sync adapters must separately qualify their recipient boundary;
fixtures do not activate any of them. O6 retains the real-contact and legal
activation steps in `NEEDS_OWNER.md`.

REV-53 adds owner-issued native agent access to this same private store. Use
`openagents sales agents anchor --agent paul` to read the current native key,
owner attestation, job role, and exact charter pins. `agents owner` reads the
owner's policy revision and manual certification history. `agents policy-check`
checks an explicit `openagents.sales-policy.v1` document and returns its digest;
`agents owner-apply` records an `openagents.sales-agent-owner-command.v1` with
that current revision. Its operations are `publish_policy`, `revoke_policy`,
`assign`, `revoke_assignment`, `review_draft`, and `record_certification`.
An assignment also requires `--new-credential FILE` outside the complete host
root, the exact current lead revision and native anchor, a current policy digest,
and an expiry within consent, retention, policy, and native attestation limits.
Assignment credentials must be new exclusive files; removed or revoked tokens
cannot be reused for another lead. Keep all input and credential files private.
The Rust types in
`coder::task::sales::agents` define the versioned JSON shapes.

The initial supported policy records explicit US jurisdiction, channels,
allowed agent public keys, recipients, America/Chicago business time, review
caps of at most 20 proposals per day across the floor, per-agent caps, trust,
execution budget, playbook, and exact read/write fields. Stage and next-action
reads are required; contact, source, workflow, permission, and customer-decision
reads are separately granted. Native recipients use `agent:PUBLIC_KEY` alongside
human recipients. Adding that recipient requires fresh owner-recorded consent
through the existing pipeline update. Unknown jurisdiction, absent or expired
channel consent, a changed recipient boundary, superseded or revoked policy,
changed native key or charter, and paused or stopped agents refuse access.
REV-52's durable crew barrier and member epoch also govern this native route;
an applying or partial stop blocks it before lifecycle cleanup, and resuming
requires a fresh assignment for the new epoch.
Declared execution budgets and trust levels grant no model execution or sending.

Use the assigned credential with `agents read`, `agents apply`, and `agents
memory`. An `openagents.sales-agent-command.v1` can only `update_stage`,
`update_next_action`, or `propose_draft` when its exact field is granted. It
cannot select another lead, change permission or policy, issue grants, or record
certification. Human and agent reads use the same canonical stage and next
action after restart. Drafts pin the native author, original policy, playbook,
template, check and recommendation references, and lead revision. They remain
proposals; even an owner's `owner_reviewed` decision grants no outbound authority.
References are attributable declarations, not independently rerun checks.

Memory projections contain randomly minted stored references, stage, whether a
next action exists, draft references, and the fixed `owner_review_required`
lesson. They contain no address, message, permission text, exact next-action
text, identifying summary, or contact-derived digest. Each projection rechecks
current native identity, assignment, policy, consent, and recipients. This command
does not write engrams or relay events, and edited memory cannot grant access.
Manual `openagents.sales-cert.v1` references retain the owner, native identity,
playbook, suite, roleplay, and draft-review references with an explicit state and
expiry. Their basis is `owner_recorded`, `measured_qualified` is false, and
outbound authority is false; REV-57's measured certification remains separate.

The additive migration preserves native identities and canonical lead, service,
partner, and financial references. Suppression and retention erase nested agent
assignments and draft content with the lead. Caps survive policy changes,
reassignment, and restart; exact retries consume no additional proposal.
Day counters retain at most 366 business dates and refuse further growth at
that bound, pending explicit operator maintenance. Reserved history and storage
space keep assignment/policy revocation and contact cleanup available.

## Assisted pilot kit

The [scope and review templates](pilot-kit.json) package the manual service
lane for REV-06. Copy them into a private, mode-0700 pilot directory with
mode-0600 files; replace required nulls before agreement or review. The proposed
USD 250 fee, zero subsidy, seven-day review window, and three-hour operator
cap come from offer v1 and still require O1 approval. Record the exact dates,
buyer provider budget, existing integrations, named humans, input rights,
checks, recipients, retention, and separately selected external payment route.

Create or inspect a qualified private lead first, and put its stable ID and
current revision into the agreement. Freeze each completed agreement's bytes
and SHA-256, then privately retain owner and buyer acceptance of that exact
version in a separate acceptance record; the frozen agreement does not contain
its own digest. An input, recipient, price, scope, or cap change creates a new agreement and new acceptance. Preserve the
old version under its retention policy. A proposed handoff does not replace
the pipeline's accepted responsible human. This is a manual service procedure;
the template does not grant execution, disclosure, or spending authority.

Link the private agreement from the pipeline's `workflow` field and its frozen
baseline manifest from `baseline_reference`. Use `stage: pilot` and a `next`
action with the agreed review date. Refer to artifacts by private relative
path and exact digest rather than copying customer material into field text.
Build the [REV-03 comparison](evidence.md) from every baseline, failed, repair,
and retry attempt, including setup, checks, support, actual charges, estimates,
subscription capacity, and unknown costs. Bind the review to that report digest,
the exact candidate, independently checked outcomes, and the delivered runbook.

Record one explicit `accept`, `extend`, or `stop` decision with the named buyer's
reference and date. Retain the completed review privately; use its digest in
pipeline `customer_decision.reference`, and update with the current revision
and a fresh stable command ID. Close the finished engagement with no next action.
An extension requires its own accepted agreement, scope/cap, consent and dated
review before returning to pilot stage; silence never extends work. Acceptance
can create a separately invoiced service obligation, but an agreement, product
funding, invoice, confirmed collection, and earned product usage stay separate.
Retain stop/partial and uncertain-effect references, and record shared-content
deletion when due. O1 customer activation remains in `NEEDS_OWNER.md`.

## Delivery, offboarding, and support

The [delivery and offboarding kit](delivery-kit.json) packages REV-07's manual
handoff. Copy it into the private pilot root beside the exact REV-06 agreement
and review. Its handoff pins the customer/account, offer, protected comparison,
candidate, final deliverables, runbook, dependencies, accepted checks, known
limits, retained artifacts, and support boundary. Record a source commit or
exact plugin release where applicable; explain unavailable optional dependencies.
The runbook shows the customer how to start, verify, stop, and recover the agreed
workflow and how to reach its support owner. This kit grants no new authority.

Freeze the handoff bytes before recording separate customer and support-owner
acknowledgments of that digest. Customer acceptance must match the exact
candidate, runbook, and deliverables in the REV-06 accept review. A defect remains
unresolved with an affected result, evidence, responsible human, next action,
and date. A proposed support contact or missing reply does not establish an
accepted support owner. Keep the customer decision and outstanding action in
the [private pipeline](#private-sales-pipeline).

Populate each cleanup class: temporary credentials, host/device grants, test
data, sandbox resources, local copies, and customer access. Keep customer-owned
items that the agreement retains. Record `removed`, `pending`, `unknown`, or
reviewed `not_required` separately for every plan item. `removed` requires the
authoritative owner's removal result or observed absence, a verifier, date,
and retained evidence. A sent request, lost reply, or unavailable owner check
leaves a next action; it never proves that access or a resource disappeared.
Use existing [device removal](../coder/guides/link-devices.md#inspect-and-undo),
[host revocation](../coder/runtime/host-serve.md),
[task cancellation](../coder/runtime/task-owner.md), and the selected resource's
retention/cleanup contract for the exact pilot objects. The template runs none
of these operations and does not retry uncertain effects.

The public blank kit is reusable. Filled customer records and outputs remain
private by default. Prepare any generic example/template separately; retain
explicit rights to its exact digest, permitted recipients/use, and a privacy
review that excludes the first customer's identity, documents, credentials,
and private configuration. Publication requires its own authority and any
applicable [plugin release review](../plugins/README.md). Delivery acceptance,
cleanup evidence, support acceptance, and service payment remain separate.
Real O1/O8 customer and support qualification is tracked in `NEEDS_OWNER.md`.

## Accepted partner assignments

REV-33 adds `propose_partner` and `advance_partner` to the same private pipeline.
The [partner types](../../crates/coder/src/task/sales/partners.rs) separate a
consented discovery brief from fulfillment with an exact deliverable scope,
protected check references, revision/rework limits, support owner, and the
existing service fulfillment obligation. Prepare an owner's approval with
`show --lead LEAD --proposal PRIVATE_PROPOSAL_FILE`; it returns the digest
that `openagents.sales.partner-approval.v1` must name. Apply the proposal with
the owner credential, current revision, and explicit private `--evidence-root`.

Use `show --lead LEAD --assignment ID` or `export --assignment ID` for the
assignment's own admitted scope. A pending recipient sees a minimal invitation,
then accepts the exact proposal digest with their own writer credential.
Acceptance changes neither lead ownership nor execution/disclosure rights.
Proposals are immutable: cancel and prepare a new assignment and owner approval
when recipient or terms change. Refusal, expiry, revoked access, or changed
admission stops the active assignment. Every handoff needs its named target's
separate acceptance; retries preserve the original receipt and obligation.

Fulfillment delivery must match the canonical accepted service sale, customer
acceptance, frozen checks, and support contract. Its payment/bill references
come from that same service record; partner actions send no invoice or funds.
Commission references require an explicitly accepted agreement and retain
attribution separately from the fulfillment human. They establish no commission
eligibility or payout. Independent paid workers still need their own admitted
labor/market contract. Real introductions, disclosure, commercial qualification,
and external evidence cleanup remain owner steps in `NEEDS_OWNER.md`.

## Invoiced services

REV-18 extends the [private pipeline](#private-sales-pipeline) with bounded
service records. Only the current owner credential can apply
`record_service_sale`, `reconcile_service_payment`, or
`reconcile_service_fulfillment`, using the current lead revision and an explicit
private `--evidence-root`. The [shared types](../../crates/receipts/src/service_sale.rs)
pin the original account, offer, agreed USD cents, external invoice route and
reference, exact accepted REV-06/REV-07 sources, result, and support acknowledgment.
Frozen check commands use `{path, sha256}` references in the agreement's
`scope.frozen_check_refs`; their digests must match the protected REV-03 checks.
The [store](../../crates/coder/src/task/sales/service.rs) checks source bytes,
authorization, revisions, duplicate invoices/payments, and exact-byte retries
under the existing pipeline lock. Updating the current lead cannot relabel its
historical invoice or disclose it to a newly added reader. Original retention
still expires after a lead extension; deletion removes the service record too.

Record `pending`, `unknown`, `paid`, `reversed`, or `disputed` with bounded
evidence and the credential-derived verifier and date. Paid requires the exact
fully collected invoice and external payment reference. An unverified or partial
claim stays pending or unknown. Refunds retain their exact cumulative amount;
disputes imply no clawback. Optional fulfillment has its own accepted fixed
price, payment trigger, bill, and verified payment evidence. It creates no
referral split. Use `show --lead LEAD --sale SALE` or `export --lead LEAD
--sale SALE --output FILE` for authorized private service metadata. The export
feeds the [operating report](evidence.md#operating-revenue-and-full-delivery-cost).
These attributable owner records send no invoice or payment and change no
product balance, subscription, entitlement, quota, or plugin accrual. Real
payment truth, customer rights, and external evidence retention remain O1 work
in `NEEDS_OWNER.md`.

## Consented weekly review

REV-26 records explicit journeys in the same private pipeline. The owner uses
`record_funnel_journey`, `record_funnel_event`, and `record_conversion_failure`
through `apply --evidence-root DIR`, with the current lead revision. Enrollment
requires separate telemetry consent, an offer version, cohort, lane, and explicit
`fixture` or `owner_records` classification. The original account, recipients,
and retention remain pinned. `revoke_funnel_consent` removes the journey;
contact deletion, permission withdrawal, and original retention also remove it.
Bounded history reserves retry-safe withdrawal and contact cleanup capacity.
Private copies in the separate evidence directory still need owner cleanup.

Use `show --lead LEAD --journey JOURNEY` or `export --lead LEAD --journey JOURNEY
--output FILE` to read authorized history. Self-serve acquisition, install,
provider, accepted-task, and purchase events remain separate from assisted
pilot/customer decisions. Install and provider observations prove no commercial
activation. Purchase observations become qualified counts only when the reader
replays independently accepted task evidence, exact account attribution, and
the matching settled financial source. Current canonical service custody must
agree; a recorded refund cannot keep an old paid count. Retries and copied task
sources cannot become repeat use. Unknown attribution, payment, failed work,
refunds, and declined pilots retain a responsible human and dated next action.

`openagents sales weekly` rebuilds a private seven-day review through the current
owner credential. `openagents sales review` rechecks the sources and current
custody before writing separately consented, owner-reviewed, delayed counts.
The [weekly evidence guide](evidence.md#consented-weekly-operating-review)
defines its manifest and review records. It publishes nothing and grants no
outreach, tracking, handoff, task execution, or payment authority. Real buyer,
payment, provider, and publication qualification remains O1/O8 work in
`NEEDS_OWNER.md`.

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
measured System One work, and the open network of agents behind one conversation. Coder supplies
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
