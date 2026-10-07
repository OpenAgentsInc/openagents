# Sales and revenue

Status: proposal, October 6, 2026. This page collects what businesses tell us
they want from coding and work agents, how we plan to earn revenue from Coder
and the products around it, and what we still need to build. It's a public
page, so it records principles, product requirements, and a build order, not
pricing negotiations, targets, or account plans.

## Contents

- [Summary](#summary)
- [What businesses want](#what-businesses-want)
- [What we sell](#what-we-sell)
- [Principles](#principles)
- [How customers arrive](#how-customers-arrive)
- [Pricing and billing](#pricing-and-billing)
- [Affiliate and referral program](#affiliate-and-referral-program)
- [Partners and fulfillment](#partners-and-fulfillment)
- [Sell in public](#sell-in-public)
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

Coder is a coding agent that works with the agents and subscriptions a person
already has, such as Claude Code, Codex, Grok Build, and Devin, and routes work
across them. It puts cheap, fast typed decisions (Jev) in front of expensive
models, so the same work costs less. Everything else is a plugin that anyone
can publish, free or paid, with a revenue share on paid use.

The revenue plan follows product-led growth: individual developers adopt Coder
because it makes their existing usage go further, they bring it into their
teams, and teams become paying business accounts. We sell usage, not seats or
multi-year contracts. Two engines feed that growth: an affiliate program that
pays people for the usage they bring, and hands-on onboarding for businesses
that want help fitting agents into how they already work.

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
| The gateway | Anyone who wants to start without their own model keys | Usage-based model access with routing to the cheapest model that can do the job |
| Plugins | Plugin authors sell; users buy | Paid plugins carry a per-call fee; we take a share and the author keeps the rest |
| Business accounts | Teams and companies | Usage plus optional paid onboarding and support |
| Department agents | Businesses | Built during onboarding, run on usage, measured in the Gym |

## Principles

- **A strict upgrade first.** Whatever a team does today with a coding agent,
  Coder should do at least as well for less. Cost savings are the first thing a
  buyer can verify on their own work, so we lead with them.
- **Usage, not contracts.** We don't sell multi-year commitments or seat
  bundles. Customers connect a payment method, use what they need, and stop
  when they want. Our incentive is to make usage worth more, not to lock it in.
- **Bring your own subscription.** Coder uses the accounts customers already
  pay for. We earn on what we add: routing, cloud work, plugins, onboarding, and
  visibility.
- **Prove it on their work.** Every claim we make to a business should come
  with evidence from their own tasks: receipts, the Gym, and before-and-after
  costs.
- **Open by default.** The protocols are open (Nostr NIPs in
  [`nips/openagents/`](../../nips/openagents/)), so customers and partners can
  build on them, and identity and reputation travel with the agent, not with
  our database.

## How customers arrive

1. **A developer installs Coder.** No setup: it finds the agents and logins
   already on the machine and routes between them. The first win is that their
   existing usage goes further.
2. **They use more of it.** Cloud computers when their machine is busy, paid
   plugins, the gateway when a subscription runs out.
3. **They bring their team.** A champion shares sessions, plugins, and
   department agents with colleagues.
4. **The team becomes a business account.** Shared billing, an admin view, and
   policies.
5. **The business asks for help.** Hands-on onboarding fits Coder to their
   workflows and builds their first department agents.

The affiliate program accelerates every step: people who bring usage earn
from it.

## Pricing and billing

- **Free credits to start.** A new business account gets a starting credit
  and a short onboarding session, so it can see value on its own work before it
  pays anything.
- **Pay as you go.** Usage is metered per resource (compute, gateway calls,
  plugin calls) and charged against a prepaid balance or a card. The retail
  cloud already defines quotes, holds, metering, and settlement
  ([compute balance](../cloud/compute-balance.md),
  [price book](../cloud/retail-prices.md)).
- **One balance across products.** Cloud work, gateway calls, and paid plugins
  draw from one balance per account.
- **Payment rails.** Cards for businesses; Lightning for individuals, plugin
  authors, and machine-to-machine payments
  ([NIP-X402](../../nips/openagents/NIP-X402.md)).
- **Margins come from what we add.** Routing work to the cheapest capable
  model, running cloud work efficiently, and services stacked on one base. More
  usage is better for us and for the customer at the same time.

## Affiliate and referral program

Affiliate programs built large businesses on the early web, and the agent space
has no real equivalent yet. Ours pays people, and their agents, for the usage
they bring. Episode 239 set the rule: refer once, earn forever.

- **Referral links.** Anyone, person or agent, can create a referral link. New
  accounts that arrive through it are attributed to the referrer permanently.
- **Earn on usage.** The referrer earns a small share of everything their
  referrals buy anywhere in the OpenAgents ecosystem, for as long as those
  accounts stay active. Business accounts they bring in count the same way.
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
- **The coding agent pool.** Agents already volunteer to build against our
  backlog. We pay them for accepted work, with receipts, and point the pool at
  software that paying customers need, so custom work ships fast.
- **Both sides at once.** Sellers (compute, data, labor, verification) join
  through the Pylon network; buyers come through the sales funnel. Supply has
  always been the easy side; demand closes the revenue loop.

## Sell in public

We built in public; now we sell in public (episode 247). We share what works
and what doesn't: launches, revenue graphs, conversion, and lessons, in the
same channels where we show the product. It keeps us honest, it recruits
affiliates and partners, and it shows buyers real results instead of claims.

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
  The first deliverable is lower cost on the work they already do.
- **Build department agents.** Turn a department's documents and procedures
  into an agent the team directs, with a test set in the Gym so its
  performance is tracked over time.
- **Measure.** Before-and-after cost, time to completion, and service-level
  adherence on the customer's own tasks.
- **Make it repeatable.** Every engagement should produce reusable plugins,
  templates, and runbooks, so the next one is faster.

## Agents as products

Agents and plugins become things people can find, trust, and pay for.

- **Identity.** Every agent and plugin has its own key pair
  ([NIP-SOV](../../nips/openagents/NIP-SOV.md),
  [NIP-HOST](../../nips/openagents/NIP-HOST.md)).
- **Reputation.** Verified work earns experience
  ([NIP-XP](../../nips/openagents/NIP-XP.md)); reviews and track records attach
  to the agent's identity, not to our database.
- **Registries.** Curated plugin registries
  ([NIP-REG](../../nips/openagents/NIP-REG.md)) and a marketplace
  ([NIP-MKT](../../nips/openagents/NIP-MKT.md)) let buyers find agents and
  plugins by skill, price, and reputation.
- **Discovery partners.** Because identity and reputation live on Nostr,
  third-party search and web-of-trust indexes can list our agents and plugins
  without an integration deal. We should make that easy, and treat partners who
  bring buyers like affiliates. A first partner plugin is planned in
  [`docs/plugins/brainstorm-v1-integration.md`](../plugins/brainstorm-v1-integration.md).

## What exists today

| Need | What exists |
| --- | --- |
| The product | Coder V1 ([`crates/coder-new`](../../crates/coder-new)), with routing across installed agents and Jev in front of models |
| Accounts and workspaces | [`tenancy::accounts`](../../crates/tenancy/src/accounts.rs), [`tenancy::sessions`](../../crates/tenancy/src/sessions.rs) |
| Billing | [`tenancy::billing`](../../crates/tenancy/src/billing.rs): plans, checkout, invoices, provider events |
| Metering and quota | [`tenancy::quota`](../../crates/tenancy/src/quota.rs), the gateway's reservations and receipts |
| Cloud computers | The retail cloud contract, price book, balance, and launch gate ([`docs/cloud/`](../cloud/)) |
| Paid plugins | Signed plugin releases with per-call fees and author shares ([`docs/plugins/`](../plugins/README.md)) |
| Identity, reputation, registries | NIP-SOV, NIP-HOST, NIP-XP, NIP-REG, NIP-MKT |
| Evidence | Receipts ([`crates/receipts`](../../crates/receipts)) and the Gym ([`crates/gym`](../../crates/gym)) |
| Lightning | [`crates/wallet`](../../crates/wallet), [`crates/x402`](../../crates/x402) |
| Payment splits and payouts | [Payments](../payments/README.md): one receiver, a split ledger, payouts to plugin authors |

## What we need to build

1. **Cost proof inside Coder.** Show each person what their work would have
   cost on frontier models and what it cost through Coder, per task and per
   week. This is the first thing a champion shows their manager.
2. **One balance and a pricing page.** A single prepaid balance for cloud,
   gateway, and plugin usage; a public page that explains usage pricing in
   plain terms; a starting credit for new business accounts.
3. **Referral links and payouts.** Link creation for people and agents,
   permanent attribution on sign-up, revenue share on paid usage across every
   product, holds, payouts over Lightning or as credit, and an earnings view.
4. **Partner and pool payouts.** Commissions to partner organizations and
   payment for accepted coding-agent pool work, on the same ledger, with
   receipts.
5. **Team accounts in the product.** Inviting colleagues from Coder, shared
   plugins and agents, and shared billing.
6. **The admin view.** Agent work, cost, wait times, and outcomes by person and
   team, with budgets and alerts.
7. **Policies.** Data, model, plugin, and placement policies that admins set
   and Coder enforces.
8. **Department agents.** A template and workflow for turning a department's
   documents into an agent with a Gym test set and a performance history.
9. **Quarterly evidence reports.** Generated from receipts and the Gym: what
   agents handled, how reliably, and where help is needed.
10. **Onboarding kit.** Runbooks, plugin templates, and a checklist that make
   each hands-on engagement faster than the last.
11. **Marketplace listings.** Plugin and agent pages with reputation, reviews,
    and pricing, readable by third-party indexes.

## Roadmap

| Phase | Goal | Includes |
| --- | --- | --- |
| 1. Launch | Developers adopt Coder and see savings | Coder V1, cost proof, the pricing page, the gateway |
| 2. First revenue | Usage pays | One balance, cloud computers behind the launch gate, paid plugins live |
| 3. Growth loop | Usage brings usage | Referral links and payouts, plugin authors as affiliates, marketplace listings |
| 4. Teams | Champions bring teams | Team accounts, shared plugins and agents, shared billing |
| 5. Business | Companies adopt with confidence | The admin view, budgets, policies, evidence reports, hands-on onboarding |
| 6. Up market | Larger customers | Department agents at scale, single sign-on, audit export, customer-owned computers |

Phases 1 and 2 build on what exists today. Phase 3 is the first new system of
substance. Phases 4 and 5 can start in parallel once accounts and the balance
are shared across products.

## Measures

- Weekly active developers, and how many of them pay for anything.
- Savings per active developer, measured on their own work.
- Paid usage per account, and how it grows month over month.
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

Since then the product has narrowed to Coder and the services around it, so
this page applies those ideas to Coder first.

## Open questions

- How large should the starting credit be, and should it depend on a
  completed onboarding session?
- What referral share is sustainable for permanent attribution, and should
  business referrals earn differently from individual ones?
- Should onboarding be paid, free with a usage commitment, or free during
  launch?
- Which admin features does the first business customer need before they pay?
- Which department agents should we build first as reusable templates?
