# Later market qualification profiles

These optional profiles define admission and qualification work. They do not
advertise a paid worker, auction, escrow, training market, or contributor reward.
The typed contracts live in `pay_ledger::markets`. They grant no command,
disclosure, wallet, or custody authority.

## Independently operated workers

`openagents.independent-worker-qualification.v1` pins the buyer and provider,
market and order, LAB terms, source, protected checker, disclosure, execution
requirements, cancellation policy, delivery rights, capacity, fixed price, fees,
rework count, and ordered deadlines. The existing MKT quote and bilateral order
confirm those exact terms; LAB binding, delivery, verification, review, and
acceptance retain separate identities. Host admission must verify the current
execution grant, disclosure, capacity, operator independence, and payout
destination. A provider's self-description cannot supply that admission.

The earned obligation key binds buyer, market, order, and provider. Invoice
replacement, retries, relay replacement, and process recovery preserve it.
Acceptance must bind the exact LAB terms and fixed amount; a passing checker
alone is insufficient. A replaced relay supplies transport, not a new grant,
claim fence, order, or obligation. Restore retained signed records and resolve
uncertain execution before another dispatch. Recovery requires the provider's
current grant and destination, not an expired discovery announcement.

Worker compensation is distinct from plugin author royalties, computer rental,
data licensing, and XP. The existing central ledger owns funded settlement,
payable shares, destination resolution, batching, and payout reconciliation;
no workbench ledger is proposed. `record_worker_earned` accepts an adapter-verified
funding receipt and a caller-verified signed LAB closure. It accrues the worker
in the existing `provider` share role, retains the platform fee, disables plugin
launch bonuses, and refuses changed terms or another payment hash for the same
obligation. It does not verify invoices, signed closures, or move funds itself. The fixed-postacceptance Lightning profile
leaves credit risk with the worker and provides no custody guarantee. Fee
limits require an enforcing wallet adapter. Missing acceptance history,
conflicting final records, or unknown payment outcomes suspend settlement.

The explicitly admitted `coder_labor::paid` adapter now joins canonical partner
custody, fresh signed provider policy, real bounded provider and protected buyer
checker results, signed acceptance, and exact authenticated resident-wallet
funding. It reuses `record_worker_earned` with zero added platform fee and no
escrow. Its scope has zero executable revisions, retains support/cancellation
and unknown cost outcomes, and preserves accepted obligations after cancellation.
Use [`openagents labor`](../coder/runtime/free-labor.md#explicit-paid-fulfillment)
with explicit private pipeline, authority evidence, resident wallet home, and
central ledger sources. The Rust scratch fixture covers actual process and relay
recovery with fake funds; it does not qualify independent commercial operation.

Run `scripts/qualification/later-worker.sh NEW_OUTPUT_DIRECTORY` with the
pinned toolchain and the reused `CARGO_TARGET_DIR`. It retains the existing
scratch buyer/provider order, duplicate execution, reconstruction after relay
shutdown, current-admission checks, and fake central payout fault tests. Its
operator is shared, source is synthetic, and total CPU/storage/energy cost is
unmetered. It does not qualify independent commercial operation. The exact
independent and funded qualification remains in `NEEDS_OWNER.md`.

## Explicit negotiated bids

RFQs and quotes stay private to admitted recipients. `markets::bids` checks
exact capability, source, and disclosure compatibility; current quote expiry,
capacity, and bounded known costs; and a maximum of 64 offers. The frozen
comparison rule orders eligible quotes by all-in declared cost, then quote
digest. It retains every nonwinner. Readiness is current admission evidence,
not a guarantee that discovery remains available.

Comparison yields an inert suggestion. The buyer separately approves one
selection that pins the complete canonical bid fingerprint, order, admission,
payer, price, and fee limit. Changed terms, stale readiness, and accepting a
losing quote under that selection refuse. Bilateral confirmation and the
host's current grants still precede execution. No comparison or selection
calls a wallet or creates a payable share; losing offers create no obligation.

Run `cargo run -p pay-ledger --example bid-qualification` to retain the
synthetic no-spend comparison baseline. It reports measured comparison
latency separately from unmeasured provider quote latency and coordination
cost. It reports zero accepted orders, dispatches, and payments. An independent
provider study must retain RFQ/quote timestamps, buyer acceptance outcomes,
and incremental search, check, and coordination costs before funded adoption.
The baseline does not satisfy that independent study.

## Commercial custody and disputes

Production custody is unavailable. The current fixed Lightning profile pays
an earned obligation after acceptance and exposes neither an escrow account
nor a reversible payment. A commercial custody adapter needs a separately
reviewed legal custodian, supported deposit/release/refund rails, authenticated
wallet lookups, fee enforcement, insolvency treatment, and an admitted resolver.
Unilateral buyer or provider statements cannot authorize custody effects.

The fake `markets::custody` study pins parties, distinct resolver, custody
policy, milestone amounts, rework bound, acceptance/resolution deadlines,
deposit, and fee budget. Milestones plus the fee reserve equal the deposit.
Delivery and verification records do not release money without verified buyer
acceptance; a dispute release instead requires the admitted resolver's exact
resolution. Cancellation, rework, and failed checks do not restart deadlines
or expand funding. LAB retains those separate review and execution records.

A known failed refund leaves the liability held; an unknown release or refund
reserves principal and fees and blocks later effects. Exact attempts replay as
observations. Lookup may resolve the same retained attempt without resending;
changed principal or a fee above the reserved cap refuses. A verified final
lookup releases unused fee reservation; final outcomes and fees cannot change. Missing parties, late resolution,
unavailable history, uncertain custody, and unsupported rails require manual
reconciliation while liabilities remain retained. The custodian bears custody
and insolvency exposure; the worker bears postacceptance credit risk without
custody. Neither a relay nor the checker acts as custodian or resolver.

Run `cargo run -p pay-ledger --example custody-qualification`. It emits a
fake deposit, unknown release, serialized restart, lookup, remainder refund,
and conservation report. The fake model uses no wallet or production ledger.
Its outcome evidence cannot qualify a custody provider or prove a live refund.

## Training and evaluation work

The training profile pins corpus, dataset, baseline checkpoint, recipe, seed,
worker class, deliverable contract, protected checker, evaluation partition,
training source groups, provenance, and explicit train/evaluate/disclose/
redistribute rights. Missing rights refuse the corresponding action. The
caller must independently verify licensing and checker evidence; worker text
cannot grant rights or choose the evaluator, metric, or protected labels.

`tenancy::training` owns corpus partitions, provenance, recipes, trial history,
and sealed artifact identity. Gym owns suite and checker evidence. LAB owns
bounded execution, delivery, and acceptance. The market profile links those
owners and grants no training engine, production model deployment, or inference
purchase authority. A training group cannot overlap the protected evaluation
groups even if a record changes its task or partition label.

Compute charges, accepted improvement bounties, and data licenses have distinct
obligation identities. An accepted improvement must match a separately verified
checker receipt's exact artifact, terms, partition, acceptance, and score.
Training, failed attempts, checking, and search costs must all be known,
independently verified, and inside the frozen budget. Compute may still cost
money when the improvement is rejected; a sealed artifact does not imply a
bounty, serving admission, or rights to redistribute it.

The `training_profile` fixture writes one explicitly synthetic checkpoint in a
scratch worker directory and checks it against separately retained protected
bytes. It tests exact artifact/replay, changed recipes, rights, group leakage,
unknown costs, and forged improvement claims. It does not train a model or
qualify independent worker operation. Real training and funded qualification
remain unverified.

## Evidence-based contributor payments

The only proposed contribution class is `verified_optimization`. Its precommit
pins source and evaluation groups, license, attribution, beneficiary, protected
evaluator, policy, acceptance authority, funding authority, artifact contract,
reward, expiry, and obligation identity. Evaluation groups must differ from
source groups; the caller independently verifies their provenance and operator
relationships before trusting a protected result.

Accepted evidence must match the frozen terms and exact artifact, evaluation,
acceptance, and measured improvement records. The protected evaluator's result
is supplied outside contribution-controlled records. Tokens, activity counters,
and XP are absent from the schema and create no payment. Worker-job fees,
plugin per-call fees, data licenses, compute purchases, and optimization rewards
remain separate obligations, even when one task earns more than one class.

`record_contribution_earned` requires a separately verified acceptance and fully
funded central receive receipt. It reuses central author payable shares,
registered destinations, replay, unknown reservations, batching, and wallet
reconciliation. It uses no plugin ID and funds no plugin launch bonus. Changed
terms or another payment hash cannot duplicate the same obligation. Missing
rights, unavailable protected evidence, unfunded claims, or unknown payout
outcomes retain refusal or liability instead of inventing earnings.

`contribution-service` implements one private operator adapter: a frozen tenant
checkpoint whose Choice accuracy improves on protected Gym confirmation. It
rereads the native corpus, recipe, complete trial history, candidate seal,
actual adapter/head/tokenizer bytes, receipt chains, locked spend, transfer,
and deployment gates. Signed current permissions bind every source group,
license, and corpus item; admitted evaluator and acceptance keys bind the
recomputed result. Source files and funding state stay outside the worker root.
The adapter retains the original state, lock, journal, and central-ledger file
identities and refuses replacement before effects or after a receiver call.
Existing shared, foreign-owned, and hardlinked state files refuse admission.
Distinct keys and private paths provide local admission boundaries; they do
not attest that different people control the keys.

The adapter joins every required training, checking, search, failed-attempt,
license, and compute bill to the same obligation and the existing REV-25
customer evidence. Unknown costs refuse funding. It requests one signed exact
central invoice through the resident's atomic node-bound operation, looks up
only that receiver, requires the exact observed inbound transferred amount,
and rechecks current sources before calling `record_contribution_earned`. A changed resident refuses before dispatch. LDK
lookup omits invoice text and inbound routing fees; the retained signed invoice
remains authoritative, a returned conflicting invoice refuses, and absent fee
expense remains unknown in the finance view. The invoice request reserves the
resident's 60-second reply window inside the earliest rights, terms, or grant
expiry; the retained signed timestamp determines its effective expiry. Lost issuance or lookup
replies retain unknown state. Reconciliation refreshes the clock after lookup
and refuses expired current admission before central accrual. A receiver-observation digest is
durable before the central mutation. There is no automatic second invoice,
caller-supplied trust verdict, training engine, serving activation, or default reward, fee, or
production threshold.

Build the `contribution-service` binary under a build lease. Its private
`--config ABSOLUTE_FILE` uses [the versioned configuration](../../crates/contribution-service/src/types.rs)
with explicit protected, worker,
state, and existing central-ledger paths. `assess`, `prepare`, `reconcile`, and
`finance --output NEW_PRIVATE_DIRECTORY` also require `--wallet-home
ABSOLUTE_DIRECTORY` for the explicitly admitted central resident. `statement`
reads funding, liability, and exact payout attempts without opening a wallet;
unknown payout attempts remain outstanding. `finance` exports a checkpointed
read-only central snapshot and a versioned contribution view joined to the
existing Gym customer cost report. The cost report keeps its original delivery
classification; a protected checkpoint does not create coding-task acceptance.
The contribution view separates collected funding, platform allocation,
contributor allocation, outstanding liability, and known or unknown expenses.
Keep exports private; they contain customer evidence. Existing central
payout commands own sending and reconciliation. No host deploy is required for
this local operator adapter.

The protected synthetic tests use temporary roots, distinct test signers, and
a fake signed Lightning invoice. They establish local verification, exact
accounting, export, and restart behavior. They do not qualify useful real work,
a license, an independent operator, funded adoption, or a sent payout.

Run `scripts/qualification/later-markets.sh NEW_OUTPUT_DIRECTORY` to retain the
bounded no-spend and fake-payment studies. The contribution fixture is
synthetic and uses one operator; it proves the local funding/replay boundary,
not independently useful work or real sats paid to a contributor. Real
independent qualification and funded adoption remain unverified.

The committed [qualification evidence](fixtures/later-market-qualification.json)
retains the synthetic worker receipt, comparison timing, fake custody
conservation, protected training artifact, and fake funded contribution report.
Its explicit limits remain part of the evidence.
