# Microluna on the eight delegate prompts

The eight read-only questions from
[the delegate door against the Gemini fallback](2026-09-23-delegate-vs-gemini.md),
asked again through Coder Terminal's new default executor: Microluna, the
mini-handoff loop of GPT-6 Luna sessions on the Codex login, with Jev
choosing each move. On the final build, Microluna answered seven of eight
correctly and one partly. It took a median 22.6 seconds and $0.0022 a
turn. The Claude Code delegate took 13.7 seconds and $0.0541, so
Microluna cost about a twenty-fourth as much and took about nine seconds
longer.

Measured on 2026-09-24 for issue
[#9585](https://github.com/OpenAgentsInc/openagents/issues/9585).

## Setup

| | Microluna |
| --- | --- |
| Build | `coder 0.1.0`, installed by `scripts/install-coder.sh`, at three commits: run a `fd7431989c`, run b `3997d95071`, and run c `54d0fba28c` |
| Door | `delegate`: Coder One's probe battery and Jev survey (`jev-1.13.0`), then `coder_one::micro` on `gpt-6-luna`, bounded at six sessions, two per requirement group, $0.25, and 600 seconds |
| Selected by | Nothing: `CODER_DELEGATE`, `CODER_DELEGATE_AGENT`, and `CODER_DELEGATE_MODEL` unset. Microluna is the default when the Codex login is usable. |
| Credential | `~/.codex/auth.json`, read only, with about 200 hours left on its access token |
| Classify | Off, as in the earlier run: no `TYPESAFE_API_KEY`, so every turn routed `respond` under an executing permit, and each ran workspace-writable |
| Working directory | This repository, at each build's commit |

Each prompt ran once per build with `coder -p --json --json-deltas`, one at
a time, from a harness that timestamped every JSON line on arrival and
checked `git status` after each turn. The harness isn't checked in. Its
records for all three runs, with every event's arrival time, each reply,
and any files a turn changed, are in
[`2026-09-24-microluna-terminal.raw.jsonl`](2026-09-24-microluna-terminal.raw.jsonl).

The columns match the earlier doc. **Cost** is what `coder -p` reports:
Jev at $0.042 per million input tokens plus Luna at its list price
($0.10 per million input tokens, $0.01 cached, and $0.50 output). The
Codex login bills nothing per token, so Luna's figure is a price
estimate, not a charge.

## Three runs, two fixes

The first run found a problem that the next two fixed:

- **Run a** (`fd7431989c`): the loop's session guidance, which another
  change had just made to demand an edit and a check before `done`, made
  question turns edit files. Six of eight turns ran more than one
  session; in four of them session 1 finished `blocked`, one saying "task
  guidance requires an edit before completion". Two turns ended with an
  edit nobody asked for: a doc comment in
  `crates/supervise/src/lib.rs` and the help text in
  `crates/coder/src/cli.rs`. The harness reverted both.
- **Run b** (`3997d95071`): the task text a terminal turn hands the loop
  says a question's answer is the whole result and changes no file. No
  turn changed a file. But its first wording, "as soon as the evidence
  supports it", made Luna answer from a nearly empty brief: six of eight
  turns ran no command, and two answered that they couldn't tell.
- **Run c** (`54d0fba28c`): the rule asks the session to search and read
  until what it found answers the question, and to cite the paths it
  read. Every turn ran one session and between one and four commands, and
  no turn changed a file.

## Results on the final build (run c)

| # | Prompt | First output (s) | Answer text (s) | Total (s) | Cost (USD) | Jev (USD) | Luna (USD) | Tokens in / out | Sessions | Commands | Answered correctly |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | What does `crates/gym` do? | 0.40 | 19.89 | 20.54 | 0.002834 | 0.000704 | 0.00213 | 50,711 / 388 | 1 | 4 | Yes |
| 2 | Which crate implements the Nostr relay, and what database does it store events in? | 0.49 | 53.08 | 53.43 | 0.001734 | 0.000744 | 0.00099 | 18,095 / 242 | 1 | 3 | Yes |
| 3 | Where is the subprocess supervisor, and what happens to a job that passes its deadline? | 0.39 | 16.07 | 16.51 | 0.002395 | 0.000835 | 0.00156 | 24,898 / 253 | 1 | 2 | Yes |
| 4 | How many crates are in this workspace? List them. | 0.36 | 13.09 | 13.42 | 0.001432 | 0.000792 | 0.00064 | 8,447 / 281 | 1 | 1 | Yes |
| 5 | What exit code does `coder -p` return when the router declines a turn? | 0.42 | 28.14 | 28.47 | 0.002030 | 0.000700 | 0.00133 | 23,811 / 211 | 1 | 3 | Yes |
| 6 | Which environment variable turns off Coder's trace, and where do traces go by default? | 0.40 | 19.57 | 19.90 | 0.001883 | 0.000723 | 0.00116 | 19,357 / 190 | 1 | 2 | Yes |
| 7 | What Jev model does Coder One pin, and what does Jev cost per million input tokens? | 0.36 | 24.20 | 24.55 | 0.002693 | 0.000713 | 0.00198 | 45,066 / 300 | 1 | 4 | Partly: right price, but named the policy `jevprobe3-luna` instead of `jev-1.13.0` |
| 8 | How does `coder-boundary` enforce a read-only boundary on Linux? | 0.44 | 36.80 | 37.21 | 0.002860 | 0.000720 | 0.00214 | 41,355 / 484 | 1 | 4 | Yes |

Token counts are Luna's, with cached input counted as input. **Commands**
counts `run_command` and `read_file` calls, which the terminal shows as
commands. Luna's column is rounded to five places, as the turn's closing
line reports it; the Jev column is the reported total less that.

| Door | Answered correctly | Median first output (s) | Median answer text (s) | Median total (s) | Mean total (s) | Slowest (s) | Total cost (USD) | Mean cost (USD) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Microluna, run c | 7 of 8, one partly | 0.40 | 22.05 | 22.55 | 26.75 | 53.43 | 0.017861 | 0.002233 |
| Microluna, run b | 5 of 8, two unanswered, one partly | 0.42 | 17.51 | 17.92 | 21.88 | 61.34 | 0.011291 | 0.001411 |
| Microluna, run a | 7 of 8, one partly; two unrequested edits | 0.43 | 38.55 | 39.02 | 40.34 | 87.74 | 0.022775 | 0.002847 |
| Delegate (Claude Code), 2026-09-23 | 8 of 8 | 0.49 | 12.93 | 13.67 | 15.39 | 25.23 | 0.432841 | 0.054105 |
| Gemini, 2026-09-23 | 6 of 8, one partly | 2.44 | 6.89 | 8.80 | 9.68 | 21.24 | 0.069665 | 0.008708 |

Run a's partial answer is prompt 4: it listed all 27 crates, called them
26, and answered twice, because the loop split "how many" and "list them"
into two requirement groups. Run b's unanswered prompts are 3 and 7, and
its partial answer is prompt 4, where it counted 28.

## Analysis

**Cost.** Microluna answered the eight prompts for $0.0179 against the
Claude Code delegate's $0.4328, about a twenty-fourth, and for about a
quarter of what Gemini cost. Jev is a third of Microluna's cost,
$0.0059, because the loop asks Jev for the move after each session and
for the combined verdict on top of the survey; Luna is the rest,
$0.0119.

**Correctness.** Seven of eight, with one partial answer on the question
Gemini also missed. It named a policy manifest as the Jev "model" and
found the price. Claude Code answered all eight.

**Time.** Microluna shows progress as early as the delegate did, a median
0.40 seconds, because the probe and survey lines stream first. The
answer comes later: a median 22.6 seconds end to end against 13.7. On run c the
probes and Jev's survey took a median 11.2 seconds before the first
session, from 1.4 to 17.8, about twice the 5.5-second median of
2026-09-23 on the same prompts. The Luna sessions took a median 12.4
seconds, from 8.6 to 37.3. The survey half varied most, as it did for the
delegate, and it's the half to cut first.

**Watching it work.** Every session's start and requirement, each
command and file read, the session's finish, its time, tokens, and Luna
cost, and each hand-off stream into the terminal and onto the `--json`
stream as they happen. Run a showed the loop doing its job: on prompts
2, 3, and 6 the combined verdict called session 1's report a failure,
Jev chose `retry`, and session 2 found the answer.

**What to fix next.**

1. The loop's session guidance assumes every task changes files. A
   terminal question needs a question-shaped session, which belongs in
   the loop (`coder_one::micro`), not in the task text the terminal adds.
2. With classify off, every turn runs under an executing permit, so a
   question runs in a writing boundary. With the router on, a question
   would run read-only, and Microluna would refuse any write whatever the
   guidance said.
3. Prompt 7 needs the evidence the survey kept: the survey ranked a
   Terminal-Bench trajectory and a suite file above
   `crates/coder-one/src/credentials.rs`, where `JEV_MODEL` is.

## Traces

Each turn's ATIF trace is under `~/.openagents/traces/` on the machine
that measured it, named in the raw records' `trace` field: run c runs from
`20260924T083446Z-39912c37` (prompt 1) to `20260924T083743Z-7c1cef61`
(prompt 8). Each turn's Microluna loop record and per-session ATIF traces
are under `~/.openagents/coder/delegate/<opened>-1/artifacts/`.
