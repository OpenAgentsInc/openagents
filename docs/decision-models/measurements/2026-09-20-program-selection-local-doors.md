# Program selection through a local door

[`2026-09-19-program-selection.md`](2026-09-19-program-selection.md) measured
the question every `coder` turn now asks — which program, or `none` — on
hosted Jev and on nothing else, and listed "ask the other doors" as the next
thing to do. [openagents#9403](https://github.com/OpenAgentsInc/openagents/issues/9403)
asks the same thing of the first CoderBench golden: the program lookup is a
short state over a small option set, which is where a local door should do
best, so measure whether one answers it well enough rather than assuming the
hosted one is needed.

This record asks three Kev checkpoints the same question on the same 44
items, into the same store, beside the hosted rows. The short version:

- **No local door clears hosted Jev.** Jev scores 0.909 on the 44 open
  items; `kev-4b` scores 0.886, `kev-0.5b` and `kev-0.6b` both 0.795. The
  `kev-4b` gap, 0.023, is inside the 0.056 two-sigma floor; the small
  checkpoints' gap, 0.114, is not.
- **The small doors miss the one real program request.** `kev-0.5b` and
  `kev-4b` answer `none` to the only real turn that asked for a fan-out.
  Hosted Jev caught it at 0.93. `kev-0.6b` caught it at 0.70 and paid with
  five spurious `answer-question` answers on ordinary turns.
- **Every local door misses more programs than it invents.** Jev missed 0
  of 9 program requests; the Kevs miss 4, 4, and 5. That is the cheap error,
  but at these rates the question would stop earning its call: an operator
  who asks for a program gets it about half the time.
- **Latency is the cost, and it runs the wrong way.** On a quiet eight-core
  CPU the small checkpoints answer at a p50 of 520 to 650 ms, roughly three
  times the hosted 192 ms; `kev-4b` answers in seconds.

So a local door did not suffice, on this question, as the question is
worded today. The section on what the errors have in common says why that
is a statement about the question as much as about the doors.

## What ran

Three doors, one checkpoint each, on 2026-09-20, each served on its own
port so `GET /v1/models` names one checkpoint and every recorded row carries
it. Weights came from `./scripts/fetch-kev-artifacts.sh kev-0.5b kev-0.6b
kev-4b`; every pinned digest matched. CPU at fp32, the `kev-serve` default
and the precision the conformance fixtures pin.

```text
CARGO_TARGET_DIR=~/target-oa cargo build -p kev --features serve --release --bin kev-serve
CARGO_TARGET_DIR=~/target-oa cargo build -p gym --release --bin gym

kev-serve --adapter-dir ~/repos/kev-artifacts/kev-0.5b \
    --base-dir ~/repos/kev-artifacts/qwen2.5-0.5b --default kev-0.5b --port 8101
gym eval --suite crates/gym/suites/program-selection-v1.json \
    --door kev-0.5b=http://127.0.0.1:8101 --timeout 300 \
    --record crates/gym/results/program-selection-v1.jsonl
python3 crates/gym/suites/score_program_selection_v1.py \
    crates/gym/results/program-selection-v1.jsonl --door kev-0.5b
```

The same three commands ran for `kev-0.6b` on `qwen3-0.6b` at port 8102 and
`kev-4b` on `qwen3-4b` at port 8104.

Two things about the fetch belong in the record because the next person
will hit them:

- The Hub repositories for `kev-0.6b` and `kev-4b` ship `result.json` rather
  than the `eval.json` the script asks for, so the fetch stops at a 404. The
  file is not pinned by either variant's manifest and the port does not
  read it; a placeholder under that name lets the script continue.
- On 2026-09-20 `jaredpalmer/kev-4b`'s `main` moved to a `decision-v7`
  recipe. The adapter, head, and `head_meta.json` at `main` no longer match
  the digests `crates/kev/fixtures/variants/kev-4b/manifest.json` pins. The
  files at Hub revision `0d68f2491a04` do, byte for byte, and that is the
  checkpoint this record calls `kev-4b` — the same one
  [`../kev/measurements/2026-09-19-variant-scores.md`](../../kev/measurements/2026-09-19-variant-scores.md)
  scored, base signature `906bfd4b`. The `main` checkpoint is a different
  door and was not asked.

Suite `program-selection-v1` at digest `7b66bc4ce8336de2`: 52 items, 16
calibration, 28 development, 8 locked and unread. Asked as
`program-selection-v1` at digest `88388df7d2b93d14`, the three-option
question. Judged by `probability-v2`, the suite's own gate today; the hosted
rows from 2026-09-19 carry `probability-v1`, which changes nothing in the
tables below, because `score_program_selection_v1.py` counts answers rather
than reading a verdict. Every door answered all 44 open items, refused
none, and lost none to the harness. The store now holds 176 rows over four
doors and the receipt chain verifies.

The items are the ones the hosted record scored: the same suite file, at
the same digest, so the comparison is paired item for item and no second
hosted run was needed.

## The baseline, again, before any accuracy

Unchanged from the hosted record, because the items are unchanged:

| Set | Items | Asks for a program | Constant `none` | Headroom |
| --- | --- | --- | --- | --- |
| real turns | 32 | 1 | 0.969 | 0.031 |
| written | 12 | 8 | 0.333 | 0.667 |
| both | 44 | 9 | 0.795 | 0.205 |

The real-turn headroom is under the floor, so nothing below is an accuracy
win on real traffic for any door. The numbers are error counts.

## What each door did

From `score_program_selection_v1.py --door <name>`, one door per block.
A *false positive* is a program answered where `none` was labelled; a
*false negative* is `none` answered where a program was labelled; *wrong
program* is one program answered where the other was labelled.

### Hosted Jev, from the 2026-09-19 record

| Set | Items | Accuracy | False positives | False negatives | Wrong program |
| --- | --- | --- | --- | --- | --- |
| real turns | 32 | 0.938 | 2 of 31 (0.065) | 0 of 1 | 0 |
| written | 12 | 0.833 | 1 of 4 (0.250) | 0 of 8 | 1 |
| both | 44 | 0.909 | 3 of 35 (0.086) | 0 of 9 | 1 |

### `kev-0.5b`

| Set | Items | Accuracy | False positives | False negatives | Wrong program |
| --- | --- | --- | --- | --- | --- |
| real turns | 32 | 0.906 | 2 of 31 (0.065) | 1 of 1 (1.000) | 0 |
| written | 12 | 0.500 | 2 of 4 (0.500) | 4 of 8 (0.500) | 0 |
| both | 44 | 0.795 | 4 of 35 (0.114) | 5 of 9 (0.556) | 0 |

### `kev-0.6b`

| Set | Items | Accuracy | False positives | False negatives | Wrong program |
| --- | --- | --- | --- | --- | --- |
| real turns | 32 | 0.844 | 5 of 31 (0.161) | 0 of 1 (0.000) | 0 |
| written | 12 | 0.667 | 0 of 4 (0.000) | 4 of 8 (0.500) | 0 |
| both | 44 | 0.795 | 5 of 35 (0.143) | 4 of 9 (0.444) | 0 |

### `kev-4b`

| Set | Items | Accuracy | False positives | False negatives | Wrong program |
| --- | --- | --- | --- | --- | --- |
| real turns | 32 | 0.938 | 1 of 31 (0.032) | 1 of 1 (1.000) | 0 |
| written | 12 | 0.750 | 0 of 4 (0.000) | 3 of 8 (0.375) | 0 |
| both | 44 | 0.886 | 1 of 35 (0.029) | 4 of 9 (0.444) | 0 |

### Side by side

`gym compare --store crates/gym/results/program-selection-v1.jsonl
--baseline "jev (hosted)"`, both partitions pooled:

| Door | Accuracy | ECE | Brier | NLL | Confident errors | False positives | False negatives | Median latency |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `jev (hosted)` | **0.909** | 0.057 | 0.069 | 0.224 | 0 | 3 of 35 | **0 of 9** | **193 ms** |
| `kev-0.5b` | 0.795 | 0.130 | 0.141 | 0.435 | 1 | 4 of 35 | 5 of 9 | 413 ms |
| `kev-0.6b` | 0.795 | 0.062 | 0.129 | 0.416 | 1 | 5 of 35 | 4 of 9 | 528 ms |
| `kev-4b` | 0.886 | 0.077 | 0.065 | 0.212 | 0 | **1 of 35** | 4 of 9 | 3,395 ms |

Under `decision-v1` every Kev fails `accuracy_does_not_fall` against the
hosted rows: 0.909 to 0.795, 0.909 to 0.795, and 0.909 to 0.886. The gate
does not know the floor; the next section does.

## Against the floor

The two-sigma floor for a two-door accuracy comparison on our suites is
0.056, from
[`../lev/measurements/2026-09-19-seed-variance.md`](../../lev/measurements/2026-09-19-seed-variance.md),
and it was measured on 98 items. These are 44, so the real floor here is
wider, and a gap under 0.056 is doubly not a finding.

| Door | Gap to hosted Jev, 44 items | Against 0.056 |
| --- | --- | --- |
| `kev-4b` | −0.023 | inside the floor: not separable from Jev on accuracy |
| `kev-0.5b` | −0.114 | outside the floor: measurably worse |
| `kev-0.6b` | −0.114 | outside the floor: measurably worse |

Paired, item by item, against the hosted rows:

| Door | Jev right, Kev wrong | Kev right, Jev wrong |
| --- | --- | --- |
| `kev-0.5b` | 7 | 2 |
| `kev-0.6b` | 7 | 2 |
| `kev-4b` | 3 | 2 |

Two items every Kev missed and Jev did not: `authored/answer-one-file`
("have an agent read `crates/gym/src/suite.rs` and tell me what the
partitions are") and `authored/answer-rounds-max` ("get a delegate to answer
what `ROUNDS_MAX` is set to"), both labelled `answer-question`, both
answered `none`. Jev got the first right and the second wrong the other way,
as `delegate-fan-out`.

So on accuracy alone, `kev-4b` is not distinguishable from hosted Jev on
these items, and the small checkpoints are. That is not the same as
sufficing.

## Which error, and where it sits

The hosted record's argument was that the two errors cost different amounts
and that Jev's mistakes were all the cheap kind on the expensive axis: three
false positives, every one of them `answer-question`, which refuses rather
than runs when the request carries no work; and zero false negatives. The
local doors invert that.

**Every local door misses program requests.** Four or five of the nine, and
they are not the hard ones. `kev-0.5b` answers `none` at 0.83 to "take the
top six open issues and hand each one to its own agent, six at once". `kev-0.6b`
answers `none` at 0.71 to the golden's own sentence, "Delegate six instances
of Devin, one for each of these six read-only questions", the request
`devin-fan-out-six` was recorded from; `kev-4b` answers it right at 0.62.
`kev-0.6b` also answers `none` at 0.99 to "get a delegate to answer what
`ROUNDS_MAX` is set to", which is the highest-confidence error on the whole
panel and the reason a confidence floor would not rescue this door.

**The one real program request.** `turn/1d845d65#16` asks to build the Gym
and "make issues for the issues and do them". Jev answered
`delegate-fan-out` at 0.93 and `kev-0.6b` at 0.70; `kev-0.5b` and `kev-4b`
answered `none`, at 0.48 and 0.69. On real traffic that is the whole
positive class, so the small doors' real-turn false-negative rate of "1 of
1" is one item, and it is the item the question exists for.

**The `answer-question` overlap is still where the false positives live.**
Eight of the ten false positives across the three Kevs are `answer-question`
on an ordinary turn — "Read `AGENTS.md` then everything in `docs/` … summarize
here", "Where do I download the training toolkit?" — the same overlap the
hosted record traced to that option's summary, which describes what an
ordinary turn does. The other two are the expensive kind: `kev-0.5b` answers
`delegate-fan-out` at 0.89 to "what does the delegate-fan-out program do?"
and at 0.78 to "how many delegations did the last fan-out start, and how
many answered?". Both are written items that name the program in the
sentence; neither asks for it. Hosted Jev and the other two Kevs answered
`none` to both. A fan-out selected there would refuse for lack of work, as
the hosted record explains, but `kev-0.5b` is the only door on the panel
that ever picked the option that starts subprocesses when nobody asked.

**`kev-4b`'s single false positive is one Jev shares.** "tell me the status
of all open issues", `answer-question` at 0.57 on `kev-4b` and 0.62 on Jev.
Read as a spurious program that refuses at the `query` step, it is the
cheapest wrong answer on the board.

### Where the true `delegate-fan-out` confidences sit

The hosted record noted that Jev's true fan-outs all landed at 0.93 or
above, clean of its errors. The local doors do not separate that way:

| Door | True `delegate-fan-out` answers, confidence | Errors answered `delegate-fan-out` |
| --- | --- | --- |
| `jev (hosted)` | 0.93, 0.97, 0.97, 0.97, 0.98, 0.99 | one at 0.71 |
| `kev-0.5b` | 0.73, 0.91, 0.93, 0.98 | two, at 0.78 and 0.89 |
| `kev-0.6b` | 0.50, 0.61, 0.70, 0.80, 0.87 | none |
| `kev-4b` | 0.62, 0.73, 0.89, 0.89 | none |

`kev-0.5b`'s wrong fan-outs sit inside the range of its right ones. No
threshold separates them, which repeats what the issue's eight-sentence
panel found about this checkpoint: it missed at 0.98 where no floor helps.

## What it cost

`gym latency --suite crates/gym/suites/program-selection-v1.json --door
<name>=<url> --blocks 8`, eight passes over the 44 items in the same order,
nearest-rank percentiles per pass, one door at a time. The machine is an
eight-core `INTEL(R) XEON(R) PLATINUM 8559C` with 31 GiB, kernel 5.15.200,
running nothing of ours other than the three `kev-serve` processes, two of
them idle. Load average before the `kev-0.6b` sweep was 2.77 and before the
`kev-4b` sweep 3.54, both the tail of the previous sweep rather than another
process; `top` between sweeps showed the CPU 98% idle.

| Door | p50, mean over blocks | p50 range | p95, mean over blocks | p95 range |
| --- | --- | --- | --- | --- |
| `jev (hosted)`, 2026-09-19, one pass | 192 ms | — | 311 ms | slowest 517 ms |
| `kev-0.5b` | 520 ms | 480 to 575 ms | 1,034 ms | 727 to 1,838 ms |
| `kev-0.6b` | 646 ms | 523 to 728 ms | 1,227 ms | 824 to 2,055 ms |
| `kev-4b` | 3,205 ms | 2,473 to 3,680 ms | 7,360 ms | 4,375 to 12,739 ms |

The hosted row is a single pass, read from the recorded rows rather than
from a sweep, so it has no range; it is network time to the service, and
the small Kevs' CPU time is about three times it at the median and
`kev-4b`'s about seventeen times, with a p95 that reaches 12.7 seconds in
the worst block. The per-block
tables and the conditions are in
[`../kev/measurements/2026-09-20-program-selection-latency.md`](../../kev/measurements/2026-09-20-program-selection-latency.md).

The `eval` rows carry their own latency and it was measured under worse
conditions: the `kev-0.5b` and `kev-0.6b` evals ran while the `kev-4b` base
was downloading. Their row medians, 413, 528, and 3,395 ms, land within
about 20% of the sweep p50s. The sweep is the number to quote.

## Did a local door suffice?

The issue's box asks for the door that answered, the alternatives tried,
and whether a local door sufficed.

- **The door that answered in the golden** is hosted Jev. The observed
  golden at `crates/coderbench/goldens/devin-fan-out-six.atif.jsonl`
  records the `program` call answered by `jev-1.13.0` at
  `https://api.typesafe.ai`: `delegate-fan-out` at confidence 0.93, with
  `none` at 0.03. Its evidence sidecar names the door and model on every
  decision call. Earlier goldens, since superseded, had `kev-4b` select the
  program; the golden on `main` today did not ask a local door.
  On the suite, the nearest item is `authored/fan-out-six-questions` — the
  same opening sentence and the same six questions, without the "Answer
  with the value and nothing else" suffix each one carries in the golden.
  It is answered right by Jev at 0.97, by `kev-4b` at 0.62, by `kev-0.5b`
  at 0.93, and wrong by `kev-0.6b`, which answers `none` at 0.71.
- **The alternatives tried** are hosted Jev, `kev-0.5b`, `kev-0.6b`, and
  `kev-4b`, on the same 44 items, all in one store.
- **Whether a local door sufficed:** not as a replacement for the hosted
  door on this question, today.
  - `kev-4b` is the only local door inside the accuracy floor, it has the
    lowest false-positive count on the panel, and it missed the one real
    program request and three of eight written ones. It answers in 3.2 s at
    the median on this CPU, 7.4 s at p95, against a hosted 192 and 311 ms,
    so it trades a call to a service for a wait an operator notices on
    every turn.
  - `kev-0.5b` and `kev-0.6b` are measurably worse than Jev, miss about half
    the program requests, and are still three times slower than the hosted
    call.
  - No local door was asked the four-option `program-selection-v2` question;
    a fourth program can only widen the overlap that produced most of the
    errors here.

The finding that survives is the one the hosted record already made: the
error mass, on every door, sits in the `answer-question` overlap, and a
rewording of that option's summary is a candidate to measure before another
checkpoint is. A local door that missed half the requests as the question
is worded today may do better when the option it has to recognise says
that the work leaves the machine.

## What this does not say

- **Nothing about `kev-8b` or Lev.** Neither was available on this machine;
  Apple hardware was not, and `kev-8b` was not fetched.
- **Nothing about the `kev-4b` at the Hub's `main`.** That is a different
  checkpoint from the one pinned here, and it has not been scored on
  anything in this repository.
- **Nothing measured on the locked partition.**
- **Nothing about the four-option question.** `program-selection-v2` has not
  been asked of any door.
- **The floor is borrowed.** 0.056 was measured on 98 items; on 44 the
  true two-sigma band is wider, so "inside the floor" for `kev-4b` is the
  conservative reading and "outside it" for the small checkpoints is the
  one that could be wrong only if the band were more than twice as wide.
- **The labels are readings**, the author's, as every row says.
