# The frozen-embedding baseline, scored through the store

[`2026-09-19-frozen-embedding-baseline.md`](2026-09-19-frozen-embedding-baseline.md)
measured frozen sentence embeddings plus logistic regression offline, with a
Python port of the metric panel. That left one gap openagents#9377 named: the
baseline was never a door, so it never went through the same harness, the same
client, the same receipt chain, and the same refusal accounting as the doors it
was compared with. This record closes that gap. The numbers here come from one
run of the unchanged Gym against `training/baseline/door.py`, recorded in
`crates/gym/results/support-v2-three-way.jsonl`.

## What was served

`door.py` fits one multinomial logistic-regression head per Choice family on
the suite's `calibration` partition at startup, over frozen, L2-normalized
`BAAI/bge-base-en-v1.5` embeddings pinned at revision
`a5beb1e3e68b9ab74eb54cfd186867f64f240e1a`, the encoder the offline record led
with. The L2 strength is chosen by stratified cross-validation inside
`calibration` (`argmin` over the same grid as the offline run; it chose
`C = 100`, not at a grid edge). The door then answers `POST /v1/systemone`:

- A Choice whose option set is the fitted one gets a `choice`, a `confidence`,
  and the full probability vector.
- A Noul or a Score gets HTTP 422 with the typed envelope the Rust client
  reads, `error.code = unsupported_primitive`, and the reason from the
  offline record: a classifier's label frequency is not the probability a
  statement holds, and a probability-weighted mean over unordered labels is
  not a position.
- A Choice over options the head was not fitted on gets `option_set_drift`.

`GET /v1/models` publishes the encoder revision as `base_model_signature`, so
the recorded rows carry a verified identity. The door runs on CPU; the whole
Python tree is pinned in `training/baseline/requirements.lock`.

## The run

```sh
cd training/baseline
.venv/bin/python door.py \
  --suite ../../crates/gym/suites/support-v2-three-way.json \
  --encoder BAAI/bge-base-en-v1.5 \
  --revision a5beb1e3e68b9ab74eb54cfd186867f64f240e1a --port 8020

cd ../..
cargo run -p gym --bin gym -- eval --door baseline-bge=http://127.0.0.1:8020 \
  --suite crates/gym/suites/support-v2-three-way.json --partition development \
  --record crates/gym/results/support-v2-three-way.jsonl --timeout 120
```

The Gym reported:

```text
Recorded 78 rows in `crates/gym/results/support-v2-three-way.jsonl`.

78 items asked: 40 scored, 38 refused by the door, 0 lost to the harness and unrecorded.

Door refusals: `unsupported_primitive` x38.

| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |
| development, raw | 0.95 | 0.131 | 0.068 | 0.241 | 0 | 40 |
```

The 38 refusals are the 14 severity (Score) and 24 urgency (Noul) items of the
development partition, every one typed and every one a recorded row. Zero
items were lost to the harness, so the refusal count and the harness-failure
count are separate quantities in the report, which is what the acceptance
criterion asked for. The 40 scored items are the routing family. The locked
partition was not read.

`gym compare --store crates/gym/results/support-v2-three-way.jsonl --baseline
baseline-bge --partition development` re-verified the receipt chain over the
1177 rows now in the store and printed the door alongside the seven already
recorded. Median latency was 25 ms per item, on a CPU.

## Against the doors on record

Every door on record answered all 78 development items, so the only shared
ground is the 40 routing items. The table pairs each door with the baseline
on those 40, item by item, from the recorded rows. "Wins" is the number of
items the baseline got right and the other door got wrong; "losses" the
reverse. The two-sided sign test on the discordant items is the paired
statistic the small count allows.

| Door | Routing accuracy, development | Baseline minus door | Wins | Losses | Sign test |
| --- | --- | --- | --- | --- | --- |
| baseline-bge | 0.950 | — | — | — | — |
| lev-adapted@1 | 0.950 | +0.000 | 1 | 1 | p = 1.00 |
| jev (hosted) | 0.875 | +0.075 | 3 | 0 | p = 0.25 |
| kev-8b | 0.875 | +0.075 | 4 | 1 | p = 0.38 |
| lev-base | 0.875 | +0.075 | 4 | 1 | p = 0.38 |
| kev-0.6b | 0.800 | +0.150 | 6 | 0 | p = 0.03 |
| kev-0.5b | 0.775 | +0.175 | 7 | 0 | p = 0.02 |
| kev-4b | 0.750 | +0.200 | 8 | 0 | p = 0.008 |

The measured two-door noise floor is 0.056 accuracy
([`2026-09-19-frozen-embedding-baseline.md`](2026-09-19-frozen-embedding-baseline.md),
from `training/baseline/compare.py`). Read against it:

- **Against Lev.** The baseline ties `lev-adapted@1` exactly and beats
  `lev-base` by 0.075, which clears 0.056 by less than one item in forty. The
  sign test does not separate them (4 wins to 1 loss). Call it: at or above
  Lev on routing, not distinguishably above the adapted one.
- **Against Kev.** The baseline beats `kev-0.5b`, `kev-0.6b`, and `kev-4b` by
  0.15 to 0.20, three to four times the floor, and on every discordant item.
  That is a real gap. Against `kev-8b` the gap is 0.075, one item over the
  floor, and the paired test does not separate them.
- **Against hosted Jev.** The baseline is ahead by 0.075 on these 40 items,
  with 3 wins and 0 losses. That clears the floor by one item and the sign
  test does not reach significance. The offline record had Jev ahead
  (0.940 to 0.920 on the 50-item `support-v2` development set); this run has
  the order reversed on the 40-item three-way development set. Both gaps sit
  within about one item of the floor, so the honest reading is that hosted
  Jev and this baseline are not separable on routing at this suite size.

Calibration is where the baseline is not competitive: raw ECE 0.131 is the
worst in the table, and NLL 0.241 trails Jev's 0.179. It is confident and
usually right; when it is wrong (`routing/067` at 0.45, `routing/082` at 0.81)
it is not always unsure about it. No confident errors under the gate's
threshold, though.

## What this does and does not say

It says the same thing the offline record said, now with the harness's own
accounting behind it: a frozen encoder and a linear head, fitted on 40
labelled items and running in 25 ms on a CPU, is at the top of the routing
table. The doors we trained are not paying for themselves on this family.

It does not say the baseline is a decision model. It refuses half the suite
by construction, and a door that cannot answer a Noul or a Score is not a
replacement for one that can. The `gym compare` prose that "a refused item
stays in the door's denominator" is not what the printed accuracy column does
for this door: 0.95 is 38 of 40 scored, not 38 of 78 asked. That is a Gym
reporting question for `crates/gym/src`, out of this record's scope, and noted
here so nobody reads 0.95 as a suite-wide number.

It also does not say anything at 50 items that 0.056 lets it say at 40. The
floor was measured on a 50-item comparison; on 40 the same reasoning gives a
wider floor, so every "one item over" verdict above is, if anything,
generous to the difference.

## What was not run

- MiniLM and MPNet were not served as doors. The offline record has them at
  0.860 and 0.900; the door takes `--encoder` and `--revision` and would score
  them the same way.
- The `one-se` strength rule was not served. The door takes `--rule one-se`.
- Nothing was asked of the locked partition.
