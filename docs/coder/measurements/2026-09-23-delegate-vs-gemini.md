# The delegate door against the Gemini fallback

Eight read-only questions about this repository, each asked once through
the [delegate door](../runtime/delegate-door.md) and once through the
Open Responses fallback, with `coder -p` from the installed build. The
delegate door answered all eight correctly; Gemini answered six, missed
one, and half-answered one. The delegate door took a median 13.7 seconds
and $0.0541 a turn; Gemini took 8.8 seconds and $0.0087.

Measured on 2026-09-23 for issue
[#9578](https://github.com/OpenAgentsInc/openagents/issues/9578).

## Setup

| | Delegate | Gemini |
| --- | --- | --- |
| Build | `coder 0.1.0 (OpenAgentsInc/openagents a55f667b04 clean)`, installed by `scripts/install-coder.sh` | The same binary |
| Door | `delegate`: Coder One's probe battery and Jev survey (`jev-1.13.0`), then Claude Code 2.1.280 on `claude-opus-5-5`, effort `low`, tools `Bash,Read,Edit,Write,Glob,Grep`, five-minute prompt cache | `live`: `google/gemini-3.8-flash` at `https://openagents.com` with the OpenAgents bearer, the shell loop on |
| Selected by | `CODER_DELEGATE` unset (`auto`) | `CODER_DELEGATE=off`, `CODER_DOOR_URL`, `CODER_DOOR_KEY` |
| Credential | `CLAUDE_CODE_OAUTH_TOKEN`, the long-lived subscription token | The OpenAgents bearer |
| Classify | Off in both: no `TYPESAFE_API_KEY` in the environment, so every turn routed `respond` under an executing permit | Same |
| Working directory | This repository, at the build's commit | Same |

Each prompt ran delegate first, then Gemini, one at a time, with
`coder -p --json --json-deltas`. A harness timestamped every line of the
JSON stream on arrival. The harness is not checked in; the per-run
records it wrote, with each event's arrival time and each reply, are in
[`2026-09-23-delegate-vs-gemini.raw.jsonl`](2026-09-23-delegate-vs-gemini.raw.jsonl).

The columns:

- **First output** is the first event after the classify note: a
  delegated turn's first progress line, such as `requirements ▸ …`, or
  Gemini's first delta, which is its command plan.
- **Answer text** is the first reply delta after the turn's last command:
  when the answer itself starts to appear.
- **Total** is the process's wall time, start to exit.
- **Cost** is exact for the delegate door: `coder -p` reports `cost_usd`,
  the sum of Jev at its published $0.042 per million input tokens and the
  executor's `total_cost_usd`, which Claude Code reports at list price on a
  subscription. Gemini's door reports tokens and no cost, so its cost is
  its reported tokens at $0.75 per million input and $3.75 per million
  output, the rates the same door reported in the
  [Terminal-Bench smoke traces](../../../bench/terminal-bench/traces/).
- **Answered correctly** is graded against the repository by hand.

## Results

| # | Prompt | Door | First output (s) | Answer text (s) | Total (s) | Cost (USD) | Jev (USD) | Executor or model (USD) | Tokens in / out | Answered correctly |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | What does `crates/gym` do? | delegate | 0.67 | 12.65 | 13.58 | 0.067730 | 0.000586 | 0.067144 | 26,270 / 987 | Yes |
| 1 | | gemini | 2.45 | 4.57 | 9.38 | 0.006766 | — | 0.006766 | 7,397 / 325 | Yes |
| 2 | Which crate implements the Nostr relay, and what database does it store events in? | delegate | 12.23 | 24.90 | 25.23 | 0.042590 | 0.000635 | 0.041955 | 22,043 / 456 | Yes |
| 2 | | gemini | 2.59 | 5.30 | 5.53 | 0.006153 | — | 0.006153 | 7,109 / 219 | Yes |
| 3 | Where is the subprocess supervisor, and what happens to a job that passes its deadline? | delegate | 0.30 | 13.20 | 13.76 | 0.062984 | 0.000643 | 0.062341 | 37,639 / 724 | Yes |
| 3 | | gemini | 2.42 | 4.54 | 5.04 | 0.005605 | — | 0.005605 | 6,378 / 219 | Yes |
| 4 | How many crates are in this workspace? List them. | delegate | 0.26 | 7.52 | 8.08 | 0.048128 | 0.000653 | 0.047474 | 24,884 / 411 | Yes |
| 4 | | gemini | 2.51 | 4.64 | 5.12 | 0.005546 | — | 0.005546 | 6,195 / 240 | Yes |
| 5 | What exit code does `coder -p` return when the router declines a turn? | delegate | 1.10 | 16.16 | 16.58 | 0.045354 | 0.000565 | 0.044789 | 31,769 / 550 | Yes |
| 5 | | gemini | 2.12 | 8.74 | 8.79 | 0.012379 | — | 0.012379 | 15,510 / 199 | Yes |
| 6 | Which environment variable turns off Coder's trace, and where do traces go by default? | delegate | 0.28 | 9.05 | 9.32 | 0.033585 | 0.000606 | 0.032979 | 10,913 / 320 | Yes |
| 6 | | gemini | 2.36 | 8.48 | 8.80 | 0.005816 | — | 0.005816 | 6,404 / 270 | Yes |
| 7 | What Jev model does Coder One pin, and what does Jev cost per million input tokens? | delegate | 0.31 | 12.61 | 12.89 | 0.050230 | 0.000598 | 0.049633 | 35,922 / 490 | Yes |
| 7 | | gemini | 2.43 | 9.39 | 13.55 | 0.011069 | — | 0.011069 | 12,314 / 489 | No: named `jev-latest` and found no price |
| 8 | How does `coder-boundary` enforce a read-only boundary on Linux? | delegate | 1.36 | 23.43 | 23.72 | 0.082239 | 0.000578 | 0.081661 | 38,721 / 1,168 | Yes |
| 8 | | gemini | 2.44 | 9.72 | 21.24 | 0.016331 | — | 0.016331 | 18,209 / 713 | Partly: said it could not establish the `bwrap` arguments |

Delegate token counts are the executor's, with cache reads and writes
counted as input; Jev's input tokens are priced in the Jev column.

| Door | Answered correctly | Median first output (s) | Median answer text (s) | Median total (s) | Mean total (s) | Slowest (s) | Total cost (USD) | Mean cost (USD) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| delegate | 8 of 8 | 0.49 | 12.93 | 13.67 | 15.39 | 25.23 | 0.432841 | 0.054105 |
| gemini | 6 of 8, one partly | 2.44 | 6.89 | 8.80 | 9.68 | 21.24 | 0.069665 | 0.008708 |

Of the delegate door's $0.432841, Jev was $0.004864 and Claude Code was
$0.427977.

## Analysis

The delegate door bought correctness with time and money. It was right on
all eight prompts and specific where Gemini was general: on prompt 5 it
added that a refused command plan also exits `2`, and on prompt 8 it
listed the `bwrap` arguments in order. Gemini was right on the six
questions a grep answers and wrong or incomplete on the two that needed a
file read: it named a model alias that Coder One does not pin, and it ran
out of command rounds before it read the argument builder.

Gemini was faster to the answer by a median six seconds and cost about a
sixth as much. The delegate door shows progress sooner, a median half
second against two and a half, because the probe and survey lines stream
while Jev works; Gemini's first output is its command plan.

The delegate door's time splits into the probes and Jev's survey, from
2.3 to 10.9 seconds with a median of 5.5, and Claude Code, from 4.0 to
12.7 seconds with a median of 7.5. The probe half is the one to cut: it
runs before the executor starts, and its Jev calls are what varied most.
Prompt 2's twelve-second first output is the one outlier: the process
spent 8.5 seconds before its classify note, before either door started,
and Jev's requirements call took another four.

Jev is 1.1% of the delegate door's cost, and Claude Code is the rest.
Every turn here was a first turn in a fresh session; a follow-up turn
resumes its session instead.

## A finding: the account's connectors

While measuring, a resumed terminal turn cost $0.7376 for a one-sentence
answer. The executor's second model call carried 135,033 input tokens,
129,201 of them new cache writes. Claude Code, signed in with its stored
login, attaches the account's claude.ai connectors partway through a
session, and their tool lists are about 115,000 tokens of prompt. A
two-command session measured $1.0025 with the connectors and $0.0438
without. The prompts above ran on `CLAUDE_CODE_OAUTH_TOKEN`, whose
sessions never reached 40,000 input tokens, so the table is not
inflated; an operator on the stored login would have paid the connectors
on any turn long enough to receive them. Every delegated turn now sets
`ENABLE_CLAUDEAI_MCP_SERVERS=false`. Coder One's Terminal-Bench adapter
does not set it yet.

The same two terminal turns on the stored login after the fix, "what does
crates/gym do? one sentence." and then "and which crate does it get its
answers from? one sentence.", cost $0.042334 and $0.077377. The second
resumed the first turn's session, its two model calls carried 16,344 and
17,481 input tokens, and it answered from the conversation: `jev`.

## Traces

Each run's ATIF trace is under `~/.openagents/traces/` on the machine
that measured it, named in the raw records' `trace` field, from
`20260923T183432Z-6bc46267` (prompt 1, delegate) to
`20260923T183732Z-88d000e2` (prompt 8, Gemini).
