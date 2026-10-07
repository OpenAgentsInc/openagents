# Monetary ledger

`tenancy::money` implements the local accounting foundation for #9491. It is
separate from resource quota and does not enable paid inference, collect a
payment, or choose launch prices. `gateway`'s
[monetary admission](monetary-accounting.md) calls it, and `tenant-money`
is the operator's account view and audited mutation path. Authorized balance
and [usage reads](usage-dashboard.md) serve the caller's workspace; they
grant no funding mutation authority.

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
refunded, available, and remaining authorized spend. Refunds restore original
credit and net spend headroom; expiry and use restrictions still govern its
availability. Reversing a refund consumes them again. No period rollover or
automatic ceiling increase is implemented.

## Reserve and settle

Every privileged mutation names the workspace, an idempotency source, and an
audit reference. Repeating the identical mutation is inert; changing any field
under its existing workspace/source pair is a conflict. Account authorization
belongs to the caller of this library. Arbitrary inference callers must not
receive authority to credit, release, or refund accounts.

1. Create the workspace account and explicitly credit a legacy grant, top-up,
   or adjustment, or install the funding policy below and record its funding
   or promotions. Purchased funding requires the account's top-up opt-in.
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
automatic provider reconciliation and external commitment publication are not
implemented. The hash chain detects altered linked records; it is not an
external commitment that proves a privileged operator did not replace or
truncate the entire log.

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
`--json` emits the complete statement for each selected workspace as NDJSON.
`--apply MUTATION.json` applies one bounded, audited `Mutation` through the
same durable ledger before reading it; exact source retries are inert. This
is a privileged local operator command, not an inference-caller route. Keep
the ledger and mutation files outside every executor's write grant.

## Pinned funding and promotional credit

[REV-21](https://github.com/OpenAgentsInc/openagents/issues/10827) adds
[`money::funding`](../../../crates/tenancy/src/money/funding.rs) to this
same ledger. `FundingPolicy` installs `openagents.money.funding-policy.v1`
on an empty workspace account. Existing credited accounts retain their
original contract; installation refuses to reinterpret their grants.
After installation, unscoped `Credit` and `Debit` refuse. `Reserve` always
allocates eligible credit lots, and the gateway checks
`Ledger::balance_for_price` before dispatch. This policy is opt-in; it
does not enable a new billing provider or change retail's fixed units.

| Unit | Integer scale |
| --- | --- |
| Gateway account and usage price | 1,000,000 units per named currency unit |
| Retail displayed credit and price | One sat per credit; the compute ledger stores 1,000 millisatoshis per sat |
| Paid-call ledger and plugin fee | Millisatoshis, 1,000 per sat |

`funding::Unit` admits sats, millisatoshis, and named currency millionths.
Same-currency scale conversion must preserve value exactly. Cross-currency
conversion requires an explicit configured rational rate, version, source
reference, validity interval, precision rule, and source-unit fee payer/cap.
No conversion is inferred, fetched, or supplied by default. Funding rounds
down with its exact uncredited remainder retained, or refuses any dust under
an exact-precision policy. A zero-credit result or overflow refuses. Usage
still rounds up under its independently pinned `Price`.

`BeginFunding` pins a conversion quote and starts pending, with no available
credit. A verified `FundingFinality` event credits it once only after the
policy's required finality. Unknown or insufficient finality stays pending;
the adapter must verify the external evidence. Payment identity is unique
across workspaces, and source retries keep the original terms. Later policy
versions cannot reprice that purchase or an existing hold.

Purchased and promotional lots retain their origin and policy digest.
`Promotion` enforces a lifetime workspace issuance cap, per-grant cap,
expiry, permitted price policies, admission allowance, and reversal policy.
The allocator uses eligible promotions first. Release and refund do not
reset the admission allowance or extend expiry. Expired, exhausted, and
differently scoped credit is excluded from the applicable spendable view.
Statements retain each lot's restrictions; the dashboard shows funding,
expired, and restricted totals and explains product eligibility.
Promotional usage is non-commissionable. A hold's `commissionable_charge`
classifies purchased-funded net usage only; delivery, funding reversals,
and a separate commission rule still determine an actual award.

`ReverseFunding` records a verified external refund or dispute under the
purchase's original policy and rate. It converts cumulative source reversals
so splitting an event cannot change rounding; customer-paid fees are outside
the reversible convertible amount. `ReversePromotion` requires the grant's
pinned permission. Reversal removes that lot's remaining credit without
discarding existing holds or charging another customer. Already spent losses
belong to the operator; uncovered holds remain explicit operator risk until
reconciliation. A usage `Refund` restores its original purchased/promotional
allocation, subject to those same expiry and use rules. Neither operation
sends money, redeems a balance, nor authorizes a second charge.

`Ledger::statement` names the workspace and accounting snapshot time, and
joins the balance, immutable policy versions, pending and final funding,
grant and hold positions, and source/audit events for funding, promotion
expense, usage, releases, and refunds. Wallet liquidity is explicitly
unobserved: a funding credit or ledger balance never stands in for a wallet
read. For policy accounts, gross credit plus operator loss and uncovered
holds equals net usage plus reservations, reversed credit, expired credit,
restricted credit, and available credit. All amounts remain exact integers.

New mutations use the `openagents.money.v2` chain with a recorded host time;
existing v1 entries replay under their original digest and report an unknown
event time. A backward clock refuses new mutations, and expired funds are
not restored on restart. Real rate sources, fees, finality, and commercial
terms remain
[O1/O5 owner activation](../../../NEEDS_OWNER.md#product-funding-policy-o1o5-rev-21-10827).
The USD service invoice in [Coder pilot v1](../../sales/README.md#first-workflow-offer-v1)
stays outside product balances and provides no promotional credit.

## Verification and remaining integration

Synthetic tests exercise fractional rounding, overflow, missing usage, repeated
and changed-content mutations, overlapping holds, spend limits, refunds and
reversals, recovery, writer exclusion, workspace separation, price-version
reporting, and damaged logs. They use no launch price and make no live
payment or model call.
Funding fixtures also exercise exact sats/msat scales, dust, fee caps,
unavailable conversion, pending finality, cross-workspace payment replay,
normal reservation enforcement, exhausted/expired trials, partial refunds,
pinned rates after policy changes, spent-credit loss, and v1/v2 recovery.
The operator CLI fixture repeats funding, admission, and release across real
scratch processes; it never reads an owner's home or moves money.

Gateway reservation and settlement, member-scoped balance and usage reads,
and quota/receipt joins are implemented under their explicit operator opt-ins.
Converted funding still needs an admitted payment adapter and the O1/O5
commercial decision; synthetic policy tests do not activate that offer.
