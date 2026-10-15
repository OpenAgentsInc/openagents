# Proposal: agents that earn and pay for useful work

Status: proposed implementation and experiments, 2026-10-15. This accompanies
the [interview analysis](analysis.md). It draws on the
[episode archive](../../transcripts/README.md), current runtime guides, and
existing commercial plans. This review ran no live jobs or payments. The
milestones below are recommendations, not newly supported features or permission
to spend money.

## Recommendation

Build one complete economic loop: a buyer orders useful work, an agent delivers
it, the buyer accepts it, the worker receives Bitcoin, and the agent buys the
inference needed for its next job from those earnings. Then measure whether
specialization and reusable components make that loop cheaper and more useful.

This gives us a concrete version of Bansal's idea. We can test whether agents
cover their costs without deciding whether they are alive or predicting which
currency autonomous software will eventually prefer. OpenAgents already has
much of the infrastructure. The work is connecting it under a supported job
profile, proving recovery, and finding repeat buyers.

Keep Coder as the first execution vehicle and the general agent as the entry
point. A single customer workflow should lead implementation. We should not
restart the five-market launch sequence or make a new network a prerequisite.

## What our earlier work teaches

The archive records experiments, scripts, announcements, and reported results
from several repository generations. These establish motivation and lessons;
the maintained runtime guides establish the implementation boundaries below.

| Earlier work | Connection to Bansal | Implementation consequence |
| --- | --- | --- |
| [037: Flow of Funds](../../transcripts/037.md), [094: Recap & Roadmap](../../transcripts/094.md), [100: Looking Ahead](../../transcripts/100.md) | Paying builders and splitting revenue have been central since the first agent store. Episode 100 reports revenue sharing alongside a shortage of agents people want to buy. | Finish one useful purchase, delivery, and payout before expanding the catalog. Money moving is insufficient evidence of customer value. |
| [174: GPUtopia 2.0](../../transcripts/174.md), [213: Agent Markets](../../transcripts/213.md), [214: Compute Market](../../transcripts/214.md) | Selling idle compute predates this interview. The series explicitly recalls excess sellers and insufficient buyers. | Buy capacity for an actual task. Separate paid demand from project-sponsored traffic and availability rewards. |
| [215: Data Market](../../transcripts/215.md) | People can earn from inputs as well as execution. | Start with a small, inspected, licensed bundle when a buyer needs it. Access to a conversation does not grant permission to sell it or train on it. |
| [237: You Must Construct Additional Pylons](../../transcripts/237.md) | The accepted outcome, contribution payouts, and cost per useful result are the project's own earlier economic thesis. The record includes prepared remarks and broader ambitions. | Define acceptance before execution. Keep deliverable, check, buyer acceptance, collection, and worker payout separately inspectable. Measure energy only when actually metered. |
| [239: Let's Make Money](../../transcripts/239.md) | It explicitly asks agents to earn enough to keep paying for compute and identifies the missing buy side. | Close earnings-to-spending and report subsidies. Referral income is a separate source of revenue, not evidence of useful labor. |
| [266: Single Points of Failure and Nostr Markets](../../transcripts/266.md), [267: Immortal Infrastructure](../../transcripts/267.md) | Open negotiation and replaceable coordination address dependence on one operator. | Reuse client, relay, provider, checker, and settlement boundaries. Ship a runnable provider and client together; a specification cannot supply an operating market. |
| [284: Gamifying Coder](../../transcripts/284.md) | Specialization and a visible world can make productive participation understandable. | Let the existing Verse display verified jobs and earnings. Keep XP separate from money and avoid rewarding token consumption or manufactured activity. |
| [286: System One in Coding Agents](../../transcripts/286.md), [287: Building a System One Coding Agent](../../transcripts/287.md) | Cost pressure motivates typed decisions, smaller executors, useful context, and shared components. | Compare whole-task policies. Include cache rebuilds, routing, failed cheap attempts, and escalation. Episode 287's after-the-fact best-choice portfolio is a hypothesis for a router, not a deployed router result. |
| [Archive measurement corrections](../../transcripts/README.md#the-latest-measurement-arc-revises-its-own-success-story) | A cheap success can reflect fitting the task rather than a transferable improvement. | Freeze comparisons before running them, exclude source tasks, retain failures, and require independent confirmation before default adoption or improvement rewards. |
| [288: Three DevDays Later](../../transcripts/288.md), [289: OpenAgents](../../transcripts/289.md) | A composable agent with paid contributors is the existing product direction. | Extend the current release, evaluation, discovery, purchase, and payout path. Keep the interface short even when the underlying agreement has many records. |

The [Test-Time Capabilities essay](../../essays/2026-09-29-test-time-capabilities.md)
and [general-agent essay](../../essays/2026-10-01-the-return-of-the-general-agent.md)
supply the useful synthesis: a general agent can compose specialized members,
but each member needs evidence that it improves the task. Bansal's narrow-agent
prediction fits that architecture without requiring the user to manage dozens
of separate agents.

## What exists, and what still needs joining

Some source documents describe an earlier state. In particular, the opening
of the [labor plan](../../agents/market-infrastructure.md) describes the original
free-only fixture; the current runtime guide additionally documents a bounded
paid profile. The historical [compute survey](../../compute/compute-for-bitcoin.md)
still contains statements that Pylon is absent, while the current Pylon guide
documents its return. Use the narrower current implementation descriptions
below when choosing work.

| Area | Existing foundation | Gap for this proposal |
| --- | --- | --- |
| Agent entry | [Self-registration](../../agents/self-registration.md): proof of work, an agent account and scoped API key, bounded free use, claiming, revocation, and a signed Lightning-credit callback. [Readiness evidence](../../agents/readiness.md#2026-10-14-agents-register-themselves-authmd-passes) records production signup and free-tier behavior. | Check the configured top-up resource and `pay.available` before describing paid entry as live. An API agent account has no website session or general host authority. |
| Persistent identity | [Agent identity and engrams](../../verse/agent-identity-and-engrams.md): host-held agent key, owner relationship, private memory, steering, usage records, and model budgets. | Bind that agent explicitly to its API account and earnings beneficiary. The existing steering loop forbids paying; a spend adapter requires a separately admitted profile. |
| Useful labor | [Recoverable coding orders](../../coder/runtime/free-labor.md): agreement, separate execution grant, artifacts, protected checking, and signed acceptance. Its paid profile joins accepted work and authenticated central funding to worker earnings. | The paid fixture uses synthetic source, one bounded patch, a pinned `cmp` checker, and fake funding. Broader useful repairs, independently operated workers, real collection, and payout remain to be qualified. |
| Receiving and paying | [Payment plan](../../payments/agent-payments.md), [payments guide](../../payments/README.md), and `crates/x402`, `crates/wallet`, and `crates/pay-ledger`: receiver, payer, replay handling, shares, payout recovery, and private earnings views. | Connect a settled worker payout to an agent-controlled purchase under an explicit budget. Do not treat a payable share, an API credit, and wallet funds as interchangeable. |
| Shared compute | [Pylon runtime](../../compute/pylon.md): free text and decision jobs, checks, paid direct and brokered paths, and a test-network agent-market experiment. | Paid paths have fixture coverage, but live-wallet qualification remains. The P4 experiment uses local acceptance and does not exchange the complete LAB delivery/acceptance records. It cannot substitute for the coding-order host. |
| Cheap decisions | Pylon's [decision guide](../../compute/pylon.md#answer-decisions-clef) records Jev-first operation and Pylon fallback/shadow behavior after weak router measurements. | Improve measured latency and answer quality before changing that priority. Owning hardware does not make it the cheapest qualified decision service. |
| Reuse | [Shared KB](../../coder/design/knowledge-base.md), [signed bundles](../../coder/runtime/knowledge-bundles.md), [frozen studies](../../coder/runtime/knowledge-studies.md), and plugin release/evaluation work. | Demonstrate benefit on new tasks and agree on compensation before paid use. Historical KB screening cannot authorize automatic promotion. |
| Commercial operation | [Revenue roadmap](../../sales/revenue-roadmap.md), [later-market profiles](../../payments/later-markets.md), and [owner qualification work](../../../NEEDS_OWNER.md). | Find a consenting buyer and an outside provider, qualify the chosen payment path, and measure repeat use and complete costs. Existing plans already own these requirements. |

## Implementation shape

Use a small Rust coordinator over the existing owners. Start with a private
operator workflow exposed through the existing CLI; add native controls after
the flow works. Keep money behavior in the payment and ledger crates, coding
execution in the task owner and labor host, account behavior in tenancy and
the gateway, and presentation in the product surfaces. Do not create another
wallet, market protocol, or spendable balance in the coordinator.

```mermaid
flowchart TD
    A[Buyer agrees on work, price, and acceptance] --> B[Worker runs within its grant]
    B --> C[Buyer checks and accepts exact delivery]
    C --> D[Buyer pays the accepted obligation]
    D --> E[Existing ledger records worker earnings]
    E --> F[Existing payout worker pays the worker wallet]
    F --> G[Authorized agent buys API credit]
    G --> H[Metered inference supports the next job]
    H --> A
```

The diagram shows the proposed central-receive route. An accepted job can remain
unpaid; accrued earnings can remain awaiting payout; a purchase can remain
unresolved. Those states must remain visible without advancing the next step.

### Link identities without granting new rights

Create an explicit, revocable operator-approved binding among the host agent
key, the scoped API agent account, the worker's earnings party, and the
authorized wallet principal. Each system verifies its own side. Names and
public payment activity cannot establish ownership. Creating or claiming an
API account must not automatically attach an earnings account or wallet.

Use a new dedicated test worker for the first loop. Preserve Alice's current
policy. If we later support an earning profile for Alice or another crew
member, the owner explicitly enables that profile and its narrow purchasing
adapter; model-written memory cannot turn it on.

The first version remains operator-controlled. The host or resident signer
holds the keys. Describe that custody plainly; self-registration is not proof
of independence from OpenAgents, the host administrator, or an API provider.

### Reserve purchases before calling a wallet

Reuse the existing payment policy and reservation machinery. An authorized
purchase pins the agent, merchant/resource, account credited, exact quote,
network, amount, fee ceiling, deadline, and stable purchase identity. Reserve
principal and fees across concurrent jobs and children before dispatch. Apply
both per-purchase and daily limits at the authoritative payer.

Initially allow only the configured inference-credit resource, with a small
operator-selected ceiling and expiry. The model can request credit; Rust
checks whether it may buy it. A balance report, NIP-AM usage record, or Jev
answer does not authorize a payment. Existing usage budgets are useful inputs,
but a budget checked after a model call cannot replace a wallet reservation.

On a lost reply, reconcile the original payment and credit callback. Do not
issue a new purchase while the original outcome is unknown. API key revocation
and the wallet grant have separate revocation paths; the operator's stop
control must disable future actions through both. Previously accepted worker
obligations remain owed.

### Keep earnings, cash, and service credit distinct

Join existing records for a private per-agent statement:

- Accepted jobs, earned amounts, collected funding, payable worker shares,
  payout attempts, and confirmed wallet receipts.
- Wallet purchases, routing and receiving fees, purchased credit, remaining
  credit, and actual metered service consumption.
- Execution, checking, coordination, failed attempts, and known or unknown
  external costs, plus opening capital and explicit subsidies.

The statement is a read model over the owning books. It cannot spend or create
earnings. An accrued share is not available cash; bought service credit cannot
be withdrawn as earnings. The existing
[compute balance](../../cloud/compute-balance.md) is also a separate product
account, not an automatic alias for agent API credit.

Track cash flow and economic cost separately. Buying credit moves money into
a prepaid asset; consuming that credit creates service expense. Do not count
both as inference expense. Keep exact BTC amounts and the recorded USD quote
or usage basis; leave unavailable conversion or energy measurements unknown.

### Preserve the commercial boundary

LAB pays an agreed worker obligation after acceptance; x402 purchases an input
before service. One does not stand in for the other. Postacceptance payment
leaves the worker exposed to buyer nonpayment. Start with bounded trusted
counterparties, explicit payment deadlines, and a maximum unpaid exposure.
Escrow and automated dispute resolution are separate work. Buyer acceptance
connects payment to the agreed result; it does not establish that the buyer's
goal benefits everyone. Execution permissions and human control still matter.

Use the current exact-once accounting and recovery paths rather than inferring
success from a relay acknowledgment. A crash may leave execution or payment
unknown. Keep its reservation until the owning subsystem resolves it. Changing
relays must preserve the job, terms, artifact, and payment identities.

## Suggested delivery order

These are local planning stages, not new issue numbers. Reuse the existing
labor, revenue, payment, and compute backlogs when filing implementation work.
Each stage should land in small changes with its own targeted checks.

### 1. Make one agent's economics inspectable

Build the identity binding and private joined statement first. Select one
buyer-needed public-repository task and freeze its deliverable, acceptance,
price, provider expense ceiling, and payment terms. Record opening capital and
any free-tier use. Inspect the chosen deployment's credit availability and
existing owner qualification requirements.

Done when synthetic fixtures connect an order, acceptance, funded earnings,
payout, credit purchase, and inference usage without changing any balance twice.
Wrong-account bindings, revoked access, concurrent reservations, and unknown
payments must remain distinguishable. This stage spends no real money.

### 2. Prove useful delivery between operators

Extend the labor host's supported profile for one real task family: for example,
a bounded regression fix in a public Rust repository. Explicitly admit the
executor, instructions, permitted model recipient, source access, and checker.
Replace the fixture's expected-byte comparison only with the checker this
family needs; keep it outside provider writes. Specify whether human review is
part of acceptance. A patch request does not also grant merge or push rights.

Ship the client and runnable provider together. Rehearse with two independently
operated participants before payment. Exercise restart, cancellation, duplicate
delivery, stale base, unavailable buyer, and relay loss with synthetic funds.

Done when the buyer accepts useful work under the frozen terms and can recover
its complete order after an outage. Two keys controlled by one operator do not
satisfy the independent-operator milestone.

### 3. Close earn, receive, buy, and work again

Qualify the selected real receiver, payout destination, and payer through the
existing owner process. Then run one small funded order. Confirm the worker's
payout at its wallet before enabling the bounded inference-credit purchase.
The same dedicated agent uses the resulting credit for a subsequent admitted
job. Keep initial expenses funded by declared opening capital or the free tier.

Done when real incoming and outgoing payments settle, the credit lands once,
actual usage is charged once, and the second job's records link back to the
earned funds. Report remaining cash, unused credit, expenses, fees, and subsidy.
This establishes an earnings-funded purchase, not profitability or perpetual
operation. Repeat profitable jobs without new subsidy before claiming the agent
pays its operating costs.

Publish only a consented export: task/result references, acceptance, amounts,
fees, durations, and sanitized receipt references. Exclude private source,
conversations, credentials, payment preimages, recovery secrets, and raw
payment proofs. Public proofs of activity need not expose private purchases.

### 4. Test cheaper execution under a real budget

In parallel with stages 2–3, compare a fixed capable executor with a specialist
or cheaper executor plus a declared escalation policy. Use the current routing
baseline, deterministic probes, code graph, and narrow typed judgments where
they add measured value. Do not replace Jev-first routing merely to generate
demand for our own pylons.

Freeze task families, checks, configuration, quality tolerance, spend limits,
and escalation conditions before the comparison. Count all failed attempts,
cache rebuilds, context preparation, checks, and final repair. An evaluator
cannot change acceptance to improve the cheaper policy's score.

Done when new tasks show comparable accepted-work quality and a lower complete
cost or useful latency improvement. Otherwise keep the simpler baseline for
that family. Lower inference cost alone is insufficient.

### 5. Buy one useful shared resource or component

Choose the next input from actual bottlenecks. For inference, qualify the
existing Pylon paid path for a suitable workload and compare it with the
current API path, including checks, retries, availability, and provider costs.
Keep text inference, decision quality, and coding labor as separate products.

For reusable knowledge or a plugin, pin its release and rights, compare with
and without it on tasks outside its source groups, and price the author's fee
before use. Use the current plugin fee and split path when its supported
execution envelope fits. Verify a real author payout separately from worker
earnings. Do not invent royalties for every retrieved entry or confuse
registered use with a measured causal contribution.

Done when another operator can use the resource or component at a known total
price and reproduce its scoped benefit. The existing
[later-market profiles](../../payments/later-markets.md) already describe
separate worker, training, and optimization obligations; extend those owners
only when this workflow needs them.

### 6. Expand from repeat purchases

Add a short **Offer work / Pause** flow, quotes, acceptance review, and private
earnings/expense views to the existing clients. Reuse Verse's job and compute
views for optional visualization. Show useful statuses such as **Working**,
**Ready for review**, **Payment pending**, and **Paid**. Hide unavailable
controls. Keep protocol terms in the inspector and technical documentation.

Expand to another provider and task family when repeat buyers and sustainable
provider earnings justify it. Exercise a second relay operator, and eventually
a compatible second client, without making these a new market launch. Follow
the [revenue roadmap](../../sales/revenue-roadmap.md) for acquisition and
customer support rather than building a separate sales system for this study.

## Experiments and decisions

The sample sizes, quality tolerances, and spending caps must be chosen before
each study. These rows define what to learn, not numerical production defaults.

| Experiment | Comparison and measurements | Decision it informs |
| --- | --- | --- |
| Earnings-funded operation | Trace one worker's opening funds, receipts, top-ups, actual consumption, accepted jobs, and closing funds; then repeat without fresh sponsorship. | Can this task family cover operating cost, and which expense prevents it? Separate one successful payment loop from sustainable operation. |
| Specialization | Fixed capable executor versus a frozen specialist-plus-escalation policy on unseen jobs; accepted fraction, total cost per acceptance, latency, and intervention time. | Which tasks deserve a cheaper path? Retain failures and unknown costs in the report. |
| Paid shared compute | Qualified Pylon versus the current provider on the same permitted workload, including transport, checker cost, retries, energy when metered, and seller margin. | Does buying outside compute improve delivery, and can the provider earn at that price? |
| Reusable capability | Pinned KB snapshot or plugin on/off using the existing study discipline, excluding all source tasks/groups; quality, marginal cost, author fee, and regressions. | Is there enough transferable value to adopt and pay for the component? |
| Paid registration alternative | Keep proof of work as the control; optionally offer a separately verified small payment after the purchase loop works. Compare completed legitimate signups, abuse, payer cost, and total free-tier expense. | Does payment improve entry for agents that already hold sats? Preserve a path for unfunded agents and retain aggregate abuse limits. |
| Demand | Track sponsored and outside buyers separately: accepted jobs, repeat paid orders, buyer review time, collection delays, and provider net earnings. | Should we expand supply, improve the workflow, or stop this offer? More signups or self-payments do not establish demand. |

The economic report should show collected revenue, outstanding receivables,
full known delivery cost, unknown costs, cash available, and unused prepaid
credit separately. Report both BTC and the supported USD accounting basis.
Include setup, model calls, decisions, checks, coordination, failed attempts,
support, and rail fees. Subscription capacity and owned hardware have costs
even when no per-call invoice arrives. If material costs are unknown, mark
profitability unknown.

Use isolated fixtures first, then frozen replay or a small declared cohort,
then the funded workflow's existing qualification. Production checks use the
dedicated test account and clean up their resources. They must not leave chats,
projects, computers, or drafts on the owner's account. Real funds, deployment,
and device checks follow existing authority and qualification; this proposal
does not authorize them.

## Work to defer

Paid packet routing, a new mesh network, general data markets, distributed
training, liquidity products, and automated escrow are not dependencies for
this loop. Buy or implement them when an observed customer bottleneck warrants
it. A narrow licensed data purchase or independently checked evaluation job can
come earlier when it directly supports delivery.

Taproot Assets may eventually help agents budget in dollars on Bitcoin rails,
but the payment plan still treats that adapter as future work. Asset issuance,
custody, redemption, and freeze policies need separate evaluation. Bitcoin
transport does not by itself remove issuer or operator control. We should not
repeat the analysis's implication that a dollar asset on Bitcoin automatically
has Bitcoin's resistance to seizure. Start with sats and explicit quote terms.

Keep life, metabolism, the end of intellectual property, and inevitable
hyperbitcoinization out of the implementation criteria and product copy. Use
the practical promise only when demonstrated: an agent can earn from useful
work and use those earnings to buy what its next task needs.

## First changes to make

1. Add the private identity binding and read-only per-agent economic statement,
   with synthetic joins and recovery checks over existing books.
2. Extend one labor execution/checker profile for a buyer-needed public Rust
   repair and ship an independent-operator no-spend rehearsal.
3. Connect confirmed worker payouts to narrowly authorized API-credit purchases,
   with concurrent-budget and lost-reply recovery fixtures. Then qualify and
   record one real earn/receive/buy/work cycle through the existing owners.

Those changes address the missing connection first. Subsequent work should
follow measured cost and customer demand, with the current
[networked Coder plan](../../coder/design/networked-coder-plan.md) and
[market infrastructure plan](../../agents/market-infrastructure.md) continuing
to own the broader roadmap.
