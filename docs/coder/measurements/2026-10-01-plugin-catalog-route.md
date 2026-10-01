# The plugin catalog in the chat, and "the map" (#10090)

On 2026-10-01, after the example plugins landed (#10086), the live chat
(worker `f04b34bad8`) named three of the Gym's six plugins for "Which
plugins are in the Gym?", offered only Project map for "What plugins can I
test?", and on the desktop read "open the map" as the Project map plugin
(route `meta` at 0.48, the `tool` and `capability` readings Project map),
answering with the plain model and no **Open the map**. The fix and its
design are in
[the chat router design](../design/2026-09-28-chat-router.md#the-plugin-catalog-in-the-chat-and-the-map-2026-10-01).
The route rubric changed, so the set's digest moved
(`chat-router-v4@` the new digest in `report.json`), the Gym suite
`chat-router-v4` and question set `chat-router-route-v5` are regenerated,
and `calibration-v2.json` is refit, which the committed-record test
requires.

`routes-v4.json` adds 17 rows tagged `plugins` (`ROUTER_EVAL_ROWS=plugins`):
asking to see the map, "test project map" and other near misses, which
plugins there are, which can be tested, what one does (including "what's
dependency check" against OWASP Dependency-Check), and testing the new
plugins. Held-out rows are capped by NIP-EVAL's 256 cases a record, so six
of the new rows were given ids that fall in the tune split; the three
owner phrasings stay where their ids put them (two held out, "What does
the Dependency check plugin do?" in calibration).

## The map rows, desktop, against hosted Jev

`ROUTER_EVAL_ROWS=map ROUTER_EVAL_SPLIT=all ROUTER_EVAL_SURFACE=desktop`,
28 rows:

| Map rows | #10085 (22 rows) | This change (28 rows) |
|---|---|---|
| Route accuracy | 22 / 22 | 28 / 28 |
| Requests to see the map served `meta.map.desktop` with **Open the map** | 11 / 14 | 13 / 17 |
| "open the map", "open the route map" | model, no offer | canned, offer |
| "test project map" / "test the project map plugin" | - | `eval.run`, Project map card |

The four requests to see the map still left to the model are "how are you
put together? show me the whole thing" (risk), "what's missing in
openagents right now, show me the gaps" (route 0.69), "draw the
composition: the router, the routes, and what serves them" (specifics
0.74), and "can you open the map for me" (answer 0.74).

## The plugin rows (published run, phone context)

| Message | Route read | Served |
|---|---|---|
| Which plugins are in the Gym? | `product.kb` 1.00 | grounded on the product notes, which now include the generated plugin list |
| What plugins can I test? | `eval.run` 0.99, no plugin | `eval.run.choose`: every catalog plugin, then Project map's test set |
| What does the Dependency check plugin do? | `product.kb` 1.00, tool Dependency check | its product note |
| what's dependency check | `product.kb` 0.92 (was `general` 0.93) | its product note |
| test project map | `eval.run` 0.71 | Project map's card and offer |
| test dependency check on coder | `eval.run` 1.00 | its card and offer |
| run the tests for explain this error | `eval.run` 0.95 (was `work.dispatch` 0.59) | its card and offer |

"test project map" clears the `eval.run` floor (0.70) narrowly; the live
check below confirms its card.

## The published eval

`ROUTER_EVAL_PUBLISH=1 cargo test -p coder --test router_eval live_router
-- --ignored`, 586 requests (256 held out, 330 calibration), 0 errors. The
record is [`2026-10-01-plugin-catalog-claims/report.json`](2026-10-01-plugin-catalog-claims/report.json),
whose per-route numbers the Map page reads.

| Held out | #10085 (254 rows) | This change (256 rows) |
|---|---|---|
| Route accuracy | 0.906 | 0.906 |
| Canned precision | 100 % (73/73) | 100 % (75/75) |
| Dispatch precision | 0.977 (42/43) | 0.977 (42/43) |
| Gym precision | 0.968 (30/31) | 0.969 (31/32) |
| `eval.run` hit / predicted / labeled | - | 9 / 9 / 9 |

The `router-v1` gate's floors pass (canned precision 1.000 against 0.98,
dispatch precision 0.977 against 0.90); with no baseline arm the record
claims no change. Calibration fitted on the calibration partition and
scored held out: `route` ECE 0.055 → 0.077, failing `probability-v2`;
`answer` ECE 0.152 → 0.070, NLL 0.486 → 0.435, passing it. Serving keeps
calibration off. Jev spend: one 586-row published run, plus about 100
rows of subset runs while tuning.
