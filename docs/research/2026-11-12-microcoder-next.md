# Microcoder next: what each change bought, and a benchmark that can separate it from Antigravity

Written 2026-11-12. GitHub #13325. Follows
[Microcoder against Antigravity on Gemini](2026-11-11-microcoder-vs-antigravity.md)
(#13231), which has the four earlier rounds, their method and their tables.

This page is written as the work lands. Each section says what was built,
what was measured and whether the piece stayed.

## Where things stand

| Item of #13325 | State | Measurement |
| --- | --- | --- |
| 1. Revert thinking by step | Reverted (26c27d5714) | Round 4: 34 of 42 against 35, 25% more cost a run, the 2x time lead gone |
| 2. Fresh reader at the finish | Built (6a29b3d144), measured, removed (62d8f074e0) | Changed no outcome in 4 of 4 runs |
| 3. Models mixed in one run | Built (0190846ae0), measured, removed (93455e7032) | 4 pairs: cost +27%, time +8%, passes equal |
| 4. Cache hits | Measured; no change made | Steps that add about 600 tokens: 0 of 6 calls cached. About 4,200 tokens: 4 of 6. Explicit caching: Flash yes, Argon no |
| 5. Seeded first step | Kept (090025aa11) | One call fewer on T1, H7 and H1; H2 in 4 calls against 5 to 7 |
| 6. H8 and T6 | Checks corrected | 39 of 42 against 40 of 42 when rescored |
| 6. Harder tasks | Six built, unseen | See "The unseen tasks" |
| 6. Verdict definition | Committed with this page, before any final run | See "The verdict" |
| 7. Read budget | Kept (b9795bd7b2) | Views counted apart (512); the ending names the limit |
| 7. Economy | Kept (913a63b6fb) | Argon: cost about halved on H4, T4 and T5. Carbon on #13263: still no edit |
| A command's time limit | Kept (66b316de85) | H6's own checks ran (120 s) where they had been killed (30 s) |
| A finish that carries edits | Fixed (96ee94bc29) | Flash dropped its edits on the first tiny task; Argon in 2 of 113 finishes |

The build the verdict runs use is 93455e7032: the round 2 loop of #13231
with the finish fix, the seeded first step, the read budget, the economy
change and the 120 s command limit. One model throughout.

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

A separate agent built six tasks, N1 to N6, from this repository's history.
The agent tuning Microcoder (the author of this page) has not seen their
prompts, their checks or their traces, and will not until the verdict runs
end. It was told only what this table holds.

| Task | Kind | Start | Known-good change | Check time | Prompt sha256 | Check sha256 |
| --- | --- | --- | --- | --- | --- | --- |
| N1 | cross-file bug fix | `032762d4ee` | 5 files, 177 lines | 11 s | `625be2722e78` | `38c9c6df1ccc` |
| N2 | multi-file feature | `e359372251` | 5 files, 334 lines | 2 s | `0db827f65e14` | `b25fa36f0527` |
| N3 | cross-file bug fix | `a331a9cfde` | 8 files, 324 lines | 11 to 15 s | `bf7e60b6c7f1` | `5b3210fbcb3a` |
| N4 | six-file refactor, behaviour kept | `185c7eee5c` | 6 files, 133 lines | 3 s | `ccbc61e896b8` | `eea8c79647aa` |
| N5 | multi-file feature with a tool | `f0f01e81ba` | 7 files, 140 lines | 5 s | `091800340375` | `dd823e04c09f` |
| N6 | multi-file feature with a tool | `3b02a1b75b` | 4 files, 142 lines | 4 s | `bf695acba7a1` | `c58f93484f4b` |

The hashes are the first 12 hex digits of each file's sha256, checked by the
author against the files without opening them. The prompts and checks are
added to this page's records after the runs.

How they were built:

- **Harder.** Each known-good change touches four to eight files, where the
  earlier tasks touched one to three. Each fits one unattended run of 15
  minutes in principle; no model had run them when this was written.
- **Judged by tests that run.** A hidden test or test program built from the
  commit's own checks. No reading tasks and no prose graded by regular
  expression.
- **The lesson of H8.** The prompt states every name and behaviour the check
  relies on, and the check accepts every spelling the prompt's text allows.
- **Four validations each,** 30 runs in the final pass: the check fails on
  the untouched start tree (6), passes on the known-good commit (6), passes
  on a second correct solution with other naming and layout (6), and fails
  on two incomplete solutions per task (12). Writing the checks took 50
  more runs and found three check bugs, one of the H8 kind.
- **No overlap** with the 14 earlier tasks or with the harnesses' own code.
- **Every start's compiler installs in seconds** from the cache.

What the builder could not verify: pass rates, and whether each task fits 15
minutes. In N3, N4 and N5 a few stated requirements cannot be reached
without a network, a display or a running service, so the check does not
test them. N4 compares against values recorded on this machine.

## The verdict

Committed with this page, before any final run.

**What is compared.** Microcoder and Antigravity on `gemini-4-argon`,
location `us`, each at its low default:

- **Microcoder:** `coder run --engine microcoder --model gemini-4-argon
  --effort low`, at 93455e7032, one model for every call.
- **Antigravity:** `coder run --engine antigravity --model
  gemini-4-argon-low`, through the same Coder. Its engine has not changed
  since #13231's rounds (low thinking by default, the stall watchdog on).

**The tasks.** N1 to N6 only. The earlier definition judged all tasks and
then the hold-outs alone. All 14 earlier tasks have now been seen and tuned
on by the agents who worked on Microcoder, so counting them would favour
Microcoder. They are reported as the tuning set and are not judged.

**The runs.**

- **Pairs.** Each task's Microcoder run and its Antigravity run start
  together, so both see the same machine load. Two pairs go at once, so
  four runs at most throughout.
- **Rounds.** A round is one pair of every task: 12 runs. Two rounds are
  run. A third is run only if, after two, the money left covers a round at
  the first two rounds' average cost. Only complete rounds are judged.
- **One early stop.** If Microcoder trails by three or more passes of six
  after round 1, the comparison stops there as "not better".
- **The 15-minute limit.** A run that hits it is a fail for its harness,
  whatever its check says.
- **Cost.** Each call's tokens at Vertex's on-demand Argon rates ($4.00 per
  million input tokens, $0.20 cached, $20.00 output). Antigravity's
  safety-monitor calls to Flash-Lite are not counted. A cut Antigravity
  run's tokens are read from its conversation store, as in #13231.

**The conditions.** Microcoder is better on this benchmark when, on N1 to
N6 over the judged rounds:

1. it passes at least as many runs as Antigravity,
2. its total cost is lower, and
3. its median wall time is lower.

**What the pass counts can resolve.** With two rounds each harness has 12
runs. If every task passed half the time, the gap between the two counts
would vary with a standard deviation of 2.4 runs, so a gap under 5 of 12 is
within noise. With three rounds (18 runs) the standard deviation is 3.0 and
the bar is 6 of 18. Tasks that always pass or always fail on both sides
lower those numbers; the report computes them from the rates seen. So
condition 1 is "not behind", as it was in #13231, and a lead in passes is
claimed only for a gap of at least two standard deviations.

**What this does not cover.** Carbon, and three runs of all 20 tasks on
both harnesses. The budget for this issue is $49 in all and about $33 is
left for these runs. "Spend" says what a full comparison would cost.

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
at and one short one (the build with the reader, 6a29b3d144 on `main`):

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

## Item 1: thinking by step is reverted

26c27d5714 reverts c2e1e45103, after #13231's Carbon runs ended. Round 4
measured it on Argon at three runs per task: 34 passes of 42 against
Antigravity's 35, about 25% more cost a run ($0.52 to $0.65), and a median
wall time tied at 155 s where Microcoder had led 68 s to 161 s. The
whole-call limit it added to the gateway plan goes with it.

Carbon was only ever measured with thinking by step on (37 of 42 against
35). Carbon without it has one run here, the issue below. The budget does
not cover more.

## Item 5: a seeded first step (kept)

**What it is.** In #13231's recorded runs, the first call of 14 of 14 tasks
only asked to view the files the task named. The loop now does that itself
before the first call. It picks the path-like words in the task and views
the first four that exist as files. It picks the code names with an
underscore and runs `git grep -n -w` for each, first 8 lines. The results
are one step in the state, journaled like any other.

**Measured.** Argon low, one run each, against the round 2 build's two runs:

| Task | Calls before | Calls with the seed | Cost before | Cost with | Wall s before | Wall s with | Passed |
| --- | --- | ---: | --- | ---: | --- | ---: | --- |
| T1 | 3, 3 | 2 | $0.12, $0.12 | $0.12 | 19, 30 | 18 | yes |
| H7 | 3, 3 | 2 | $0.15, $0.16 | $0.16 | 39, 45 | 40 | yes |
| H1 | 4, 4 | 3 | $0.21, $0.22 | $0.25 | 48, 67 | 77 | yes |
| H2 | 7, 5 | 4 | $0.46, $0.34 | $0.21 | 88, 59 | 131 | yes |
| T5 | 11, 4 | 10 | $0.95, $0.43 | $1.01 | 130, 85 | 203 | yes |
| T3 | 4, 5 | 6 | $0.21, $0.36 | $0.47 | 44, 81 | 95 | no ("warn") |

- **One call fewer on the short tasks:** T1, H7 and H1 each lost exactly
  their first call, and H2 took 4 calls against 5 and 7.
- **No gain on T5 and T3.** In T3 the seed viewed `coax/web/tool.sh`, which
  the task names only as a command, and searched for "GitHub", a word from
  the harness's preamble. Names are now words with an underscore only.
- **Wall times** in this table are not comparable: #13231's Carbon runs
  loaded the machine during these.

## Item 7: the read budget, and economy

The coordinator ran issue #13263 (a wording change across Everglade's
pages) on Carbon at the round 4 build. It ended "frozen capability, journal
or run budget refused" after 219 s, 17 calls and $2.55, with no edit.

### The limit (kept, b9795bd7b2)

A journaled run had 128 effects, and a file view counted as one. The run
had made 75 views and 53 commands.

- `coax/lib/agent/native-profile.coax` (an `#authority` file) now counts
  views apart: 512 a run. Commands and edits keep 128, model calls 64 and
  the run 15 minutes. `coax mutate` on the file: 146 mutants, 146 killed.
- A run that fails on a limit says which one: "read limit reached", "command
  limit reached", "model call limit reached" or "time limit reached".
- The coordinator asked for this change and said the owner was being told.
  No sign-off is recorded, so it is in `NEEDS_OWNER.md` with how to undo it.

### Economy (kept, 913a63b6fb)

**What the trace showed.** The run viewed 33 files 75 times. Viewed once
each, those files are 539 KB; the state holds 147 KB of recent results.
Each compaction kept only the names of the files a step had viewed, so the
model viewed them again: five files five times each. It ran the same
extraction of one audit section seven times. Nothing in its prompt said how
its state forgets.

**The change.** The system prompt now says how the state works: about
140 KB of recent results, and what a compacted step keeps. It says to
search before reading, to read line ranges, to write what will be needed
later in the step's rationale, not to view or run the same thing again to
remember it, and to change each place as it is found. A compacted step
keeps 1,200 bytes of its rationale, where it kept 300.

**On Argon, one run each,** against each task's nearest earlier run (H4 and
T4 at the reader's build, before the seed; T5 and T1 at the seed's build).
All passed on both sides:

| Task | Calls | Cost | Whole-file views | Cached input |
| --- | --- | --- | --- | --- |
| H4 | 7 to 6 | $0.62 to $0.47 | 5 to 1 | 25% to 57% |
| T4 | 14 to 7 | $1.33 to $0.59 | 9 to 3 | 25% to 33% |
| T5 | 10 to 4 | $1.01 to $0.33 | 8 to 3 | 30% to 62% |
| T1 | 2 to 2 | $0.12 to $0.11 | | |

H6 took 7 calls, $0.87 and 285 s, against 23 to 29 calls, $1.04 to $1.77
and 315 to 900 s in #13231's five runs.

**On the issue it came from, Carbon, one run** (start e4bcac425c, as the
coordinator's run; `main` has since done the issue):

| | Round 4 build | With the seed, the read budget, economy and 120 s |
| --- | --- | --- |
| Ended | failed: "run budget refused" | failed: "command limit reached: a run has 128 commands and edits" |
| Edits made | 0 | 0 |
| Model calls | 17 | 28 |
| File views | 75 | 38 |
| Commands | 53 | 128 |
| Cost | $2.55 | $3.15 |
| Wall time | 219 s | 410 s |

It did not finish and changed nothing. Carbon read less by view and more by
command (`cat`, `sed`, `git grep`), and it still never edited. Its
rationales are plans ("Let's read ... without truncation"), not findings,
and from the sixth call on they say it is reading again "the files we viewed
in compacted steps". So telling the model how the state forgets was not
enough for Carbon on a task this wide.

**What to try next there,** not built: a required field in the action for
what the step found (paths, lines, exact text), kept through compaction and
shown first, so the notes exist whether or not the model thinks to write
them. It changes the action's schema, so it needs its own measurement on
the tuning tasks and on this issue: about $5.

## A command's time limit: 120 s in Coder (kept, 66b316de85)

A command had 30 s everywhere. H6's own checks
(`coax test coax/lib/nostr/nipkeys.coax`) take 40 s on a quiet machine. In
#13231's H6 runs the model spent 5 to 8 calls starting them in the
background and polling.

Coder's worker now gives a command 120 s and says so in the facts
(`environment.command_seconds`). The Lab keeps 30 s.

| H6, Argon low, one run each | 30 s | 120 s |
| --- | --- | --- |
| The task's own checks | killed at 30 s; the run finished without their result | ran; failed once; the model fixed what they found; then passed |
| Calls, cost, wall time | 7, $0.87, 285 s | 7, $0.77, 264 s |
| Hidden check | passed | passed |

The cost is that STOP waits for a running command, so for up to 120 s.

## Item 4: cache hits (measured; nothing changed)

**In the recorded runs.** When a call hits the cache, the cached part is
the whole of an earlier request less its last 2,054 tokens, or a multiple
of 2,048 tokens. About 40% of calls whose prompt starts with an earlier
prompt get nothing. The hit often comes from the request two calls back,
not the one just before.

**A controlled probe** on Argon in `us` ($1.24): one prompt of about 12,800
tokens, grown at its end between calls 3 s apart.

| Series | Calls after the first that hit | Cached share of their input |
| --- | ---: | ---: |
| Grows by about 590 tokens a call | 0 of 6 | 0% |
| Grows by about 4,200 tokens a call | 4 of 6 | 51% |
| The same prompt each time | 4 of 5 | 67% |
| Grows by about 4,200 tokens, one part per step | 4 of 6 | 57% |

- **Step size matters.** Small steps were never cached; large ones were
  about half the time. This agrees with #13231's reading that a request's
  prefix is kept only when the next request adds about 2,050 tokens.
- **Request shape does not.** One text part or one part per step gave the
  same result, so Microcoder's single growing message is not the cause.
- **What moved the cached share in real runs** was not batching but fewer
  compactions: with the economy change H4 and T5 kept one growing prompt
  and 57% and 62% of their input was cached, against 25% and 30%.
- **Explicit caching:** Vertex accepts a `cachedContents` entry for
  `gemini-3.8-flash` (8,782 tokens, then deleted) and refuses one for
  `gemini-4-argon` with 400. Google's page lists neither Argon nor Carbon
  as supporting it. Carbon was not probed.

No change was made for this item. Padding small steps to pass the threshold
would buy cache hits with tokens that carry nothing.

## Item 3: models mixed inside one run (built, measured, removed)

**What it was.** The call after a clean working step (its edits all
applied, every command exit 0, every view found) went to Gemini Flash. The
first call, a call after reading only and a call after anything failed went
to Argon.

**Measured.** Argon low, the same build with and without mixing, one run
each. This changes what "the same model" means, so these runs are reported
apart, and the verdict uses one model.

| Task | One model | Mixed | Calls on Flash |
| --- | --- | --- | ---: |
| T5 | 3 calls, $0.41, 118 s | 3 calls, $0.41, 65 s | 1 |
| H1 | 3 calls, $0.23, 69 s | 3 calls, $0.19, 42 s | 1 |
| H2 | 5 calls, $0.31, 116 s | 7 calls, $0.38, 90 s | 2 |
| H3 | 6 calls, $0.44, 88 s | 14 calls, $0.80, 225 s | 2 |
| All | $1.39, 391 s, 4 passed | $1.77, 422 s, 4 passed | |

- A Flash call that only reads results takes 3 to 13 s.
- The Argon call after it finds none of its prompt cached, so the money
  saved on the Flash call is spent again.
- In H3 Flash wrote edits itself, and the run took eight more calls.

No gain, so it is out (93455e7032).

## Spend

Of the $49 for this issue, before the verdict runs: about $15.40.

| What | Cost |
| --- | ---: |
| The reader: offline probes and four runs | $2.46 |
| The seed: six runs | $2.22 |
| Economy: five runs | $2.37 |
| 120 s and mixed models: nine runs | $3.93 |
| Issue #13263 on Carbon, one run | $3.15 |
| The cache probe | $1.24 |
| Tiny test turns | $0.06 |

A full comparison (three runs of all 20 tasks on both harnesses, on Argon
and on Carbon) is costed after the first round, from its cost per run.
