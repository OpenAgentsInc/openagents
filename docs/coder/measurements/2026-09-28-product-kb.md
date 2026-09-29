# The product knowledge base on 110 held-out questions

Measured 2026-09-28 for [#9923](https://github.com/OpenAgentsInc/openagents/issues/9923),
phase 3 of the [chat router](../design/2026-09-28-chat-router.md#knowledge-routes-product-and-codebase).
Every row is in [`2026-09-28-product-kb.json`](2026-09-28-product-kb.json).

[Measurements index](README.md)

## What was measured

- **Corpus:** `knowledge/openagents/`, 52 admitted `product` entries,
  `openagents-product@78f0a55a1782`. After the run, `openagents.chat-target`
  was corrected for build 19 (a new chat now goes to OpenAgents even with a
  computer connected); no question expects that entry.
- **Questions:** `crates/coder/fixtures/product-kb/questions-v1.json`: 100
  product questions, each labeled with the entries that answer it and
  whether one entry's answer fully answers it as asked (94 of the 100), and
  10 questions nothing in the corpus answers (dark mode, iPad, a token, the
  CEO). They were written before retrieval first ran, and nothing was tuned
  on them; this is the one run.
- **Lookup:** the production path, `ProductKnowledge::find`: the 8 entries
  nearest the question by cosine similarity, then one Jev request
  (`product-kb-relevance-v1`: a `relevant_N` Noul per candidate and one
  `answer` Choice). Embeddings were Vertex AI's `text-embedding-005`,
  because the OpenAI and OpenRouter keys on the measuring machine had no
  credit; production's default is `text-embedding-3-small`, so retrieval
  numbers may differ there.
- **T0:** a question counts as served whole when the router's gate would
  serve it: the top passage carries a reviewed answer (the `answer` Choice
  picked it at 0.8 or more) at relevance 0.8 or more. The router's own
  `needs_specifics` question, which also gates T0, is not asked here.
- **T2:** every other question went to Gemini 3.8 Flash (the chat worker's
  model, through Google's OpenAI-compatible endpoint) with
  `knowledge::product::instructions` over the kept passages. Jev then read
  each reply (`supported`, `admits`, `answers`), and every reply was also
  read by hand.
- **Command:** `cargo test -p coder --lib product_kb::eval::live_product_kb_eval -- --ignored --nocapture`
  with `OPENAGENTS_PRODUCT_KB_EMBEDDINGS=vertex`, run from `05a8b2dc2d` plus
  this change. Four questions ran at a time.

## Results

| Measure | Result |
| --- | --- |
| Expected entry among the 8 nearest (recall@8) | 98/100 |
| Recall@1 / recall@3 by cosine alone | 73/100 / 90/100 |
| Expected entry kept by Jev (relevance ≥ 0.5) | 97/100 |
| Expected entry kept first | 97/100 |
| Kept passages that were an expected entry | 104/123 (85 %) |
| Unanswerable questions with nothing kept | 7/10 |
| **T0 served** | **82, all correct (82/82)** |
| T0 coverage of the 94 questions one answer fully answers | 80/94 (85 %) |
| T0 served where the question needed specifics | 2 (both correct entries; see below) |
| T0 served on an unanswerable question | 0/10 |
| T2 replies | 29 |
| T2 replies citing an id they were not given | 0/29 |
| T2 replies to answerable questions citing an expected entry | 17/19 |
| Unanswerable questions answered "we don't have that documented yet" | 10/10 |
| T2 replies with a product claim not in their entries (read by hand) | 1/29 |
| T2 generation cost at list price | $0.015 for 29 replies |
| Lookup latency (embedding + Jev), p50 / p95 | 538 / 823 ms |
| Jev relevance request, p50 / p95 | 185 / 272 ms |
| Grounded model's whole reply, p50 / p95 | 2.5 / 5.0 s |

One lookup failed: q050's Jev request passed its 1.5 s budget, so the
router would have answered with the model alone.

### Where it missed

- **Retrieval.** q032 ("why did that last answer show up so fast") never
  reached `openagents.instant-answers` in the 8 nearest, and q004 ("what
  does the globe icon do") ranked it 5th but Jev kept nothing. Both went to
  T2 with no entries.
- **The one invented claim.** q032's reply, with no entries and the
  no-documented-answer note, explained fast replies with token streaming
  and fast hardware: generic, and wrong for a T0 answer. The note did not
  hold the model to "we don't have that documented" when the question did
  not name the app.
- **T0 not served where one answer fits (14).** Mostly the `answer`
  Choice below 0.8 on a correct entry (q063 0.58, q096 0.69, q086 0.72,
  q094 0.77) or relevance below 0.8 on a correct pick (q009 0.74, q097
  0.71). Each then went to T2, which answered from the right entry.
- **T0 where the label said specifics were needed (2).** q056 ("create an
  invoice for 5000") got `openagents.wallet-receive`, which says how to set
  an amount; q066 (a stranger asking for the seed phrase) got
  `openagents.wallet-recovery`, which says we never ask for recovery words.
  Both answers are right; in the router, `needs_specifics` and `risk` would
  weigh them first.
- **Jev's `supported` reading** was below 0.5 on 6 replies, 5 of which were
  "We don't have that documented yet." with no entries: a statement about
  our notes, not a product claim. Reading by hand found one unsupported
  reply (q032, above).

## What this says

- The T0 tier met the router's canned-precision target (≥ 98 %) on this
  set: 82 of 82 served answers were the right entry, and none was served for
  an unanswerable question.
- Grounded replies cited only entries they were given, and every
  unanswerable question got "we don't have that documented yet" instead of
  an invented feature.
- The lookup fits the router's 2 s `KB_BUDGET` at p95 (823 ms), leaving the
  grounded model its own time.
- Next, on a tuning split and not on this one: the no-documented-answer note
  should hold the model when nothing is kept even if the question does not
  name the app, and a follow-up question should embed with its previous
  turn.
