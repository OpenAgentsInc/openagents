# Product and commercial readiness

This audit evaluates the product at source revision `07805e6a7c3513a057d226b488cb2d40fd974a64`. It treats the [product specification](../../product/self-improving-codebases.md) as a proposal, the [sales documents](../../sales/README.md) as contracts and qualification records, and issue closure as evidence that a scoped implementation landed. None of these alone establishes customer acceptance, a profitable service, or recursive improvement. Findings below are source-reviewed; this audit did not run customer jobs, spend money, or repeat the qualification suites.

The repository has much of the machinery needed to measure and account for a useful service. Its strongest assets are the separation of independent acceptance from agent assertions, the retention of failed attempts, explicit cost uncertainty, and conservative financial reporting. The missing product is the demonstrated connection between these assets: a repeatable maintenance service whose learned decision layer reduces total cost per accepted change across successive versions, then does the same for another repository and a consenting customer.

## What the product should mean

For this audit, a self-improving codebase is a repository with a controlled maintenance loop that uses independently observed outcomes to improve its future decisions. The first learned decisions are file selection, task briefing, judgment, and calibration. A model that writes code, a collection of agents, an automatically refreshed index, or a training command does not by itself satisfy that definition.

The recursive claim is narrower still: version N helps produce trustworthy outcomes that train version N+1, and N+1 improves a fixed external measure of useful work. The acceptance standard, customer authority, protected evaluation data, budget, and publication authority must remain outside the learner's control. A faster learner that weakens checks or chooses easier issues has not improved this product.

The first customer is this repository. The initial product unit should be an independently accepted maintenance change with a complete attempt history. A merged pull request is a useful delivery milestone, but merge alone does not prove the original defect was fixed, deployment succeeded, the change remained healthy, or the customer accepted it. These states need separate records:

| State | Evidence required | What it does not establish |
| --- | --- | --- |
| Check-passing candidate | Exact base, patch, check definition, and results | Independent replay or customer acceptance |
| Replayed candidate | Clean replay tied to those exact inputs | Adequacy of the checks or customer value |
| Accepted change | Independent review and the responsible maintainer's decision | Deployment or payment |
| Delivered change | Merge/deploy identity and the agreed operational check | Improvement over a baseline |
| Earned sale | Accepted delivery and verified settlement under the agreement | Profit after all costs |
| Improved learner | Protected comparison with unchanged task selection and accounting | Transfer to another repository |

The existing code already distinguishes several of these states. The product should preserve those distinctions in the new runner, reporting, and pricing.

## Readiness by product gate

The specification's seven gates are the right organizing structure. Their status must be read conservatively.

| Gate | Evidence at the audit snapshot | Required next evidence |
| --- | --- | --- |
| S1: own-repository work | Runner, briefing, checks, replay, and a reported 41 admitted traces exist | A frozen inventory of at least 20 qualifying V1 maintenance issues, accepted merges, cloud execution identities, failures, and complete costs |
| S2: better economics | The specification reports a two-issue comparison with roughly 2–5× lower cost and no speed claim | [#11211](https://github.com/OpenAgentsInc/openagents/issues/11211): at least 20 issues × 3 repetitions, equal or better accepted success, at least 30% lower cost per accepted PR, and no worse median time |
| S3: recursion | Training and measurement components exist | Two successive retraining cycles that beat the frozen baseline beyond noise on protected issues |
| S4: owned decision layer | Multiple serving and fallback components exist | Production evidence without a required Jev key; reconcile the spec's connected-Pylon/Vertex-only rule with [#11225](https://github.com/OpenAgentsInc/openagents/issues/11225), which adds hosted Clef before Vertex |
| S5: another repository | Finder accepts a repository path | A second public repository built from its own history, isolated indexes and corpus, and an independently scored transfer study |
| S6: customer pilot | Detailed assisted-pilot contract, comparison schema, finance schema, and delivery kit exist | One real buyer's exact agreement, acceptance, payment evidence, complete costs, and a separately permitted improvement chart |
| S7: self-service product | Accounts, repositories, environments, billing, and team controls provide foundations | Qualified repo connection through accepted delivery, pricing activation, workspace isolation, training controls, support, and revocation |

No complete S1–S7 qualification packet was found in the reviewed sales artifacts. This is an evidence statement, not an assertion that no private customer or operator work exists. Private evidence should remain private; a public audit can reference its reviewed digest, scope, and limitations.

## Findings

### PRODUCT-01 — The product claim is ahead of the end-to-end qualification

**Priority: high.** The specification clearly calls itself a proposal, but its component accomplishments are easy to read as proof of the full product. File recall, replayed labels, a trained head, and issue throughput are intermediate measures. They do not prove accepted-outcome economics, repeatable recursive gains, or paid demand.

The [install qualification](../../sales/install-qualification.json) explicitly sets `real_provider_qualified`, `real_customer_qualified`, and `commercial_activation` to false. It exercised an offline Codex protocol fixture, a pinned earlier release, and a warm build cache. The [team qualification](../../sales/team-qualification.json) explicitly sets `production_qualification` to false. These are useful tests of implementation behavior; relabeling them as installed-customer proof would erase their most important limitation.

**Acceptance:** retain one gate register for S1–S7 with an exact source revision, evidence digest, selection method, execution environment, approver, and unresolved limitations. Each claim must name the gate it supports. A failed or missing gate remains visible. The first public statement should describe the measured repository and task cohort, not “codebases improve themselves” without qualification.

### PRODUCT-02 — The sold pilot and the proposed recursive product have different contracts

**Priority: high before a customer pilot.** [Coder pilot v1](../../sales/README.md) is an assisted public-repository maintenance service. It freezes a commit, a bounded task, and 1–8 checks. Its proposed USD 250 service fee is due only on accepted delivery. It allows one repair, a seven-day review, and three operator hours. The customer supplies their own computer and provider budget. Training, private repositories, publication, remote shells, and guaranteed savings are excluded by default.

The recursive product proposes per-accepted-PR pricing, company-funded failed attempts, training on outcomes with separate permission, and an improvement chart. These are not minor wording changes. In the pilot, a buyer can still incur provider charges for failed attempts even when the service fee is not earned. In the proposed outcome-priced service, OpenAgents is expected to absorb failed work. A sales presentation must not borrow the latter promise while delivering the former contract.

**Acceptance:** choose the first offer explicitly. For a manual pilot, keep the current bounded contract and add a separately accepted research agreement if an improvement study is included. For an outcome-priced service, freeze who pays for retries, replay, support, failed installs, cancellations, and regression repairs. State the acceptance deadline and dispute path. Record the buyer and owner's acceptance of the exact offer digest before work. The owner activation steps already exist in [NEEDS_OWNER.md](../../../NEEDS_OWNER.md); code completion does not authorize the sale.

### PRODUCT-03 — New-runner evidence is not yet joined to the mature sales accounting path

**Priority: high.** [Sales evidence](../../../crates/gym/src/sales_evidence.rs) has a useful contract: a frozen attempt inventory, exact check digests, primary/repair/retry relationships, full ATIF traces, separate setup/queue/check/support time, an independent checker distinct from the executor, and the customer's exact decision. It rejects omitted or reordered inventory entries and keeps `deployed_routing_improvement` false.

The newer [issue runner](../../../crates/coder-new/src/issue_run/setup.rs) emits a different summary, while [trace capture](../../../scripts/bench/traces/traces.py) emits replay and training records. A targeted search of `coder-new`, the briefing benchmark, the trace tool, and the finder did not find an emitter for the sales comparison manifest or its customer-acceptance fields. The reviewed implementation therefore does not establish an automatic join from a new runner attempt through REV-03 comparison and REV-25 finance. Manual preparation remains possible, but it is work with omission risk.

The trace tool's `accepted()` means that replayed checks passed. Sales `Acceptance` means an exact complete candidate passed checks and has independent and customer evidence. Those labels must not become interchangeable through an adapter.

**Acceptance:** write one converter or shared record contract that retains source identities and translates new-runner attempts without inventing missing acceptance. Exercise a successful attempt, a failed attempt, a repair, cancellation, missing cost, incomplete trace, and a customer rejection. Rebuild the same private REV-03 and REV-25 report from retained inputs. Replayed success alone must never create customer acceptance or earned revenue.

### PRODUCT-04 — Missing cost can become zero in the new trace path

**Priority: high for the economic comparison.** In `capture_issue_run`, [traces.py](../../../scripts/bench/traces/traces.py) computes `(agent_usd or 0) + (decision_usd or 0)`. The runner's [agent cost](../../../crates/coder-new/src/issue_run/agent.rs) is optional, and its [summary card](../../../crates/coder-new/src/issue_run/cards.rs) also defaults absent numeric cost to zero. This loses the distinction between a free operation and an unreported charge.

The older sales schema handles this correctly: `CostBasis::Unknown` carries no amount, missing cost components become unknown, and subscription capacity is not cash. [Sales finance](../../../crates/gym/src/sales_finance.rs), `calculate`, withholds an unqualified profitability result if costs are incomplete, estimated, subsidized, or in incompatible denominations.

**Acceptance:** preserve amount, denomination, price version, payer, basis, completeness, and source for every cost component. Missing provider usage must remain unknown through capture, training reports, comparison, UI, and exports. A null provider amount plus a known decision charge should yield a known subtotal and an unknown component, never a complete total. Include this case in the S2 report qualification before quoting savings.

### PRODUCT-05 — Accepted-outcome unit economics need every failed and supporting operation

**Priority: high.** Inference price per token or elapsed agent time is only part of the denominator. The product must pay for discovery, indexing, embeddings, brief generation, routing, fallback, model execution, compilation, independent replay, human review, failed installs, support, training, calibration, storage, and idle infrastructure. Rejected candidates are especially important: training may benefit from them while they still consume real money and reviewer time.

The existing [finance module](../../../crates/gym/src/sales_finance.rs) already separates contractual charges, collections, funding, unspent balances, earned shares, liabilities, refunds, losses, and expenses. It supports declared payer and baseline allocation. Reuse it; do not add a simpler “revenue minus tokens” dashboard.

**Acceptance:** produce both cash contribution and fully loaded operating cost per accepted change for the same frozen cohort. Declare amortization and labor assumptions rather than hiding them. Preserve zero accepted changes as an undefined per-acceptance result with a visible total loss. A study with unknown costs may report partial observations, but cannot satisfy a claim about total cost reduction.

### PRODUCT-06 — Customer value has not been qualified by the sales fixtures

**Priority: high for commercialization.** The sales material provides a concrete delivery kit, agreement schema, qualification checklist, support terms, cleanup record, and private evidence export. That reduces operational risk. It does not identify a completed real-customer cohort, willingness to pay, repeat use, or positive margin.

The right next demand experiment is small: one authorized warm buyer, one public repository, one maintenance task, a frozen baseline, a capped repair, and exact acceptance. Cold outreach automation, a referral campaign, a public sales floor, and a training marketplace are not prerequisites for this experiment. The existing manual service can test the buyer's problem while the recursive layer remains experimental.

**Acceptance:** retain the reason the buyer chose the task, the current alternative, installation friction, first useful result, accepted/rejected decision, review and support minutes, collection/refund status, and a second distinct use or an explicit reason for non-repeat. Report conversion denominators, including qualified prospects who decline. Do not infer product demand from created leads, scheduled meetings, credits, or agent activity.

### PRODUCT-07 — Sales readiness documents disagree about implemented work

**Priority: medium.** [Revenue handoff](../../sales/revenue-handoff.md) describes later sales-floor work as not started. Later appendices in [agent-sales-floor.md](../../sales/agent-sales-floor.md) describe implemented hiring, floor reporting, earned-sales display, role desks, batches, and standing follow-ups. Corresponding source modules exist under [Coder task sales](../../../crates/coder/src/task/sales.rs).

This matters to recursive work because a planner using stale roadmaps can recreate code, count a closed issue as outstanding product demand, or schedule a large sales subsystem ahead of the missing learning evidence. The reverse mistake is treating a closed feature issue as live commercial qualification. The recent issue record repeatedly closes code-complete work while preserving owner-only activation in `NEEDS_OWNER.md`.

**Acceptance:** update the existing handoff and roadmap status against the current issue snapshot and implementation. Keep implementation, qualification, and activation in separate columns. Make a planner cite the status source and date before choosing work from a roadmap. Do not reopen completed implementation merely because customer credentials, payment, or owner review remain outstanding.

### PRODUCT-08 — The earned-sales display has a narrower delivery rule than the service offer

**Priority: medium.** [Service sale admission](../../../crates/receipts/src/service_sale.rs) permits `fulfillment: None`; separately priced fulfillment is optional. In [earned.rs](../../../crates/coder/src/task/sales/earned.rs), `row` treats delivery as reconciled only when `effective_fulfillment()` returns `Some` with a bill. A valid accepted service sale with no separate fulfillment supplier therefore cannot qualify for the earned-sales bell through this predicate.

This is a display/accounting-definition mismatch, not evidence of lost money. It can nevertheless make the first genuine pilot look unearned even when its accepted result and invoice satisfy the main service contract.

**Acceptance:** define “delivered” using the accepted service-result evidence. Require a fulfillment bill only when the accepted offer includes that obligation. Check paid accepted delivery without fulfillment, with a supplier bill, with an unresolved supplier bill, a refund, and a dispute. Keep the bell a projection of the books; it must not create revenue or payment authority.

### PRODUCT-09 — Self-service requires qualified control and isolation, not just screens

**Priority: high before S7; not a blocker for local dogfood.** Open [#11226](https://github.com/OpenAgentsInc/openagents/issues/11226) covers repository-scoped cloud GitHub credentials for push/PR work. [#11227](https://github.com/OpenAgentsInc/openagents/issues/11227) covers integration coordination and idle stop, and [#11228](https://github.com/OpenAgentsInc/openagents/issues/11228) covers cloud-fleet control in the web and phone. These affect the end-to-end service and its idle cost.

Closed [#11186](https://github.com/OpenAgentsInc/openagents/issues/11186) repaired workspace isolation for provider keys and related account state. Its closing evidence explicitly leaves pooled decision quota and classify concurrency to open [#11190](https://github.com/OpenAgentsInc/openagents/issues/11190). The current gateway still has `tenant_slots` keyed by registry tenant in [serve.rs](../../../crates/gateway/src/serve.rs). Independent signup workspaces sharing that tenant can therefore share capacity pressure even after credential isolation is fixed.

**Acceptance:** qualify two unrelated workspaces from repo connection through execution, cancellation, evidence export, deletion, and key revocation. Saturation in one must not consume the other's contracted allowance. A removed repository grant must stop new effects. An orphaned environment must not continue consuming budget indefinitely. Qualify the supported client matrix explicitly; the synthetic team matrix does not establish all phone, desktop, browser, and placement combinations.

### PRODUCT-10 — Broader payment and market work must remain separate from proof of the loop

**Priority: medium.** The repository has inference billing, credit ledgers, Lightning/x402, contribution obligations, and sales service records. These are separate units of commerce. Open [#11082](https://github.com/OpenAgentsInc/openagents/issues/11082) still requires an outside caller's accepted result and worker payout for paid agent work. Closed [#10856](https://github.com/OpenAgentsInc/openagents/issues/10856) connected a selected contribution obligation to protected acceptance and a funded liability under synthetic evidence; its closing comment explicitly disclaims a live market, funded payout, or serving activation.

Training execution fees, data licenses, accepted-improvement bounties, customer service invoices, and inference token charges must remain distinct. Paying a worker to train does not establish model improvement. Sealing a candidate does not authorize deployment. A customer top-up is not earned service revenue. Old open issue bodies that propose additional payment rails also need reconciliation with later owner direction; they are not an instruction to broaden the first product.

**Acceptance:** select one payment route for the initial offer, retain its real qualification privately, and join it to the exact accepted task. Keep other markets optional until they solve an observed delivery constraint. Any contributor reward must preserve independent protected evaluation, complete failed-trial costs, rights, and one obligation identity.

## Economic measurement contract

Use a frozen cohort, not a gallery of successful PRs. Record every selected task before execution, including tasks that fail setup or prove unsuitable. Define task exclusion reasons before looking at the candidate's performance. Use the same base, task, provider/account policy, time allowance, and check standard for baseline and candidate; label retrospective selection explicitly.

For each arm, report:

- Selected, attempted, replayable, accepted, merged, deployed, reverted, and customer-rejected tasks.
- Primary attempts, retries, repairs, fallbacks, cancellations, timeouts, and infrastructure failures.
- Total cash outlay, known estimates, unknown components, provider grants, and subscription capacity separately.
- Setup, queue, execution, check/replay, review, support, and end-to-end wall time. Parallel execution reduces elapsed time but does not erase worker or reviewer cost.
- Quality after acceptance: escaped regressions, rollback/revert, repair obligation, and support burden over a declared observation window.
- Per-task differences and uncertainty, not only an aggregate mean. Keep repeated trials grouped by issue so they do not masquerade as independent customer tasks.

Let `A` be the number of independently accepted tasks and `C` the complete cost of all selected tasks and their associated work. Then cost per accepted task is `C / A`, only when `A > 0` and required cost coverage is complete. The denominator must not count multiple accepted repairs of one task as several successes. A training experiment's failures stay in its training cost even when their labels later become useful.

| Cost class | Include | Accounting treatment |
| --- | --- | --- |
| Find and brief | History refresh, embeddings, retrieval, decision calls, fallback, prompt preparation | Per request plus explicit index amortization |
| Execute | Every generation, tool run, worker retry, failed attempt, and provider error with a charge | Actual billed amount where available; list price and capacity separate |
| Verify and deliver | Build lease wait, compilation, fixtures, independent replay, review, merge/integration, deployment check | Separate elapsed and resource time; reviewer labor declared |
| Learn | Corpus creation, labeling, adjudication, rejected training runs, calibration, protected evaluation, model packaging | Period cost plus a declared allocation over accepted work |
| Operate | Idle VMs, storage, backups, network, observability, on-call, retention cleanup | Cohort allocation with utilization assumptions |
| Sell and support | Qualification calls, failed installs, onboarding, support, repairs, refunds, payment fees, commissions where authoritative | Include failed buyers and unearned work; distinguish customer payer |

For the proposed USD 250 pilot, no margin estimate follows from the quoted fee alone. Three operator hours, even before provider costs, can dominate contribution depending on the declared labor rate. The price is a proposal awaiting activation, not an observed willingness-to-pay result. For subscription capacity, report incremental cash and opportunity cost separately rather than pretending the subscription is either free or a metered API bill.

The recursive investment case adds a break-even measure: the incremental cost of collecting, training, evaluating, and operating the learned layer divided by its supported savings per future accepted change. If savings are uncertain or negative, the break-even count is unknown. Report the number of subsequent accepted tasks needed to amortize a training cycle and whether the codebase produces that volume before the model becomes stale.

## Sequence that produces a saleable result

1. Join the existing attempt, replay, and sales schemas; preserve cost uncertainty and distinct acceptance states.
2. Freeze the own-repository study and complete S1/S2 with all failures, replay costs, and human time.
3. Run two protected learning cycles without changing the acceptance target. Keep the old version available for rollback and comparison.
4. Qualify one second public repository with isolated data and no shared improvement claim.
5. Deliver one bounded assisted pilot under an exact agreement. Add a separate training agreement only if the buyer chooses the improvement study.
6. Use observed installation, support, acceptance, repeat-use, and cost results to set the self-service scope and price.

The previous health audits identify many legitimate maintenance candidates across native apps, mobile, payments, gateway, and Verse. Use them as possible task strata, after verifying the finding still exists. Do not turn every surface into a prerequisite for proving the loop. For example, the old gateway audit's synchronous silent receipt-write problem has changed: current [receipt_log.rs](../../../crates/gateway/src/receipt_log.rs) writes on the blocking pool and counts/logs failures. A self-improvement planner that blindly replays the old audit would waste work. Conversely, a public benchmark, real payment, or device qualification still needs its own evidence even when all associated code has landed.
