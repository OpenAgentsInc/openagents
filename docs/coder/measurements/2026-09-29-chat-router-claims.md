# The chat router as a capability claim: the first evidence record

Measured 2026-09-29 for [#9959](https://github.com/OpenAgentsInc/openagents/issues/9959),
against hosted Jev (`hosted_http:jev`), with the question set
`chat-router-v2@dade99e606c5` and the bank `chat-answers-v1@89920646b42c`,
at commit `440d13b793` of the router. The essay
([Test-Time Capabilities](../../essays/2026-09-29-test-time-capabilities.md#the-thesis-capability-is-something-you-can-acquire-at-test-time))
says every capability is a claim with three layers: a claim key, evidence
records on it, and a decision policy over the records. This measurement
puts the chat router under that discipline for the first time. The
previous numbers are in
[the `chat-router-v2` measurement](2026-09-29-chat-router-v2.md).

## What changed

- **The record.** A published router eval is now an
  `openagents.eval-report.v1` under NIP-EVAL's extension profile
  ([the decision-service record](../../../nips/openagents/NIP-EVAL.md#reports)),
  built by `crates/coder/src/router_claim.rs` from ids, labels,
  probabilities, tiers, and timings only. Its subject is the router as a
  decision service, with `configuration` pinning the question set and the
  bank by digest; its suite's cases are the committed Gym suite
  `crates/gym/suites/chat-router-v2.json` and its partition the suite's
  locked rows. The bytes pass `nostr::eval_ext::parse_report`, so they are
  publishable as a `3189` once the suite has a release to cite.
  [`2026-09-29-chat-router-claims/report.json`](2026-09-29-chat-router-claims/report.json)
  is the record this page reports (36,828 bytes, 15 artifacts beside it
  under `target/router-eval/2026-09-29/`; the
  [limitations](2026-09-29-chat-router-claims/limitations.json),
  [configuration](2026-09-29-chat-router-claims/configuration.json), and
  [suite](2026-09-29-chat-router-claims/suite.json) documents are kept
  here too).
- **The gate.** `crates/gym/gates/router-v1.json` is the decision policy,
  named as the Gym suite's `gate`. Inside its digest
  (`gate:4433b518bbffd29d55451cf63e864e8086d8ffa0f9b4f3a47e066459164a5619`):
  the primary outcome, canned precision (higher); the measures held
  non-inferior, route accuracy, canned recall, dispatch precision, and
  ECE; the product floors, canned precision ≥ 0.98 and dispatch precision
  ≥ 0.90, judged with or without a baseline; and `max_decrease` 0.053,
  the spread between the two held-out runs recorded before this change,
  which is the margin a measure may fall by before the fall is a loss.
  When the Gym scores this suite door against door, the same rule reads
  accuracy as route accuracy and ECE as the calibration it holds.
- **The digest on the wire.** A judgment's `set` is now
  `chat-router-v2@<digest>`, the SHA-256 of the canonical JSON of the
  `route` question computed as the Gym digests a question set, so the
  committed `crates/gym/questions/chat-router-route-v3.json`, a Gym row,
  and a running worker name one question by one digest
  (`the_wire_names_the_question_set_by_the_gyms_digest`). The worker logs
  it at start beside the bank's, and every `router` line carries both and
  the calibration map applied or `null`.
- **Calibration.** The published eval fits one reliability table per
  question (`gym::calibrate::Map`) on the labeled set's calibration
  partition (246 rows), scores raw and mapped probabilities on the
  held-out split, asks `probability-v2` whether the map may be served, and
  writes the result as `crates/coder/fixtures/chat-router/calibration-v2.json`.
  Serving it is the operator's call: `CODER_WORKER_ROUTER_CALIBRATION=on`
  ([the deployment note](../../deployment/chat-worker.md)), off by
  default.

```sh
set -a; . ~/work/.secrets/typesafe.env; set +a
ROUTER_EVAL_PUBLISH=1 cargo test -p coder --test router_eval live_router -- --ignored --nocapture --test-threads 1
```

The run reads the held-out rows (188) and the calibration partition
(246), 434 requests one at a time, in 95 to 98 s; it writes the record
and `calibration-v2.json` to `target/router-eval/<date>/`.

## The claim key

| Scope | In this record |
| --- | --- |
| A, the subject | `chat-router/decide`: the `chat-router-v2@dade99e606c5` question set and the `chat-answers-v1@89920646b42c` bank, decided by `crates/coder/src/router/policy.rs` at the thresholds in `configuration.json`, in `router` mode with calibration off. `identity` is `version`: the judge is named (`hosted_http:jev`), the model behind it is not pinned |
| B, the baseline | None. The previous router version was not kept runnable, so this first record claims no change and its verdict is `inconclusive`, as NIP-EVAL requires |
| D, the distribution | `chat-router/first-messages`: first messages to the OpenAgents chat as the route catalog frames them, `constructed`, synthetic |
| S, the suite | `crates/gym/suites/chat-router-v2.json` (digest `aab46e3c…`), 645 rows; the locked partition, 188 rows, is the held-out split |
| E and G | Hosted Jev over HTTP, one row at a time, the CLI route and the tool question wired as the deployed worker asks them, no knowledge base |
| M, the measurement | `metrics.json`: per-route precision and recall, canned and dispatch precision and recall, Gym and refusal precision, secret recall, abstention, `cases_passed`, latency, ECE, Brier, and NLL per question raw and calibrated, and the operating points at nine thresholds |
| P, the policy | `router-v1` (`gate:4433b518…`), outside the key |

## Held-out split (188 rows), two runs

Two published runs, 12:40 and 12:43 UTC-5; the second is the committed
record. The spread between them is the run-to-run variation the gate
names as its pending measurement.

| Metric | Run 1 | Run 2 (the record) |
| --- | --- | --- |
| Cases passed (route right, no harmful error, canned rows served whole, model rows left alone) | | **145 / 188** (77.1 %) |
| Canned precision (floor 0.98) | **100 %** (51/51) | **100 %** (50/50) |
| Canned recall (right answer served, of 75 canned rows) | 68.0 % | 66.7 % |
| Dispatch precision (floor 0.90) | **100 %** (18/18) | **94.7 %** (18/19) |
| Dispatch recall | 100 % (18/18) | 100 % (18/18) |
| Gym and interview precision / recall | 96.6 % (28/29) / 82.4 % | 96.7 % (29/30) / 85.3 % |
| Refusal precision / recall | 100 % / 90 % | 100 % / 90 % |
| Secret recall | 100 % (7/7) | 100 % (7/7) |
| Route accuracy | 87.8 % | 88.3 % |
| Abstention (the model alone) | 24.5 % (46/188) | 25.5 % (48/188) |
| Judgment latency p50 / p95 | 211 / 266 ms | 215 / 278 ms |

The gate's reading of the record (`limitations.json`): `scored_rows>=30`
passed (188); `canned_precision_at_or_above_floor` passed (1.000 against
0.980); `dispatch_precision_at_or_above_floor` passed (0.947 against
0.900); `baseline_arm_ran` unverifiable, so the primary outcome and the
non-inferiority criteria are not judged. Verdict: **inconclusive**, which
is what a first record can say.

The one false dispatch offer in run 2 is the one recorded before this
change ("which file handles the x402 spending policy", `codebase.kb`,
read `work.dispatch` at the lane rule). The one Gym tier on another
route's row is also unchanged ("how's my test set doing, anyone used
it?", `eval.credit`, read `eval.result`).

### Per route (run 2, hit / predicted / labeled)

| Route | Precision | Recall | Counts |
| --- | --- | --- | --- |
| `meta` | 100 % | 84.4 % | 27/27/32 |
| `smalltalk` | 100 % | 100 % | 11/11/11 |
| `general` | 50.0 % | 100 % | 8/16/8 |
| `product.kb` | 80.0 % | 76.2 % | 16/20/21 |
| `codebase.kb` | 100 % | 45.5 % | 5/5/11 |
| `work.dispatch` | 94.7 % | 100 % | 18/19/18 |
| `cli` | 100 % | 88.9 % | 8/8/9 |
| `wallet` | 88.9 % | 100 % | 8/9/8 |
| `account` | 75.0 % | 85.7 % | 6/8/7 |
| `clarify` | 60.0 % | 100 % | 6/10/6 |
| `end` | 100 % | 100 % | 5/5/5 |
| `refuse` | 100 % | 80.0 % | 8/8/10 |
| `gym.news` | 100 % | 100 % | 7/7/7 |
| `eval.run` | 100 % | 100 % | 7/7/7 |
| `eval.author` | 100 % | 100 % | 7/7/7 |
| `eval.check` | 100 % | 83.3 % | 5/5/6 |
| `eval.result` | 87.5 % | 100 % | 7/8/7 |
| `eval.credit` | 87.5 % | 87.5 % | 7/8/8 |

`general` and `clarify` are where the misses land (their recall is 100 %
and their precision low): a `codebase.kb`, `product.kb`, or `meta` row
that Jev is unsure of reads as `general`, and the model answers it. Over
both partitions (434 rows, run 1) the 43 route misses were `meta` read as
`product.kb` (5), `product.kb` as `general` (5), `codebase.kb` as
`general` (5), and a long tail of one or two each; 42 of the 163 canned
rows were served by the model instead of their answer. Nothing was tuned
here (the issue rules out threshold and route changes).

## Operating points: the risk–coverage curve (run 2, held out)

What serving on each question's reading at `t` or above would give. The
`route` reading is right when it is the labeled route (188 readings); the
`answer` reading, the `answer` question's argmax entry served or not, is
right when the row accepts it (149 readings; 39 rows had no answer
reading, which the record counts as unavailable). The policy serves a
whole answer at `answer` ≥ 0.80 with `route` ≥ 0.80 and
`needs_specifics` ≤ 0.30, so its canned precision (100 %) sits above the
`answer` curve's 0.80 point (92.3 %): the specifics ceiling and the
route floor buy the rest.

| Threshold | `route` precision | `route` coverage | `answer` precision | `answer` coverage |
| --- | --- | --- | --- | --- |
| 0.50 | 90.3 % (176) | 93.6 % | 74.8 % (119) | 79.9 % |
| 0.60 | 93.2 % (161) | 85.6 % | 83.2 % (101) | 67.8 % |
| 0.70 | 95.4 % (153) | 81.4 % | 86.5 % (96) | 64.4 % |
| 0.75 | 96.5 % (144) | 76.6 % | 89.5 % (86) | 57.7 % |
| **0.80** | **97.0 %** (134) | **71.3 %** | **92.3 %** (78) | **52.3 %** |
| 0.85 | 98.4 % (123) | 65.4 % | 97.0 % (66) | 44.3 % |
| 0.90 | 99.0 % (104) | 55.3 % | 100 % (55) | 36.9 % |
| 0.95 | 98.9 % (90) | 47.9 % | 100 % (46) | 30.9 % |
| 0.99 | 100 % (60) | 31.9 % | 100 % (15) | 10.1 % |

Abstention at each threshold is one minus coverage; the record carries it
as `<question>.abstention_at_<t>`.

## Calibration (fitted on the calibration partition, scored held out)

| Question | Fitted on | Held out | ECE raw → mapped | Brier raw → mapped | NLL raw → mapped | Confident errors | `probability-v2` |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `route`, run 1 | 246 | 188 | 0.037 → 0.030 | 0.080 → 0.086 | 0.258 → 0.277 | 1 → 2 | failed |
| `route`, run 2 | 246 | 188 | 0.048 → 0.032 | 0.082 → 0.088 | 0.262 → 0.285 | 1 → 2 | failed |
| `answer`, run 1 | 182 | 149 | 0.115 → 0.052 | 0.150 → 0.130 | 0.439 → 0.401 | 1 → 1 | passed |
| `answer`, run 2 | 181 | 149 | 0.140 → 0.050 | 0.146 → 0.120 | 0.429 → 0.368 | 0 → 0 | passed |

Two findings, the same in both runs:

- **The `route` probability is already calibrated, and the map hurts
  it.** Raw ECE is 0.04 to 0.05 on 188 rows; the ten-bin map lowers ECE
  a little and raises Brier, NLL, and the confident-error count, and
  `probability-v2` refuses it (log loss rose). 187 of the 246 fitting
  rows sit in the top bin, so the lower bins rest on 4 to 16 rows each.
  The map is committed as fitted, with its verdict, and should not be
  served for `route`.
- **The `answer` probability is overconfident in the middle, and the map
  fixes it.** Raw ECE 0.12 to 0.14: an `answer` reading of 0.4 to 0.6 is
  right 14 to 24 % of the time, and one of 0.7 to 0.8 about half the
  time, while 0.9 and above is right 97 %. The map brings ECE to 0.05,
  lowers Brier and NLL, and `probability-v2` passes it. The policy's
  0.80 `answer` floor already sits where the raw reading turns reliable,
  which is why canned precision holds at 100 % without the map.

Whether the worker serves the maps is `CODER_WORKER_ROUTER_CALIBRATION`,
off. Turning it on would put both maps in front of the policy table,
whose thresholds were tuned on raw probabilities; the `route` map's
verdict says not to, and a per-question switch is not built. The record
carries both maps' scores either way.

## Offline checks

`cargo test -p coder` covers the record without a network: the pass rule
and a synthetic five-row record through `nostr::eval_ext::parse_report`,
every cited artifact beside it by digest, no message text in any file,
and the 64 KiB bound at 256 cases (`router_claim`); the observations and
operating points against labels (`router_eval`); the committed
calibration fitted for this build's question set, a map for another set
refused, and a map moving only probabilities (`router::calibration`);
the wire's `set` digest equal to the Gym's (`router::wire`,
`tests/router_suite.rs`). `cargo test -p gym` covers the gate: its primary
outcome and non-inferiority inside the digest, the floors judged without
a baseline, Better only past the margin, Worse on any measured loss, and
a door comparison read as routing and calibration (`gate`,
`tests/gate_digest.rs`). `cargo test -p nostr` covers the
`decision-service` profile fixture against the schema and the parser.

## Not done here

- No baseline arm, so no verdict beyond `inconclusive`; the next router
  version's record can name this one's configuration as its baseline and
  be judged on the primary outcome.
- No `3189` was published: the suite has no NIP-EXT release to cite, and
  the subject's definition is not a published NIP-CAP head (its id's
  publisher is the evaluator's local provenance id).
- The run-to-run spread rests on two runs; the gate's `max_decrease`
  (0.053) is the spread recorded before this change, and the two runs
  here moved dispatch precision by the same 0.053, route accuracy by
  0.005, and canned recall by 0.013.
- The missing-capability route ([#9960](https://github.com/OpenAgentsInc/openagents/issues/9960))
  lands with its own labeled rows; the record here is on the 18 routes.
