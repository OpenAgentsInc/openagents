# The chat router's missing-capability route: `chat-router-v3`

Measured 2026-09-29 for [#9960](https://github.com/OpenAgentsInc/openagents/issues/9960),
against hosted Jev (`jev-latest`), with the question set `chat-router-v3`
(19 routes, the `tool` Choice over the three tool notes, and the
`capability` Choice over the admitted-capability set: the six built-ins
and the catalog, since no `coder-defaults` adoption is read offline) and
the bank `chat-answers-v1` with the `capability.missing` entries. What
changed is in
[the chat router design](../design/2026-09-28-chat-router.md#the-admitted-capability-set-and-capabilitymissing-2026-09-29).
The previous numbers are in
[the chat-router-v2 measurement](2026-09-29-chat-router-v2.md).

## The labeled set

`crates/coder/fixtures/chat-router/routes-v3.json` is `routes-v2.json`
unchanged (645 rows, the same ids, labels, and splits) plus 67 rows: 40
for `capability.missing` (booking a flight, a table, a train in French,
reading or sending email and messages, calendars, browsing or opening a
site, devices at home, a timer and a reminder, live prices, weather,
traffic, a package, a voicemail, a screenshot, a printer, step counts)
and 27 near misses an admitted capability answers (questions about what
we can do, Coder work naming a tool, commands, the wallet, the account,
the Gym, making a capability, and the card's own follow-up message with
its turn before it). The split is the id's hash, as before: 19 of the new
rows are held out, 10 of them `capability.missing`. The Gym suite
`chat-router-v3` (`crates/gym/suites/chat-router-v3.json`) is generated
from it, scored with `chat-router-route-v4`, the production router's
`route` question; `chat-router-v2` and `chat-router-route-v3` are kept
as recorded.

```sh
set -a; . ~/work/.secrets/typesafe.env; set +a
ROUTER_EVAL_SPLIT=held_out cargo test -p coder --test router_eval live_router -- --ignored --nocapture --test-threads 1
```

`ROUTER_EVAL_ROWS=capability` restricts a run to the new rows.

## How it was tuned

On the tune split's new rows only (48 rows). The first run read every
`capability.missing` row as its route (30 of 30) with the `capability`
reading's `none` at 0.96 to 1.0, and served the bank's line on 29: "buy
100 shares of AAPL" read `money_movement` at 0.74, so rule 1 answered
`wallet.send`. The `risk` rubric now says money movement is a payment
from the wallet and not buying goods, shares, tickets, or a service
somewhere else, with that message as an `ok` example; the second tune
run served the line on all 30 (canned precision 100 %, 33 of 33). The
card's own follow-up, "Help me make a capability for that", read
`clarify` at 0.49 as a lone first message; the row now carries the turn
before it, as the phone sends it, and reads `eval.author`. No threshold
moved: the route floor is the eval routes' 0.70, the `none` floor 0.60,
the closest floor 0.20, and the named-capability floor 0.60, the
design's starting values.

## Held-out split (207 rows), one run

| Metric | Value |
| --- | --- |
| Canned precision (target ≥ 98 %) | **100 %** (61/61) |
| Canned served, of the 88 rows labeled canned | 61 |
| Dispatch precision (target ≥ 90 %) | **95.7 %** (22/23) |
| Dispatch recall | 100 % (22/22) |
| Gym and interview tiers: precision / recall | 96.6 % (28/29) / 82.4 % |
| Refusal precision / recall | 100 % / 90 % |
| Secret recall | 100 % (7/7) |
| Route accuracy | 88.9 % |
| Judgment latency p50 / p95 | 233 / 350 ms |

### The new route (precision, hit / predicted / labeled)

| Route | Held-out | Tune |
| --- | --- | --- |
| `capability.missing` | **100 %** (10/10/10) | 100 % (30/30/30) |

Every held-out `capability.missing` row read its route and served the
bank's `capability.missing` line: no false card on a canned or dispatch
row, the target. None served `capability.missing_near`: beside a `none`
argmax the most likely admitted entry read at 0.00 to 0.02, under the
0.20 floor, so no line named a closest capability. The `capability`
reading's `none` was 0.96 to 1.0 on every one; on the near misses it was
0.00 to 0.01, and the entry the request calls for was read: Coder at
0.98 or more on three work rows, Code finder at 0.60 on "run code finder
over my monorepo and tell me where payments are handled", which served
the stem that names it (`dispatch.capability_stem`), and the account at
0.69 on "where do I report a crash".

The rest of the held-out set is inside the spread the v2 measurement
recorded: the one false dispatch offer is the same `codebase.kb/017`
("which file handles the x402 spending policy"), and the one Gym tier
on another route's row the same `eval.credit/018`. Canned precision
stayed 100 % over the whole set. The judgment carries one more Choice of
about eight options, and its latency moved from 226 to 233 ms at the
median.

### The near misses

All 9 held-out near misses read their own route, and none read
`capability.missing`; "do you have a calendar capability" (`meta`) read
`meta` at 0.99 with no prepared answer at confidence, so the model
answered it. On the tune split, "what is project map"
(`product.kb`) read `general` at 0.64, the same bare-question miss the
v2 measurement noted for the tool notes.

## Offline checks

`cargo test -p coder` covers the rest without a network: the admitted
set's shape (`router::capability`), the `capability` question and its
reading (`router::judge`), the policy branch and the named dispatch stem
(`router::policy`), the adoption reader over a signed `coder-defaults`
release and its admissions (`gym_kb`), the wire and the card fixture
(`router::wire`), and the worker's turn end to end: the bank line, the
`capability` card, the Gym offer, and the dropped model call
(`coder-worker`). `cargo test --manifest-path crates/openagents-mobile/Cargo.toml`
covers the phone's card and its button.

## Not measured

- **On the deployed worker.** The chat worker still runs `chat-router-v2`
  until it is redeployed with this change; the live numbers above are
  from the eval harness against hosted Jev, not from the worker.
- **Adoptions.** No `coder-defaults` release has been published, so the
  admitted set's adoptions are empty everywhere; the reader is checked
  offline against a signed test release.
- **Calibration of the `capability` question.** The published eval
  ([#9959](https://github.com/OpenAgentsInc/openagents/issues/9959)) was
  rerun for `chat-router-v3@a19f8c201312` after this change, over the
  held-out rows (207) and the calibration partition (264): canned
  precision 100 % (136/136) and `capability.missing` 22 of 22 across
  both; the refit `calibration-v2.json` is committed (the `route` map is
  refused again, raw ECE 0.039 with Brier rising; the `answer` map
  passes, ECE 0.121 to 0.055), and the `router-v1` gate reads the record
  `unverifiable` with no baseline. It fits no map for the `capability`
  question yet, and its operating points are not measured.
