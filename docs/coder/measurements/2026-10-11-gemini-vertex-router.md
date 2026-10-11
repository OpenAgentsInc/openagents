# Gemini on Vertex as the router's judge (routes-v5, 2026-10-11)

The production judge asks TypeSafe's Jev first, then our `POST
/v1/systemone` with `X-Decision-Order: pylon`: the CoderOS-4080 Clef
Pylon, then Gemini 3.8 Flash on Vertex (`ask_vertex` in
`crates/gateway/src/decision_dispatch.rs`). Gemini is the last door, and
its answers are read on Jev's calibration map (`calibration-v2`), because
`router::calibration::apply_served` reads every model except Clef on it.
Gemini had never been measured on routes-v5. This is that measurement
(#11302).

[Measurements index](README.md)

## How it ran

- **The door.** `scripts/bench/decision-throughput/vertex_door.py` is the
  gateway's Vertex step alone, as a local `/v1/systemone`. It builds the
  same `generateContent` body as `vertex_body` (compact JSON prompt,
  response schema, temperature 0, thinking `low`), reads the answer as
  `vertex_answers` does, and returns the same envelope (`model:
  gemini-3.8-flash`, `service.door: vertex`). It asks as the automation
  service account, logs tokens and cost per call, stops at a dollar cap,
  and caches answers by request digest. The cache let the map variants
  below be scored on exactly the same readings at no extra cost.
- **The eval.** `CODER_DECISION_PROFILE=direct_local` pointed at the door,
  `ROUTER_EVAL_SPLIT=published`, flat turns (the shape the Jev-first
  worker asks). To stay under budget it ran every fifth row
  (`ROUTER_EVAL_EVERY=5`, new): 136 of the 678 rows, 79 in the
  calibration partition and 57 held out.
- **The goldens.** `chat-goldens router` against the same door.
- **Spend.** $3.94 of Vertex in all: $2.66 for the eval, including a
  pilot and a rerun after an expired token failed 52 rows with 401, and
  $1.28 for the goldens, which reached the cap after 76 of 123 cases.
  TypeSafe was not asked: it answers 402 until the top-up.

## Router eval, 136 rows

Each column reads Gemini's raw probabilities through a different map.
The Jev and Clef rows are the published held-out numbers from #11247 and
#11297, on all 261 held-out rows.

| Judge, map | Route acc. | Canned prec. / cov. | Dispatch prec. | Gym recall | Refusal recall |
| --- | --- | --- | --- | --- | --- |
| **Gemini, Jev's map (production today)** | **93.4 %** | **98.0 / 87.5 %** (48/49) | **100 %** (18/18) | **90.9 %** (20/22) | **100 %** (5/5) |
| Gemini, its own route map + Jev's answer map | 93.4 % | 96.2 / 92.9 % (50/52) | 100 % (19/19) | 100 % (22/22) | 100 % (5/5) |
| Gemini, raw (no map) | 93.4 % | 96.1 / 91.1 % (49/51) | 100 % (18/18) | 90.9 % | 100 % |
| Jev (TypeSafe), `calibration-v2` | 91.5 % | 98.9 / 66.9 % | 98.0 % | 88.9 % | 91.3 % |
| Clef-Flash flat, its map | 69.1 % | 94.6 / 33.3 % | 93.8 % | 43.2 % | 40.0 % |

On the held-out 57 rows alone, Gemini on Jev's map served 17 of 17
prepared answers right (route 52/57). On its own route map it served 19,
and one was wrong (`clarify/002`). In that case a 0.6–0.8 route reading
was mapped to 0.81, which is just over `ROUTE_CONFIDENCE`. On these rows,
Gemini on Jev's map meets the router bar: route ≥ 90 %, canned precision
≥ 98 %, dispatch ≥ 97 %, Gym recall ≥ 85 %.

**Calibration.** The published eval fitted Gemini's own maps on the
calibration partition and scored them on the held-out rows:

| Question | Raw ECE (Gemini) | Mapped ECE | Gate | Raw ECE (Jev / Clef flat) |
| --- | --- | --- | --- | --- |
| `route` | 0.087 | 0.041 | passed | 0.048 / 0.379 |
| `answer` | 0.178 | 0.163 | unverifiable (33 held-out rows) | 0.105 / 0.094 |

- Gemini's `route` reading is close to Jev's scale. Its fitted map moves
  only the 0.6–0.8 band (to 0.81), and on the held-out rows that cost
  canned precision.
- Gemini's `answer` probability says little about whether the answer is
  right. It read every prepared answer at 0.5 or more and was right on
  about two in three, so its own map is nearly flat at 0.63.
- On Jev's map, the policy's other checks (the route must agree, and
  `needs_specifics` must stay low) kept prepared answers at 48 of 49
  right.

**Decision: Gemini stays on Jev's map.** Its own map was fitted but is not
wired in. The only half that passed its gate (`route`) made the router
worse on the held-out rows, and the `answer` half cannot serve.

## Latency is the real gap

| | p50 | p90 | p95 | Over 6 s (`first::LATE`) |
| --- | --- | --- | --- | --- |
| Vertex door, one request (312 requests) | 4.84 s | 5.99 s | 6.76 s | 31 (10 %) |
| Goldens judge, whole turn (74 cases) | 5.72 s | 7.95 s | | 2 timed out |
| Jev, goldens judge (for comparison) | 0.89 s | 1.13 s | | |

- Every Vertex request took longer than the 2.5 s first-response budget
  (`first::BUDGET`).
- In production, Gemini is asked only after Jev and the Pylon have used
  part of the turn's 6 s, so most turns that fall to Gemini will be judged
  late or go unrouted.
- The product notes' relevance judgment has a 1.5 s budget
  (`product_kb::JUDGE_BUDGET`), and Gemini never answers in time. When
  Gemini is the door, every knowledge turn loses its notes.
- A turn costs about $0.012–0.018 of Vertex: 15.0k Gemini input tokens
  and about 800 output tokens per request, with thinking `low`. That is
  roughly thirty times Jev's list price.

## Goldens (router mode, 76 of 123 cases before the cap)

Judge-only columns. Every case fails on time, so Pass is 0.

| Judge | Cases | Route right | Answer right |
| --- | --- | --- | --- |
| Gemini, Jev's map | 76 | 75 (98.7 %) | 71 (93.4 %) |
| Jev, `calibration-v2` (#11195, all 123) | 123 | 120 (97.6 %) | 108 (87.8 %) |
| Clef-Flash, its map (#11195, all 123) | 123 | 110 (89.4 %) | 56 (45.5 %) |

The 76 are the first cases in the set's order, not a random sample.

## Reproduce

```sh
H=scripts/bench/decision-throughput
CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config \
  python3 -I $H/vertex_door.py --port 18199 --log calls.jsonl --cache cache --budget-usd 4 &
CODER_DECISION_PROFILE=direct_local CODER_DECISION_URL=http://127.0.0.1:18199 \
CODER_DECISION_MODEL=jev-latest ROUTER_EVAL_SPLIT=published ROUTER_EVAL_EVERY=5 \
ROUTER_EVAL_PUBLISH=1 ROUTER_EVAL_CALIBRATION_FILE=calibration-gemini-flash-v1.json \
  cargo test -p coder --test router_eval live_router -- --ignored --nocapture --test-threads 1
# the same with ROUTER_EVAL_MAPS=on instead of ROUTER_EVAL_PUBLISH scores it on Jev's map, from the cache
CODER_DECISION_PROFILE=direct_local CODER_DECISION_URL=http://127.0.0.1:18199 \
CODER_DECISION_MODEL=jev-latest cargo run -p coder --bin chat-goldens -- router
```
