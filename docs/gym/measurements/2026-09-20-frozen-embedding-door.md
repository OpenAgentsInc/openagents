# A door that refuses two primitives, through the store

openagents#9377 asked for the frozen-embedding baseline to be scored the way
every other door is: through `POST /v1/systemone`, by the unchanged Gym, into
the receipt-chained store. This note records what the harness did with a door
that refuses two of the three primitives by type. The measurement itself is
[`docs/decision-models/2026-09-20-frozen-embedding-door.md`](../../decision-models/2026-09-20-frozen-embedding-door.md).

## The door

`training/baseline/door.py`, serving `BAAI/bge-base-en-v1.5` at revision
`a5beb1e3e68b9ab74eb54cfd186867f64f240e1a` with one logistic-regression head
per Choice family, fitted on `calibration`. Noul and Score requests return
HTTP 422 with `error.code = unsupported_primitive`; a Choice over an unfitted
option set returns `option_set_drift`. No Gym schema or source change was
needed: the refusal envelope `crates/gym/src/eval.rs` already reads
(`error.code` or top-level `code`) is enough, and a 422 is not retried by the
client.

## The run

```sh
cargo run -p gym --bin gym -- eval --door baseline-bge=http://127.0.0.1:8020 \
  --suite crates/gym/suites/support-v2-three-way.json --partition development \
  --record crates/gym/results/support-v2-three-way.jsonl --timeout 120
```

```text
78 items asked: 40 scored, 38 refused by the door, 0 lost to the harness and unrecorded.
Door refusals: `unsupported_primitive` x38.
```

The three counts are the point. 40 routing items scored; 14 severity and 24
urgency items refused, each one a recorded row with `refusal` set and
`answered = false`; zero harness failures. A transport error or a malformed
body would have gone into the third count and left no row. The store grew
from 1099 to 1177 rows, and `gym compare --store ... --baseline baseline-bge
--partition development` re-verified the chain afterwards. A second run with
`--partition calibration` appended 79 more rows (40 scored, 39 refused, 0
lost), because `crates/gym/tests/rederived_floors.rs` requires every door in
a results file to cover both open partitions; those rows are fitted-on and
labelled as such. The store holds 1256 rows.

## What the harness got right, and one thing to look at

Right: refusals are typed, recorded, and counted apart from losses; identity
is verified from `base_model_signature`; the question set is recorded on every
row, so the door compares against the two doors that also record it and is
correctly refused a comparison against the five that do not.

To look at, in `crates/gym/src` and out of this issue's scope: the compare
table prints accuracy 0.95 for this door beside "Scored 40, Refused 38", while
the prose under the table says a refused item stays in the denominator. One
of the two is wrong for a door that refuses by primitive. The decision record
reads the number as 38 of 40 routing items and says so.
