# Advisory clause-review results

This is an independent read-only summary of the completed post-panel review.
The original native panel, patches, checks, and scores are unchanged. All raw
values, full label probabilities, source catalogs, and identity checks are in
[analysis.json](analysis.json).

## Coverage and accounting

- **12 candidates retained; 11 called; one mandatory-overflow skip.** Beta A2
  (slot 9) required 148,403 serialized bytes before optional evidence, exceeding
  the 131,072-byte request limit. Its mandatory changed source alone was 110,657
  bytes. No prefix or reduced mandatory request was sent, and no call occurred.
- **116 questions and 116 answers:** 50 beta clauses and 66 gamma clauses. All 11
  requests returned valid typed answers. Sent requests ranged 118,315–129,776
  bytes, so these were substantial full-source reviews, not the native 16 KiB packs.
- **$0.014120610** gateway-reported total, complete accounting. Usage reported
  336,205 input and 7,428 output tokens. This is reported review usage; no native
  inference, setup, engineering, or machine cost is added here.
- Preparation took **3.257710s**, execution batch **8.037672s**, sum **11.295381s**.
  The sum excludes the intentional pause between those phases and execution's
  initial registration/evidence validation and exclusive claim creation.
  Per-call median was **0.686159s**; range **0.631230–0.890705s**. Summed call wall
  time was 7.990539s; the outer batch also includes validation and durable writes.
- There were **11 application calls and 12 provider attempts**. Slot 5 had an
  internal DigitalOcean 503 followed by a TypeSafe 200 within its single gateway
  request. Final providers were TypeSafe 10 and DigitalOcean 1. The returned model
  was `typesafe-ai/jev` throughout, an unversioned alias. There was no application
  retry, replacement, unknown charge, or stopping-limit violation.

## Bindings verified

No mismatch was found. The analysis checked the registration/execution digest, original
plan and ended panel, all 78 bound evidence files, frozen preparer/protocol/tests,
five imported modules, and runner/test identities. Each exact sent request
matches the registered bytes, preparer digest and gateway receipt. Every
response and receipt digest matches the retained artifacts. All 116 answer IDs
and labels match the original questions.

The analysis also verified 60 complete included source/contract bodies: 28 candidate bodies
match both candidate-manifest postimages and exact payload bytes; 32 unchanged
or contract bodies match immutable Git blobs. This checks provenance and whole
file inclusion, not Rust semantics or exhaustive dependency coverage. No candidate code or tests were executed while computing this analysis; the
separate diagnostic receipts below record earlier executions.

## Labels

| Task | Answered clauses | Demonstrated | Missing handling | Insufficient evidence | Non-code requirement |
|---|---:|---:|---:|---:|---:|
| Beta, five called candidates |50|33|0|7|10|
| Gamma, six called candidates |66|36|0|13|17|
| Total |116|69|0|20|27|

**No clause received `missing_handling`.** Abstention should remain distinct
from a concrete defect flag. Every beta review abstained on the scope/test-run
process clause r09, which the source-only request cannot establish. The two
called beta C reviews also abstained on fixture/archive compatibility r07.
The later beta diagnostic below is separate from primary acceptance; a
primary-check pass is not a correctness label.

## Six known gamma failures

All six native gamma candidates failed the independent primary check. Every
one nevertheless received `demonstrated` on both central validation clauses:

- **r02:** finite ranges, probability mass tolerance, selected identities and
  score legend bounds before typed decisions.
- **r03:** required coverage, answer types and option identities at the typed
  request-aware boundary.

| Original slot/arm/repetition | r02 label probability | r03 label probability |
|---|---:|---:|
|4 / C /1|0.88|0.91|
|5 / A /1|0.89|0.98|
|6 / B /1|0.92|0.87|
|10 / A /2|0.93|0.95|
|11 / B /2|0.89|0.80|
|12 / C /2|0.91|0.97|

These are the returned probability for the selected label, not a calibrated
probability that the program is correct. There were no independently frozen
per-clause truth labels, so these observations do not define a 12-example
accuracy score. They do establish that this review emitted no concrete
validation-gap warning on these six known failing candidates.

Unlike the earlier native span packs, **every gamma review included complete
current `answers.rs` and `client.rs`, plus the complete pinned score contract**.
The lack of caller bodies in the native pack therefore does not explain away
this review's positive r02/r03 judgments. Complete files still do not establish
that the model followed every validation path or interpreted the contract
correctly. No specific semantic failure is inferred from labels alone.

All six reviews selected `insufficient_evidence` for r05 (async/blocking test
coverage) and r06 (read skill and preserve the selected-answer/score contract).
Every gamma case omitted at least one changed test file for the optional byte
budget; all omitted `tests/validation.rs`. This makes the r05 abstentions
plausible evidence-limit responses, not demonstrations that the implementation
failure was found. Some other unchanged dependency files were omitted too.

The complete score contract was included. The TypeSafe skill named in r06 was
an `applicable_instruction_path`, rather than a `required_public_reading`, and
this frozen review policy did not include instruction files. Its absence is a
real limitation for that compound clause. It was outside the source catalog,
so the catalog does not list it as a byte-budget omission. The native coding
arms did receive their common instructions; this limitation concerns only the
later review's evidence.

## Separate post hoc beta evidence

After review responses were fixed, the additional executable beta diagnostic
completed on both controls and all six beta candidates. The analysis verified the eight
checks receipts and eight test-output hashes and joined candidate identities
to the original review. The historical reference passed; the unmodified base
failed. Compilation succeeded for all eight variants.

- Slots 1 (A1), 7 (B2), 8 (C2) failed the form-feed interior-record case.
- Slot 3 (C1) failed the step-before-session case.
- Slots 2 (B1) and 9 (A2) passed this additional diagnostic. Slot 9 was the review's
  mandatory-overflow skip, so it has no semantic answer.

All four newly failing beta candidates were reviewed; each received
`demonstrated` on lifecycle clause r04 and strict-ingestion clause r06, with
no `missing_handling` answer anywhere. Combined with the six primary gamma
failures, ten reviewed candidates now have executable failure evidence and
none received a concrete missing-handling flag. This is a descriptive
post hoc result, not a preregistered accuracy estimate. The original primary
scores remain unchanged, the two passing beta candidates are not proven
correct in general, and these later checks add no review-call charge.

## Bounded conclusion

The mechanics worked: immutable full-file evidence, exact clauses, valid typed
responses, retained skips, known cost and short latency. The useful semantic
result is narrower: the reviewer expressed uncertainty about missing evidence
and process requirements, yet gave positive implementation judgments to the
known failing gamma outputs. This does not support using its labels as an
acceptance gate or a demonstrated repair trigger. Do not count generic
abstentions as six successful defect detections, or label beta uncertainty as
false positives merely because beta passed the primary check. No questions,
selection policy, patches, or thresholds were changed after this review.

## Reproduce and inspect

The [frozen review protocol](../../../../../bench/jev-lifecycle/clause-coverage/protocol.md)
and [runner guide](../../../../../bench/jev-lifecycle/clause-coverage/RUNNER.md)
define preparation, immutable registration, and single-use execution. The
[registration](registration.json) binds all 12 slots before calls.
[Execution](execution.json) records the completed batch; `prepared/NN/` holds
the complete evidence and omissions, and `calls/NN/gateway/` holds exact
requests, responses, and receipts. The retained `live.claim` forbids rerunning
this batch. An offline rerun of preparation requires a new output directory;
it does not require inference.

The [native comparison](../README.md) and [post hoc trace diagnostic](../posthoc-beta/README.md)
supply the executable outcomes used only after the answers were fixed. Neither
checker contents nor outcome labels were sent to Jev. The original six SDK
failures were already known to the coordinator when the review was designed;
this is a development experiment on exposed candidates.
