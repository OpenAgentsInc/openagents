# The chat router's Gym and eval routes: `chat-router-v2`

Measured 2026-09-29 for [#9936](https://github.com/OpenAgentsInc/openagents/issues/9936)
(epic [#9931](https://github.com/OpenAgentsInc/openagents/issues/9931)),
against hosted Jev (`jev-latest`), with the question set `chat-router-v2`
(18 routes and the `tool` Choice over the three tool notes) and the bank
`chat-answers-v1` with the Gym entries. What changed is in
[the chat router design](../design/2026-09-28-chat-router.md#gym-and-eval-routes-2026-09-29).
The previous numbers are in
[the tuning measurement](2026-09-28-chat-router-tuning.md).

## The labeled set

`crates/coder/fixtures/chat-router/routes-v2.json` is `routes-v1.json`
unchanged (457 rows, the same ids, labels, and splits) plus 184 rows: 22
to 32 for each new route (`gym.news`, `eval.run`, `eval.author`,
`eval.check`, `eval.result`, `eval.credit`), five multi-turn interview
replies ("looks good", "change the third test to use a bigger PR"), and
near misses against their neighbors (what a test or a tool is, which our
notes answer; unit tests in the user's own repository, which Coder does;
the XP command; the eval engine's code). The split is the id's hash, as
before: 48 of the new rows are held out, at least 6 per new route. The
Gym suite `chat-router-v2` (`crates/gym/suites/chat-router-v2.json`) is
generated from it, scored with `chat-router-route-v3`, the production
router's `route` question; `chat-router-v1` and `chat-router-route-v2` are
kept as recorded.

```sh
set -a; . ~/work/.secrets/typesafe.env; set +a
ROUTER_EVAL_SPLIT=held_out cargo test -p coder --test router_eval live_router -- --ignored --nocapture --test-threads 1
```

`ROUTER_EVAL_ROWS=v1` or `gym` restricts a run to the v1 rows or the new
rows.

## How it was tuned

On the tune split only. The first tune run of the new rows (136 rows)
read every new route at 91.7 % to 100 % precision; its misses set the
changes: rubric examples for `eval.author` ("write me a set of tests for
my changelog helper"), `eval.check` ("confirm the latest test reader
result"), `eval.credit` ("did my result hold up when others checked it"),
and `product.kb` ("what does a test check?", "what is a tool in the
gym"); `not_for` lines that send another trainer's result to `eval.check`
and what others did with the user's work to `eval.credit`; a `not_for` on
`eval.credit.mine` and two examples on `eval.credit.how`. Two policy
choices came from the same run: a sure credit `answer` reading (at 0.70)
serves that credit entry even at the specifics ceiling ("how do I get
credit for a tool I made" read `eval.credit.how` at 0.95 with specifics
at 0.30), and `gym.what_test` and `gym.what_tool` answer on `general` as
well as `product.kb`, since the first run read "what does a test check?"
as `general` (0.87) with `gym.what_test` at 0.98.

## Held-out split (186 rows), two runs

| Metric | Run 1 | Run 2 |
| --- | --- | --- |
| Canned precision (target ≥ 98 %) | **100 %** (50/50) | **100 %** (50/50) |
| Canned served, of the 73 rows labeled canned | 50 | 50 |
| Dispatch precision (target ≥ 90 %) | **94.7 %** (18/19) | **100 %** (18/18) |
| Dispatch recall | 100 % (18/18) | 100 % (18/18) |
| Gym and interview tiers: precision / recall | **96.4 %** (27/28) / 79.4 % | **96.7 %** (29/30) / 85.3 % |
| Refusal precision / recall | 100 % / 90 % | 100 % / 90 % |
| Secret recall | 100 % (7/7) | 100 % (7/7) |
| Route accuracy | 88.7 % | 88.7 % |
| Judgment latency p50 / p95 | 226 / 466 ms | 226 / 335 ms |

### The new routes (precision, hit / predicted / labeled)

| Route | Held-out run 1 | Held-out run 2 | Tune |
| --- | --- | --- | --- |
| `gym.news` | 100 % (7/7/7) | 100 % (7/7/7) | 95.0 % (19/20/19) |
| `eval.run` | 100 % (7/7/7) | 100 % (7/7/7) | 100 % (22/22/22) |
| `eval.author` | 100 % (7/7/7) | 100 % (7/7/7) | 100 % (25/25/25) |
| `eval.check` | 100 % (5/5/6) | 100 % (5/5/6) | 100 % (16/16/16) |
| `eval.result` | 87.5 % (7/8/7) | 87.5 % (7/8/7) | 91.7 % (22/24/23) |
| `eval.credit` | 87.5 % (7/8/8) | 87.5 % (7/8/8) | 100 % (18/18/20) |

The router's confidence policy applies to the new routes as to the
others: a card, an offer, or an interview step needs the route at 0.70
(`EVAL_ROUTE`), `gym.news` at 0.60, and anything less is the model, told
it has no Gym records. Of the 48 held-out new rows, 43 read their own
route in both runs; the ones under their tier's floor went to the model
with that note ("What are people working on?" at 0.53 to 0.54, "Is there
a result I can check?" at 0.51 to 0.56). The misses, the same in both
runs:

- "how's my test set doing, anyone used it?" (`eval.credit`) read
  `eval.result` at 0.84; it gets `eval.result.mine` ("results of your own
  tests stay on your phone") and **See your result**. The one Gym tier
  given to another route's row.
- "let me verify a result" (`eval.check`) read `clarify` at 0.54, and gets
  a clarifying question.
- "What's a test?", "what's a test set?", and "what are tools for coder"
  (`product.kb`, prepared answers) read `general` at 0.66 to 0.91 with
  `gym.what_test` or `gym.what_tool` at only 0.40 to 0.75, so the model
  answers, led by an opener. Jev can't tell from the bare question that
  it's about the Gym, which the canned floor is there for; it was not
  tuned further, since only held-out readings point that way.

### The v1 rows (138) against the recorded runs

| Metric | This change, two runs | After #9928's retune, two runs |
| --- | --- | --- |
| Canned precision | 100 % (44/44, 44/44) | 100 % (45/45, 44/44) |
| Canned served, of 62 | 44, 44 | 45, 44 |
| Dispatch precision | 94.7 % (18/19), 100 % (18/18) | 100 % (18/18, 18/18) |
| Route accuracy | 88.4 %, 88.4 % | 91.3 %, 89.9 % |

The one false dispatch offer, "which file handles the x402 spending
policy" (`codebase.kb`), read `work.dispatch` at 0.58 and 0.65 with the
lane at computer (0.86), so the lane rule offered Coder in one run; the
tune split has no codebase question that reads this way, so it was left.
Route accuracy on these rows is inside the spread recorded before #9928
(87.7 % and 89.9 % in step 5 of the tuning); one v1 row now reads a new
route ("where do playtest rewards come from" as `eval.credit` at 0.59,
under the floor, so the model answers it).

### Old phones

A turn that asks only for `opener` (builds 19 and earlier) is decided in
legacy mode: on the held-out split it served 38 whole answers, all right
(100 %), and no offers. Build 20 names `chat-router-v1`, which the worker
routes with `chat-router-v2`; it ignores the new `card` feedback and the
eval offers, whose words it doesn't know, and shows the text.

## Tune split (455 rows), final

| Rows | Canned precision | Dispatch precision | Gym and interview precision | Route accuracy |
| --- | --- | --- | --- | --- |
| All | 100 % (112/112) | 100 % (40/40) | 100 % (96/96), recall 91.4 % | 93.0 % |
| v1 rows (319) | 100 % (91/91) | 100 % (37/37) | | 90.9 % |
| New rows (136) | 100 % (21/21) | 100 % (3/3) | | 97.8 % |

## Offline checks

`cargo test -p coder` covers the rest without a network: the tier each
route gets (`router::policy`), the reply each route builds from fixture
records (`router::gym`), the cards' numbers against their records
(`router::card`), the wire against the NIP-CJ fixtures and NIP-CJ's own
parser (`router::wire`), the Gym corpus's admission of signed
publications (`gym_kb`), and each tier end to end through the worker with
a loopback judge and door (`coder-worker`).
