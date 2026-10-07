# Pilot comparison evidence

`gym sales-evidence` rebuilds a private comparison from a frozen manifest and
retained ATIF logs. It runs no engine, contacts nobody, and chooses no deployed
router. Use it for REV-03 and the [first workflow offer](README.md#first-workflow-offer-v1).

```sh
cargo run -p gym --bin gym -- sales-evidence \
  --root /absolute/private/pilot-snapshots \
  --manifest /absolute/private/comparison.json \
  --output /absolute/private/report.json
```

The [typed manifest](../../crates/gym/src/sales_evidence.rs) uses schema
`openagents.gym.sales-evidence.v1`. Every reference contains a root-relative
path and the lowercase SHA-256 of its exact bytes. Source paths cannot escape
the explicit root or use symlinks. Inputs have size and count limits; missing,
changed, oversized, or conflicting evidence refuses rebuilding rather than
silently dropping an attempt. No home directory is discovered automatically.

Record the offer version, source commit, baseline and candidate methods,
retrospective-selection flag, and frozen task/check digests. Retain a separately
frozen `openagents.gym.sales-inventory.v1` artifact whose `attempts` map names
`TASK_ID/baseline` and `TASK_ID/candidate` and lists every attempt in order.
Record failed, repair, and retry attempts too; each repair/retry names its
parent on the same side. A manifest that omits or reorders an inventory entry
refuses. Freezing that inventory and documenting its completeness is a human
responsibility; a digest cannot prove that an unrecorded attempt never happened.

For each attempt, retain its ATIF log, output artifact, every frozen check's
status and command digest, and evidence for known check outcomes. Declare setup,
queue, independent-check, and support milliseconds separately. Execution time
comes from the log's recorded whole-second duration. Retained compute metering
needs its own milliseconds and evidence; absent metering stays unknown. Reported
step token counts and missing generation usage stay separate from cash cost.
A partial log stays labeled
incomplete and cannot establish an accepted result. A session cannot count
as two attempts. Model changes and provider identity remain in the private report.

Acceptance binds the exact candidate digest, a checker distinct from the
executor, a retained independent-check review, and the customer's retained
acceptance decision. Every frozen check must pass. These are attributable
records, not remote attestation: the reviewer verifies their truth and input
rights privately. An optional `gym_store` reference verifies the existing Gym
row chain without creating another trace or result-store format.

Each provider, compute, and support cost is either billed, a list-price
estimate, subscription capacity, or unknown. Known amounts need retained
provenance; estimates also need a pinned price version and source artifact.
Amounts are checked integers in an explicit denomination and scale. Missing
components remain unknown. Subscription capacity uses
`subscription_capacity_units`; it cannot become cash. Totals keep denominations
and bases separate, show known subtotals and unknown item counts, and never
infer monetary savings. Retain declared skipped evidence as limitations.

To prepare shareable output, review the exact private report and create a
private `openagents.gym.sales-review.v1` record with `report_digest` equal to
its SHA-256, a named `reviewer`, and explicit `approved: true`:

```sh
cargo run -p gym --bin gym -- sales-evidence \
  --report /absolute/private/report.json \
  --review /absolute/private/review.json \
  --output /absolute/private/shareable.json
```

Both commands create exclusive mode-0600 files, sync them, and refuse overwrite.
Changing the report requires a new review. The shareable projection contains
aggregate counts, separate timings/costs, and generated limitations. Customer
text, IDs, source paths, revisions, artifacts, acceptance references, reviewer
identity, and provider/model names remain private. Unrecognized denomination
names and their amounts are withheld. Review customer rights and aggregates
before publishing; this command publishes nothing. Retrospective selection
stays labeled, and neither projection claims deployed routing improvement.

Synthetic tests cover a failed attempt followed by an accepted repair,
rebuilding, complete totals, unknown costs, independent acceptance, inventory
omission, changed evidence/checks, partial traces, duplicate sessions,
subscription units, overflow, reviewed projection, and private output. Real
baseline permission, billed evidence, acceptance, and publication review remain
owner/customer steps under O1/O8 in [NEEDS_OWNER.md](../../NEEDS_OWNER.md).

## Operating revenue and full delivery cost

`gym sales-finance --root DIR --manifest FILE --output FILE` rebuilds the
private `openagents.gym.sales-finance.v1` operating report. Its
[typed manifest](../../crates/gym/src/sales_finance.rs) pins a period, customer,
offer version, cohort, complete financial inventory, and declared assumptions.
It verifies REV-03 comparisons again and includes failed and repair attempts;
the allocation record names the payer and whether baseline costs also belong
to this engagement. The explicit source root must be private. Paths, files,
entries, references, and output have bounds; symlinks and conflicting digests
refuse. No home directory, live bill, or live payment is discovered.

The settlement adapter reads a checkpointed copy of the existing receiver
ledger through its read-only owner. A retained attribution binds the exact
snapshot and settlement to the customer and offer. The commercial adapter
reads retained funding, agreement, and trial attestations; generic service
attestations refuse. Their amounts, account, offer, time, and price-term digest
must agree. These are attributable input records, not remote attestations.
One payment hash or prepaid debit ID can occur only once in a report, even
when later ledger snapshots or different offer attestations name it.
Commercial collections record gross invoice/funding collection; processing
fees use retained expense rows. Outstanding claims reflect the ledger
snapshot's book state; a sent payout label alone does not attest wallet movement.

The `service_sale` source requires REV-18's [private service export](README.md#invoiced-services).
It verifies the frozen scope, customer and support acknowledgments, exact
invoice terms, owner-verified payment history, and optional fulfillment
obligation. It reconstructs REV-03 again and compares the exact candidate,
independent check, customer evidence, task digest, and retained comparison
report. Before verified collection, invoice-only, pending, and unknown records
earn nothing. Later unresolved states retain previously verified historical
amounts and prevent a profitability verdict. Collected,
accepted service uses declared USD scale-100 cents, checked into
`USD_millionths`; no currency exchange is inferred. The period must contain
the invoice's full retained payment and fulfillment history. One sale, invoice,
or external payment cannot count twice across exports or offer entries.
Refunds and restorations come from that history, not separate manual adjustment
rows. A separate fulfillment bill reduces contribution once; its payment clears
the obligation without charging the expense again. An unbilled triggered
obligation or unresolved payment prevents a profitability verdict.

Gross external collections, net wallet collections, consumed prepaid balance,
unspent funding, contractual and prospective agreed charges, unearned charges,
original allocations, remaining author/resource liabilities, OpenAgents earnings,
refunds, refund reversals, losses, and incentives stay separate. Funding,
agreements, and free trials earn nothing. Accepted charges require the exact
REV-03 customer/independent acceptance. A contract-billable failure requires
explicit retained terms and failure evidence; pending/unaccepted work earns
nothing. Funding refunds never become earned-sale refunds.

Billed provider/compute/support costs come from all selected attempts, with
missing components retained as unknown. Incident records bind failed setup,
replacement, repair, onboarding, or support work to an expense or explicit
unknown cost. Additional bills require unique retained line/allocation evidence.
Missing cost classes require a retained no-cost declaration; silence is not
zero. Estimates pin price terms. Bills, estimates, grants, subscription
capacity, and customer-paid expense remain inspectable separately. Amounts in
`msat` and `USD_millionths` never imply an exchange rate.

Contribution subtracts billed OpenAgents expense, refunds/losses, and funded
first-call awards once. The recorded share already excludes author/resource
shares, launch incentives, and inbound fees. Incomplete inventories, declared
gaps, unknown expense, noncash/estimated costs, or incompatible cash units
prevent an unqualified profitability flag. Commission enrichment is explicitly
unavailable; measuring this lane does not depend on launching referrals.

To export a scrubbed cohort aggregate, use `gym sales-finance --report FILE
--review FILE --output FILE` with an `openagents.gym.sales-finance-review.v1`
record containing the exact report SHA-256, its recorded `owner`, and
`approved: true`. Changed bytes, another reviewer, or absent approval refuse.
Exports omit customer/offer/cohort identities, source paths/digests, terms,
individual incidents, and support humans. Outputs are exclusive mode-0600
files. The command publishes nothing; the owner reviews aggregate disclosure
rights and genuine source coverage under O1/O5/O8 before any margin claim.
