# Monetary ledger

`tenancy::money` implements the local accounting foundation for #9491. It is
separate from resource quota and does not enable paid inference, collect a
payment, or choose launch prices. `gateway`'s monetary admission calls it —
`monetary-accounting.md` covers that opt-in — and the `tenant-money` binary
is the operator's account view. No HTTP balance route or customer-facing
usage API exists yet; that surface is #9493's.

## Prices and balances

Amounts are unsigned integer millionths of an explicitly named currency unit.
A price version binds a model, capacity, policy, currency, and rational rates.
The price rounds each resource up once per attempt to the nearest millionth.
All arithmetic is checked; overflow refuses the operation.

Billable resources are input, cached input, output, and reasoning tokens, plus
compute milliseconds. Input excludes cached input, and output excludes
reasoning. A provider adapter must resolve overlapping counters before using
these units. Usage must name every priced resource explicitly, even when its
count is zero. Missing usage is unknown, not zero. A zero retail rate is valid
and makes no claim about provider or hosting cost.

An account belongs to a stable workspace, not an API key. It has one currency,
an explicit top-up permission, and a lifetime net spending ceiling. Credits do
not increase that ceiling. The balance reports credited, reserved, settled,
refunded, available, and remaining authorized spend. Refunds restore available
credit and net spend headroom; reversing a refund consumes them again. No
period rollover or automatic ceiling increase is implemented.

## Reserve and settle

Every privileged mutation names the workspace, an idempotency source, and an
audit reference. Repeating the identical mutation is inert; changing any field
under its existing workspace/source pair is a conflict. Account authorization
belongs to the caller of this library. Arbitrary inference callers must not
receive authority to credit, release, or refund accounts.

1. Create the workspace account and explicitly credit a grant, top-up, or
   adjustment. A top-up requires the account's opt-in.
2. Reserve the worst-case usage under a complete price before dispatch. The
   hold binds an attempt, request digest, price terms, and maximum units. Every
   retry, reviewer, and fallback attempt needs its own hold. Reusing the
   attempt with another mutation source refuses; retry the original source.
3. Settle known usage with a receipt reference. Usage must fit each authorized
   resource bound. The retail charge follows the pinned price; provider-reported
   and allocated hosting costs remain separate optional amounts in the same
   currency. `None` means unknown, including after retail settlement.
4. Mark unknown completion explicitly and keep its full reservation. An
   oversized or incomplete settlement also leaves the hold unchanged. Reconcile
   with a later known settlement or an explicitly evidenced release.

Release requires evidence that no retail charge is due. A timeout alone is not
that evidence. The library cannot validate an external receipt's truth; the
integration must verify it. Refused or failed attempts that consumed known
billable resources settle those resources. Work known not to have dispatched
can release its reservation. A policy that charges differently requires explicit
terms and an authorized integration; the ledger does not infer that policy from
an executor's success claim.

Refunds refer to a settled attempt and cannot exceed its retail charge. Refund
reversals cannot exceed the recorded refund. A debit correction cannot remove
credit already committed to holds or settled charges. Failed operations do not
partially mutate the in-memory or durable balance.

## Durability and recovery

Open the ledger in a protected host directory outside all executor write grants.
It requires a private regular file and holds an operating-system file lock for
its entire lifetime. A second writer refuses. Each accepted mutation appends a
versioned, digest-linked record and syncs the file before returning, so admission
can wait for durable funding before dispatch.

Reopening replays the log and treats remaining holds as unknown completion,
without releasing funds or redispatching work. A partial tail, invalid chain,
unsupported schema, or invalid transition refuses recovery. The ledger never
silently truncates a damaged file. A write failure poisons the current instance;
stop dispatch and reconcile the file before reopening.

This is a bounded, single-host implementation: the log limit is 16 MiB and
admission refuses at that bound. Compaction, multi-host database transactions,
payment reconciliation, and external commitment publication are not implemented.
The hash chain detects altered linked records; it is not an external commitment
that proves a privileged operator did not replace or truncate the entire log.

## The account view

The balance an account API serves is `Balance`: the account's currency,
credited, reserved, settled, refunded, available, and remaining authorized
spend, plus `price_versions` — every price version the account has
transacted under, so a caller can name the terms a charge was quoted under
rather than inferring them from a current price list. `Ledger::workspaces`
enumerates accounts and `Ledger::holds` lists a workspace's holds with
their phases, so an outstanding liability reads as outstanding.

`tenant-money --ledger PATH [--workspace NAME]` is the operator's read of
the same state: one balance line per workspace and one line per hold —
phase, reserved, retail, refunded, provider and hosting cost, price
version, and receipt — in the ledger's own millionths, unscaled.

## Verification and remaining integration

Synthetic tests exercise fractional rounding, overflow, missing usage, repeated
and changed-content mutations, overlapping holds, spend limits, refunds and
reversals, recovery, writer exclusion, workspace separation, price-version
reporting, and damaged logs. They use no launch price and make no live
payment or model call.

Remaining #9491 work includes authorized customer-facing account APIs
(#9493), authenticated price publication, provider counter mapping,
integration with quota and execution receipts, and an explicit commercial
launch-price decision. Gateway reservation and settlement are landed in
`gateway::money` under an explicit operator opt-in.
