# Measured records for a caller's labels

`gym report` renders the record a store of rows carries, and `gym verify`
walks the store's receipt chain without rendering anything. Together they
are the packaging half of scoring a caller's own data: the caller hands
over labelled examples, the examples become a digested suite, the suite is
evaluated like any other, and the record the caller gets back is one they
can check rather than trust.

## From a caller's file to a record

The flow is three commands, each one verifiable on its own:

1. Build the suite. `gym build` takes a JSONL file of the caller's
   records — `family`, `kind`, `state`, `truth`, and the `question` the
   item asks — and writes a suite and a question set under one name. The
   caller names the label source, the labelling rule, the data's origin,
   and the licence the measurement runs under:

   ```text
   cargo run -p gym --bin gym -- build \
       --input acme-records.jsonl \
       --name caller-acme-v1 \
       --label-source acme \
       --label-rule "support staff labelled each ticket at close" \
       --source "acme's ticket exports, 2026-09" \
       --licence "acme retains the labels; measurement use only" \
       --agreement routing=0.91
   ```

   `--agreement family=ceiling` is optional and repeatable: it records the
   agreement ceiling a family's labels rest on — inter-annotator
   agreement, or whatever bound the caller can honestly state — so the
   report can print it beside the family's scores. A ceiling is
   provenance, not an item, so it does not move the suite digest, and a
   family the suite does not hold cannot claim one.

2. Evaluate a door. `gym eval` asks the suite exactly as it asks the
   committed suites: same partitions, same gate, same refusal semantics.
   The rows land in a receipt-chained store:

   ```text
   cargo run -p gym --bin gym -- eval \
       --door kev-0.6b=http://127.0.0.1:11453 \
       --suite caller-acme-v1.json \
       --partition development \
       --record results/caller-acme-v1.jsonl
   ```

3. Render the record, declaring what the run was meant to ask and writing
   the commitment the caller keeps:

   ```text
   cargo run -p gym --bin gym -- report \
       --store results/caller-acme-v1.jsonl \
       --suite caller-acme-v1.json \
       --partition development \
       --expect kev-0.6b \
       --commitment results/caller-acme-v1.commitment.json \
       --out docs/gym/measurements/2026-09-21-caller-acme-v1.md
   ```

   `--partition`, `--family`, and `--items` carry the same narrowing the
   `eval` run used — defaulting to calibration and development — and
   `--expect` names every door the run was meant to ask, including one
   that left no rows. Together they are the declared selection: the other
   half of completeness that the store alone cannot supply.

   The record states the suite digest, the question-set digest, the gate
   digest, the store's chain head, and the recording window, then a
   coverage verdict per suite — **complete** when every expected item of
   every expected door recorded exactly once, **incomplete** with the
   gaps named, or **not declared** when no selection was passed. One
   section per door follows: what the door published as its identity and
   trial configuration, expected versus recorded counts, missing item ids,
   how many items were scored and refused, accuracy and calibration per
   partition and per family, coverage per family, the agreement ceiling
   beside each family that states one, and the label evidence each family
   rests on. Rows recorded under different artifact identities, digests,
   or trial configurations render as separate run groups and are never
   pooled. `--suite` is enrichment and evidence of selection — a suite
   file whose digest no row pins is refused, because lending another
   suite's provenance or items to these rows would mislabel the record.

   An **incomplete** or **not declared** verdict is not a failed render —
   the record still carries everything the store holds — but it cannot
   read as a completed evaluation, and a benchmark or promotion claim
   needs complete coverage.

   `--commitment` writes the anchor a caller keeps: the chain head, the
   row count, the declared selection expanded to its `(partition, item)`
   pairs, the suite, question-set, gate, and provenance digests, and each
   door's run identities with their coverage — all under one
   self-verifying digest. It needs `--suite`, because a commitment over
   an undeclared selection anchors nothing. The caller holds the file
   apart from the store; it is what makes the checks in the next section
   able to see a rewritten or shortened store.

## What the record does not know

A row the harness never wrote is not in the store. An item lost to a
timeout or a dead door leaves no row — and when the selection is
declared, the record names that item as missing rather than letting the
denominator shrink. When the selection is not declared, the record says
it cannot tell what is missing, and that is the first thing a reader
sees. A refused item is different: the row exists, the refusal is named,
and it counts against the door that refused it.

Missing is missing, always. No metric, table, or latency figure fills a
gap with a zero, and a transport or harness failure never reads as a
wrong answer.

## Checking a record someone hands you

The record is a view over the store; the store is the evidence:

- `gym verify --store results/caller-acme-v1.jsonl` walks the receipt
  chain and names where it breaks — an edited row digests to a different
  receipt than the one it carries, and an inserted or removed row breaks
  the `previous_receipt` link.
- `gym verify --store … --commitment results/caller-acme-v1.commitment.json`
  then checks the store against the commitment the caller kept: a store
  shorter than the committed row count, a prefix whose head is not the
  committed one, a recomputed chain over different rows, and a door whose
  run identities or coverage moved are each named as a divergence. Rows
  appended after the commitment are not a fault — the chain is meant to
  grow — and `verify` reports how far past the committed horizon the
  store now runs.
- Every store-reading command (`report`, `compare`, `fit`, `regress`)
  walks the same chain before it reads, so the check is not optional.
- Each row pins the suite digest and the question-set digest, so the
  items and the wording are the ones the record names, and the gate
  digest names the rule that judged them (`docs/gym/gate-digests.md`).

The chain proves the store is internally consistent, and the commitment
proves the store's committed prefix is the one the record was taken over.
Neither proves the commitment itself is authentic — that is the channel
the caller carried it over — and neither proves which weights executed
remotely; no document can attest that. The execution receipts that close
the remaining gap are the open work the issue list names.

A caller's labels remain the caller's evidence: `label_source` names them,
`label_rule` says how the labels were produced, and the record prints both
rather than letting the numbers read as authored truth.

`gym publish` renders the public view over the same store — the
status-and-benchmark page a stranger reads, with the manifest a reader
recomputes and the preserved snapshot index.
`docs/gym/published-snapshots.md` covers what its numbers may claim.

## A walked example

[`measurements/2026-09-24-caller-pilot/`](measurements/2026-09-24-caller-pilot/plan.md)
retains one full pass: the frozen plan, the synthesized caller records and
their generator, the constant baseline door, the built suite and question
set, the receipt-chained store, the rendered record, and the commitment
`gym verify` checks the store against. The caller is synthetic — labelled
`acme`, a fictional returns desk — so the pass validates the offering's
mechanics end to end and says plainly that it validates nothing about
customer demand.
