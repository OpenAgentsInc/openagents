# Opening the route map from chat (#10085)

On 2026-10-01 the desktop app gained a Map page
([design](../../desktop/route-map.md)), and the owner asked that "show me
how you route things" open it from chat through the typed router. The
request stays on `meta`; the `meta` rubric now covers asking to see how we
route or are put together (`codebase.kb`, `presentation.open`, and
`clarify` name it in their `not_for`), and two bank entries answer it:
`meta.map` off the desktop, and `meta.map.desktop` in it, with a typed
`open_screen` offer for `routes.map` ("Open the map"). The route list is
unchanged, so the set stays `chat-router-v4`; its digest moved with the
rubric to `chat-router-v4@398b034caf30`, and the bank to
`chat-answers-v1@dcd8f3f06ad8` (measured), then `@8b7c39100770` when this
change was rebased onto the bank's "plugin" wording (`e41b693d7c`, which
changes no `when` text). The Gym suite `chat-router-v4` and question
set `chat-router-route-v5` are regenerated, and `calibration-v2.json` is
refit.

`routes-v4.json` adds 22 rows tagged `map` (14 asking to see the map, 8 near
misses: what Jev is, what we can do, opening a deck, mapping the user's own
repository, a map of Europe, where the rubric lives in the code, making a
plugin, and how network routers work).

## The map rows, against hosted Jev

`ROUTER_EVAL_ROWS=map ROUTER_EVAL_SPLIT=all ROUTER_EVAL_SURFACE=desktop
cargo test -p coder --test router_eval live_router -- --ignored`, 22
requests, after the rubric change:

| Map rows (22) | Value |
|---|---|
| Route accuracy | 22 / 22 |
| `meta` precision / recall | 16 / 16, 16 / 16 |
| Canned precision | 100 % (11 / 11), every one `meta.map.desktop` with the `routes.map` offer |
| Canned recall | 11 / 17 canned rows |
| Near misses served `meta.map` | 0 / 8 |

Before the rubric change the same rows read route accuracy 15 / 22: the
answer question already chose `meta.map.desktop` at 0.98 to 0.99, and the
route question sent "show me how you route things" to `codebase.kb` (0.45)
and "open the route map" to `presentation.open`. The five map rows the
router stands back on (the model answers, with no offer) are "how are you
put together? show me the whole thing" (risk read as asking for a secret,
0.67), "open the route map" and "open the map" (the `tool` reading takes
"map" for Project map, specifics 0.64 and route 0.48), "where are you
weak? what should we fill in" (route 0.72), and "draw the composition"
(route 0.74). The owner's phrasing, "show me how you route things", is
served at route 1.00 and answer 0.99.

## The published eval

`ROUTER_EVAL_PUBLISH=1 cargo test -p coder --test router_eval live_router
-- --ignored`, 577 requests (held out and calibration), 0 errors. The
record is [`2026-10-01-route-map-claims/report.json`](2026-10-01-route-map-claims/report.json),
whose per-route numbers the Map page reads. "Before" is
[the engine request measurement](2026-09-30-engine-request.md).

| Held out | Before (246 rows) | This change (254 rows) |
|---|---|---|
| Route accuracy | 0.907 | 0.906 |
| Canned precision | 100 % (71/71) | 100 % (73/73) |
| Dispatch precision | 0.977 (42/43) | 0.977 (42/43) |
| Gym precision | 0.967 (29/30) | 0.968 (30/31) |
| `meta` precision / recall | - | 1.000 (35/35) / 0.833 (35/42) |
| `capability.missing`, `presentation.open` hit / predicted / labeled | 11/11/11, 9/9/9 | 11/11/11, 9/9/9 |

The `router-v1` gate's floors pass (canned precision 1.000 against 0.98,
dispatch precision 0.977 against 0.90); with no baseline arm the record
claims no change. Calibration fitted on the calibration partition (323
rows) and scored held out: `route` ECE 0.063 → 0.049 but NLL 0.221 →
0.290, failing `probability-v2`; `answer` ECE 0.131 → 0.092, NLL 0.484 →
0.454, passing it. Serving keeps calibration off.

The phone context (the set's default) served `meta.map` on the held-out and
calibration map rows with no offer, as it should: the desktop variant is
shown only when the request's `context.surface` is `desktop`.
