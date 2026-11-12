# Microcoder next: what each change bought, and a benchmark that can separate it from Antigravity

Written 2026-11-12. GitHub #13325. Follows
[Microcoder against Antigravity on Gemini](2026-11-11-microcoder-vs-antigravity.md)
(#13231), which has the four earlier rounds, their method and their tables.

This page is written as the work lands. Each section says what was built,
what was measured and whether the piece stayed.

## Where things stand

| Item of #13325 | State | Measurement |
| --- | --- | --- |
| 6. H8 | Check corrected | All 12 recorded H8 runs pass the corrected check; the old one failed 10 of them for a layout the prompt itself uses |
| 6. T6 | Check corrected | All 6 recorded T6 failures pass the corrected check; the old one could not match a fact across a line break |
| 6. Harder tasks | Being built by a separate agent | See "The unseen tasks" |
| 6. Verdict definition | Not committed yet; no final run before it is | See "The verdict" |
| 2. Fresh reader at the finish | Built, measured, removed | Changed no outcome in 4 of 4 runs |
| 5. Seeded first step | Built; being measured | |
| 1. Revert thinking by step | Waits for #13231's Carbon runs to end | |
| 4, 3 | Not started | |

## What the earlier failures were

Round 4 of #13231 (Argon, three runs per task) ended 34 passes to 35 of 42.
Read one by one, 11 of the 15 failures on the two sides are the judge's
fault, not the harness's:

| Task | Microcoder | Antigravity | What failed |
| --- | ---: | ---: | --- |
| H8 | 3 | 3 | The check wants each of two constants at the start of a line. The prompt writes the three constants on one line, and both harnesses copied it. The hidden test, which checks the values, passed every time. |
| T6 | 2 | 2 | The check looks for "one commit per issue" on one line. The task asks for lines wrapped near 80 columns, and the phrase broke across a line. |
| H5 | 1 | 0 | The answer gives the value the check wants, in bold code (`**\`1\`**`); the regular expression does not match through the markup. |
| H4 | 0 | 1 | A fact the regular expression did not find. Not examined further. |
| T3 | 2 | 1 | The warning line does not say "warn". The task says the command "only warns". This one is a fair reading of the task. |

So the 14 tasks mostly measured their checks. That is the reason for harder
tasks judged by tests that run, below.

## H8 and T6: the checks are corrected, not dropped

Both tasks are real work with a hidden test that already judges behaviour.
Dropping them would shrink a small set, so each check keeps everything it
tested and loses only the fault:

- **H8.** The two constants may follow another constant's `;` on the same
  line, as in the prompt's own text
  (`ENV_GLADE_DAY_ID :: 2;  ENV_GLADE_DAY_KEYS :: 4;  ...`). The hidden test
  still asserts both values.
- **T6.** The facts are searched in the added text with line breaks folded
  to spaces. Every other test of the check (the place, no removed lines, 85
  columns, the banned words) is unchanged.

Rescored with the corrected checks, from the recorded diffs:

| Runs | Microcoder | Antigravity |
| --- | ---: | ---: |
| Argon round 4, as judged then | 34 of 42 | 35 of 42 |
| Argon round 4, corrected checks | 39 of 42 | 40 of 42 |

The gap is still one run, and both harnesses now pass 93% to 95% of runs.
These tasks cannot separate the two. They are the tuning set from here on:
every one has been seen by the agents who worked on Microcoder.

The reading tasks (T4, H4, H5) keep their regular expressions. Their checks
were not changed, because there is no single fault to correct: each fact has
its own pattern. The new tasks have no such judge.

## The unseen tasks

A separate agent is building six tasks, N1 to N6, from this repository's
history. The agent tuning Microcoder (the author of this page) does not see
their prompts, their checks or their traces until the verdict runs end; it
is told only each task's kind, start commit, size and file hashes.

The brief the builder got:

- **Harder.** The known-good change touches at least three files (or two
  and about 150 changed lines), needs several files read, and fits one
  unattended run of 15 minutes.
- **Judged by tests that run.** A hidden test or test program built from the
  commit's own checks. No reading tasks and no prose graded by regular
  expression.
- **The lesson of H8.** The prompt states every name and behaviour the check
  relies on, and the check accepts every spelling the prompt's text allows.
- **Four validations each:** the check fails on the untouched start tree,
  passes on the known-good commit, passes on a second correct solution laid
  out differently, and fails on a plausible incomplete one.
- **No overlap** with the 14 earlier tasks or with the harnesses' own code.

The list, with hashes of each prompt and check, is added here before any
final run.

## The verdict

Not committed yet. It will be added to this page, with the task list, before
any final run: the runs per task, the conditions, and the gap in passes that
many runs can resolve.

## Item 2: a fresh reader at the finish (built, measured, removed)

**What it was.** In #13231 the loop asked the model to check its own diff
before a finish, and in 4 of 4 runs it read the diff and finished at once.
This time a separate call did the reading: Gemini Flash at low thinking,
given the task, `git diff HEAD` and the new files, and nothing of the run.
It listed what the task asks for that it did not find. An empty list took
the finish as it was. Anything listed went back into the loop as one step,
and the answer after that was taken. Once a run, never for a plain answer.

**Offline first.** Before it was wired in, the reader's prompt was tried on
the recorded final states of 24 Microcoder runs from #13231 (Flash, low,
$0.10 in all):

| Final states | Count | The reader listed something |
| --- | ---: | ---: |
| Failed: the answer was left out of EXPLAIN.md (T4) | 1 | 1, the four unanswered questions |
| Failed: the judge was at fault (T6, H5) | 2 | 0, rightly |
| Failed: T3's wording, a name H4's pattern wanted | 2 | 0 |
| Passed (one of them H8, by its corrected check) | 19 | 1 |

A first wording of the prompt listed something for 2 of 9 passing finals,
both doubts about how the code behaves. The wording that was built says the
reader cannot judge behaviour from a diff and must list only what is absent.

**In real runs.** Argon low, one run each, on the three tasks it was aimed
at and one short one (the build with the reader, 90df30a29c):

| Task | Before (round 2 build, two runs) | With the reader | The reader listed |
| --- | --- | --- | --- |
| T3 | 1 of 2 passed; $0.21, $0.36; 44 s, 81 s | passed; $0.22; 56 s | nothing |
| T4 | 1 of 2 passed; $0.64, $0.63; 81 s, 57 s | passed; $1.33; 226 s | nothing |
| H4 | 0 of 2 passed; $1.09, $1.62; 126 s, 201 s | passed; $0.62; 97 s | nothing |
| T1 | 2 of 2 passed; $0.12, $0.12; 19 s, 30 s | passed; $0.12; 40 s | nothing |

- **It changed no outcome.** All four passed, and none because of the
  reader: it listed nothing each time. T3 passed because the model wrote
  "warning:" itself in this run.
- **It was cheap:** one Flash call of $0.002 to $0.005 and 1 to 2 s a run.
  T4's long run (14 Argon calls) is the model reading more files, not the
  reader.
- **Removed.** The rule is a measured gain or out, and there is none. The
  tuning tasks hold almost no real incompleteness for a reader to find: one
  recorded run in about 130.

**How to measure it again without putting it in the loop.** After the
verdict runs, read each Microcoder run's final state offline with the same
prompt ($0.005 a run). Failed finals where it names the real gap are its
possible gain, and passing finals where it lists something are its cost.
Those are tasks it was not built from.

**Found on the way: a finish that carried edits was dropped.** A reply with
`finished: true` ended the run at once, so edits and commands in the same
reply never ran, and the reply said they had. Flash did this on the first
tiny task tried (the run ended "finished" with the tree unchanged). Argon
did it in 2 of 113 recorded finishes. Such a reply is now a working step:
its edits and commands run, and the run finishes in a turn of its own.

## Spend

Of the $50 for this issue, to this point: about $2.60 (the offline reader
probes $0.17, the reader's four runs $2.29, tiny test turns $0.05).
