# Published snapshots: the public benchmark and status view

`gym report` renders the measured record — the whole evidence for the
caller who ran the evaluation. `gym publish` renders the page a stranger
reads: a public status and benchmark view over the same receipt-chained
store, under the same honesty rules, plus the ones a public audience adds.

```
gym publish --store results/kev-quiet.jsonl --suite crates/gym/suites/kev-v1.json \
    --partition development --expect kev-quiet \
    --commitment results/kev-quiet.commitment.json \
    --costs costs.json --status status.json \
    --out docs/gym/snapshots
```

Each publication writes one immutable directory —
`docs/gym/snapshots/<generated>-<head8>/` — holding `snapshot.md`, the
public page; `manifest.json`, the checkable form of every claim on the
page (`openagents.gym.snapshot.v1`); and `commitment.json`, a copy of the
report commitment when one is supplied. A snapshot directory is never
rewritten: publish again and a new id appears. `index.json` at the root
is append-only and preserves every earlier snapshot with its store head,
coverage verdict, and commitment digest — the history the page joins.

## What a number may claim

- **Coverage before quality.** `--suite` with `--partition`, `--family`,
  `--items`, and `--expect` declares what the run was meant to ask — the
  same selection `report` uses. A workload reads **complete** only when
  every expected item of every expected door recorded exactly once.
  Anything else renders **partial** with answered, refused, missing,
  duplicate, and unexpected counts named. No `--suite` renders **not
  declared**: the page carries the record and says first that it cannot
  read as a completed evaluation.
- **One workload per measurement.** Rows that disagree on artifact or
  execution identity, question-set digest, gate digest, estimator, draw
  count, or seed base are different measurements. Each renders its own
  section; the page never pools them into one number, and it says a
  controlled ranking across them is not supported.
- **Denominators stay whole.** Accuracy is correct over answered, shown
  beside asked, refused, and missing columns. A refused item is the
  door's outcome and counts against it; a missing item is work the store
  does not hold. Neither is folded into the accuracy figure, and an
  answered-only figure never wears the whole workload's name.
- **Latency is measured or absent.** p50 and p95 are nearest-rank over
  the timed calls in the workload's recording window, printed with the
  call count and the door's execution settings — host, backend, dtype,
  whatever the door published. A p95 over a handful of calls is that
  run's nth-largest call, not a floor, and the page says so. An untimed
  group reads **not timed**, never zero. Wall clock moves with whatever
  else the host ran; a comparison across hosts is not controlled.
- **Cost follows the `Cost` discipline.** `--costs` is a JSON list of
  per-door declarations, and it is the only way a price appears:

  ```json
  [{"door": "kev-quiet", "cost": {"lane": "metered", "usd_per_decision": 0.002},
    "source": "kev-price-list-2026-09", "unit": "usd_per_decision"},
   {"door": "lev-local", "cost": {"lane": "unmetered_local_lane"}}]
  ```

  A metered price without its source and unit is refused at publish — a
  number without provenance is not a price. An unmetered local lane is
  declared as unmetered, never as zero: own hardware still costs device
  time, power, and the machine it occupies. A door with no declaration
  renders **not measured**.

- **Live status is a separate section.** `--status` is a JSON note —
  `source`, `checked_at`, `state`, an optional `detail`, and `stale` —
  and it is the only place a claim about *now* may appear:

  ```json
  {"source": "operator probe", "checked_at": "2026-09-21T00:00:00Z",
   "state": "operational"}
  ```

  The page prints the source, the state, and the freshness against the
  snapshot's generation time; a note the source declares stale, or one
  whose check time cannot be read, says so. No `--status` publishes
  historical measurement only, and the page says that too — the absence
  of a status claim is not a claim of health.
- **Aggregates only.** The page and manifest are built of counts,
  digests, identities, and suite structure. Item text, answer
  distributions, label text, and anything a tenant sent stay in the
  store; a snapshot cannot quote them because it never holds them.

## Checking a snapshot someone hands you

The manifest is the page in checkable form; the store is the evidence:

```
gym verify --store results/kev-quiet.jsonl --snapshot docs/gym/snapshots/<id>
```

`verify` walks the store's receipt chain, takes the manifest's recorded
horizon — the store's first `store.rows` rows — and checks that the
prefix's recomputed head is the published one. It then recomputes every
row-derived claim in the manifest — coverage, per-family counts,
accuracy, latency profile, refusals, label sources — and names each
divergence. It checks the rendered page against the digest the manifest
records, so an edited page fails, and it checks the snapshot's
`commitment.json` against the store the way `gym verify --commitment`
does.

A store that has grown since the snapshot verifies over the published
prefix and reports the growth — the chain is meant to grow. A store
whose prefix changed is different evidence, and verify says so.

Claims that come from the suite file rather than the rows — label rules
and agreement ceilings — travel under the commitment's provenance
digest, not the row chain.

## What the snapshot does not know

Everything `docs/gym/measured-records.md` says a record does not know
applies unchanged: a row the harness never wrote is not here, missing is
missing, and no number fills a gap with a zero. The snapshot adds one
more limit: it is a view, not a probe. Nothing on the page was measured
at read time, and the live-status section is only as fresh as the note
the publisher declared.
