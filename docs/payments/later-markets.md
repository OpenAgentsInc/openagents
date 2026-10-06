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
