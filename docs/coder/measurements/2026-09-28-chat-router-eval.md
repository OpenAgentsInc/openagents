# The chat router on its labeled route set, and codebase knowledge

Measured 2026-09-28 for [#9925](https://github.com/OpenAgentsInc/openagents/issues/9925)
(the labeled set and eval) and [#9924](https://github.com/OpenAgentsInc/openagents/issues/9924)
(codebase knowledge), against [the chat router design](../design/2026-09-28-chat-router.md)'s
metrics. Router code at `554d82200e` (bank `chat-answers-v1`, 39 answers),
hosted Jev.

## The labeled route set

`crates/coder/fixtures/chat-router/routes-v1.json`: 457 realistic first
messages, labeled by hand across all 12 routes (meta 98, smalltalk 32,
general 46, product.kb 35, codebase.kb 31, work.dispatch 56, cli 31,
wallet 32, account 25, clarify 21, end 20, refuse 30). Each row carries the
route, the prepared answer a correct router serves (or none), other
acceptable answers, the tier, the risk, and the CLI group. It includes the
owner-reported messages ("Who are you?", "Connect to my GitHub", "What can
you do") and their variants, 43 near-misses between neighboring routes, 38
multilingual rows, 20 with typos, and adversarial rows (prompt injection,
pasted secrets with fake values, requests for keys, harmful requests,
money movement).

A fixed 30 % (138 rows) is held out by a hash of the row id
(`router_eval::split_of`, checked by a test). The set is also a Gym suite,
`crates/gym/suites/chat-router-v1.json`, with the held-out rows as its
locked partition and the route question as `chat-router-route-v1`.

One relabel was made after the bank landed, uniformly across splits and
before any held-out read that the numbers below come from: rows expecting
`meta.data_retention` also accept `meta.privacy` (its text says the worker
keeps no message text), and rows expecting `meta.team` also accept
`meta.who` (its `when` covers who built us). The bank has neither entry.

## Results, held-out split (138 rows)

`cargo test -p coder --test router_eval -- --ignored --nocapture --test-threads 1`

| Metric | Target | chat-router-v1 | Legacy first response | Embedding NN |
| --- | --- | --- | --- | --- |
| Canned precision | ≥ 98 % | **100 %** (36/36) | 100 % (26/26) | 100 % (4/4) |
| Canned coverage of canned rows | reported | 58 % (36/62) | 42 % (26/62) | 6.5 % (4/62) |
| Dispatch precision | ≥ 90 % | **75 %** (18/24) | none offered | 57 % (12/21) |
| Dispatch recall | | 100 % (18/18) | 0 % | 67 % |
| Refusal precision / recall | | 100 % / 90 % | none | none |
| Secret recall | | 100 % (7/7) | 100 % | 0 % |
| Route accuracy | | 83 % | 83 % | 50 % |
| Judgment latency p50 / p95 | | 170 / 235 ms | 169 / 209 ms | 389 / 634 ms (embedding call) |

- **Legacy** is the same judgment decided in `Mode::Legacy`, which is what
  a turn that asks only for `opener` gets today: a whole prepared answer
  with no offer, an opener, or nothing.
- **Embedding NN** is the design's baseline: nearest bank `when` text by
  cosine similarity (`text-embedding-3-small`), serving it whole above a
  threshold fit on the tune split to reach 98 % there (0.67), and the
  nearest route description as the route. The router beats it on canned
  coverage at equal precision and on every other metric.
- On the tune split (319 rows) the router's canned precision is 98.6 %
  (69/70; the miss serves `account.keys` to a CLI request for the key
  list) and dispatch precision 77 % (34/44).

### What misses

- **Dispatch precision is below its 90 % target.** The six held-out false
  offers are "can you work on my Rails app?" and "can you push to my github
  repos" (labeled as questions about us), three repository questions about
  OpenAgents itself ("what's in crates/coder/src/first.rs", "which file
  handles the x402 spending policy", "what tests cover the coder dispatch
  invariants"), and the owner-reported **"Connect to my GitHub", which the
  router answers with `dispatch.stem` instead of `meta.github`**. On the
  tune split four CLI requests ("which of my computers are online") also
  become dispatch offers. The codebase questions are arguable (reading a
  file is also Coder work); the GitHub and CLI cases are not.
- **"Connect to my GitHub"** gets the right answer in neither mode: the
  router offers a dispatch; legacy mode shows the model. "connect github"
  on the tune split is routed `grounded`. The `meta.github` `when` text and
  the `lane` question both read "connecting to or using the user's GitHub"
  as computer work; that needs a wording change measured on the tune
  split.
- **Canned coverage** is 58 %: most unserved canned rows are wallet and
  account how-tos and meta questions whose entry is absent from the bank
  (`meta.data_retention`, `meta.computers`, `meta.offline`, `meta.team`).

### Gym

`gym eval --jev --suite crates/gym/suites/chat-router-v1.json --partition development`
asks the `route` question alone: accuracy 0.87, ECE 0.046, Brier 0.083 on
151 development items. The locked partition is unread.

## Codebase knowledge (#9924)

`codebase-kb eval` over the 52 held-out questions in
`crates/coder/fixtures/chat-router/codebase-questions-v1.json`, index at
`e1d499def7` (19,898 chunks, 10.7 MB), Jev as judge, and the chat worker's
door (`google/gemini-3.8-flash` through the AI Gateway) as the answer
model. Evidence: [2026-09-28-codebase-kb.json](2026-09-28-codebase-kb.json).

| Metric | Value |
| --- | --- |
| Answerable questions answered (not escalated) | 41 of 42 |
| Escalation recall (10 need a computer) | 90 % (9/10) |
| Gold file retrieved in the top 10 / kept by the judge | 83 % / 81 % |
| Answers judged correct against the reference (Jev Noul ≥ 0.5) | 80 % of answered (33/41); 79 % of answerable |
| Mean correctness probability | 0.73 |
| Citations inside the excerpts shown (validity) | 100 % (198/198) |
| Citations in a gold file | 42 % |
| Answers citing at least one gold file | 76 % |
| Retrieve + judge | about 0.6 s + 0.3 s |
| Whole answer, p50 / p95 | 6.4 / 10.8 s (the model's own time dominates) |

Six of the eight answers judged wrong had the gold file kept, so the
model's reading of the excerpts, not retrieval, is most of the loss. The
missed escalation (cb-045, "what's the chat worker's current median time
to first token in production?") was answered from the recorded
measurement instead of live numbers; the answerable question that
escalated (cb-041, how Coder answers with no model provider) retrieved no
gold chunk and read as needing a computer (`needs_live` 0.66).
Citation precision against gold is low because answers also cite
neighboring docs that say the same thing; every citation is a range the
model was shown.
