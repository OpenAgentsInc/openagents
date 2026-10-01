# An explicit delegation request routes to Coder (#10073)

On 2026-09-30 the owner asked the desktop chat "who are you", then "who
can you delegate to", then "do a test delegation now". The router read the
last message as `eval.run` and answered with the Project map test card, a
Gym capability test, instead of dispatching Coder
([#10073](https://github.com/OpenAgentsInc/openagents/issues/10073)).

The fix is in the typed router, with no keyword matching: the
`work.dispatch` rubric in `crates/coder/src/router/rubric.rs` covers asking
us to delegate to Coder, hand the conversation to Coder, or run or start
Coder now, including a trial or test delegation that names no task; its
examples are three tune rows. The `eval.run` rubric's `not_for` sends a
test or trial delegation to Coder itself, which names no Gym tool, to
`work.dispatch`, and the `meta` rubric keeps asking whom or how we
delegate. No route was added, so the set stays `chat-router-v4`; the
`route` question's digest moves from `chat-router-v4@12ea6aac036f` to
`chat-router-v4@ca74e9da5045`.

The labeled set `routes-v4.json` keeps its 743 rows and adds 36 tagged
`delegation`: 26 `work.dispatch` rows (the owner's three-turn chat, the
bare request, other wordings, a Spanish one, and two follow-ups such as
"ok try it" after we said whom we delegate to) and 10 near misses (asking
whom or how we delegate, `meta`; testing a Gym tool on Coder, `eval.run`;
where delegation is implemented, `codebase.kb`). The split is the ids'
hash, as before: 10 of the 36 are held out. The Gym suite `chat-router-v4`
and its question set `chat-router-route-v5` are regenerated from it
(`ROUTER_SUITE_WRITE=1`, `tests/router_suite.rs`), and
`calibration-v2.json` is refit.

## The delegation rows, before and after

Hosted Jev (`TYPESAFE_API_KEY`), `ROUTER_EVAL_ROWS=delegation
ROUTER_EVAL_SPLIT=all cargo test -p coder --test router_eval live_router
-- --ignored`, 36 requests each, in the set's default phone context.
"Before" is the same rows asked with the rubric at `4ee52b9444`.

| Delegation rows (36) | Before | After |
|---|---|---|
| Route accuracy | 0.667 | 0.972 |
| `work.dispatch` recall | 16 / 26 | 26 / 26 |
| `work.dispatch` rows read as `eval.run` | 3 | 0 |
| `eval.run` precision | 4 / 9 | 4 / 4 |
| Dispatch precision | 100 % (9/9) | 100 % (26/26) |

Before, the owner's three-turn chat (`work.dispatch/delegate-001`), "kick
off a coder run as a test", and "fire off a test job to coder" were read
as `eval.run`, and five rows as `clarify`. After, the one miss is a `meta`
near miss ("can you delegate work to my computer?") read as `product.kb`.

## The published eval

`ROUTER_EVAL_PUBLISH=1 cargo test -p coder --test router_eval live_router
-- --ignored`, 525 requests (held out and calibration), 0 errors. "Before"
is the v4 record in
[the presentation route measurement](2026-09-30-presentation-route.md) on
the 220 held-out rows v4 had.

| Held out | v4 (220 rows) | This change (230 rows) |
|---|---|---|
| Route accuracy | 0.905 | 0.891 |
| Canned precision | 100 % (72/72) | 100 % (70/70) |
| Dispatch precision | 0.958 (23/24) | 1.000 (30/30) |
| Gym precision | 0.966 (28/29) | 0.967 (29/30) |
| `eval.run` hit / predicted / labeled | - | 8 / 8 / 8 |
| `presentation.open` hit / predicted / labeled | 9 / 9 / 9 | 9 / 9 / 9 |
| `capability.missing` hit / predicted / labeled | 11 / 11 / 11 | 11 / 11 / 11 |

All 10 held-out delegation rows were read right. On the 220 rows v4 had,
three more rows were misread than in the v4 run. Three held-out rows read
as `work.dispatch` that are labeled otherwise ("can you work on my Rails
app?" and "can you push to my github repos", `meta`; "which file handles
the x402 spending policy", `codebase.kb`), each at 0.46 to 0.53, below the
0.70 dispatch floor, so none was served a dispatch: served dispatch
precision is 100 %. The calibration partition (295 rows) reads route
accuracy 0.922, canned precision 100 % (89/89), and dispatch precision
100 % (28/28). The `router-v1` gate's floors pass (canned precision 1.000
against 0.98, dispatch precision 1.000 against 0.90); with no baseline arm
the record claims no change. Calibration maps, fitted on the calibration
partition and scored held out: `route` ECE 0.034 → 0.025, `answer` ECE
0.137 → 0.080, both passing `probability-v2`. Serving keeps calibration
off.

## What the chat does with the reading

A `work.dispatch` reading at 0.70 or more serves the bank's
`dispatch.stem` with a `run_coder` offer, as before. The desktop starts
Coder at once for it (`coder.start: at_once`), and the run's title and
prompt now lead with the message that asked for the work, "do a test
delegation now", with at most six earlier turns as context and never the
reply after it (`openagents_chat::delegation::prompt`). A reply that
carries a Gym card or `start_eval` and no `run_coder` offer no longer
starts Coder as well, even when the worker judged the thread's lane a
computer's (`openagents_chat::delegation::offered`).
