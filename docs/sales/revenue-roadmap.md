# Unified revenue roadmap

Status: proposal, October 6, 2026. This is the delivery plan for the
[sales strategy](README.md). It joins acquisition, customer delivery, payments,
and product integration in one order. Existing product backlogs retain their
technical contracts and issue ownership. Milestones below are proposed exit
criteria, not claims of completed launches or commitments to dates.

The review uses episodes 275–289, the earlier revenue episodes, the October 6
operator conversation, maintained documentation, and targeted source reads.
No live service, customer, or funded payment was tested for this review.

## Strategic decision

Use Coder to win the first recurring workflow. Earn from a useful paid addition
or a bounded onboarding engagement, then make that relationship repeatable.
OpenAgents supplies the common capabilities, identities, evidence, and payment
path as the customer expands. The first sale does not require a complete
general-agent marketplace, every client surface, or every market profile.

Run two demand paths together:

| Path | First value | First revenue | Expansion |
| --- | --- | --- | --- |
| Developer | Install, use an existing login, finish a checked task | A useful paid plugin, admitted cloud task, or funded gateway call | Repeat use, an internal champion, team billing and controls |
| Assisted business | Select one recurring workflow with its owner and prove an improvement | An agreed onboarding service and/or settled paid usage | A reusable department capability, more workflows, team adoption |

The shared sequence is **prove value → purchase → checked delivery → repeat
use → referrals and expansion**. Engineering must support purchase and settlement;
sales must supply a buyer, a problem, and acceptance criteria. A release,
funded wallet, or growing plugin catalog alone does not complete that sequence.

## What the background changes

The [archive guide](../transcripts/README.md) distinguishes historical intent
from current implementation. Carry these lessons into the commercial plan:

| Source | Lesson | Change to the plan |
| --- | --- | --- |
| [239: Let's Make Money](../transcripts/239.md), [247: Sell in Public](../transcripts/247.md) | Buyer demand closes the loop; affiliates, fulfillment partners, and contributors connect to actual purchases | Build the lead-to-paid-use funnel alongside the product; qualify commissions and delivery before broad recruitment |
| [275: Coder](../transcripts/275.md), [277: Coder Terminal](../transcripts/277.md) | A dependable daily tool gives the suite an entry point; free inference depended on subsidies | Measure first useful work and retention; cap trial expense and price against unsubsidized delivery cost |
| [276: Coder Cloud](../transcripts/276.md), [278: Coder Commands Codex & Claude](../transcripts/278.md), [279: Raiding Claude Code](../transcripts/279.md) | Placement and provider choice are useful only when readiness, execution, and results are honest | Sell a supported resource with its actual payer, limits, recovery, and receipt; old prices are not current offers |
| [280: CoderOS](../transcripts/280.md), [281: Coder Mobile](../transcripts/281.md) | One workflow spans computers and devices; local capacity and continuity cause real friction | Use a busy local machine as a cloud trigger; connect clients to the same task and account rather than rebuilding execution |
| [282: Not Asking Permission](../transcripts/282.md), [283: Coder for Gamers?](../transcripts/283.md), [284: Gamifying Coder](../transcripts/284.md) | Broad access and a productive world are distribution ideas; rewards should follow useful work | Test audiences through accepted workflows; keep XP separate from money and defer progression as a revenue dependency |
| [285: Bendcoder](../transcripts/285.md), [286: System One in Coding Agents](../transcripts/286.md), [287: Building a System One Coding Agent](../transcripts/287.md) | Useful context and lean execution can reduce waste, but false completion, failed trials, and hypothetical best-choice routing remain | Include repairs, checks, setup, and failed attempts; prove actual routing on new tasks before advertising a saving |
| [288: Three DevDays Later](../transcripts/288.md), [289: OpenAgents](../transcripts/289.md) | The larger product is a composable agent whose contributors can earn | Connect capability creation, evaluation, exact release, discovery, purchase, and author payout; Coder is the first workflow inside it |
| October 6 operator conversation | Businesses need visibility, coherent processes, and help adopting tools; a prospective sales collaborator and discovery partner expressed interest | Start assisted pilots now; define the sales role and partner experiment without treating interest as an agreement |

The archive repeatedly describes market infrastructure ahead of paying demand.
Expand supply from demonstrated customer needs. Reusable components should
improve unfamiliar work, not only the tasks that produced them.

## First offers and their limits

Recommend a useful paid plugin as the first small product purchase, because the
pay front and signed-release path already exist. Select it from a recurring
customer task; an inexpensive call with no useful result is not a viable offer.
Qualify retail cloud in parallel for customers whose local machines are busy.
If a buyer's immediate need favors cloud or a gateway call, that lane can lead
after its own acceptance passes. Do not wait for every lane to launch.

| Offer | Buyer reason | Required before selling |
| --- | --- | --- |
| Coder with existing accounts | More accepted work from tools already paid for | Supported install, honest provider readiness, checked result, and measured comparison; local adoption is not OpenAgents revenue |
| Paid workflow plugin | Saves repeated manual work or adds a missing capability | Exact signed release, visible total price, payment approval, successful invocation, receipt, and author payout evidence |
| Retail cloud repository task | Continue work while the local machine is occupied | Mounted customer service, purchased balance, admitted offer, funded qualification, production launch, and client controls |
| Funded gateway use | Obtain a supported capability without bringing that provider's key | Real funding, configured price and capacity, customer account binding, metering, and settlement; decision calls and hosted agent/model execution are separate resources |
| Bounded onboarding | Fit one workflow to existing tools and leave something that runs | Agreed scope, service price or explicitly capped free trial, billing route, customer acceptance, support boundary, and reusable deliverable |

Keep the first cloud offer inside the [retail v1 contract](../cloud/retail-contract.md):
one Boat computer class, public repository source, the customer's own OpenAI
key, a checked repository change, and retained delivery. That contract does not
sell hosted inference, private-repository access, an interactive rented shell,
or GCE retail. Operator Boat/GCE placement and sponsored inference are separate
capabilities.

For onboarding, start with either a recurring coding workflow or a bounded
document-to-action workflow with a human review step. Examples include meeting
actions drafted into tickets or a department procedure turned into a tested
plugin. These are pilot candidates; the customer chooses the needed tool
integration and success criteria. A department agent initially means documents,
rules, admitted operations, and evaluations, not a promise to train a new model.

## Missing pieces and existing owners

This is the gap register. Named owners are responsibility areas, not staffing
assignments. A person takes each responsibility before its milestone starts.

| ID | Foundation | Missing revenue dependency | Owner and exit evidence |
| --- | --- | --- | --- |
| G1 | [Coder live paths](../../crates/coder-new/README.md) and the [workbench plan](../terminal/workbench-roadmap.md) | One supported customer install and first-task path; paid account, offer, purchase, result, and receipt controls in that client | Client/release: a new external user completes the supported flow; demo fixtures do not count |
| G2 | Traces, [Gym](../gym.md), and [retained benchmarks](../terminal-bench/README.md) | Customer baseline, full cost attribution, independent acceptance, and a shareable result | Product/evidence: compare accepted tasks with failures and repair included; label actual spend, estimates, and subscription capacity separately |
| G3 | [Central receiver, splits, and payouts](../payments/README.md) | Current funded plugin evidence, author publishing in the installed release, customer paid invocation, and support handling | Payments/client: one payment, result, settlement, author accrual and payout; retries do not duplicate charges or payouts |
| G4 | [Compute balance](../cloud/compute-balance.md), [retail lifecycle](../cloud/retail-service.md), and [qualification tooling](../cloud/retail-qualification.md) | A durable production customer service/transport and purchase controls; contract confirmation, real funded qualification, deployment | Cloud/client: fund, quote, approve, run, check, cancel/recover, settle, retain, and tear down; gate remains closed until the supported configuration qualifies |
| G5 | `pay-ledger` compute and paid-call records; gateway `tenancy::money` | Canonical customer/workspace mapping and adapters for one spendable balance across products | Accounts/payments: one funding source, no concurrent overspend, and a statement reconciling available funds, holds, charges, and liabilities |
| G6 | [Decision gateway](../decision-models/service/gateway.md), accounts, monetary admission, quotas, and receipts | Supported door/backend and capacity, real funding, usage-first catalog, production monetary configuration, customer connection and metered settlement | Gateway/payments/client: a funded account obtains the admitted decision result with matching charge and receipt; failures, duplicate attempts, and unknown costs reconcile |
| G7 | [Sales strategy](README.md) and existing product demos | Offer page, qualified lead intake, pilot kit, sales handoffs, follow-up, and a private account pipeline | Sales/onboarding: each lead has a stage, responsible person, next action, and dated customer decision |
| G8 | Versioned split and payout infrastructure | Referral identity, permanent attribution, eligible-revenue rules, holds/reversals, earnings view, and commissions | Growth/payments: an attributed settled purchase earns once; a reversal adjusts it; statement and payout match |
| G9 | [Workspace roles and invitations](../../crates/tenancy/src/accounts.rs), gateway usage reads | Team identity in Coder, shared billing and capabilities, budgets, data/model/plugin policies, practical outcome reports | Accounts/client: a champion invites a colleague; both charge the correct account under enforced limits and revocation |
| G10 | [Brainstorm proposal](../plugins/brainstorm-v1-integration.md), signed releases, and evaluations | Qualified discovery, useful listings, partner fulfillment terms, reusable onboarding, and admitted contribution payments | Ecosystem/onboarding: discovery produces an activated buyer; reuse passes on new work; delivery and payout follow explicit acceptance |
| G11 | [Sandbox billing](../decision-models/service/billing.md), signed events, and recovery | Live card provider, prepaid/usage funding adapter, denomination/conversion rules, and deployment configuration | Payments: real processor confirmation credits once; duplicate, delayed, refund, and dispute events reconcile; browser return alone credits nothing |

The October 3 [plugin receipt](../payments/2026-10-03-end-to-end-demo.md)
proves release lookup, pricing, the payment challenge, and a ceiling refusal.
It explicitly leaves real settlement and payout unverified. Obtain a newer
funded receipt or perform the remaining qualification before claiming that
path. Its first-payment pointer no longer has a matching section in
`NEEDS_OWNER.md`; reconcile that tracking before scheduling the owner action.

Retail acceptance and live adapters have fake/simulated evidence. The
[owner runbook](../cloud/retail-qualification.md#owner-runbook) and
[operations](../cloud/retail-operations.md) define the remaining funded and
launch checks. The current compute projection reads account state; it does
not provide spending controls. Library implementation and advertisement tools
do not establish a running customer service.

## Delivery order

Keep one milestone list for sales and engineering. Work in the next milestone
can start early when its dependencies are satisfied; a milestone label does
not force unrelated issue work to wait.

| Milestone | Revenue outcome | Deliverables and dependencies | Exit evidence |
| --- | --- | --- | --- |
| R0. Prove the offer | A buyer understands and accepts a useful workflow | G1 first-task path, G2, relevant G7: supported install, one offer/price page, baseline and result report, support contact; pilot kit for the assisted lane | An external developer completes an accepted task, or a business owner agrees to a bounded pilot and review date; each lane can proceed independently |
| R1. Collect first revenue | First settled payment for delivered product use or a service, with evidence distinguished | R0 for the chosen lane; G3 or G4 or G6 for one paid product, including G1 purchase controls, account binding, quote, payment, receipt and recovery; or an agreed separately invoiced onboarding service | A customer pays and receives the agreed result. A product sale retains funded settlement and required author payout evidence; a service sale records its paid invoice and customer acceptance, without qualifying product payments |
| R2. Earn repeat use | Customers return and delivery supports sustainable pricing | R1 on the chosen lane; statements, usage visibility, reconciliation, margin reporting, reusable onboarding. G5/G11 before offering cross-product balances or card funding | Same customer buys again; accounting reconciles; full delivery costs support the offer. One proven lane can repeat before other payment rails and products launch |
| R3. Expand demand | Referrers and partners bring qualified paying users | R1/R2 evidence; G8 and relevant G10: attribution, agreed commissions, abuse controls, payout qualification, partner handoff and discovery pilot | Attributed buyer activates and pays; commission settles once and can be reversed; partner-delivered work has a customer acceptance and support owner |
| R4. Convert teams | A champion expands proven workflows inside a business | R2; G9 plus reusable G10 onboarding: invitations, shared billing, permitted capabilities, budgets, policies and outcome reports | Multiple users repeat the workflow on the correct account; the admin sees cost and results; limits and revocation are enforced |
| R5. Extend the ecosystem | New workflows, authors, and larger accounts compound value | R3/R4; proven capability creation/evaluation/publication, broader listings, department templates, requested enterprise controls | An external author earns from useful reuse; a new workflow passes its tests; larger-customer features satisfy a specific paying requirement |

Start live-card and common-account implementation alongside R1 where possible.
A small Lightning purchase can qualify before cross-product funding is complete.
Conversely, a card-paying pilot cannot be called self-serve until real card
funding works. A separately invoiced onboarding service must record its own
payment and delivery; it must not mint product credit through sandbox events.

Capture consented acquisition source and referrer information during R0.
Publish attribution and commission terms before promising permanent earnings;
retain early agreed attribution when automated links arrive. Sales preparation
and narrow partner conversations need not wait for R3, while commission
payouts depend on its contract and accounting.

Client breadth follows demonstrated demand. Terminal, standalone, desktop,
phone, web, and Verse use the same domain owners and account records. Follow
the [workbench issue directory](../terminal/issue-roadmap.md),
[execution router](../api/2026-10-02-agentic-execution-router.md), and cloud and
payment contracts for implementation. A new pane, world decoration, leaderboard,
or broader market does not gate R1.

## Sales and pilot operations

Prepare an offer page with the supported workflow, result example, prices or
quote basis, account payer, trial cap, limitations, and a clear install or
pilot-request action. Use launch demos, founder introductions, developer
communities, and partners to test demand. Buy broader traffic after activation,
purchase conversion, and contribution margin justify the expense.

For self-serve developers, instrument install → first accepted task → settled
purchase → repeat use, with source/referrer and next support action. They do
not need a negotiated pilot. Use the fuller private pipeline below for direct
and referred accounts that need assisted adoption:

| Stage | Required record | Next owner |
| --- | --- | --- |
| Lead | Source/referrer, contact permission, problem, next action | Sales |
| Qualified | Recurring workflow, workflow owner, decision maker, current tools, baseline cost/time, data boundary | Sales and customer owner |
| Pilot agreed | Deliverable, duration, credit/spend cap, service price, acceptance checks, review date, support boundary | Onboarding |
| Activated | First accepted task, account/payer, supported purchase path, observed setup friction | Product and onboarding |
| Pilot reviewed | Before/after evidence, failures/rework, accept/extend/stop decision | Customer owner and sales |
| Paying and retained | Settled usage or paid service, repeat activity, actual margin, next workflow | Account owner |
| Expanded or referred | Additional users/workflows or a new attributed buyer | Account owner and growth |

The first pilot kit needs:

1. One workflow and accountable customer owner, with inputs and an explicit
   quality or service-level target. Choose a small repeatable task.
2. A comparable baseline, a frozen review method, supported integrations, and
   agreed data recipients. Enforce spend and disclosure limits from the first
   pilot even before an admin UI exists.
3. A capped trial or priced service with an agreed payment route. Starting
   credit is a marketing cost; it is neither earned revenue nor referral income.
4. Retained results, independent checks, cost and elapsed time, repair work,
   setup burden, and customer feedback. Keep unknowns visible.
5. A dated decision to pay, expand, extend within a new cap, or stop. Leave a
   reusable plugin/template and runbook, plus a clear support handoff.

Propose a bounded sales-and-pilot-operations role for the prospective sales
collaborator: qualify leads, identify workflow owners, coordinate onboarding,
collect result evidence, and follow up conversion. Agree availability,
deliverables, authority, and compensation privately. Referral commission and
onboarding/fulfillment work are separate obligations; interest in the call does
not establish either agreement. Engineering owns technical delivery, while the
customer owns workflow acceptance.

## Economics, referrals, and partners

For each offer, retain three different records: customer payment and earned
usage; third-party liabilities and payouts; and full delivery cost. Unused
top-ups remain purchased balances, not earned usage. Track sponsored and
paid use separately.

Evaluate contribution from collected usage or service charges after author
and resource shares, model and compute expense, payment fees, affiliate
commission, promotional credit/bonuses, and onboarding/support labor. Include
failed provisioning and replacement computers even when the customer is not
charged for them. Compare with costs after provider grants expire. Set trial
caps and sustainable commission rates from this record before expanding spend.

Under the current [plugin split contract](../payments/2026-10-02-central-receive-and-splits.md#3-who-gets-paid),
the author receives the whole declared fee and OpenAgents earns the separate
endpoint charge. Fund referral incentives from the agreed OpenAgents economics;
do not silently reduce an author's signed fee. A later commercial rule needs
an explicit version and acceptance.

Before recruiting affiliates broadly, resolve these rules:

- A referrer can be a person, agent, author, or partner. Bind its stable identity
  to an attributed customer and define how personal-to-team conversion works.
- Preserve permanent attribution under published terms. Resolve competing
  introductions, existing customers, and changes in workspace ownership.
- Define eligible settled paid usage per product, including treatment of
  pass-through author fees and separately invoiced services. Exclude self-referral,
  promotional credits, recycled credits, and unused purchased balances.
- Version the share, currency/unit, payout destination, hold period, minimum,
  refund/dispute reversal, and statement. Reconciliation must agree with the
  payment ledger; claims of referral income are not payment evidence.

Separate discovery partners from fulfillment partners. Discovery brings buyers;
fulfillment delivers an agreed part of their workflow. A fulfillment agreement
names the deliverable, price, responsible party, acceptance, support handoff,
and payment trigger. Paid coding-pool work needs its own admitted labor and
dispute contract; do not imply free-host support already provides it.

Use [Brainstorm](../plugins/brainstorm-v1-integration.md) as a bounded discovery
experiment. V1 proposes two public reads and a separately approved public
profile pilot. Measure exact-key discovery → referral → install → accepted task
→ paid use. Social reputation is distinct from evaluation evidence. Broader
capability indexing, executable assistant identities, and a full marketplace
are later qualified integrations.

## Evidence and operating review

Review the funnel and delivery together each week. Name a responsible person,
next action, and blocker for every active milestone. Customer records and
commercial terms stay private; publish approved aggregate results and product
lessons through the existing sell-in-public channels.

| Measure | Decision it supports |
| --- | --- |
| Qualified leads by source; install-to-first-accepted-task conversion and time | Which offers and channels deserve more effort |
| Pilot accept/extend/stop outcomes; time to first settled purchase | Which onboarding and product gaps stop revenue |
| Repeat paid use and retained accounts by cohort | Whether the workflow creates continuing value |
| Quality, actual spend, API-equivalent estimates, subscription capacity, setup/queue/completion time | What benefit can be claimed and which routing/placement helps |
| Earned usage/service charges, purchased balances, liabilities, delivery cost, incentives, contribution | Whether growth is sustainable after subsidies |
| Referrals that activate/pay, reconciled commissions, partner acceptance, author earnings | Whether demand and contribution reinforce each other |

Receipts prove recorded execution or payment; independent checks and customer
acceptance establish different aspects of quality. Publish comparative claims
only with the version, task set, baseline, failures, complete cost, and limits.
An after-the-fact best-choice portfolio is not a deployed router. A fixed
subscription with more capacity is not necessarily a smaller monthly bill.

Code-complete issues close under the repository's own acceptance rules.
Owner-only real-money runs, keys, and commercial confirmations stay in
[`NEEDS_OWNER.md`](../../NEEDS_OWNER.md). Their absence can keep a paid offer
unavailable without holding completed implementation issues open. Record funded
qualification and deployed configuration separately from issue status.

## Next actions and unresolved decisions

Start with these proposed assignments, without creating a competing issue
backlog:

1. Product and sales choose the first recurring workflow, supported launch
   client, paid addition, and customer review criteria. Prepare the offer page
   and private pilot record.
2. Client and payments turn G1/G3 into the smallest installed-customer purchase
   slice. Reconcile the funded-plugin evidence and owner-action record.
3. Cloud closes the G4 service and client integration gaps against the existing
   backlog while the owner completes the retained contract and qualification
   runbook. Keep the retail gate authoritative.
4. Accounts and payments specify G5/G11 account mapping and live-card funding,
   using existing ledgers and retry/recovery contracts. Start implementation
   beside the first Lightning slice.
5. Sales scopes the first assisted pilots, agrees the collaborator's role, and
   measures acceptance and conversion. Build the reusable onboarding kit from
   these engagements.
6. Growth prepares attribution and partner terms; launch paid commissions after
   their economics and settlement qualify. Test Brainstorm as one discovery
   source, then use customer demand to choose the next capability.

Decisions still open: the first paid workflow, credit and onboarding caps,
service pricing, live card provider and unit conversion, commission base/share,
the first team's required controls, and support responsibility. Record each
decision with its owner and affected milestone. Do not wait for a complete
enterprise package to resolve the offer, payment path, and first pilot.
