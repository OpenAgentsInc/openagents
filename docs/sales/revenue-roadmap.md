# Unified revenue roadmap

Status: delivery plan and filed build issue inventory, October 7, 2026.
This is the single commercial delivery roadmap for the
[sales strategy](README.md). It joins acquisition, customer delivery, payments,
and product integration in one order. Existing product backlogs retain their
technical contracts and issue ownership. The [build issue list](#build-issue-list)
joins the product gaps and agent-sales-floor work in this document.
Milestones below are proposed exit criteria, not claims of completed launches
or commitments to dates.

The review uses episodes 275–289, the earlier revenue episodes, the October 6
operator conversation, maintained documentation, targeted source reads, and
the October 7 [agent identity epic #10807](https://github.com/OpenAgentsInc/openagents/issues/10807)
and its [specification](../verse/agent-identity-and-engrams.md).
No live service, customer, or funded payment was tested for this review.

## Contents

- [Strategic decision](#strategic-decision)
- [What the background changes](#what-the-background-changes)
- [First offers and their limits](#first-offers-and-their-limits)
- [Missing pieces and existing owners](#missing-pieces-and-existing-owners)
- [Delivery order](#delivery-order)
- [Build issue list](#build-issue-list)
- [Owner qualification and activation gates](#owner-qualification-and-activation-gates)
- [Sales and pilot operations](#sales-and-pilot-operations)
- [Economics, referrals, and partners](#economics-referrals-and-partners)
- [Evidence and operating review](#evidence-and-operating-review)
- [Next actions and unresolved decisions](#next-actions-and-unresolved-decisions)

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

The frozen [first workflow offer v1](README.md#first-workflow-offer-v1)
selects a bounded assisted Coder public-repository change as the first
service lane: proposed USD 250 on accepted delivery, zero promotional credits,
and buyer-paid local model use. O1 and the buyer's private agreement activate
its commercial defaults; REV-02/REV-03/REV-04/REV-06/REV-07/REV-18 supply the
selected service path. This specification is not an accepted customer order.

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

The current [paid-plugin service](../payments/2026-10-02-central-receive-and-splits.md#6-paid-endpoints-on-the-central-receiver)
admits one guest step requiring no capabilities; snapshot reads receive an
empty snapshot. The first paid plugin must fit that envelope. A richer
department or connector workflow needs a separately admitted executor and
payment contract, or can start as a scoped onboarding service.

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
path. The [owner gate list](#owner-qualification-and-activation-gates) keeps
this action visible; reconcile its `NEEDS_OWNER.md` entry before scheduling it.

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

## Build issue list

This is the filed build inventory for G1–G11, R0–R5, and the agent sales
floor. Each of the 76 stable IDs, `REV-01` through `REV-76`, links to its
detailed GitHub issue with scope, owning code/contracts, acceptance, focused
verification, and activation limits. The dedicated
[Revenue and Sales project](https://github.com/orgs/OpenAgentsInc/projects/21)
contains all 76 issues plus 19 shared foundations and their native dependency
closure. Its views cover [first revenue](https://github.com/orgs/OpenAgentsInc/projects/21/views/2),
[ready work](https://github.com/orgs/OpenAgentsInc/projects/21/views/3),
[blockers](https://github.com/orgs/OpenAgentsInc/projects/21/views/4),
the sales floor, conditional work, and unresolved scope decisions.
All issues also remain on the required
[OpenAgents project](https://github.com/orgs/OpenAgentsInc/projects/19);
client/world follow-ups also appear on the existing
[terminal and workbench project](https://github.com/orgs/OpenAgentsInc/projects/20).
Responsibility areas don't assign a person or claim work. The repository's
issue claims and native blockers remain authoritative.
Keep the dedicated project's status, readiness, and dependency mirrors current
using the [project update instructions](../project-board.md#revenue-project-updates).

Native dependencies represent mandatory completion blockers. Conditional
features and alternative product lanes stay explicit in the issue body;
add their blockers when the implementation scope enables them. The four
conditional extensions remain deferred proposals even though their issues
are filed. Their evidence and activation conditions remain explicit; filing
doesn't establish a launch or completed qualification.

At filing, 74 issues remain open and two Agora build items (REV-67/REV-68)
are complete in [c07c359771](https://github.com/OpenAgentsInc/openagents/commit/c07c359771).
Reuse their admitted building, layout, stations, and bell hook. REV-69–REV-71
retain world-tree/private-board, agent-activity, and earned-sale integration.

The acceptance column describes the scoped outcome; completed rows say so.
It doesn't imply every underlying primitive is missing. Reuse completed owners and APIs.
Don't recreate an account service, money ledger, gateway dashboard, task
runtime, approval engine, or CRM to satisfy a row. In particular, the private
pipeline in REV-04 becomes the agent pipeline in REV-53; it is one record set.
Exercise code acceptance with isolated adapters/fixtures and the issue's
targeted checks. Real funds, external-user evidence, owner credentials, and
commercial activation belong to O1–O8 below; they don't hold code-complete
implementation issues open.

### Existing work to reuse

GitHub states below were checked on October 7. Closure establishes the scoped
issue result; it doesn't establish funded qualification, physical-device
acceptance, or a deployed customer offer.

| Foundation | Existing issues and contracts | Remaining boundary |
| --- | --- | --- |
| Client packaging and shared panes | Closed [#10644](https://github.com/OpenAgentsInc/openagents/issues/10644), [#10683](https://github.com/OpenAgentsInc/openagents/issues/10683), [#10685](https://github.com/OpenAgentsInc/openagents/issues/10685), [#10686](https://github.com/OpenAgentsInc/openagents/issues/10686); [terminal issue directory](../terminal/issue-roadmap.md) | Choose and qualify one installed commercial flow; reuse other clients when demand needs them |
| Component creation, checks, and reuse | Closed [#10665](https://github.com/OpenAgentsInc/openagents/issues/10665), [#10672](https://github.com/OpenAgentsInc/openagents/issues/10672); [plugin contracts](../plugins/README.md) | One useful, supported, priced release and installed customer purchase path |
| Plugin receiver, splits, and payouts | Closed [#10200](https://github.com/OpenAgentsInc/openagents/issues/10200), covering #10185–#10199; [payment contract](../payments/README.md) | Customer integration, authenticated earnings access, and actual funded delivery/payout evidence |
| Retail lifecycle and adapters | Closed #10704–#10719, #10722–#10724 and [#10748](https://github.com/OpenAgentsInc/openagents/issues/10748); [retail service](../cloud/retail-service.md), [qualification](../cloud/retail-qualification.md) | Production customer transport/worker and client spending controls; [#10719](https://github.com/OpenAgentsInc/openagents/issues/10719) is a read projection |
| Gateway, account, money, and usage APIs | Closed [#9468](https://github.com/OpenAgentsInc/openagents/issues/9468), [#9490](https://github.com/OpenAgentsInc/openagents/issues/9490), [#9491](https://github.com/OpenAgentsInc/openagents/issues/9491), [#9492](https://github.com/OpenAgentsInc/openagents/issues/9492), [#9493](https://github.com/OpenAgentsInc/openagents/issues/9493); [gateway](../decision-models/service/gateway.md) | Supported funded door, live card provider, client connection, and cross-product financial mapping |
| Agent records and memory | Existing keyed Alice wrapper, owner decisions, journals, finite jobs, stop/pause/retire; closed [#10785](https://github.com/OpenAgentsInc/openagents/issues/10785), [#10787](https://github.com/OpenAgentsInc/openagents/issues/10787); [crew machinery](../verse/crew.md#shared-machinery) | Her own identity, engrams, and steering loop belong to open #10807; sales adds narrowing roles, records, grants, training, and coordinated operation |
| World prerequisites | Closed [#10786](https://github.com/OpenAgentsInc/openagents/issues/10786), [#10789](https://github.com/OpenAgentsInc/openagents/issues/10789); open [#10788](https://github.com/OpenAgentsInc/openagents/issues/10788), [#10790](https://github.com/OpenAgentsInc/openagents/issues/10790), [#10791](https://github.com/OpenAgentsInc/openagents/issues/10791); [generative agents](../verse/generative-agents.md#phases-and-dependencies) | Reuse the landed town clock and checked-citation reflection; finish the existing world-tree, day-plan, and roster issues without duplicate foundations |
| Artifact pack delivery | Closed [#10763](https://github.com/OpenAgentsInc/openagents/issues/10763), landed while this inventory was being prepared; [artifact queue](../coder/runtime/artifact-queue.md) | Submit Agora pack changes through the existing serialized queue; no new merge-queue implementation or direct pack repin push |
| Later paid-market profiles | Closed [#10725](https://github.com/OpenAgentsInc/openagents/issues/10725)–[#10729](https://github.com/OpenAgentsInc/openagents/issues/10729); [later-market contracts](../payments/later-markets.md) | Customer/provider integration and funded availability; reviewed profiles and fake tooling aren't a live paid labor pool |

### Shared agent foundation: #10807

The [identity and engrams specification](../verse/agent-identity-and-engrams.md)
owns the common runtime. Phases 1 (#10798) and 2 (#10799) landed before this
filing update was pushed; the remaining phases retain their existing issue ownership. Reuse
them rather than filing REV duplicates for agent keys, memory storage,
steering, sync, or lifecycle. Each member has its own key, definition, engrams,
policy, and conversations. The implementation is shared; identities and
private memory are separate.

| Existing phase issue | Native dependency | Sales integration boundary |
| --- | --- | --- |
| 1. Primitives [#10798](https://github.com/OpenAgentsInc/openagents/issues/10798) (completed) | None | Shared attestation and NIP-AE codec; no sales-specific event kind |
| 2. Local engrams [#10799](https://github.com/OpenAgentsInc/openagents/issues/10799) (completed) | #10798 | Shared write-through beneath the existing memory stream and scored recall; sales reads assigned host records by reference |
| 3. Alice steers Coder [#10800](https://github.com/OpenAgentsInc/openagents/issues/10800) | #10799 | First owner-visible target: plan, plain Coder prompts, judge, bounded corrections, verify, and report; separate owner/agent and agent/Coder conversations |
| 4. Identity [#10801](https://github.com/OpenAgentsInc/openagents/issues/10801) | #10798 | Key custody, private definition, SOV authority/controller/custodian roles, signed profile, and expiry warning; sales job roles and charters remain REV-51 |
| 5. Relay sync [#10802](https://github.com/OpenAgentsInc/openagents/issues/10802) | #10799, #10801 | Optional owner-device continuity through NIP-AA/NIP-AE, off per agent until enabled; not a Paul or revenue launch gate |
| 6. Consolidation [#10803](https://github.com/OpenAgentsInc/openagents/issues/10803) | #10799, reflection #10789 | Owner-reviewed core proposals and memory reachability; no new sales memory system or authority from reflection |
| 7. Lifecycle [#10804](https://github.com/OpenAgentsInc/openagents/issues/10804) | #10801, #10802 | Shared rotation, retained owner-readable history, retirement, migration, and export; REV-64 adds floor caps and lead reassignment |
| 8. Spend records [#10805](https://github.com/OpenAgentsInc/openagents/issues/10805) | #10800, #10802 | Both the steering agent's and Coder's calls; REV-58 adds aggregate floor reservations, real-day limits, and all helper work |
| 9. Crew [#10806](https://github.com/OpenAgentsInc/openagents/issues/10806) | #10800, #10801 | Name-generic loop/definition/policy and Bob as the second member; REV-54 supplies Paul, REV-70 supplies Bob's sales placement work |

Continue with phase 3 on the completed phases 1/2; phase 4 can run in parallel.
Phase 9 then makes that runtime reusable for Paul. Prepare sales records,
playbook adapters, synthetic training, and Agora record integration in parallel. The
remaining epic phases keep their native blockers; sync, consolidation,
migration, and relay spend publication don't all have to land before local
Paul preparation. Local spend admission and sales privacy do.

The epic estimates about 33 agent-hours of shared work. The floor's roughly
66 agent-hours are incremental sales scope, not another copy of those phases
or a combined calendar promise. Human pilots and every smallest R1 path
remain independent of #10807.

Apply these boundaries when integrating the phases:

- Agent keys identify actors. Customer/workspace/payer mapping in REV-09 and
  REV-19, referral attribution in REV-27/REV-28, and qualified payout
  destinations remain separate commercial records. Rotation never transfers
  host grants, certification, payment rights, or earnings eligibility silently.
  Preserve historical attribution under a stable commercial referrer ID and
  reviewed lineage; changing an agent key doesn't mint a new eligible referral.
- Engrams hold screened memory and checked insights. The private host
  pipeline, permission/suppression lists, owner approvals, certification, and
  money ledger remain authoritative. Use opaque record references and
  non-identifying summaries in persistent memory; keep lead identity and
  message content in the assigned host records. A tombstone isn't proof of
  deletion from a relay, cache, trace, or plaintext export.
- Paul's loop may answer only its own permitted routine tool approvals.
  Its charter narrows the shared defaults to sales operations; it cannot
  approve another member's step. Sending, hiring, pricing, payment, and
  publication use the owner gate and host adapters outside the steering
  loop. NIP-OA/NIP-AA attest identity and relay access, not permission to act.
- REV-58 reserves the steering, Coder, Jev, helper, reflection, and retry
  costs together before dispatch. Reuse #10805's call records when available;
  until then require a bounded local cost source and retain unknown holds.
  NIP-AM cost estimates are advisory; actual bills stay separate. Its
  telemetry neither admits spending nor establishes revenue or settlement.

### R0: supported offer, evidence, and pilot operations

Owners: product, client/release, sales, and onboarding. These rows cover G1,
G2, and the manual portion of G7. Founder-led pilots can proceed while the
agent floor is being built.

| ID | Issue scope and owner | Depends on | Acceptance |
| --- | --- | --- | --- |
| [REV-01](https://github.com/OpenAgentsInc/openagents/issues/10808) | Product/sales: freeze the first workflow offer and commercial defaults | Customer discovery; existing supported contracts | [Coder pilot v1](README.md#first-workflow-offer-v1) defines the service workflow, proposed price/caps, acceptance, data and support policies; O1 and buyer agreement remain activation gates |
| [REV-02](https://github.com/OpenAgentsInc/openagents/issues/10809) | Client/release: qualify the selected customer install and first-task path | REV-01; existing install/provider paths | Fresh external-user install finds a usable provider, completes a checked task, and exposes honest unavailable states and recovery/support instructions |
| [REV-03](https://github.com/OpenAgentsInc/openagents/issues/10810) | Evidence: join baseline, accepted results, timing, and full task cost | REV-01; existing traces and Gym | [Private evidence adapter](evidence.md) joins frozen inventories and retained ATIF/Gym sources, failed/repair attempts, independent/customer acceptance, separate time and cost bases, and exact-byte reviewed public projection |
| [REV-04](https://github.com/OpenAgentsInc/openagents/issues/10811) | Sales: implement the private lead/account pipeline and accepted human handoff | REV-01 | One durable record set holds permission/source/jurisdiction, stage, responsible human, next action/date, workflow, baseline, data boundary, and customer decision; access and retention are enforced |
| [REV-05](https://github.com/OpenAgentsInc/openagents/issues/10812) | Sales/product: publish the offer page and connect permissioned intake | REV-01, REV-04 | Supported result example and install/pilot action reach the correct private pipeline; source/referrer and contact permission survive intake without public customer data |
| [REV-06](https://github.com/OpenAgentsInc/openagents/issues/10813) | Onboarding: deliver the bounded pilot kit and review workflow | REV-01, REV-03, REV-04 | Scope, customer owner, inputs, duration, cap/payment, protected comparison, data recipients, review date, and accept/extend/stop decision are recorded privately |
| [REV-07](https://github.com/OpenAgentsInc/openagents/issues/10814) | Onboarding/support: package setup, delivery, offboarding, and support handoff | REV-06 | [Manual handoff kit](delivery-kit.json) pins the exact result/runbook, customer and support-owner acceptance, cleanup states/evidence, retained artifacts, and separately reviewed reuse rights |
| [REV-08](https://github.com/OpenAgentsInc/openagents/issues/10815) | Evidence/sales: build the versioned claims and price register | REV-01; REV-03 for comparative claims | [Canonical claims register](evidence.md#reviewed-claims-and-prices) rereads current sources, authoritative price terms, playbooks, and full REV-03 evidence; immutable reviewed clauses/drafts retain rejection and withdrawal reasons, and proposed commercial terms remain unavailable without separate O1 activation |

### R1: installed customer purchase and delivery

Owners: client, payments, cloud, gateway, and onboarding. These rows cover
G1/G3/G4/G6 and the separately invoiced service lane. Choose one lane for
first revenue; the others proceed independently.

| ID | Issue scope and owner | Depends on | Acceptance |
| --- | --- | --- | --- |
| [REV-09](https://github.com/OpenAgentsInc/openagents/issues/10816) | Client/accounts: bind the launch client to its customer, workspace, and payer | REV-02; existing account/session and product-account contracts | Sign-in/recovery and account selection preserve identity; every quote/approval names the correct payer and rights; wallet possession never substitutes for spending authority |
| [REV-10](https://github.com/OpenAgentsInc/openagents/issues/10817) | Plugins: deliver and publish the first useful paid workflow release | REV-01, REV-08; existing publishing/evaluation | Installed publisher signs the exact tested release with fee/destination metadata; the packet fits the current one-guest/no-capability envelope and produces an accepted useful result |
| [REV-11](https://github.com/OpenAgentsInc/openagents/issues/10818) | Client/payments: connect the paid-plugin purchase flow | REV-09, REV-10 | Exact release and total endpoint-plus-author price precede approval and invocation; unsupported packets refuse before payment; customer receives result and receipt; changed terms need new consent |
| [REV-12](https://github.com/OpenAgentsInc/openagents/issues/10819) | Payments/client: expose durable paid-plugin failure and recovery states | REV-11; existing replay and settlement owners | Lost acknowledgment and restart cause no blind repayment or repeated execution; payment, settlement, result, failed delivery, and unknown liability remain separate and supportable |
| [REV-13](https://github.com/OpenAgentsInc/openagents/issues/10820) | Cloud: mount a durable customer retail transport and service worker | REV-01; existing retail lifecycle/adapters | Authenticated customers are isolated; provisioning, dispatch, metering, cancellation, cleanup, and settlement survive client loss/restart without duplicate resources or discarded unknown costs |
| [REV-14](https://github.com/OpenAgentsInc/openagents/issues/10821) | Client/cloud: add retail funding, quote, execution, and cancel controls | REV-09, REV-13 | The actual client completes fake-funded top-up → quote → disclosure/approval → run → checked result → cancel/reconnect → receipt; read access grants no spend/control |
| [REV-15](https://github.com/OpenAgentsInc/openagents/issues/10822) | Cloud/operations: package the production service and paid-availability gate | REV-13; existing operations/qualification tooling | Exact deployed revision/configuration, secret custody, health, recovery, rollback, capacity, and cleanup are observable; paid availability remains closed without matching funded qualification |
| [REV-16](https://github.com/OpenAgentsInc/openagents/issues/10823) | Gateway: configure the first supported funded decision offer | REV-01; existing gateway/backend/money owners | Bound backend identity, capacity, versioned rates, and resource counters produce honest results and charges; unavailable/unknown usage refuses or retains liability; decision access isn't advertised as hosted generation |
| [REV-17](https://github.com/OpenAgentsInc/openagents/issues/10824) | Client/gateway: connect funding, credentials, admitted decision calls, and receipts | REV-09, REV-16 | Each dispatched attempt settles verified observed usage once against the correct account under its pinned price; duplicate request/attempt pairs never redispatch; unknown attempts retain holds and reconcile before a new attempt |
| [REV-18](https://github.com/OpenAgentsInc/openagents/issues/10825) | Onboarding/accounts: retain invoiced service payment and accepted delivery | REV-06, REV-07 | Agreed invoice, payment reference, deliverable, customer acceptance, and support owner reconcile; service revenue never creates product credit through sandbox billing |

### R2: shared funding, repeat use, and sustainable economics

Owners: accounts, payments, product analytics, and support. These rows cover
G5/G11 and repeatable operation. A proven single-product Lightning lane can
repeat before the common balance or live cards are available.

| ID | Issue scope and owner | Depends on | Acceptance |
| --- | --- | --- | --- |
| [REV-19](https://github.com/OpenAgentsInc/openagents/issues/10826) | Accounts: implement canonical cross-product customer/workspace identity mapping | REV-09; existing tenancy and product accounts | Host/device/Nostr identities map to one commercial account; recovery, rotation, team conversion, and revocation preserve attribution without widening rights |
| [REV-20](https://github.com/OpenAgentsInc/openagents/issues/10828) | Payments: connect product ledgers to one authoritative spend/funding path | REV-19, REV-21; existing reservation/ledger contracts | Plugin, compute, and gateway adapters reserve/settle against the admitted account; concurrent purchases cannot overspend; credits, retries, refunds, and unknown holds reconcile once |
| [REV-21](https://github.com/OpenAgentsInc/openagents/issues/10827) | Payments/product: implement versioned denomination, conversion, and trial-credit rules | REV-01; agreed commercial units | Funding and quotes pin units, conversion, rounding, fees, finality, and reversal terms; purchased balance, promotional credit, wallet sats, and XP stay distinct |
| [REV-22](https://github.com/OpenAgentsInc/openagents/issues/10829) | Payments: implement a live card processor and durable verified event adapter | REV-19, REV-21; provider/terms decision | Confirmed processor events credit prepaid usage once; browser return credits nothing; duplicates, delay, refund, dispute, and already-spent credit follow explicit reconciliation/loss rules |
| [REV-23](https://github.com/OpenAgentsInc/openagents/issues/10830) | Client/accounts: expose live checkout, balance funding, and billing recovery | REV-09, REV-20, REV-22 | Customer sees pinned funding terms and actual confirmation/state; interrupted checkout recovers correctly; a cash/card event doesn't imply Lightning wallet liquidity |
| [REV-24](https://github.com/OpenAgentsInc/openagents/issues/10831) | Accounts/payments: join cross-product statements and authenticated exports | REV-20; existing gateway usage/dashboard reads | Funding, available balance, holds, earned charges, author/resource liabilities, releases, reversals, and payout references reconcile; member/payee reads reveal only permitted records |
| [REV-25](https://github.com/OpenAgentsInc/openagents/issues/10832) | Product/operations: join revenue, delivery cost, margins, and support cases | REV-03; REV-12 or REV-13 or REV-17 or REV-18; REV-24 for shared statements | Earned OpenAgents revenue, gross collections, unused funding, third-party shares, model/compute/payment expense, promotions, commissions, and support labor remain separate; unknown expense isn't zero |
| [REV-26](https://github.com/OpenAgentsInc/openagents/issues/10833) | Product/sales: instrument activation, repeat purchase, and weekly operating review | REV-04; one qualified R1 lane; REV-25 | Consented source → install → accepted task → settled purchase → repeat is recorded separately from assisted-pilot stages; failed conversion has an owner/next action; approved aggregates can be published |

### R3: referrals, partners, and discovery

Owners: growth, payments, sales, and plugins. These rows cover G8/G10. Warm
partner conversations and discovery preparation can start early; commission
promises and payouts require their actual contract and qualified accounting.

| ID | Issue scope and owner | Depends on | Acceptance |
| --- | --- | --- | --- |
| [REV-27](https://github.com/OpenAgentsInc/openagents/issues/10834) | Growth/accounts: add stable referrer identity and shareable referral links | REV-09 and acquisition-source capture; REV-04 for assisted-pipeline integration; REV-19 for cross-product attribution | Person, agent, author, or partner identity binds to a consented acquisition source; own sales-agent links identify the source without earning circular commission |
| [REV-28](https://github.com/OpenAgentsInc/openagents/issues/10835) | Growth/accounts: persist permanent attribution and migration rules | REV-27; agreed attribution policy | Competing introductions, existing accounts, early agreed referrals, personal-to-team conversion, and workspace ownership changes resolve without silently rewriting attribution |
| [REV-29](https://github.com/OpenAgentsInc/openagents/issues/10836) | Growth/payments: version and publish the commission contract | REV-25, REV-28; commercial share decision | Eligible revenue/products/services, base/share/unit, holds, minimum, destination, reversals, and permanence are explicit; author signed fees remain intact and unused/free credit earns nothing |
| [REV-30](https://github.com/OpenAgentsInc/openagents/issues/10837) | Payments: implement commission accrual, reversal, and reconciliation | REV-29; selected product's settlement adapter | Attributed eligible settled usage creates one liability; retry doesn't multiply it; refund/dispute adjusts it; self-funded/recycled or promotional amounts don't earn commission |
| [REV-31](https://github.com/OpenAgentsInc/openagents/issues/10838) | Accounts/payments: add creator/referrer earnings access and qualified payout management | Existing author-payee records; REV-30 for commissions | Authorized payees see exact fees, attributed eligible use, holds, destination, accrual, and sent/failed/unknown payout state; statement and actual rail evidence match |
| [REV-32](https://github.com/OpenAgentsInc/openagents/issues/10839) | Growth/security: implement referral abuse checks and scratch qualification | REV-28, REV-30 | Self-referral, fake identities, recycled credits, and destination abuse trigger bounded holds/review; model or reputation signals cannot grant payout authority |
| [REV-33](https://github.com/OpenAgentsInc/openagents/issues/10840) | Sales/onboarding: integrate discovery and fulfillment partner assignments | REV-04; REV-06/REV-18 for charged fulfillment; REV-29 when commissions apply | Partner accepts the relevant private introduction or deliverable, price, owner, acceptance, support handoff, and payment trigger; discovery and paid fulfillment remain separate responsibilities |
| [REV-34](https://github.com/OpenAgentsInc/openagents/issues/10841) | Plugins: implement the bounded Brainstorm discovery/search/rank client | [V1 adapter contract](../plugins/brainstorm-v1-integration.md#adapter-contract) | Rust client normalizes public observations with limits, cancellation, provenance, relevance/rank distinction, unknown coverage, and equivalent failure fixtures |
| [REV-35](https://github.com/OpenAgentsInc/openagents/issues/10842) | Client/plugins: add disabled Brainstorm settings and explicit commands | REV-34 | Enabling/installing makes no lookup; explicit commands work without a model/provider; settings and bounded observations preserve source/limits through the supported live conversation |
| [REV-36](https://github.com/OpenAgentsInc/openagents/issues/10843) | Client/plugins: bind Brainstorm to admitted provider dispatch | REV-34, REV-35 | Shared client/configuration supports OpenRouter and explicit local commands; model-proposed queries require exact outbound text/recipient admission through a host-held reference; disabling blocks dispatch and external content grants no authority |
| [REV-37](https://github.com/OpenAgentsInc/openagents/issues/10844) | Growth/plugins: distribute guidance and add exact-key discoverability qualification | REV-35, REV-36; REV-05/REV-26 for commercial funnel integration | Guidance pack/install checks pass and metadata accurately states native-host requirements; bounded exact-key discovery/provenance and conversion capture work; owner publication and real buyer conversion remain O7 operating evidence |

### R4: team adoption and proven client expansion

Owners: accounts, client, policy/admission, and onboarding. These rows cover
G9 and shared G10 workflows. First assisted pilots use bounded existing
grants before a complete admin UI is available.

| ID | Issue scope and owner | Depends on | Acceptance |
| --- | --- | --- | --- |
| [REV-38](https://github.com/OpenAgentsInc/openagents/issues/10845) | Client/accounts: integrate invitations, membership, and workspace switching | REV-09; existing tenancy roles/invitations; REV-19 for cross-product identity | Champion invites a colleague; both use the correct account; current membership/revocation and recovery are checked without quota or billing rebinding |
| [REV-39](https://github.com/OpenAgentsInc/openagents/issues/10846) | Plugins/accounts: share admitted team capabilities and exact releases | REV-38; existing create/evaluate/publish owners | Members discover and enable only granted versions; narrowing, revocation, publisher identity, and source/data rights survive reuse |
| [REV-40](https://github.com/OpenAgentsInc/openagents/issues/10847) | Accounts/policy: enforce per-workspace, team, and person budgets and alerts | REV-38; selected-product reservation owner; REV-20 for cross-product budgets | Concurrent reservations and unknown liabilities count against limits on every enabled route; unavailable routes stay disabled; alerts explain an enforced bound rather than replace it |
| [REV-41](https://github.com/OpenAgentsInc/openagents/issues/10848) | Policy/host: enforce data, model, plugin, and placement rules across clients | REV-38; existing admission/disclosure contracts; REV-39 for shared capabilities | Enabled local/cloud/customer-host routes obey exact allowed recipients/capabilities; policy change or revocation blocks new effects without relabeling work in flight; unqualified routes remain disabled |
| [REV-42](https://github.com/OpenAgentsInc/openagents/issues/10849) | Accounts/product: project team work, cost, waiting time, outcomes, and reports | REV-03, REV-38; selected-lane statements; REV-24 for cross-product reports | Admin/member views attribute real tasks, failures, accepted results, models/plugins, costs, and service-level waits to the correct account under current read rights |
| [REV-43](https://github.com/OpenAgentsInc/openagents/issues/10850) | Accounts/security: qualify joined access, recovery, and scoped audit export | REV-38–REV-42 for the enabled scope | Invite/recovery/revocation, narrower grants, concurrent limits, and export redaction pass acceptance for supported clients/routes; unavailable paths stay disabled and read-only clients cannot approve or spend |
| [REV-44](https://github.com/OpenAgentsInc/openagents/issues/10851) | Client/release: carry the proven commercial flow to requested phone/browser/desktop clients | Qualified selected-client R1 path; existing thin-client transports/panes | Same payer, quote, disclosure, exact approval, cancellation, result, and receipt survive cross-client reconnect; qualify only the demanded surfaces |

### R5: reusable workflows and broader paid supply

Owners: ecosystem, onboarding, payments, and enterprise accounts. Choose these
from paying demand; they don't gate the first offer, sale, or repeat purchase.

| ID | Issue scope and owner | Depends on | Acceptance |
| --- | --- | --- | --- |
| [REV-45](https://github.com/OpenAgentsInc/openagents/issues/10852) | Onboarding/plugins: package reusable department workflow templates | REV-07; protected tests and data rights; REV-39/REV-41 for team sharing/policies | Authorized documents/rules/operations pass on new customer work; release/runbook explains limits; reuse avoids customer-data leakage and can start before team distribution |
| [REV-46](https://github.com/OpenAgentsInc/openagents/issues/10853) | Ecosystem: integrate curated publisher/service listings and qualified discovery | REV-10; existing publication/registry/discovery contracts; REV-37 only for Brainstorm integration | Exact identity/release, supported capability, current price, evaluation evidence, provenance, and withdrawal are inspectable; listings qualify independently of a third-party index and reputation grants no execution rights |
| [REV-47](https://github.com/OpenAgentsInc/openagents/issues/10854) | Plugins/payments: extend the paid executor beyond the one-guest envelope | Demonstrated customer need; REV-11, REV-12, relevant policy | Separately versioned richer packet/operation contract admits bounded data/effects before payment; partial failure, delivery, retries, and fees reconcile without broadening old grants |
| [REV-48](https://github.com/OpenAgentsInc/openagents/issues/10855) | Labor/onboarding: integrate a customer-facing paid fulfillment path | REV-33; existing worker/bid/dispute profiles and qualification tooling | Independently admitted provider, price, funds, acceptance/rework/dispute, support owner, actual payment, and delivery evidence align; free-host support isn't sold as qualified paid labor |
| [REV-49](https://github.com/OpenAgentsInc/openagents/issues/10856) | Ecosystem/payments: connect accepted training, evaluation, and contributor work to payment | REV-25; existing later-market/contribution profiles | Protected checks establish attributable accepted work; separately authorized payment/payout reconciles once; participation, XP, and self-reported success never mint money |
| [REV-50](https://github.com/OpenAgentsInc/openagents/issues/10857) | Enterprise/accounts: add customer-required SSO and audit integration | A paying requirement; REV-38, REV-41, REV-43 | One agreed identity/audit integration preserves owner/admin/member rights, revocation, data boundaries, and export retention; verify the customer's required flow |

### Parallel sales-floor build

Owners: crew/host, sales, policy/security, Gym, Verse/art, and payments. These
rows are the implementation inventory for the [sales-floor design](agent-sales-floor.md),
whose S0–S6 phases describe scope and rough effort. This roadmap owns ordering.
R0/R1 can proceed through humans while any of these rows is incomplete.

REV-54 includes the sales steering adapter; REV-53/REV-60 include its memory
and privacy integration; REV-64 includes lifecycle reconciliation when those
operations are enabled. These are incremental integrations with #10807,
not new copies of its phase issues. Current host records are checked at each
admission; memory and model-written preferences never grant authority.

| ID | Issue scope and owner | Depends on | Acceptance |
| --- | --- | --- | --- |
| [REV-51](https://github.com/OpenAgentsInc/openagents/issues/10858) | Crew/host: add sales job roles, narrowing charters, and typed evidence verdicts | Identity definition/roles #10801; #10806 for generic creation | Reuse the shared record/key schema; sales job roles remain distinct from SOV authority/controller/custodian roles; old records load without replacing keys; charters only narrow and verdicts retain author/evidence as data |
| [REV-52](https://github.com/OpenAgentsInc/openagents/issues/10859) | Crew/host: implement crew-wide owner stop/pause and dispatch revocation | REV-51 | Owner stops selected members and pending dispatch together; restart never silently resumes stopped work; existing individual stop/retire semantics remain authoritative |
| [REV-53](https://github.com/OpenAgentsInc/openagents/issues/10860) | Host/sales: extend the private pipeline with policy, draft, assignment, and certification records | REV-04, REV-51 | One durable private record set survives restart; agents read assigned leads only; jurisdiction/permission/version/expiry and grant scope are explicit; memory uses opaque references/non-identifying summaries, never an automatic copy of CRM contents |
| [REV-54](https://github.com/OpenAgentsInc/openagents/issues/10865) | Sales/host: instantiate Paul on the shared steering loop and expose pipeline/lead/draft/suppression/certification controls | REV-53, REV-58; generic crew #10806; REV-55/REV-57 for real-prospect drafts | Paul has his own definition/key/engrams and owner conversation, steers plain Coder in a separate session, and plans from admitted queues; checks/report cite host evidence; interested replies are data, never owner requests; recommendations grant no effects and empty work stays idle |
| [REV-55](https://github.com/OpenAgentsInc/openagents/issues/10862) | Sales/evidence: connect the reviewed playbook, claims, price, and answer helpers | REV-08, REV-53, REV-58; existing evidence/document readers | Minimum host adapters feed exact reviewed evidence, price, and answer references into versioned drafts; proposed Peggy/Victor/Judy/Olivia/Ivan roles may wrap them later; full named crew is optional |
| [REV-56](https://github.com/OpenAgentsInc/openagents/issues/10863) | Gym/sales: add synthetic Carole personas and written role-play harness | REV-55 | Objections, misleading answers, uncertainty, and opt-outs are covered; fixtures describe no real buyers and practice never enters real-contact queues |
| [REV-57](https://github.com/OpenAgentsInc/openagents/issues/10864) | Gym/sales: implement claims/compliance/tone suites and certification | REV-55, REV-56; existing Gym | Calibrate on owner-labeled development data and evaluate frozen checks on locked data; serious failures block certification; required practice/samples, playbook changes, complaints, and two failed weekly checks enforce certification or suspension |
| [REV-58](https://github.com/OpenAgentsInc/openagents/issues/10861) | Host/payments: enforce sales model reservations and wall-clock policy | REV-53; reuse spend records #10805 when available | One floor ledger reserves agent planner/reporter, Coder, Jev, helpers, training, embeddings/reflection, day plans, verification/corrections, and retries within $5; unknown cost holds capacity; local bounded admission works before relay telemetry; real `America/Chicago` limits persist across restart/rotation/migration |
| [REV-59](https://github.com/OpenAgentsInc/openagents/issues/10866) | Sales/host: add meeting-slot proposals and accepted qualified human assignments | REV-04, REV-54, REV-55 | Owner-published slots and full private brief reach an accepting human; agents don't read calendars, accept prices/terms, or inherit mailbox/payment access |
| [REV-60](https://github.com/OpenAgentsInc/openagents/issues/10867) | Host/privacy: implement suppression, contact admission, retention, and deletion | REV-53; sales jurisdiction/channel policy | Opt-outs suppress across hires/channels; ambiguity blocks contact; inactive leads expire while minimal suppression survives; scratch memory/prompt/trace/cache/sync/export checks enforce approved recipients and retention; relay tombstones never count as verified erasure |
| [REV-61](https://github.com/OpenAgentsInc/openagents/issues/10868) | Host/security: add one email adapter with broker-owned credentials and compliance checks | REV-58, REV-60; owner domain/mailbox setup | Minimum host credential/authority/compliance/privacy adapters implement the proposed Faythe/Walter/Grace/Eve responsibilities outside Coder; agent context receives no credentials; identity, footer, unsubscribe, recipient admission, and delivery authentication are verified |
| [REV-62](https://github.com/OpenAgentsInc/openagents/issues/10869) | Host/client: implement durable level-0 approval and outbox dispatch | REV-52, REV-57, REV-58, REV-60, REV-61 | Host-owned dispatch outside the steering loop binds exact recipient/content/attachments/versions; owner approval sends once; routine tool policy never authorizes it; edits/revocation block, unknown delivery reconciles, and complaints/false claims/suppression/authentication failures pause/reset until corrected owner restart |
| [REV-63](https://github.com/OpenAgentsInc/openagents/issues/10870) | Host/sales: ingest replies and schedule bounded follow-ups | REV-60, REV-62; REV-59 for booking/handoff branches | Opt-out precedes model classification; Mallory scratch-inbox injection tests pass before real sending and after handler changes; replies/links/attachments grant no effects; hard bounces suppress/pause the channel; two follow-ups use real weeks/current permission |
| [REV-64](https://github.com/OpenAgentsInc/openagents/issues/10871) | Crew/host: add confirmed hiring, retirement, and lead reassignment | REV-51–REV-54, REV-57, REV-58; generic creation #10806 | Start Paul alone; queues justify Erin → Frank → Pat; exact proposals create/attest once within Paul-plus-three cap and budget; hires train before real drafting; retire stops/reassigns without losing owner-readable history or suppression; reuse #10804 for rotation/migration, with explicit grants and reviewed certification bindings |
| [REV-65](https://github.com/OpenAgentsInc/openagents/issues/10872) | Sales/operations: project the private floor report and Wendy escalation | REV-54, REV-58; REV-62/REV-63 for outbound metrics; REV-25/REV-26 for financial enrichment | Pipeline and immediate complaint/pause escalation work before revenue; real counts and known/unknown costs remain distinct; Paul drafts a weekly aggregate update for owner review/publication |
| [REV-66](https://github.com/OpenAgentsInc/openagents/issues/10873) | Host/client: add qualified reviewed-batch grants | Measured REV-62/REV-63 operation; REV-57 | Frozen five-item batches bind exact recipients/content/versions/expiry/caps and consume sends once; edits/revocation block remaining sends; interested replies, prices, first partner messages, public posts, and new channels stay level 0; thresholds never auto-grant authority |
| [REV-67](https://github.com/OpenAgentsInc/openagents/issues/10874) | Completed: survey and admit the Agora parcel | Existing Everglade layout/style contracts | Landed in c07c359771: west-of-Lantern rejected; south-facing market fallback, complete footprint, clear approaches, reachable stations, and 3 m clearance retained in layout/tests |
| [REV-68](https://github.com/OpenAgentsInc/openagents/issues/10875) | Completed: Agora kit generation, review, and artifact admission | Completed REV-67; existing original-art/pack tools and artifact queue #10763 | Landed in c07c359771: original kit, generation/review/admission targets, checked manifests/pack, and 7,618/700 near/far triangles. Historical queue receipt was not recovered; future artifact changes still require the queue |
| [REV-69](https://github.com/OpenAgentsInc/openagents/issues/10876) | Verse: bind Agora stations to world-tree nodes and private sales boards | REV-53, REV-68; world tree [#10788](https://github.com/OpenAgentsInc/openagents/issues/10788) | Reuse the landed layout and reachable stations; add stable nodes and authorized record-backed views with no prospect identity/message; licensed hire art remains optional |
| [REV-70](https://github.com/OpenAgentsInc/openagents/issues/10877) | Crew/Verse: extend Bob's town adapter for sales bodies and day plans | REV-64, REV-69; shared Bob #10806; [#10790](https://github.com/OpenAgentsInc/openagents/issues/10790), [#10791](https://github.com/OpenAgentsInc/openagents/issues/10791) | Reuse Bob's identity/loop; sales placement code validates/captures tables and places Paul plus admitted hires through owner-reviewed work; activities cite actual work and idle stays honest; town routines never reset limits or authorize effects |
| [REV-71](https://github.com/OpenAgentsInc/openagents/issues/10878) | Verse/payments: implement the earned-sale bell and reviewed shared projection | REV-65, REV-69; attributed settlement and delivery evidence | Reuse the landed bell animation/hook; earned-sale attribution fires it once and reversals adjust totals; scratch boards/ticker/capture/animation tests hide raw leads, live amounts, and deal timing from shared viewers; owner can capture/publish only reviewed delayed aggregates or labeled demos |
| [REV-72](https://github.com/OpenAgentsInc/openagents/issues/10879) | Sales/growth: instantiate Arthur/Vanna against qualified partner/referral operations | REV-59, REV-62, REV-64; REV-33 for partner assignments; REV-29–REV-32 for commission activity | Arthur can prepare partner research before commission infrastructure; paid assignments/earnings claims wait for their contracts and qualification; Vanna preserves attribution; own-agent links earn nothing; Sybil scratch referral-abuse checks pass |

### Conditional extensions after launch

Each row needs evidence that its benefit warrants implementation. These are
explicitly outside the initial written US email scope and first-revenue gate.

| ID | Issue scope and owner | Depends on | Acceptance |
| --- | --- | --- | --- |
| [REV-73](https://github.com/OpenAgentsInc/openagents/issues/10880) | Host/sales: qualify standing follow-up policies | Measured REV-66 operation and a separate owner grant | Only explicitly invited threads use exact templates, expiry, revocation, and bounds; no calendar-based automatic promotion or expansion of contact permission |
| [REV-74](https://github.com/OpenAgentsInc/openagents/issues/10881) | Host/plugins: qualify one additional automated public-reply channel | REV-60, REV-62, REV-63; recipient/community/platform permission | Labeled account, threading, suppression, approval, delivery reconciliation, and platform-specific rules work; X bot approval or any other external permission is obtained before activation |
| [REV-75](https://github.com/OpenAgentsInc/openagents/issues/10882) | Sales/host: add consented, booked, human-supervised voice participation | Proven written workflow; separate voice authority and legal review | Recipient requests the meeting; AI identity, recording/transcription/retention, human control, and call rules are explicit; no AI cold calling |
| [REV-76](https://github.com/OpenAgentsInc/openagents/issues/10883) | Sales/policy: add one reviewed international outbound jurisdiction | REV-60–REV-63; separate jurisdiction review | Supported recipient category/channel, consent evidence, footer, retention, suppression, and current local rules are enforced; unknown scope blocks contact |

### Dependency and filing rules

- IDs are stable. Add new rows rather than renumbering referenced work.
- Dependencies name finished contracts/outcomes, not whole future milestones.
  `A or B` selects the relevant product lane; a range includes its named rows.
  Existing open issues retain their own native blockers.
- File a missing row only after reconciling current code and issues. Its body
  includes the owning crate/surface, exact scope, acceptance, targeted checks,
  blocker issue numbers, and links to the authoritative product contract.
- Claim the filed issue and update the required board before implementation.
  Close it after its scoped code, checks, merge, and required host deploy are
  complete. Track owner-only gates separately; failures open focused defects.
- Product implementation stays in Rust with the existing thin native adapters.
  Docs-only work needs link/path/artifact checks, not Rust tests. Ordinary code
  work uses the repository's targeted checks; no full release gate or GitHub
  workflow is introduced by this inventory.

### Smallest revenue paths

| Lane | Minimum build slice | Separate launch evidence |
| --- | --- | --- |
| Paid plugin | REV-01, REV-02, REV-08–REV-12 | Supported external user, real result/payment/split/author payout, and retained reconciliation |
| Retail cloud | REV-01, REV-02, REV-09, REV-13–REV-15 | Confirmed retail contract/price, bounded funded qualification, admitted production configuration, and external client acceptance |
| Funded decision | REV-01, REV-02, REV-09, REV-16, REV-17 | Supported door/backend, real funding/result/charge/receipt, and external client acceptance |
| Invoiced onboarding | REV-01, REV-03, REV-04, REV-06, REV-07, REV-18 | Privately agreed service scope/price, paid invoice, customer acceptance, and support handoff |

Build the common account/card lane, pipeline/intake, and Paul preparation in
parallel when their contracts are ready. REV-27 onward expands a proven
business; REV-51 onward automates and visualizes sales. Neither set is a
blanket R1 dependency. An offer page helps acquisition but an existing warm
buyer can accept a private scoped offer before it launches.

For early invoiced onboarding, existing private pilot, baseline, invoice,
acceptance, and support records can satisfy that path manually. REV-03,
REV-04, REV-06, REV-07, and REV-18 automate and standardize it; new CRM or
reporting software doesn't delay an otherwise deliverable human service.

## Owner qualification and activation gates

These are activation conditions, not unfinished implementations to attach to
closed issues. Keep their concrete run/decision in
[`NEEDS_OWNER.md`](../../NEEDS_OWNER.md), with private credentials and
commercial records off-repository. Each lane uses only its applicable gates.

| Gate | Required action and evidence | Applies to |
| --- | --- | --- |
| O1. Offer and service agreement | Confirm workflow, buyer acceptance, price/trial caps, data recipients, support, and any collaborator/partner's privately accepted role; paid service retains invoice/payment and acceptance | Every chosen offer; REV-01/REV-06/REV-18/REV-33 |
| O2. Funded plugin | Use the [existing demo/runbook](../payments/2026-10-03-end-to-end-demo.md) for real payment, useful execution, split, author payout, and reconciliation; qualify the installed customer, not only the challenge endpoint | REV-10–REV-12 and R1 plugin availability |
| O3. Funded retail | Confirm the [retail contract](../cloud/retail-contract.md) and price, then retain the bounded [funded qualification](../cloud/retail-qualification.md#owner-runbook) with supported wallet/Boat bindings | REV-13–REV-15 and R1 retail availability |
| O4. Production retail activation | Follow [retail operations](../cloud/retail-operations.md) against the qualifying receipt and exact deployed revision/configuration; verify gates, recovery, cleanup, and customer route | Retail offer activation after O3 |
| O5. Funded gateway/card | Select door/provider and commercial units, supply restricted production credentials, and retain genuine funding/usage/reversal evidence for the configured path | REV-16/REV-17 or REV-22/REV-23; sandbox billing is excluded |
| O6. Outreach and broader grants | Complete [sales outreach launch](../../NEEDS_OWNER.md#sales-outreach-launch), certification marks, mailbox/domain/footer/jurisdiction checks, exact campaign cap, and REV-63 scratch reply-injection qualification; later batches/voice/channels need their own grants | REV-57/REV-61–REV-63, then REV-66/REV-73–REV-76 |
| O7. Paid distribution and supply | Accept referral/partner terms and payout destinations, qualify actual commission/creator/worker rails, and authorize any public profile publication and required external platform permission | REV-29–REV-37/REV-48/REV-49/REV-74 |
| O8. Supported-client acceptance | A new customer uses the installed chosen client; retain actual outcome and limitations. Device/store/release steps follow their existing runbooks | REV-02 and each selected paid lane; later REV-44 |

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

The [agent sales floor](agent-sales-floor.md) proposes doing much of the lead
work through OpenAgents' own agents: research from public information, drafted
outreach that the owner approves before it is sent, follow-ups, and qualified
handoffs into the pipeline above. It is G7 tooling and operations. Its
Everglade building, leaderboard, and hires do not gate R1.

Its [October 7 operating decisions](agent-sales-floor.md#initial-operating-decisions)
start Paul alone with owner-sent drafts, permissioned US business email, and
human closing. Add hires from actual queues, qualify batch approvals before
granting them, and keep live revenue private. The bell requires earned
settlement plus delivery evidence, excluding unused top-ups. Launch setup
and campaign grants remain explicit [owner steps](../../NEEDS_OWNER.md#sales-outreach-launch).

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

Use the build IDs above to select the first scoped issues:

1. Product/sales use [Coder pilot v1](README.md#first-workflow-offer-v1);
   client/evidence prepare REV-02/REV-03. Sales
   prepares REV-04/REV-06 and can operate a bounded manual pilot immediately.
2. Client/payments select REV-09–REV-12 for the first installed paid-plugin
   slice and prepare O2/O8. Cloud REV-13–REV-15 and gateway REV-16/REV-17 can
   run independently when their supported contracts are ready.
3. Accounts/payments start REV-19/REV-21, then the relevant common-funding and
   live-card adapters REV-20/REV-22. Preserve first-lane availability while
   cross-product and card funding are being built.
4. Shared-agent work continues with #10800 on completed #10798/#10799, with #10801
   alongside and #10806 before Paul's steering integration. Sales prepares
   REV-04/REV-53 record schemas, REV-55 adapters, and training fixtures in
   parallel; reuse the landed Agora art/layout and add its record-backed
   views in REV-69. Integrate REV-51/REV-58 before budgeted Paul (REV-54).
   Automated sending waits for REV-60–REV-63 and O6. Optional sync and the
   remaining world issues don't delay founder-led revenue.
5. Growth prepares REV-27–REV-33; Brainstorm REV-34–REV-37 is one independently
   qualified discovery experiment. Team and wider-supply rows follow buyer
   evidence, not completion of every preceding product lane.

Decisions still open: O1 activation of Coder pilot v1's proposed service price,
provider budget and support assignment, later business credit, live card
provider and unit conversion, commission base/share,
the first team's required controls, and support responsibility. Record each
decision with its owner and affected milestone. Do not wait for a complete
enterprise package to resolve the offer, payment path, and first pilot.
