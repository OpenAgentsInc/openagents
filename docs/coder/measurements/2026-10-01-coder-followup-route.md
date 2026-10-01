# Follow-ups after a Coder run go through the router (#10094)

On 2026-10-01 the owner asked the Mac app "do a test delegation to
claude", Coder ran on Codex, and every follow-up then went straight to
Coder: "summarize what happened" started a second Codex turn whose answer
was a summary the chat could have given. Now a follow-up in a chat whose
Coder run's turn ended goes to the router, with the run's result as typed
context (NIP-CJ `context.coder_run`: how it ended, its engine and model,
its summary, the files it changed, and its commands). The chat model is
told what the run reported; Jev reads only that the run ended and how, as
a fixed line the worker puts before the latest message
(`coder::router::CoderRun::marker`). The router answers a question about
the run in chat (`general`, the model tier) or hands more work to Coder
(`work.dispatch`), which continues the same task as its next turn.

The route list is unchanged, so the set stays `chat-router-v4`; its digest
moved with the rubric to `chat-router-v4@1d266532e7d0`, and the bank to
`chat-answers-v1@43063f287db6` (`meta.privacy@3` says a follow-up after a
run carries what it reported, the files it changed, and its commands, which
only our worker and our chat model read). The Gym suite `chat-router-v4`
and question set `chat-router-route-v5` are regenerated, and
`calibration-v2.json` is refit.

What changed in the rubric (`router::rubric`): `general` covers, only when
the conversation has the line saying Coder's run in this chat ended, a
question about that run (what happened, what it changed or ran, whether it
passed, which engine it used and why, a summary); `work.dispatch` covers,
in that case, more work on it (another change, a fix, a test, undoing a
step, the same for another place); `meta` sends a question about the run to
`general`; and the `lane` rubric answers a question about the run in chat.

`routes-v4.json` adds 46 rows tagged `coder_followup`: 21 questions about
the run (`general`), 20 requests for more work (`work.dispatch`, one naming
Claude Code), and 6 near misses that take their own routes after a run (a
concept question, who we are, whether a laptop is online, thanks, what
Coder is, whether we write code). 15 are held out.

## The follow-up rows, against hosted Jev

`ROUTER_EVAL_ROWS=coder_followup ROUTER_EVAL_SPLIT=all
ROUTER_EVAL_SURFACE=desktop cargo test -p coder --test router_eval
live_router -- --ignored`, 46 requests:

| Follow-up rows (46) | Before (main's rubric, the 13 held-out rows read) | This change |
|---|---|---|
| Route accuracy | 8 / 13 | 45 / 46 |
| Dispatch precision / recall | - | 20 / 20, 20 / 20 |
| `general` precision / recall | - | 20 / 20, 20 / 21 |
| Engine named where asked | - | 1 / 1, none where none was asked |

With main's rubric, the marker alone was not enough: "what did you change?"
and "what does the new function do" read `work.dispatch`, "what commands did
it run" `eval.result`, and "why did it stop?" and "was that on claude or
codex" `meta`. The issue's rows all read as labeled after the change:
"summarize what happened", "what did you change?", and "why did it use
Codex?" are `general`; "now add a test", "fix that too", and "do the same
for the other crate" are `work.dispatch`. The one miss is a `general` row
read as another route at low probability, which the model answers.

## The published eval

`ROUTER_EVAL_PUBLISH=1 cargo test -p coder --test router_eval live_router
-- --ignored`, 600 requests (256 held out, 344 calibration), 0 errors. The
held-out `coder_followup` rows are measured above and left out of the
record: with them the held-out split is 271 rows, past NIP-EVAL's 256
cases. The record is
[`2026-10-01-coder-followup-claims/report.json`](2026-10-01-coder-followup-claims/report.json),
whose per-route numbers the Map page reads.

| Held out (256 rows) | #10090 | main's rubric, read again today | This change |
|---|---|---|---|
| Route accuracy | 0.906 | 0.910 | 0.902 |
| Canned precision | 100 % (75/75) | - | 100 % (75/75) |
| Dispatch precision | 0.977 (42/43) | - | 0.977 (42/43) |
| Gym precision | 0.969 (31/32) | - | 0.967 (29/30) |
| `capability.missing`, `presentation.open` hit / predicted / labeled | - | - | 11/11/11, 9/9/9 |

The two rows apart from today's read of main's rubric are close calls
(route probability under 0.5) on both sides. The `router-v1` gate's floors
pass (canned precision 1.000 against 0.98, dispatch precision 0.977
against 0.90); with no baseline arm the record claims no change.
Calibration fitted on the calibration partition and scored held out:
`route` ECE 0.052 → 0.015, NLL 0.222 → 0.220, and `answer` ECE 0.147 →
0.072, NLL 0.488 → 0.448, both passing `probability-v2`. Serving keeps
calibration off.

A first wording that also told every turn's route question about the
marker, and put the run questions in `general` without tying them to it,
read the same held-out rows at 0.883: questions about Gym tests and results
moved to `general` or `clarify`. Both were withdrawn before this record.

Jev spend: 46-row follow-up runs (three), one 256-row read of main's
rubric, and four published runs of about 600 rows (two stopped or refused
before writing a record: a bank change after the first started, and the
271-case record the second wrote).
