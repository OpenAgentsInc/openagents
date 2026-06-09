# OpenAgents Revenue Share System

Date: 2026-06-05

Status: product and implementation planning note. This document does not create
payment obligations, set final legal terms, enable withdrawals, change runtime
policy, or move money by itself.

## Source Set

Local OpenAgents/OpenAgents product surface sources reviewed:

- `docs/sites/vida_referral_model_for_openagents.md`
- `docs/sites-plan.md`
- `docs/sites.md`
- `docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md`
- `docs/2026-06-02-mdk-l402-agent-checkout-audit.md`
- `docs/2026-06-03-autopilot-billing-credits.md`
- `docs/2026-06-04-stripe-effect-service-audit.md`
- `docs/sites/2026-06-05-oa-sites-vibesdk-gap-analysis.md`
- `docs/sites/2026-06-05-targeted-site-remake-outreach-roadmap.md`
- `docs/sites/2026-06-05-customer-site-revision-feedback-api.md`
- `docs/sites/2026-06-05-customer-site-revision-review-ui.md`
- `docs/sites/2026-06-05-ben-otec-site-quality-postmortem.md`
- `docs/sites/2026-06-05-stable-slug-latest-revision-policy.md`
- root workspace `docs/omni/README.md`
- root workspace `docs/omni/signature-marketplace-and-streaming-money.md`
- root workspace `docs/omni/bitcoin-payments-infrastructure.md`
- root workspace `docs/omni/agent-cloud-edge-synthesis.md`
- root workspace `docs/omni/gavin-baker-gap-analysis-roadmap.md`
- root workspace `openagents/docs/MVP.md`
- root workspace `openagents/docs/2026-04-21-run-pylon-get-paid-for-training.md`
- root workspace `openagents/docs/nexus-treasury.md`
- root workspace `openagents/docs/pylon/README.md`
- root workspace `openagents/docs/pylon/LDK_ACCEPTED_WORK_PAYOUT_PROOF.md`
- root workspace `openagents/docs/pylon/LDK_WALLET_TELEMETRY.md`
- root workspace `openagents/docs/pylon/LDK_WALLET_CHANNEL_LIQUIDITY.md`
- root workspace
  `openagents/docs/reports/nexus/2026-05-18-ldk-accepted-work-production-proof.md`

This document also uses the user-provided older Flow of Funds objective:
OpenAgents should make it easy for agent users to pay, make it easy for useful
contributors to earn, and keep payment code and aggregate accounting public.

## Objectives

- Pay Bitcoin to as many people as possible, as much as possible, as fast as
  possible, with the easiest user experience.
- Make it as easy as possible for end users and agents to pay for agent usage.
- Tie contributor rewards to real paid usage, accepted outcomes, or accepted
  contribution value, not to vague activity or raw signups.
- Let balances update quickly while keeping final payout claims tied to
  receipt-backed settlement.
- Keep OpenAgents as a business with a clear platform share, reserves, and
  margin memory.

## Vida Pattern To Adapt

The useful Vida lesson is product-native, usage-funded referral rewards. The
referral program matters because it is tied to paid product events. A referrer
should earn when referred users, agents, customers, or contributors create
real paid volume, not merely because an account was created.

OpenAgents should adapt the simplest defensible form:

- one direct referrer;
- one optional upstream referrer;
- rewards funded from platform revenue or platform share;
- rewards triggered by paid usage, accepted outcomes, or retained paid volume;
- no payout for unfunded signups alone; and
- clear cap, reserve, and compliance controls.

This keeps the upside of a two-layer referral network without turning the
system into recruiting-first network marketing.

## Who Pays

The payers are agent users:

- individual users paying for agent usage through the self-serve web platform;
- agents paying for paid actions, tools, endpoints, source packages, or Sites
  actions with scoped spend caps;
- companies buying agent usage in bulk through credits, invoices, or account
  contracts;
- Sites customers paying for generated Sites, paid Site actions, or hosted
  checkout products; and
- future workroom buyers paying for accepted outcomes.

The current OpenAgents product surface starting point is the D1-backed billing credit ledger. Users
can receive launch credits, coupons, manual credits, and future Stripe credit
purchases. The Sites and MDK roadmap adds hosted checkout intents and L402
paid-action flows.

## Who Gets Paid

The contributor graph should include:

- Pylon compute providers and validators;
- model authors;
- data providers and curators;
- algorithm authors;
- app developers;
- signature tool and agent module authors;
- grader, verifier, and fixture authors;
- source-package and context-pack contributors;
- UI binding and runtime adapter authors;
- referrers and distributors;
- educators and course or onboarding creators;
- sales or outreach contributors when a lead, meeting, or customer is accepted;
  and
- OpenAgents as platform operator, treasury reserve, and business owner.

The old Agent Store framing paid agent and plugin authors. The current Omni
framing is broader: any reviewed, versioned capability that contributed to a
paid accepted event can have a split.

## Pylon Compute Provider Lane

Pylon is the concrete current compute-provider app for the OpenAgents Compute
Market. It should not be treated as a generic contributor category only. A
Pylon operator can run Pylon, go online, advertise compute capability, register
an LDK-compatible payout target with Nexus, accept assigned work, produce
artifacts or validation results, and get paid when the work is accepted.

This lane covers providers that supply:

- CPU, GPU, Apple Silicon, or other local execution capacity;
- inference, embedding, training, fine-tuning, evaluation, or validation work;
- packaged runtimes such as Psionic homework/training workers;
- artifact generation, verification, and acceptance evidence; and
- future specialized provider roles where the machine contributes scarce
  execution, locality, reliability, or operator-controlled capacity.

Pylon payments are provider payments for accepted work. Bare compute can be
priced as capacity, runtime, token, embedding, job, or training-unit cost.
Expensive or high-value workloads can add premiums for GPU class, isolation,
turnaround, checkpointing, data locality, validator responsibility, scarce
capacity, or a higher-value accepted outcome. Those premiums should be explicit
split inputs, not hidden in referral math.

The current OpenAgents proof path is LDK-first:

- Pylon registers a payout target such as `bolt12_offer`, `bolt11_invoice`,
  `bip353_name`, or `lnurl_pay`; BOLT12 is the preferred durable target.
- Nexus assigns paid work only to providers with the required eligibility and
  LDK-compatible target.
- Treasury dispatches accepted-work payouts through the LDK provider boundary.
- Pylon observes the receive in its wallet.
- Receipts tie worker lease, artifact/window acceptance, validator claim,
  Treasury operation, provider payment id hash, and final settlement state.

MDK should help with user and agent checkout, paid endpoint ergonomics, wallet
onboarding, liquidity support, and future Site/agent payment primitives. It
should not replace Nexus/Treasury/LDK as the payout authority for current
Pylon accepted-work payments.

2026-06-05 status addendum: the next OpenAgents product surface roadmap batch should prioritize the
agent Sites, Pylon setup, and Site commerce contracts before public launch
claims. Pylon's current public floor remains the narrow v0.1.x earning path:
`pylon-v0.1.16` or newer with the same guarantees, and the package-managed
launcher at `@openagentsinc/pylon@0.1.17` or newer. Pylon v0.2 should not be
advertised as broadly ready until the LDK-compatible payout target
registration path, hosted/runtime package gates, asset coverage, eligibility
telemetry, and public instructions are reviewed. Generated commerce Sites
should use OpenAgents product surface-hosted checkout/L402 boundaries for buyer payment evidence,
while Pylon accepted-work payout claims remain tied to Nexus/Treasury/LDK
receipts.

Important state boundaries:

```text
online/present != eligible
eligible != assigned
assigned != accepted
accepted != paid
paid != settled until receipt-backed reconciliation
```

For Bitcoin or Lightning-funded accepted work, Pylon providers can earn sats.
For credit-funded compute usage, the safe default is provider credits or an
internal payable until OpenAgents has an explicit conversion policy, reserve,
and settlement path. Credit spend should not silently mint immediate Bitcoin
withdrawal liability for compute payouts.

## Balance Types

Revenue share must distinguish the asset used to pay from the asset used to
credit contributors.

| Buyer payment source                                                       | User-facing spend asset         | Contributor revshare asset  | Withdrawal posture                                                                                  |
| -------------------------------------------------------------------------- | ------------------------------- | --------------------------- | --------------------------------------------------------------------------------------------------- |
| Direct Bitcoin or Lightning payment                                        | sats                            | sats                        | Eligible for Bitcoin/Lightning withdrawal after settlement, reserves, and compliance checks.        |
| MDK/L402 paid endpoint or Site action                                      | sats                            | sats                        | Eligible for Bitcoin/Lightning withdrawal after MDK reconciliation and OpenAgents receipt closeout. |
| Pylon accepted work funded by Bitcoin/Lightning or treasury starter budget | sats                            | sats                        | Eligible after accepted closeout, LDK payout dispatch, and settlement receipt.                      |
| Credit-funded compute usage routed to Pylon                                | credits                         | credits or internal payable | Bitcoin payout only after explicit conversion policy, reserve, and settlement approval.             |
| Bitcoin-funded sats balance                                                | sats                            | sats                        | Eligible for Bitcoin/Lightning withdrawal after settlement, reserves, and compliance checks.        |
| Product credits spent from account balance                                 | credits                         | credits                     | Spendable inside OpenAgents; not automatically withdrawable as Bitcoin.                             |
| Credit card or Stripe top-up                                               | credits                         | credits                     | Card-funded value remains credit-based, chargeback-aware, and not Bitcoin-withdrawable in v0.       |
| Coupon, launch grant, free beta, or operator promo                         | credits                         | normally no revshare        | Promo value should not create withdrawable contributor rewards unless explicitly budgeted.          |
| Company invoice or contract                                                | credits or invoiced entitlement | credits by default          | Bitcoin conversion requires a separate approved policy and reserves.                                |

The key rule:

```text
Bitcoin revenue can create Bitcoin revenue share.
Credit spend creates credit revenue share.
Free or promotional spend does not create withdrawable Bitcoin revenue share.
```

This avoids the most dangerous mismatch: using card-funded or promotional
credits to create immediate Bitcoin withdrawal liability.

## Flow Of Funds

### Direct Sats Flow

```text
user or agent pays sats
-> checkout, invoice, or L402 payment reconciles
-> OpenAgents product surface records payment event and accepted usage/action
-> split definition is applied
-> contributor sats ledger entries are created
-> reserve/compliance checks run
-> contributor withdraws through Bitcoin/Lightning when eligible
```

### Credit Flow

```text
user or company buys or receives credits
-> credits are spent on agent usage, Site usage, or workroom usage
-> OpenAgents product surface records usage debit and accepted event
-> split definition is applied
-> contributor credit ledger entries are created
-> contributor can spend credits inside OpenAgents
-> withdrawal to Bitcoin is not available in v0 for credit-sourced revshare
```

### Accepted Outcome Flow

```text
buyer pays or spends balance
-> workroom assignment or Site paid action starts
-> modules, providers, models, data, and referrers are attributed
-> artifact or action is accepted
-> receipt bundle closes
-> split calculation writes ledger entries
-> settlement or withdrawal state is projected safely
```

### Pylon Compute Provider Flow

```text
user, agent, Site, or workroom needs compute
-> payment policy reserves sats, credits, or treasury starter budget
-> scheduler selects an eligible Pylon provider
-> provider has an LDK-compatible payout target registered with Nexus
-> Pylon runs compute, training, inference, embeddings, or validation
-> artifacts, checkpoints, or validator receipts close out as accepted
-> provider split and payout row are created
-> Nexus/Treasury dispatches LDK payout when sats-funded or approved
-> Pylon wallet observes receive
-> receipt bundle reconciles payment and accepted outcome
```

The product can show fast balance ticks, but public payout truth must keep
states separate:

```text
pending -> credited -> withdrawable -> withdrawal_requested -> dispatched
-> settled
```

## How Payments Are Split

Each agent, Site paid action, agent module, signature tool, or accepted
workroom product can have its own split definition.

Any part of an [agent module](https://github.com/OpenAgentsInc/openagents/wiki/Agent-Modules)
may have an associated cost that is paid to one or more contributors. In the
newer OpenAgents product surface/Omni model, this becomes a versioned split definition attached to
reviewed capability objects:

- agent or Site product;
- signature tool version;
- module version;
- data/source package;
- model or inference provider;
- compute provider;
- API-key or service provider;
- grader/verifier;
- app or UI binding;
- referrer and upstream referrer; and
- OpenAgents platform share.

The split engine should calculate from net distributable value:

```text
gross buyer spend
  - refunds, discounts, payment fees, taxes, chargeback reserves,
    operator-approved promos, and risk reserves where applicable
  = net distributable

net distributable
  -> contributor buckets
  -> referral buckets
  -> OpenAgents platform share
  -> reserve or insurance bucket when needed
```

For direct sats payments, the net distributable unit is sats. For credit spend,
the net distributable unit is credits.

Pylon provider payment should be a first-class cost bucket before optional
referral and upside splits. A provider's bare compute floor should not be
diluted below policy by referrers, module authors, or platform share. Higher
value provider work can also participate in accepted-outcome upside when the
provider supplied more than commodity compute, such as scarce GPU capacity,
trusted isolation, validation, long-running training, or specialized runtime
operation.

## Example

Bob configures his AutoDev agent to use a "Resolve GitHub Pull Request" module.
He pays `12000 sats` to run the job. The accepted run uses a specific chain of
APIs, models, vector indexes, source packages, and compute providers.

Example split:

| Recipient  | Why they earn                                                    | Amount    |
| ---------- | ---------------------------------------------------------------- | --------- |
| Alice      | Assembled and published the reviewed skill/module.               | 2000 sats |
| Carol      | Provided compute for embeddings and open model calls.            | 1000 sats |
| Dave       | Provided the paid API key used for one model call.               | 500 sats  |
| Eve        | Referred Alice months ago.                                       | 120 sats  |
| Frank      | Referred Bob yesterday.                                          | 120 sats  |
| Grace      | Authored the finetuned model used in the flow.                   | 300 sats  |
| Heidi      | Curated the custom dataset used in the flow.                     | 300 sats  |
| Ivan       | Ran the Pylon worker that produced the accepted artifact.        | 2500 sats |
| OpenAgents | Platform operation, orchestration, reserves, growth, and profit. | remainder |

If Bob paid the same job with OpenAgents credits rather than sats, the same
logical split would create credit ledger entries, not Bitcoin withdrawal
claims.

## Immediate OpenAgents product surface Connection

OpenAgents product surface can connect referral revenue to the current system before the full
signature marketplace exists.

### 1. Add Attribution Capture

Add a minimal referral-attribution layer:

- invite/referral code on signup and onboarding;
- referral code capture on software-order submission;
- referral code capture on billing checkout start;
- referral code capture on Sites checkout or paid-action intent;
- durable `direct_referrer_user_id` and optional `upstream_referrer_user_id`;
- first verified attribution wins unless an operator resolves a dispute; and
- no payout for signup alone.

Near-term fields or tables:

```text
referral_invites
referral_attributions
referral_events
```

### 2. Use Current Credit Ledger Debits As The First Trigger

The current billing system already writes positive credit entries and negative
usage debits for SHC container time and Codex token usage. The first referral
revshare can trigger from actual paid usage debits:

```text
billing_ledger_entries debit
-> referred account?
-> debit is paid/source-eligible?
-> write revshare credit entry to referrer
```

This should not trigger from a launch grant, coupon, free beta credit, or an
operator-only smoke credit unless the program explicitly budgets that promo.

### 3. Keep The First Payout As Credits

The first implementation should credit referrers with OpenAgents credits for
credit-funded usage. That immediately makes referral value useful without
introducing Bitcoin withdrawal risk from card-funded balances or promo grants.

Minimum first state:

```text
referrer earns credits
-> credits appear in billing/referral dashboard
-> credits can be spent on OpenAgents usage
-> no Bitcoin withdrawal from credit revshare
```

### 4. Add Direct Sats Referral Later In The MDK/Sites Slice

When Sites MDK checkout and L402 paid actions are live, direct sats payments
can create sats revshare entries:

```text
site payment event or L402 redemption
-> payment source = sats
-> accepted paid action or checkout entitlement
-> split/referral calculation
-> contributor sats ledger
-> withdrawal eligible after settlement and compliance
```

This should be the first Bitcoin referral payout path because the revenue
source is Bitcoin/Lightning, not card-funded credit.

### 5. Bridge Pylon Accepted-Work Events Into OpenAgents product surface Accounting

Pylon/Nexus/Treasury should remain the payout authority for current
accepted-work compute payments, but OpenAgents product surface needs to understand those events so
contributors, referrers, Sites, agents, and accounting dashboards share one
truthful revenue-share model.

Near-term bridge records should import or reference:

```text
pylon provider identity
registered payout target kind/hash
accepted outcome id
Nexus operation id
Treasury operation id
payout class = accepted_work
payout amount and asset
LDK dispatch and settlement status
receipt bundle ref
```

This lets OpenAgents product surface attribute Pylon work to a Site, agent, workroom, module, or
campaign without pretending that the OpenAgents product surface credit ledger itself paid the Pylon
operator. It also keeps MDK in the correct role: checkout and agent payment
primitive now, liquidity and wallet support later, not the source of truth for
Pylon accepted-work payouts.

## Future Contributor Revenue Share

After the referral-only slice, revenue share should expand to all accepted
contribution types.

### Agent Module And Signature Share

```text
accepted Program Run
-> selected agent module/signature versions
-> verified attribution graph
-> split definition version
-> credit or sats ledger entries
```

### Pylon Provider Share

Pylon providers should earn for accepted work, not uptime alone:

```text
Pylon provider eligible
-> work assigned
-> artifact produced
-> outcome accepted
-> provider payout row
-> LDK settlement receipt
```

Provider split definitions should support both:

- base compute payment for measured capacity, runtime, token, embedding,
  inference, training, validation, or artifact work; and
- premium or accepted-outcome upside for scarce capacity, expensive workloads,
  validator responsibility, runtime packaging, customer-trusted isolation, or
  operator-controlled high-value execution.

Presence, liveness, or availability stipends may exist as explicit starter or
operator-subsidy programs, but they are separate payout classes. They should
not be confused with accepted-work revenue share.

### Sales And Outreach Share

The targeted Site remake/outreach lane should support referral-style revenue
for internal operators and later user-owned agents:

```text
lead proposed
-> prospect accepted into campaign
-> preview generated
-> outreach approved and sent
-> meeting or customer accepted
-> revenue event occurs
-> sales/referral revshare entry
```

Pay only on accepted outcomes such as paid customer conversion, not on raw
lead count, scrape count, or email sends.

### Educator Share

Educators can earn when their course, tutorial, prompt pack, onboarding flow,
or agent-training material drives retained paid usage:

```text
education attribution
-> user or contributor completes onboarding
-> paid usage or accepted contribution occurs
-> bounded educator share accrues
```

## Ledger Model

Use separate but linked ledgers instead of one mutable balance.

Suggested records:

```text
revshare_accounts
revshare_split_definitions
revshare_split_participants
revshare_events
revshare_ledger_entries
revshare_withdrawal_requests
revshare_settlement_receipts
revshare_provider_work_events
referral_invites
referral_attributions
```

Important fields:

```text
asset = "credits" | "sats"
source_payment_kind = "stripe_credit" | "bitcoin" | "lightning" | "mdk_l402"
  | "coupon" | "operator_grant" | "invoice"
source_payment_ref
accepted_event_ref
pylon_provider_pubkey
pylon_payout_target_kind
pylon_payout_target_hash
nexus_operation_id
treasury_operation_id
payout_class = "accepted_work" | "availability_stipend" | "beta_bonus"
provider_payment_id_hash
compute_work_class
compute_units
runtime_artifact_ref
acceptance_receipt_ref
ldk_settlement_status
split_definition_version
recipient_user_id
recipient_contributor_kind
amount
state = pending | credited | reserved | withdrawable | withdrawal_requested
  | dispatched | settled | failed | clawed_back
idempotency_key
receipt_ref
```

Balances should be derived from ledger entries. Do not store a mutable balance
as payment truth.

## User Experience

### Payer UX

End users should see the easiest possible payment path:

- prepaid credits for normal web usage;
- Stripe Checkout for credit-card top-ups where enabled;
- Bitcoin/Lightning top-up or direct L402 payment where enabled;
- clear spend caps for agents;
- explicit price preview before an agent pays;
- clean return URLs after checkout; and
- recoverable `402` or top-up paths for economic limits.

### Contributor UX

Contributors should see:

- pending, credited, withdrawable, requested, and settled amounts;
- asset type, credits or sats;
- what event generated each credit;
- which module, referral, provider job, or accepted outcome caused it;
- for Pylon operators, the job, artifact, accepted outcome, payout class, and
  settlement receipt state;
- current reserve/withdrawal limitations;
- public-safe aggregate stats; and
- withdrawal controls when eligible.

### Referrer UX

Referrers should see:

- their invite link or code;
- direct referrals;
- optional upstream referral count;
- paid usage volume, not just signups;
- credits or sats earned by source;
- caps and expiration windows; and
- anti-abuse/compliance holds.

## Limitations And Guardrails

- Credit card top-ups may require additional verification.
- Card-funded or credit-funded balances must not automatically become
  Bitcoin/Lightning withdrawals.
- Card-funded value needs chargeback, refund, fraud, and dispute reserves.
- Credit purchases should not be refunded unless an explicit refund policy
  says otherwise.
- Withdrawals may be disabled or delayed on a per-user basis for compliance,
  sanctions, fraud, abuse, tax, chargeback, or operator-review reasons.
- Promotional credits should not generate withdrawable revenue share.
- Pylon accepted-work payments require an accepted outcome and a registered
  LDK-compatible payout target.
- Online presence, liveness, or availability by itself should not create a
  provider revenue-share claim unless a separate explicit stipend or starter
  subsidy policy says so.
- MDK checkout and wallet primitives should not be represented as the payout
  authority for current Pylon accepted-work payments.
- Referral rewards should trigger from paid usage, retained revenue, accepted
  outcomes, or accepted contribution value, not account creation alone.
- Multi-layer referral should stop at two layers by default.
- Public dashboards must show aggregate/anonymized data only.
- Public claims must distinguish pending credit, settled credit, dispatched
  payout, failed settlement, and modeled revenue.
- No raw wallet secrets, Pylon wallet recovery material, raw LDK entropy,
  preimages, MDK credentials, card data, provider grants, private buyer data,
  or private workroom payloads should enter public projections.

This is not legal, tax, or compliance advice. The implementation needs a real
compliance review before broad public Bitcoin withdrawals, card-funded reward
programs, or multi-jurisdiction contributor payouts.

## Transparency

The transparency goal should match the old Flow of Funds principle:

- payment and revshare code remains open source where it can safely be public;
- split definitions are versioned and inspectable;
- aggregate payment data appears on a public accounting dashboard;
- private user, company, wallet, and workroom details are redacted;
- public proof pages use claim states; and
- every strong payout claim links to a receipt or settlement state.

Useful public aggregates:

- total paid usage;
- total credits spent;
- total sats paid;
- total contributor credits issued;
- total contributor sats credited;
- total sats settled;
- total pending/reserved;
- top capability categories by accepted revenue;
- referral volume by tier in aggregate;
- Pylon provider accepted-work funnel;
- Pylon payouts by class, such as accepted work, availability stipend, beta
  bonus, weak-device lane, or strong lane; and
- OpenAgents platform share.

## Implementation Order

### Phase 0: Documentation And Policy Shape

- Import the Vida referral research doc.
- Adopt the asset rule: Bitcoin revenue creates Bitcoin revshare, credit spend
  creates credit revshare.
- Add public-safe terms for pending, credited, withdrawable, dispatched, and
  settled states.

### Phase 1: Referral Attribution For Current OpenAgents product surface

- Add referral invite and attribution tables.
- Capture referral codes at signup, order submission, and billing checkout
  start.
- Show operator-visible attribution on users, orders, and billing accounts.
- Do not pay on signup alone.

### Phase 2: Credit Revshare From Existing Usage Debits

- Add `revshare_events` and `revshare_ledger_entries`.
- Trigger referrer credit-share from eligible usage debit entries.
- Credit referrers in OpenAgents credits.
- Add dashboard summary for earned credits.
- Keep withdrawals disabled for credit-sourced revshare.

### Phase 3: Sites And MDK Direct Sats Revshare

- When Site MDK checkout and L402 paid actions are live, attach split
  definitions to Site products and paid actions.
- On reconciled sats payment and accepted entitlement/action, create sats
  revshare entries.
- Add withdrawal request flow only after settlement, reserve, and compliance
  checks are implemented.

### Phase 4: Pylon Provider Settlement Bridge

- Add OpenAgents product surface-side provider work event records that can reference Pylon accepted
  outcomes without replacing Nexus/Treasury/LDK as payout authority.
- Import or link accepted outcome ids, Pylon provider identities, payout
  target hashes, Nexus operation ids, Treasury operation ids, payout classes,
  and LDK settlement states.
- Add split inputs for bare compute cost, expensive workload premiums,
  validator work, and accepted-outcome upside.
- Preserve the rule that sats-funded accepted work can create sats provider
  payout, while credit-funded compute creates credits or internal payable
  until an explicit conversion policy exists.
- Expose public-safe aggregate Pylon payout and accepted-work funnel metrics.

### Phase 5: Agent Module And Signature Marketplace

- Attach split definitions to agent modules, signature tools, module versions,
  source packages, providers, graders, and UI/runtime adapters.
- Calculate splits from accepted Program Runs and accepted outcomes.
- Add contributor dashboards for selection, acceptance, earnings, failures,
  and deprecation.

### Phase 6: Provider And Accepted-Outcome Settlement

- Route provider payout truth through Pylon/Nexus/Treasury/LDK receipts.
- Keep internal balances fast, but public settlement claims receipt-backed.
- Add aggregate public accounting dashboards with redaction and claim states.

## Bottom Line

The OpenAgents revenue-share system should start with the simplest useful
slice: referral attribution tied to real paid usage and paid in the same asset
the user spent. That lets OpenAgents product surface connect the current credit ledger to referral
value immediately, while avoiding the mistake of turning card-funded credits
or promo grants into instant Bitcoin liabilities.

The long-term system is bigger: every accepted agent module, Site action,
signature, data package, model, Pylon provider, educator, and referrer can earn
when it contributes to paid accepted value. Pylon compute-provider payments are
not an afterthought; they are the current concrete proof that machines can go
online, do accepted work, and receive sats through LDK-backed settlement. The
invariant is the same across all of it:

```text
Paid usage or accepted outcome creates the event.
Versioned split definitions decide who earns.
Ledgers update quickly.
Bitcoin settlement claims require Bitcoin revenue and settlement receipts.
```
