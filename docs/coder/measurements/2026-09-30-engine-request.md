# The engine a person names reaches the Coder start (#10076)

On 2026-09-30 the owner asked the desktop chat "Do a test delegation to
claude". The router dispatched Coder, and the run started on Codex
(`gpt-6-luna`), the first engine in the person's settings: nothing carried
the engine they named
([#10076](https://github.com/OpenAgentsInc/openagents/issues/10076)).

The router now asks a typed `engine` Choice on every turn, in the same Jev
request as the others: `codex`, `claude_code`, `grok_build`, `opencode`,
`devin`, or `none` (`crates/coder/src/router/judge.rs`, rubric in
`router/rubric.rs`). Only a dispatch reads it, and only at
`ENGINE_CONFIDENCE` (0.70): the `run_coder` offer then names the engine
(NIP-CJ `engine`) and the reply is `dispatch.engine_stem`. The `route`
question did not change, so the set is still `chat-router-v4@ca74e9da5045`.
The bank moved (`dispatch.engine_stem`, and #10077's placed entries) to
`chat-answers-v1@d3ff1e68a58a`, so `calibration-v2.json` is refit for it.

The labeled set `routes-v4.json` adds 40 rows tagged `engine` (16 held
out): 22 dispatch rows that name an engine, by name, maker, model, or in
Spanish ("Do a test delegation to claude", alone and after the owner's two
earlier turns; "have opus look through my repo"; "run it on codex" after
a dispatch), 6 dispatch rows that name none ("delegate this", "whichever
agent is free"), and 12 near misses that name an engine only as a subject
("ask Claude what a monad is", "what's the difference between codex and
claude code?", "where is the claude code adapter implemented?"). Every
other `work.dispatch` row is labeled as asking for none. The Gym suite
`chat-router-v4` is regenerated (`ROUTER_SUITE_WRITE=1`); its question set
is unchanged.

## The engine rows

Hosted Jev (`TYPESAFE_API_KEY`), `ROUTER_EVAL_ROWS=engine
ROUTER_EVAL_SPLIT=all cargo test -p coder --test router_eval live_router
-- --ignored`, 40 requests:

| Engine rows (40) | Value |
|---|---|
| Rows that ask for an engine, offer named it | 22 / 22 |
| Offer named another engine | 0 |
| Offer named an engine where none was asked | 0 |
| Route accuracy | 38 / 40 |
| Owner's message, alone and in the three-turn chat | `work.dispatch` 0.97 / 0.95, `claude_code` 0.98 / 0.95 |

The two route misses are near misses that were not dispatched either way:
"who made devin?" read as `meta`, and "how does coder pick between codex
and claude code?" as `general` (labeled `codebase.kb`). Every near miss
read `engine` = `none`.

## The published eval

`ROUTER_EVAL_PUBLISH=1 cargo test -p coder --test router_eval live_router
-- --ignored`, 557 requests (held out and calibration), 0 errors. "Before"
is [the delegation route measurement](2026-09-30-delegation-route.md) on
230 held-out rows.

| Held out | Before (230 rows) | This change (246 rows) |
|---|---|---|
| Route accuracy | 0.891 | 0.907 |
| Canned precision | 100 % (70/70) | 100 % (71/71) |
| Dispatch precision | 1.000 (30/30) | 0.977 (42/43) |
| Gym precision | 0.967 (29/30) | 0.967 (29/30) |
| Engine: named right / named where none asked | - | 9 / 9, 0 |
| `presentation.open` hit / predicted / labeled | 9 / 9 / 9 | 9 / 9 / 9 |
| `capability.missing` hit / predicted / labeled | 11 / 11 / 11 | 11 / 11 / 11 |

Over the 557 rows the offer named the asked-for engine on all 16 rows that
ask for one and named one on no other row (engine precision 1.000). The one
wrong dispatch is `codebase.kb/017`, "which file handles the x402 spending
policy", the row the delegation measurement already named at 0.46 to 0.53
on `work.dispatch`; this run read it at 0.67, below the 0.70 route floor,
and the computer lane (0.87) served the dispatch. The route question is
unchanged and the questions are answered independently, so this is the
judge's spread on that row, not the new question. The `router-v1` gate's
floors pass (canned precision 1.000 against 0.98, dispatch precision 0.977
against 0.90); with no baseline arm the record claims no change. The
calibration partition (311 rows) reads canned precision 100 % (89/89) and
dispatch precision 100 % (38/38). Calibration maps fitted on it and scored
held out: `route` ECE 0.041 → 0.031, passing `probability-v2`; `answer`
ECE 0.097 → 0.065, with NLL 0.490 → 0.493, failing it. Serving keeps
calibration off.

## What the start does with it

The desktop's local run, `openagents chat`, and a host's `thread.run` or
handoff read the engine from the reply's typed offer and put its routes
first among the ones the settings (or the owner's auto-start policy) allow.
They fall back only when it is not signed in, refused for a limit, near its
usage threshold, or not allowed, and the start card says so: "You asked
for Claude Code; it is signed in and has capacity." or "You asked for
Claude Code; it reached its usage limit until …, so Codex is running."
(`the_engine_the_person_asked_for_runs_first_or_says_why_not` in
`crates/coder/src/task/local.rs`, with fake engines).
