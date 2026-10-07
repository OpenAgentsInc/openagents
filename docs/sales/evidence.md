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
