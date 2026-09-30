# The chat router opens a deck: `presentation.open` (#10058)

`chat-router-v4` adds the `presentation.open` route to the `route` question
and, on a desktop turn only, the `deck` question over the decks the desktop
app ships (`openagents_deck::decks()`: Three DevDays Later and Test-Time
Capabilities); see
[the design](../design/2026-09-28-chat-router.md#opening-a-deck-presentationopen-2026-09-30).
The labeled set is `routes-v4.json`: the 712 rows of `routes-v3.json`
unchanged, plus 31 `presentation.open` rows (decks named by title, by part
of a title, described, unnamed, decks we don't have, one in Spanish, and
one that resolves "the second one" from the turn before) and 12 near misses
(writing or advice about talks and slides, opening another site or app,
sharing a screen, our deck code, and slides in the user's own project). The
split is the ids' hash, as before: 9 deck rows are held out. The rubric's
examples are tune rows only (`no_example_is_a_held_out_message`).

The route question changed, so the set is `chat-router-v4@12ea6aac036f`
(was `chat-router-v3@c86d4a2ebeb2`), the Gym suite `chat-router-v4` and
its question set `chat-router-route-v5` are generated from it
(`ROUTER_SUITE_WRITE=1`, `tests/router_suite.rs`), the v3 suite and
question set are kept as recorded, and `calibration-v2.json` is refit. The
bank is `chat-answers-v1@f1b498639ec6` (three new records entries).

## The published eval

2026-09-30, hosted Jev (`TYPESAFE_API_KEY` from `typesafe.env`),
`ROUTER_EVAL_PUBLISH=1 cargo test -p coder --test router_eval live_router
-- --ignored`, 501 requests (held out and calibration), 0 errors, in the
set's default phone context. "Before" is the v3 record in
[the product-kb measurement](2026-09-30-product-kb-connect-needs.md) on the
207 held-out rows v3 had.

| Held out | v3 (207 rows) | v4 (220 rows) |
|---|---|---|
| Route accuracy | 0.889 | 0.905 |
| Canned precision | 100 % (62/62) | 100 % (72/72) |
| Dispatch precision | 0.957 (22/23) | 0.958 (23/24) |
| Gym precision | 0.966 (28/29) | 0.966 (28/29) |
| `presentation.open` hit / predicted / labeled | - | 9 / 9 / 9 |
| `capability.missing` hit / predicted / labeled | - | 11 / 11 / 11 |

On the calibration partition (281 rows) route accuracy is 0.929, canned
precision 100 % (90/90), and `presentation.open` 13 / 13 / 13. The
`router-v1` gate's floors pass (canned precision 1.000 against 0.98,
dispatch precision 0.958 against 0.90); with no baseline arm the record
claims no change. Calibration maps, fitted on the calibration partition
and scored held out: `route` ECE 0.047 → 0.024 (probability-v2 failed, map
refused), `answer` ECE 0.133 → 0.070 (passed). Serving keeps calibration
off.

## The deck question on the desktop

`ROUTER_EVAL_SURFACE=desktop ROUTER_EVAL_ROWS=presentation
ROUTER_EVAL_SPLIT=all` asks the 43 deck rows and near misses as the desktop
does. Route accuracy was 100 % (43/43) and no near miss read as a deck. Of
the 31 deck rows, the served line was:

| Served | Rows |
|---|---|
| `presentation.open`, the right deck | 16 |
| `presentation.unknown`, the message names no deck or one we don't have (right) | 12 |
| `presentation.unknown`, "DevDays" alone read below 0.60 (a miss, answered with the deck list) | 3 |
| `presentation.open`, the wrong deck | 0 |

The three misses ("present the devdays talk", "show me the slides from
your devdays talk", "open slides for the devdays talk") name the Three
DevDays Later deck by one word; the reading put it at 0.38, 0.49, and none,
so the reply lists the decks instead of opening one. No deck was opened
that the message did not name.
