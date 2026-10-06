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
changed amount, fee, or outcome refuses. Missing parties, late resolution,
unavailable history, uncertain custody, and unsupported rails require manual
reconciliation while liabilities remain retained. The custodian bears custody
and insolvency exposure; the worker bears postacceptance credit risk without
custody. Neither a relay nor the checker acts as custodian or resolver.

Run `cargo run -p pay-ledger --example custody-qualification`. It emits a
fake deposit, unknown release, serialized restart, lookup, remainder refund,
and conservation report. The fake model uses no wallet or production ledger.
Its outcome evidence cannot qualify a custody provider or prove a live refund.

