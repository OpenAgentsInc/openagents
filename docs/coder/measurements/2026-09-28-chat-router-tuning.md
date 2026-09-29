# The chat router with structured questions, tuned, and its seams wired

Measured 2026-09-28 for [#9920](https://github.com/OpenAgentsInc/openagents/issues/9920),
after [the first eval](2026-09-28-chat-router-eval.md). Router code at
`4ddb59a364` and `75cecb8dba`, bank `chat-answers-v1@df722f59fb54`
(41 answers), hosted Jev (`jev-latest`). What changed is in
[the chat router design](../design/2026-09-28-chat-router.md#structured-questions-and-tuning-2026-09-28).

## How it was tuned

Every change was chosen on the **tune** split (319 rows) of
`crates/coder/fixtures/chat-router/routes-v1.json`; the **held-out** split
(138 rows) was run after each change to record it, and no held-out row's
reading was used to pick wording, examples, or thresholds. Every rubric
example is a tune-split message (`no_example_is_a_held_out_message`).

```sh
set -a; . ~/work/.secrets/typesafe.env; set +a
ROUTER_EVAL_SPLIT=held_out cargo test -p coder --test router_eval live_router -- --ignored --nocapture --test-threads 1
```

The eval now asks `cli_group` over the CLI route's 29 groups, as the
deployed worker does (`ROUTER_EVAL_CLI=off` leaves them out), and writes
each row's full reading beside its report (`*-rows.json`).

## Held-out split, one change at a time (138 rows)

| Step | Canned precision | Canned served (of 62) | Dispatch precision | Route accuracy |
| --- | --- | --- | --- | --- |
| Before (`ca4212102a`, no CLI groups) | 100 % (36/36) | 36 | 75.0 % (18/24) | 82.6 % |
| 0. The CLI route's groups asked | 100 % (36/36) | 36 | 75.0 % (18/24) | 83.3 % |
| 1. `route` as structured instructions and `{what, not_for, examples}` rubrics | 100 % (40/40) | 40 | 90.0 % (18/20) | 89.1 % |
| 2. `lane` and `risk` rubrics, structured `needs_specifics` criteria | 100 % (41/41) | 41 | 90.0 % (18/20) | 88.4 % |
| 3. Bank rubrics (`not_for`, `examples`), `meta.data_retention`, `meta.team` | 100 % (43/43) | 43 | 90.0 % (18/20) | 89.1 % |
| 4. Policy: no lane-only or close-call dispatch against a route with its own answer; CLI from a sure route or a sure group | 100 % (43/43) | 43 | **100 %** (18/18) | 89.1 % |
| 5. CLI groups and levels as subtrees, with a beam (two runs) | **100 %** (44/44, 43/43) | 44, 43 | **94.7 %** (18/19) | 87.7 %, 89.9 % |

Dispatch recall stayed at 100 % (18/18), refusal precision at 100 %, and
secret recall at 100 % (7/7) in every step. Judgment latency p50 / p95 went
from 181 / 275 ms to 213 / 276 ms: the structured entries are longer.

Step 5 is kept although its one false offer, "can you push to my github
repos" (labeled `meta.github`), is new: it is the same in both runs, the
tune split and the CLI route's own set did as well or better with it, and
the design had already called that row arguable.

The owner-reported held-out rows after step 5: "Who are you?" gets
`meta.who` whole; **"Connect to my GitHub" gets `meta.github` whole**
(before: a Run Coder offer); "how do I link my github account" goes to
the model.

### Tune split (319 rows)

| Step | Canned precision | Canned served (of 140) | Dispatch precision | Refusal recall | Secret recall | Route accuracy |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 100 % | 71 | 79.1 % (34/43) | 65 % | 58.3 % | 81.2 % |
| 1 | 98.9 % | 87 | 88.1 % | 65 % | 58.3 % | 91.2 % |
| 2 | 100 % | 89 | 92.5 % | 90 % | 91.7 % | 90.9 % |
| 3 | 100 % | 93 | 90.2 % | 90 % | 91.7 % | 91.2 % |
| 4 | 100 % | 92 | 97.4 % (37/38) | 90 % | 91.7 % | 91.5 % |
| 5 | 100 % | 92 | 97.4 % (37/38) | 95 % | 91.7 % | 92.2 % |

The tune split's two owner-reported misses: "connect github" and "Conecta
mi GitHub" now get `meta.github` whole. "which of my computers are online"
is read `cli` at p = 1.00 with `computer` at only 0.32, so the policy's
beam (a sure route descends an unsure group beside `reach`) proposes
`openagents computer list`. Of the 22 CLI rows, 15 get the CLI tier (14
before step 5); the rest read as `wallet` or `account` questions and get
their own answer or the model.

### Old phones

A turn that asks only for `opener` (builds 19 and earlier) is decided in
legacy mode: on the held-out split it served 34 whole answers, all right
(100 %), and no offers.

## The CLI route's own set (62 rows)

`cargo run -p coder --example cli_route_eval` (gateway door for free text),
now passing the next likely groups as the beam:

| Metric | Before (best of three) | After |
| --- | --- | --- |
| Proposal validity | 94.4 % (34/36) | 94.4 % (34/36) |
| No offer where none is labeled | 24/24 | 24/24 |
| `spends` or `secret` offers, phone offers off the list | 0, 0 | 0, 0 |
| Group correct (level 0) | 94.7 % | 94.7 % |
| p50 / p95 | 341 / 520 ms | 374 ms / 3.2 s (free-text fills) |

The example takes a group only at p ≥ 0.60, as before; "which of my
computers are online?" read `computer` at 0.49 there, and is proposed only
through the router's sure-route rule, which the example does not have.

## Product knowledge (110 questions)

`cargo test -p coder --lib product_kb::eval::live_product_kb_eval -- --ignored --nocapture`
with `OPENAGENTS_PRODUCT_KB_EMBEDDINGS=gateway` (the embeddings through the
Vercel AI Gateway with the door key, the deployed configuration):

| Metric | Value |
| --- | --- |
| Retrieval recall at 1 / 3 / 8 | 90 % / 97 % / 99 % |
| Expected entry kept by Jev | 97 % |
| Answers served whole (T0): precision | **100 %** (76/76) |
| T0 coverage of T0 questions | 79 % (74/94) |
| Grounded replies with no invented citation | 100 % (36/36) |
| Unanswerable questions admitted | 100 % (10/10) |
| Embed / judge p50 | 433 / 157 ms |

## Codebase knowledge (52 questions)

`codebase-kb eval` with the index refreshed to `42a83dc18a` through the
gateway (582 of 20,347 chunks re-embedded, $0.003):

| Metric | Value (before: `e1d499def7` index) |
| --- | --- |
| Answerable answered | 41 of 42 (41 of 42) |
| Escalation recall | 90 % (90 %) |
| Gold file retrieved / kept | 79 % / 76 % (83 % / 81 %) |
| Answers judged correct, of answered | 76 % (80 %) |
| Citation validity | 100 % (100 %) |
| Whole answer p50 / p95 | 6.7 / 10.4 s (6.4 / 10.8 s) |

The small drop is within the run-to-run spread of the answer model; the
index grew by 449 chunks of new docs that compete with the gold files.

## Live, on the deployed worker

Release `b8dc0057fb` on `oa-coder-worker-1`, from this Mac through
`relay.openagents.com`, a fresh key per message
(`live_basic_coder_streams_a_reply`):

| Message | Phone | First words | Done | Tier | Answer or offer |
| --- | --- | --- | --- | --- | --- |
| "Who are you?" | build 20 | 0.57 s | 0.57 s | canned | `meta.who` |
| "Connect to my GitHub" | build 20 | 0.54 s | 0.54 s | canned | `meta.github` |
| "What's the Grid?" | build 20 | 1.19 s | 1.21 s | canned from the product KB | `openagents.verse-grid` |
| "Where is the chat worker's quota enforced?" | build 20 | 1.23 s | 11.3 s | grounded (codebase) | cites `crates/coder/src/relay/quota.rs` |
| "Fix the failing test in my repo" | build 20, no computer | 0.59 s | 0.61 s | offer | Connect a computer |
| "Fix the failing test in my repo" | build 20, computer ready | 0.88 s | 1.36 s | offer, personalized | Run Coder: "We'll dispatch Coder to fix the failing test in your repo." |
| "Which of my computers are online?" | build 20 | 0.84 s | 0.84 s | cli | `openagents computer list` |
| "In three short sentences, what does a Nostr relay do?" | build 20 | 0.65 s | 6.7 s | opener, then the model | |
| "Who are you?" | build 19 (legacy) | 0.54 s | 0.56 s | canned | `meta.who` |
| "Connect to my GitHub" | build 19 (legacy) | 0.59 s | 0.61 s | canned | `meta.github` |
| "In three short sentences, what does a Nostr relay do?" | build 19 (legacy) | 0.58 s | 4.9 s | opener, then the model | |

The personalized continuation came from OpenRouter's
`google/gemini-2.5-flash-lite`. On the first deploy (`75cecb8dba`) the
first codebase question ran past the seam's 2 s budget on the embedder's
cold connection and fell back to the model; `b8dc0057fb` keeps that
connection warm, and the question above was the first after the restart.
