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

## Summaries of our essays, and the map on the desktop (#10102)

On 2026-10-01 the owner asked the live chat (worker `c59ec00d1a`, TestFlight
build 42) "summarize both of the essays, please" and got "we need to
dispatch Coder to read the essay files on the computer". With no earlier
turn, the router read "the essays" as a referent it lacked: `clarify` (0.45,
`general` 0.48 runner-up), lane `computer`-leaning (chat 0.24), and the
clarifying reply went on to talk about files. The 28 `essays` rows covered
questions about what the essays say, not requests to summarize, explain, or
compare "the essays" themselves. In the same change the owner asked that
requests to open the route map or the plugin map on the desktop get the
typed `routes.map` offer, and that the explain opener read "We'll look that
up for you." instead of "Here's how that works."

- **Route rubric** (the set's digest moves to `chat-router-v4@71bde73d1610`):
  `product.kb` covers asking to summarize, give an overview of, explain, or
  compare our essays, and says "the essays", "both essays", "the two
  essays", and our essays, posts, or writing mean the two published essays,
  never files on the user's computer; it also names what makes a claim
  reproduced or externally validated (the #10099 gap). `product.kb`'s
  `not_for` sends a README, a file, or a document in the user's own
  repository or computer to `work.dispatch` and someone else's essay or
  writing help to `general`; `work.dispatch`, `general`, and `clarify` name
  our essays in their `not_for`; the `chat` lane covers a summary of them.
  `codebase.kb` covers what a file of the OpenAgents repository (a path
  under `crates/`) contains, which `work.dispatch` names in its `not_for`.
  For the map: `meta` covers asking to open or show the route map or the
  plugin map and where we are thin or missing something; `gym.news` sends
  that to `meta`; `eval.run` lists "test project map"; the `risk` rubric
  says asking to see how we are put together or our route map is ordinary;
  `needs_specifics` says showing or drawing how we are put together, and our
  own essays, are not the user's particulars.
- **Bank** (`chat-answers-v1@e158a330ddca`): `meta.map` and
  `meta.map.desktop` list "can you open the map for me" and send testing a
  plugin such as Project map to `eval.run`; the `explain` opener reads "We'll
  look that up for you." (openers carry no version; the bank digest names
  the change).
- **Knowledge.** `openagents.ttc-overview` and `openagents.gen-overview`
  (version 2) cover asking about, summarizing, or comparing our essays
  together, so a request for both finds both overviews. Their answers are
  unchanged.
- **Labeled rows.** `routes-v4.json` adds 22 rows tagged `essays` (13
  `product.kb` requests, four of them the owner's and one the #10099 gap; six
  `work.dispatch` near misses: "summarize the README in this repo", "…in my
  repo", "summarize this file", an essay draft on the user's laptop, two
  markdown essays in their notes folder, tightening an essay draft in their
  repository; three `general` near misses: Paul Graham's essay, help writing
  an essay, what makes a scientific claim valid) and six tagged `map` (five
  requests to open or show the route or plugin map, and "try the project
  map plugin on coder" as an `eval.run` near miss). All are in the tune split.

### Retrieval for "both essays"

The product knowledge base's lookup (embedding candidates, then Jev's
relevance per candidate, `coder::product_kb`) was run live, with
`OPENAGENTS_PRODUCT_KB_EMBEDDINGS=gateway` as the worker runs it, on the
owner's four questions and two more. Before the entry change, both overviews
were kept for every request about both essays, but "what are your essays
about?" and "compare the two essays" kept them at relevance 0.53 to 0.60,
just over the 0.5 floor. After it:

| Message | Kept (relevance) | Whole answer served |
|---|---|---|
| summarize both of the essays, please | ttc-overview 0.96, gen-overview 0.95, ttc-thesis, gen-thesis | none (grounded) |
| summarize both essays | gen-overview 0.95, ttc-overview 0.95 | none |
| what are your essays about? | ttc-overview 0.84, gen-overview 0.81 | none |
| compare the two essays | gen-overview 0.90, ttc-overview 0.86 | none |
| summarize your essay on general agents | gen-overview 0.90, gen-thesis, gen-launch, gen-why-stalled | none (`answer` 0.66) |
| what makes a claim externally validated? | ttc-term-validated 0.97 | its reviewed answer (1.00) |

No single overview's answer is served whole for a request about both, so
the grounded model summarizes each from its overview, with its GitHub link.

### The rows against hosted Jev (TypeSafe)

`ROUTER_EVAL_ROWS=essays ROUTER_EVAL_SPLIT=all`, 50 rows, phone context:

| | Before | After |
|---|---|---|
| Route accuracy | 92.0 % (46 / 50) | 100 % (50 / 50) |
| "summarize both of the essays, please" | `clarify` 0.45 | `product.kb` 1.00, grounded |
| "compare the two essays" | `clarify` 0.75 | `product.kb` 1.00, grounded |
| "summarize both essays" | `clarify` 0.51 | `product.kb` 1.00, grounded |
| "what makes a claim externally validated?" | `general` 0.92 | `product.kb`, grounded |
| README and file summaries kept on `work.dispatch` | 6 / 6 | 6 / 6 |

`ROUTER_EVAL_ROWS=map ROUTER_EVAL_SPLIT=all ROUTER_EVAL_SURFACE=desktop`, 34
rows:

| | Before | After |
|---|---|---|
| Route accuracy | 100 % | 100 % |
| Canned (`meta.map.desktop` with **Open the map**) | 20 / 25 | 23 / 25 |
| The five new route and plugin map requests | 5 / 5 | 5 / 5 |
| "how are you put together? show me the whole thing" | model (risk read `asks_for_secret` 0.71) | canned, offer |
| "draw the composition: …" | model (specifics 0.74) | canned, offer |
| "can you open the map for me" | model (answer 0.79) | canned, offer |
| "test project map" | `eval.run` 0.65, model, no card | `eval.run` 0.98, Project map's card |

Still left to the model: "what's missing in openagents right now, show me
the gaps" (held out; route 0.99, answer 0.76) and "what is jev" (a near
miss whose `meta.jev` answer waits on route 0.75).

### The published eval

TypeSafe answered 402 (the organization's TypeSafe credits ran out) partway
through the first published run, so the published runs went through Jev's
first fallback door, the Vercel AI Gateway's TypeSafe-compatible API
(`TYPESAFE_BASE_URL=https://ai-gateway.vercel.sh/typesafe`,
`TYPESAFE_DEFAULT_MODEL=typesafe-ai/jev`), the door the chat worker itself
fails over to. For a same-door baseline, the rubric before this change
(`c59ec00d1a`) was run on the held-out split through the gateway too.

`ROUTER_EVAL_PUBLISH=1 cargo test -p coder --test router_eval live_router
-- --ignored`, 626 requests (256 held out, 370 calibration); 3 held-out and
9 calibration requests timed out at the 2.5 s budget and are not scored.
The record is [`2026-10-01-essays-summary-claims/report.json`](2026-10-01-essays-summary-claims/report.json).

| Held out | #10099 (TypeSafe) | Before, gateway | This change, gateway |
|---|---|---|---|
| Route accuracy | 0.902 | 0.898 (230 / 256) | 0.897 (227 / 253) |
| Canned precision | 100 % | 100 % | 100 % (73 / 73) |
| Dispatch precision | 0.977 | 0.977 (42 / 43) | 0.977 (42 / 43) |

Held-out route accuracy is within a row of the same-door baseline and
0.003 under the 0.90 the #10099 record reached on TypeSafe; a second
published run of the same build (15 held-out timeouts, not committed)
read 0.913 (220 / 241), canned 100 %, dispatch 0.976. Every held-out
row that differs from the same-door baseline in either run was read at
route probability 0.65 or less on both sides (for example "fix it", "can
you push to my github repos", "check the code finder result"), so the
difference is the door's run-to-run spread, not a moved boundary. A first
run of the change read "what's in crates/coder/src/first.rs" as
`work.dispatch` (0.86); `codebase.kb` now covers what a file of our
repository contains, checked on the tune split (703 rows: route 0.934,
canned 99.5 %, dispatch 0.961) before the committed run, which reads it
right. The `router-v1` gate's floors pass (canned precision 1.000 against 0.98,
dispatch precision 0.977 against 0.90); with no baseline arm the record
claims no change. Calibration fitted on the calibration partition and
scored held out: `route` ECE 0.070 to 0.073, failing `probability-v2`;
`answer` ECE 0.136 to 0.058, NLL 0.489 to 0.451, passing it. Serving keeps
calibration off. Jev spend: one 612-row published run, plus about 60 rows
of subset runs while tuning, at about $0.000014 a request.

## Summaries of our essays, and the map on the desktop (#10102)

On 2026-10-01 the owner asked the live chat (worker `c59ec00d1a`, TestFlight
build 42) "summarize both of the essays, please" and got "we need to
dispatch Coder to read the essay files on the computer". With no earlier
turn, the router read "the essays" as a referent it lacked: `clarify` (0.45,
`general` 0.48 runner-up), lane `computer`-leaning (chat 0.24), and the
clarifying reply went on to talk about files. The 28 `essays` rows covered
questions about what the essays say, not requests to summarize, explain, or
compare "the essays" themselves. In the same change the owner asked that
requests to open the route map or the plugin map on the desktop get the
typed `routes.map` offer, and that the explain opener read "We'll look that
up for you." instead of "Here's how that works."

- **Route rubric** (the set's digest moves to `chat-router-v4@71bde73d1610`):
  `product.kb` covers asking to summarize, give an overview of, explain, or
  compare our essays, and says "the essays", "both essays", "the two
  essays", and our essays, posts, or writing mean the two published essays,
  never files on the user's computer; it also names what makes a claim
  reproduced or externally validated (the #10099 gap). `product.kb`'s
  `not_for` sends a README, a file, or a document in the user's own
  repository or computer to `work.dispatch` and someone else's essay or
  writing help to `general`; `work.dispatch`, `general`, and `clarify` name
  our essays in their `not_for`; the `chat` lane covers a summary of them.
  `codebase.kb` covers what a file of the OpenAgents repository (a path
  under `crates/`) contains, which `work.dispatch` names in its `not_for`.
  For the map: `meta` covers asking to open or show the route map or the
  plugin map and where we are thin or missing something; `gym.news` sends
  that to `meta`; `eval.run` lists "test project map"; the `risk` rubric
  says asking to see how we are put together or our route map is ordinary;
  `needs_specifics` says showing or drawing how we are put together, and our
  own essays, are not the user's particulars.
- **Bank** (`chat-answers-v1@e158a330ddca`): `meta.map` and
  `meta.map.desktop` list "can you open the map for me" and send testing a
  plugin such as Project map to `eval.run`; the `explain` opener reads "We'll
  look that up for you." (openers carry no version; the bank digest names
  the change).
- **Knowledge.** `openagents.ttc-overview` and `openagents.gen-overview`
  (version 2) cover asking about, summarizing, or comparing our essays
  together, so a request for both finds both overviews. Their answers are
  unchanged.
- **Labeled rows.** `routes-v4.json` adds 22 rows tagged `essays` (13
  `product.kb` requests, four of them the owner's and one the #10099 gap; six
  `work.dispatch` near misses: "summarize the README in this repo", "…in my
  repo", "summarize this file", an essay draft on the user's laptop, two
  markdown essays in their notes folder, tightening an essay draft in their
  repository; three `general` near misses: Paul Graham's essay, help writing
  an essay, what makes a scientific claim valid) and six tagged `map` (five
  requests to open or show the route or plugin map, and "try the project
  map plugin on coder" as an `eval.run` near miss). All are in the tune split.

### Retrieval for "both essays"

The product knowledge base's lookup (embedding candidates, then Jev's
relevance per candidate, `coder::product_kb`) was run live, with
`OPENAGENTS_PRODUCT_KB_EMBEDDINGS=gateway` as the worker runs it, on the
owner's four questions and two more. Before the entry change, both overviews
were kept for every request about both essays, but "what are your essays
about?" and "compare the two essays" kept them at relevance 0.53 to 0.60,
just over the 0.5 floor. After it:

| Message | Kept (relevance) | Whole answer served |
|---|---|---|
| summarize both of the essays, please | ttc-overview 0.96, gen-overview 0.95, ttc-thesis, gen-thesis | none (grounded) |
| summarize both essays | gen-overview 0.95, ttc-overview 0.95 | none |
| what are your essays about? | ttc-overview 0.84, gen-overview 0.81 | none |
| compare the two essays | gen-overview 0.90, ttc-overview 0.86 | none |
| summarize your essay on general agents | gen-overview 0.90, gen-thesis, gen-launch, gen-why-stalled | none (`answer` 0.66) |
| what makes a claim externally validated? | ttc-term-validated 0.97 | its reviewed answer (1.00) |

No single overview's answer is served whole for a request about both, so
the grounded model summarizes each from its overview, with its GitHub link.

### The rows against hosted Jev (TypeSafe)

`ROUTER_EVAL_ROWS=essays ROUTER_EVAL_SPLIT=all`, 50 rows, phone context:

| | Before | After |
|---|---|---|
| Route accuracy | 92.0 % (46 / 50) | 100 % (50 / 50) |
| "summarize both of the essays, please" | `clarify` 0.45 | `product.kb` 1.00, grounded |
| "compare the two essays" | `clarify` 0.75 | `product.kb` 1.00, grounded |
| "summarize both essays" | `clarify` 0.51 | `product.kb` 1.00, grounded |
| "what makes a claim externally validated?" | `general` 0.92 | `product.kb`, grounded |
| README and file summaries kept on `work.dispatch` | 6 / 6 | 6 / 6 |

`ROUTER_EVAL_ROWS=map ROUTER_EVAL_SPLIT=all ROUTER_EVAL_SURFACE=desktop`, 34
rows:

| | Before | After |
|---|---|---|
| Route accuracy | 100 % | 100 % |
| Canned (`meta.map.desktop` with **Open the map**) | 20 / 25 | 23 / 25 |
| The five new route and plugin map requests | 5 / 5 | 5 / 5 |
| "how are you put together? show me the whole thing" | model (risk read `asks_for_secret` 0.71) | canned, offer |
| "draw the composition: …" | model (specifics 0.74) | canned, offer |
| "can you open the map for me" | model (answer 0.79) | canned, offer |
| "test project map" | `eval.run` 0.65, model, no card | `eval.run` 0.98, Project map's card |

Still left to the model: "what's missing in openagents right now, show me
the gaps" (held out; route 0.99, answer 0.76) and "what is jev" (a near
miss whose `meta.jev` answer waits on route 0.75).

### The published eval

TypeSafe answered 402 (the organization's TypeSafe credits ran out) partway
through the first published run, so the published runs went through Jev's
first fallback door, the Vercel AI Gateway's TypeSafe-compatible API
(`TYPESAFE_BASE_URL=https://ai-gateway.vercel.sh/typesafe`,
`TYPESAFE_DEFAULT_MODEL=typesafe-ai/jev`), the door the chat worker itself
fails over to. For a same-door baseline, the rubric before this change
(`c59ec00d1a`) was run on the held-out split through the gateway too.

`ROUTER_EVAL_PUBLISH=1 cargo test -p coder --test router_eval live_router
-- --ignored`, 626 requests (256 held out, 370 calibration); 3 held-out and
9 calibration requests timed out at the 2.5 s budget and are not scored.
The record is [`2026-10-01-essays-summary-claims/report.json`](2026-10-01-essays-summary-claims/report.json).

| Held out | #10099 (TypeSafe) | Before, gateway | This change, gateway |
|---|---|---|---|
| Route accuracy | 0.902 | 0.898 (230 / 256) | 0.897 (227 / 253) |
| Canned precision | 100 % | 100 % | 100 % (73 / 73) |
| Dispatch precision | 0.977 | 0.977 (42 / 43) | 0.977 (42 / 43) |

Held-out route accuracy is within a row of the same-door baseline and
0.003 under the 0.90 the #10099 record reached on TypeSafe; a second
published run of the same build (15 held-out timeouts, not committed)
read 0.913 (220 / 241), canned 100 %, dispatch 0.976. Against the
same-door baseline one held-out row flipped each way among those both runs
answered: "what's in crates/coder/src/first.rs" read `work.dispatch` in the
first run of the change (the rubric then had `codebase.kb` cover what a
file of our repository contains; the committed run is after that) and
"check the code finder result" moved between `eval.check` and
`eval.result` at under 0.45. The `router-v1` gate's floors pass (canned
precision 1.000 against 0.98, dispatch precision 0.977 against 0.90).
Calibration fitted on the calibration partition and scored held out:
`route` ECE 0.038 to 0.019, `answer` ECE 0.111 to 0.075, both passing
`probability-v2`. Serving keeps calibration off. Jev spend: three
626-row published runs (the first lost to TypeSafe's 402), one 703-row
tune run, one 269-row baseline, and about 200 rows of subset runs, at about
$0.000014 a request.
