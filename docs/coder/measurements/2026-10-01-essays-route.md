# Chat answers from our two essays (#10099)

On 2026-10-01 the live chat (worker `9afc94bd92`) answered "what is a
test-time capability?" and "what's a capability claim?" from the model
alone, with the security and procurement meanings, and "what's your thesis
about general agents?" with a local-first answer that is not ours: the
router read each as `general`, because no product note covered our two
essays, [Test-Time Capabilities](../../essays/2026-09-29-test-time-capabilities.md)
and [The Return of the General Agent](../../essays/2026-10-01-the-return-of-the-general-agent.md).
The fix is knowledge and a route description.

- **Knowledge.** `knowledge/openagents/` gains 42 entries: an overview of
  each essay and an entry per section (the lexicon's eleven terms, the
  lifecycle, the numbers, the protocol, the pressures, the decision models,
  what is and is not shown), each written from the essay, naming it, and
  linking it on GitHub.
- **Route.** `product.kb` covers what our own essays say, by name and by
  idea (`router::rubric`), and `general` names it in `not_for`. The set
  stays `chat-router-v4`; its digest moved to `chat-router-v4@ef02faf055e7`.
  The Gym suite `chat-router-v4` and question set `chat-router-route-v5`
  are regenerated and `calibration-v2.json` is refit.
- **Labeled rows.** `routes-v4.json` adds 28 rows tagged `essays`
  (`ROUTER_EVAL_ROWS=essays`): 20 questions about the essays and the ideas
  in them, and eight near misses (concept questions not about our essays,
  `general`; where the code implements something, `codebase.kb`). All are
  in the tune split, so the published held-out record stays within
  NIP-EVAL's 256 cases.

## The essay rows against hosted Jev

`ROUTER_EVAL_ROWS=essays ROUTER_EVAL_SPLIT=all`, 28 rows, phone context:

| | Rubric before | Rubric after |
|---|---|---|
| Route accuracy | 78.6 % (22 of 28) | 100 % (28 of 28) |
| `product.kb` essay questions read `product.kb` | 14 / 20 | 20 / 20 |
| Near misses kept off `product.kb` | 8 / 8 | 8 / 8 |

Before the rubric named the essays' ideas, "what's the capability
flywheel?", "why did general agents stall?", "what do reach and restraint
mean for capabilities?", "what's a judgment budget?", and "why does
extending an agent at machine speed need a brake?" read `general` (0.48 to
0.90), and "what have you shown and not shown about the general agent?" read
`meta`. The four questions the owner asked read `product.kb` (0.88 to 1.00)
with the first, shorter rubric, and 1.00 after.

## The published eval

`ROUTER_EVAL_PUBLISH=1 cargo test -p coder --test router_eval live_router
-- --ignored`, 612 requests (256 held out, 356 calibration), 0 errors.
The record is [`2026-10-01-essays-claims/report.json`](2026-10-01-essays-claims/report.json),
whose per-route numbers the Map page reads.

| Held out | #10094 | This change |
|---|---|---|
| Route accuracy | 0.902 | 0.902 |
| Canned precision | 100 % | 100 % |
| Dispatch precision | 0.977 | 0.977 |

The `router-v1` gate's floors pass (canned precision 1.000 against 0.98,
dispatch precision 0.977 against 0.90); with no baseline arm the record
claims no change. Calibration fitted on the calibration partition and
scored held out: `route` ECE 0.070 to 0.073, failing `probability-v2`;
`answer` ECE 0.136 to 0.058, NLL 0.489 to 0.451, passing it. Serving keeps
calibration off. Jev spend: one 612-row published run, plus about 60 rows
of subset runs while tuning, at about $0.000014 a request.
