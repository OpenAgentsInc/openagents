# Microcoder: a plan to be clearly better than Antigravity

Written 2026-11-12. GitHub #13325 (follows #13231). Evidence:
[the transcript analysis](../research/2026-11-12-microcoder-transcript-analysis.md),
[the benchmark and its verdict](../research/2026-11-12-microcoder-next.md),
[the earlier rounds](../research/2026-11-11-microcoder-vs-antigravity.md).

## Where it stands

On six unseen multi-file tasks (N1 to N6), Gemini 4 Argon at low thinking,
each task run on both harnesses at the same moment, two rounds:

| | Microcoder | Antigravity |
| --- | --- | --- |
| Passes | 8 of 12 | 10 of 12 |
| Total cost | $25.73 | at least $30.97 |
| Median wall time | 513 s | 606 s |

Microcoder is cheaper and faster but not better, because it passes less
often on wide tasks. On the 14 earlier tasks the two were level: 39 against 40 of 42 on Argon
with the judge's faults removed, and 37 against 35 of 42 on Carbon as
judged. The goal of this plan is a lead on every measure, large enough
that a fresh benchmark shows it beyond noise.

## What the transcripts show

Two analyses read about 270 runs step by step: every pair on the tuning
set, the 24 verdict runs, and the two runs on issue #13263. One read the
transcripts; the other reconstructed both harnesses' designs from their
code, logs and stored requests.

**Both harnesses use the same tools in practice.** Antigravity declares 13
tools but used four: `read_file`, `run_command`, `edit_file`,
`write_file`. It has no search tool and greps through the shell, as
Microcoder does. Its tools are not why it wins.

**Microcoder's losses are one problem: it forgets what it read.**

| | Microcoder | Antigravity |
| --- | --- | --- |
| What a call carries | about 30K tokens; recent results capped at 144 KB, then compacted to 24 KB | 100K to 230K tokens; history kept whole |
| Context shrinks | 98 in 673 Carbon calls | 3 in 822 |
| Reads of a file already read | 66% (Carbon), 39% (Argon) | 33%, 25%, mostly the next page |
| A file view | whole file up to 64 KB, no line numbers, no range | 800 numbered lines a page, any range |
| Long command output | head and tail kept, the middle lost | cut, and the whole output saved to a file it can page |

- **On wide tasks this ends runs.** In N4 (failed by Microcoder both
  rounds, passed by Antigravity both), Antigravity read every file the task
  touches in its first four calls, kept them, and edited all five in one
  call. Microcoder's first step was compacted at once; it read one file 72
  times in guessed ranges, made its first edit at call 42 to 46, and ran
  out of its 128 commands or its time. N3 round 1 and both #13263 runs
  ended the same way, with one file or none edited.
- **It also breaks edits.** The state records an applied edit as
  "applied" and nothing of the text, so a second edit to the same place is
  written from memory. 23% of Microcoder's edits failed on N1 to N6 (35 of
  152), against Antigravity's 2% (3 of 151); 30 of the 39 failures on the
  tuning set were on a file the run had already changed. Failed edits then
  turn into whole-file rewrites that take 60 to 105 s a call.
- **N1 round 2 is the same problem in another form.** The test setup it
  needed was viewed at call 1 and compacted at call 2, so it guessed three
  setup functions that don't exist through four rewrites of its own test
  file (about 315 s) and was cut at 900 s with a correct fix.

**Microcoder pays more per token than it should.** Input tokens are 80% of
its spend, and only 37% to 48% of them were served from cache, against 79%
to 91% for Antigravity on the same endpoint and model. When the prompt grew
by under 1K tokens, Antigravity's call hit the cache 98% of the time and
Microcoder's 41%. At Antigravity's cache rate, Microcoder's Carbon arm would
have cost about $21 instead of $54. The cause is the request's shape, not
timing: Antigravity sends a multi-turn conversation behind a fixed prefix of
about 12.5K tokens; Microcoder sends one growing user message behind a fixed
prefix of about 1.1K, under Vertex's 4,096-token caching minimum. Which part
matters has not been tested.

**Silent calls cost 90 s each.** Seven Microcoder calls on N1 to N6 sent
nothing for 90 s and were retried: 630 s in all.

**What Microcoder already does better, and must keep:**

- Edit, test and read the diff in one step (H7 in 5 calls against 19; N2 in
  7 against 17 to 24).
- A short system prompt and few calls on small tasks (H1, H7, T1, H2: 4 to
  6 calls against 9 to 26).
- Stopping when the task is met. Antigravity puts the repository's whole
  `AGENTS.md` in its prompt and so runs repository rituals such as
  `coax mutate` after the work is done (28 of 86 tuning runs, 5 of 12 N
  runs); that cut its N1 round 1 run at 900 s.
- The seeded first step, and the finish fix.

**Where Antigravity is weak, for the record:** stalls on large calls (three
120 s silences on 184K-token calls ended N3 round 1), a context that stops
caching past about 180K tokens (H4 Carbon ran out of time), a safety review
before every edit (64 s for eight edits in one T5 run), and rituals from
`AGENTS.md`.

## The changes, in order

Each change keeps Microcoder's one loop: one structured model call per step.
Each is a hypothesis built from runs it can't be judged on, so it stays only
with a measured gain on tasks it was not built from (N1, N2, N5, N6, the
tuning set as regression, and the fresh set below). No change names a task
or carries a hint for one.

### Phase 1: stop forgetting (passes)

1. **A budget for what one step adds, and reads that never flood the
   state.**
   - `view` takes a range (`path:A-B`) and returns numbered lines: at most
     about 300 lines or the range asked for, with a line saying which
     lines are left and how to ask for them.
   - A command's output over about 8 KB keeps its head and tail in the
     state, and its whole text is saved under the run's session folder.
     The cut line names that file, and the model can view it by range.
   - What one step adds to the state is capped at about 32 KB, so a single
     step can't push out everything before it.

   Expected: context shrinks and re-reads on wide tasks down to
   Antigravity's level, the first edit before about call 15, and no run
   that ends at a limit with one file done.
2. **Edit results that show what was written.** An applied edit returns the
   new text with its line numbers, and keeps it in the state as a view of
   that region would. A failed edit returns the closest matching region of
   the file with line numbers. Expected: edit failures from 23% (N) and 7%
   (tuning) to a few percent, and no whole-file rewrites caused by them.
3. **A shorter wait for a silent call at low thinking**, set from the
   recorded first-byte times of every Argon and Carbon call (checked
   offline first, at no cost). Expected: about 60 s saved per silent call.

Changes 1 and 2 are measured together in one round (they share the state
work); change 3 needs no run.

### Phase 2: pay Antigravity's rate per token (cost)

4. **A replay probe of request shapes,** about $3 to $9. One recorded run is
   replayed in three shapes:
   - a multi-turn conversation, each step a model turn and its results;
   - streaming calls throughout;
   - a fixed prefix over 4,096 tokens.

   The probe decides which shape gets 85% or more of calls cached. Nothing
   is built before it.
5. **Adopt the winning shape.** Expected: 50% to 65% less per input token.
6. **A larger working memory once it is cheap.**
   - The request limit (`AF_REQUEST`, 256 KB) rises to about 1 MB.
   - The step log rises to about 600 KB.
   - Compaction then becomes rare below about 150K tokens, where
     Antigravity's runs show the model still works well.

   `AF_REQUEST` is in an `#authority` file, so this needs the owner's
   sign-off.

### Phase 3: only if Phases 1 and 2 leave re-reading

7. **A required `found` field** in the action: paths, lines and exact text
   the step learned, kept whole through compaction.
8. **Context kept outside the prompt.** The Recursive Language Models
   library being built in Coax keeps a long context in an environment the
   model queries in pieces instead of holding it. It could replace
   compaction for the largest tasks.

### Not doing, and why

- **Putting `AGENTS.md` in the prompt.** It would lengthen the fixed prefix
  past the cache minimum, but it is what drives Antigravity's rituals.
  Phase 2 finds a cheaper way to a long stable prefix.
- **Counting read-only commands as reads, or raising the 128-command
  limit.** These treat the symptom: N4 round 2 would have hit the time
  limit 5 s later. Changes 1 and 2 remove the cause.
- **Already measured and removed:**
  - thinking by step: 25% more per run for a pass gain within noise;
  - a fresh reader at the finish: changed no outcome;
  - mixed models: 27% more cost for equal passes;
  - a self-review at the finish: changed no outcome.

## How "clearly better" will be judged

The owner capped further spend at $200 of Google credits (2026-11-12), so
the verdict is on one model and two runs a task. The definition is
committed before any final run, as before.

- **A fresh unseen set.** N1 to N6 are now seen. A separate agent builds
  twelve new tasks, U1 to U12, from this repository's history: multi-file
  changes and bug fixes of the size of N3 and N4, judged by tests that run.
  No Microcoder agent sees them until the verdict ends.
- **The runs.** Two runs per task per harness on Gemini 4 Argon at low
  thinking, as pairs that start together. A run that hits 15 minutes fails.
  Antigravity runs at its current engine (low thinking, the stall
  watchdog). Carbon is not judged.
- **The conditions:**
  1. Passes: Microcoder is not behind (24 runs a side). A lead in passes is
     claimed only beyond two standard deviations of the count difference.
  2. Cost: Microcoder's total is at most 60% of Antigravity's, and its
     average is cheaper on at least 10 of the 12 tasks (a sign test at
     p < 0.05).
  3. Time: Microcoder's median is at most 75% of Antigravity's, and its
     average is faster on at least 10 of the 12 tasks.

  "Clearly better" means all three. The lead is expected in cost and time,
  with passes level; a fresh set and a per-task sign test make that lead
  hard to put down to noise.

**The fresh set, recorded before any run** (2026-11-12). A separate agent
built and validated it; each check fails on the start tree, passes on the
known-good commit and on a second correct solution, and fails on two
incomplete ones. No model has run on these tasks. Hashes are the first 12
hex digits of each file's sha256.

| Task | Kind | Start | Known-good change | Prompt | Check |
| --- | --- | --- | --- | --- | --- |
| U1 | cross-file bug fix | `8876665195` | 5 files, 173 lines | `2cc11880faa6` | `3ece4613c0cc` |
| U2 | cross-file bug fix | `e18b86bede` | 4 files, 311 lines | `2df015539181` | `49f4a5be15ea` |
| U3 | multi-file feature | `6793c725f7` | 5 files, 80 lines | `190d4a4cdc6b` | `b5491c28a7d0` |
| U4 | multi-file feature (new module and its checks) | `1779c42d44` | 4 files, 269 lines | `ed631f8e5262` | `224e1aaef1ab` |
| U5 | multi-file feature | `5882381068` | 4 files, 343 lines | `5994cd216c79` | `3ad05841f8e8` |
| U6 | multi-file feature | `da4492f73a` | 6 files, 247 lines | `3b6128fac8de` | `e3771ed0edab` |
| U7 | multi-file feature | `f50ad88fcf` | 6 files, 458 lines | `8d05fd4974be` | `e615259d8ba5` |
| U8 | multi-file bug fix and feature | `b2046998bf` | 5 files, 176 lines | `0ba1e09ea889` | `a2aae86ed56f` |
| U9 | cross-file bug fix | `1b1b6a3914` | 4 files, 203 lines | `44c380ba5c51` | `af63ec603d1b` |
| U10 | refactor onto a shared component, one stated behaviour change | `c5dd3dc9c1` | 4 files, 138 lines | `ddeb30200e8f` | `dc3f7a5134df` |
| U11 | refactor across eight files, behaviour kept | `bbe291e06a` | 8 files, 223 lines | `efc27781a045` | `007176dda519` |
| U12 | cross-file bug fix | `dbbb17a7c2` | 4 files, 196 lines | `bbbeba429add` | `bd0989141247` |

Known limits: U11's check takes 23 to 43 s; U3 is smaller than N3 and N4;
U8 and U11 touch the same large app file; parts of U3, U5, U6, U7, U10, U11
and U12 need a service, browser or C build and are not tested.

## Microcoder RLM, and the verdict it is put to (2026-11-12, before any fresh-task run)

The owner asked for Recursive Language Models (`coax/lib/rlm/`, #13367) to
be connected to Microcoder. The result is a second way to run it, "RLM
code" (`coax/lib/rlm/code.coax`): the repository stays in the environment
and the model writes code against it (`search`, `lines`, `edit`, `sh`), so
file contents never sit in its prompt and nothing is compacted.

What the coordinator's own tests on the seen tasks N1 to N6 showed (one to
four runs each, about $80):

- **Argon as the model** passed N1, N2 and N3 on the first try, faster than
  Antigravity, at about Antigravity's cost. N4 passed once in four.
- **Changing the conversation's shape did not lower cost:** a smaller
  output cap, a larger one, and a manager with workers each cost more.
  Vertex serves a growing prompt's cache from two calls back, so what a run
  costs is how much new text the model reads.
- **Flash as the model** (90 turns) passed N1, N2, N3 and N6 on every run
  at $0.41 to $1.81 and 161 to 436 s, where Antigravity spent $1.22 to
  $3.97 and 304 to 900 s. It passed N5 two times in five and N4 never.
- **Flash, then Argon reviewing its diff** passed N5 three times in three
  (Argon made the fixing edit once) for about $0.35 and a minute more.
- Gemini 4 Argon asked for free text with code in it often wrote a native
  function call, which Vertex refuses; the model now answers in a schema
  (`c083ac8779`).

**The configuration judged ("Microcoder RLM"):**

1. `code.coax run --root-model gemini-3.8-flash --max-turns 90 --max-usd 2
   --max-minutes 8 --max-depth 1` on the task.
2. `code.coax run` on Gemini 4 Argon (low), `--max-usd 2.5 --max-minutes 7
   --max-depth 1`, with the task, what `git status` shows and the first
   pass's summary, told to check each requirement against the code, run the
   task's checks and fix or finish.

Both at `--profile danger-full-access` in a throwaway worktree, as the
earlier harness runs were. The two passes together can't pass 15 minutes.

**Against:** Antigravity through Coder on Gemini 4 Argon at low thinking
with its watchdog, as in every earlier round. That is each harness at its
best measured setting, not the same model: Microcoder RLM mostly runs
Flash. Antigravity on Flash was measured on 2026-11-11 (5 of 6, slower and
with more steps than on Argon).

**Tasks and runs.** U1 to U12, hashes above. One round first: each task
once on both harnesses, started together, two pairs at once. A second round
only if the first leaves the money for it (the owner's $200 cap) and
Microcoder RLM is not behind on passes by three or more.

**Conditions,** over the judged rounds, as above: passes not behind; total
cost at most 60% of Antigravity's and cheaper on at least 10 of 12 tasks;
median wall time at most 75% and faster on at least 10 of 12. A run over 15
minutes fails. Costs are from the rates card (Flash $1.50 / $0.15 / $7.50,
Argon $4.00 / $0.20 / $20.00 per million input, cached, output tokens).

### Round 1 on the fresh tasks: stopped after four pairs, Microcoder RLM not better (2026-11-12)

Run as defined above (the RLM tool and Coder at `3a96677c63`). The
coordinator stopped the round after U1 to U4 had finished on both sides and
U5 on Antigravity's, because the result was already clear and the money was
better kept. U1 to U6 are now seen; U7 to U12 are not.

| Task | Microcoder RLM | Antigravity |
| --- | --- | --- |
| U1 | pass, 724 s, $3.13 (Flash 84 turns to its $2 cap, then Argon 17 turns) | pass, 720 s, $2.18 |
| U2 | fail, 388 s, $2.52 (Flash stopped at 11 turns; Argon hit its $2.50 cap) | fail, cut at 900 s (cost not read) |
| U3 | pass, 729 s, $3.35 (Flash to its cap; Argon to its time limit) | pass, 360 s, $2.41 |
| U4 | pass, 776 s, $2.32 | pass, 522 s, $1.62 |
| U5 | not finished | pass, 275 s, $0.98 |

- **Passes level (3 of 4 each), and Microcoder RLM dearer and slower on
  every finished pair:** about 40% more money and up to twice the time. No
  condition for cost or time can be met from here.
- **Why the tests on N1 to N6 did not carry over.** On these tasks Flash
  used its whole budget (80 turns and $1.90) without finishing, and Argon
  then did 17 to 20 turns of its own. On N1 to N6 Flash usually finished in
  30 to 55 turns and Argon's pass was a minute. Those six were also the
  tasks the configuration was chosen on, by the coordinator, in single
  runs.
- **What stands from the RLM work:** the repository-in-the-environment
  agent on Argon passed wide tasks the JSON-action loop failed (N1, N2, N3
  first try; no failed edits), at Antigravity's speed or better and about
  its cost; and a root that answers in a schema. What does not stand: any
  claim that it is cheaper or faster than Antigravity on unseen work.

Spend on this section's work: about $105 of the owner's $200 (about $80 on
the tests on seen tasks, about $25 on this round).

## Cost and order of work

Within $200 of Google credits from 2026-11-12:

| Step | Spend |
| --- | --- |
| Phase 1, changes 1 and 2: one run each of N4, N3, N1 and three tuning tasks, against their recorded runs | about $15 |
| Change 3: offline | $0 |
| Phase 2: the request-shape probe | about $5 |
| Phase 2: one check round with the winning shape (six runs) | about $10 |
| The verdict: 12 tasks, 2 runs a side, 48 runs | about $115 |
| Reserve, for a rerun of a broken task or a failed setup | about $55 |

Order: Phase 1 is built and checked; the fresh set is built in parallel;
then the probe and the shape change; then the verdict. A step that shows no
gain on its check is removed and not paid for again. Change 6 (a larger
window) is built only if the probe makes it cheap, and needs the owner's
sign-off for `AF_REQUEST`.

## Decisions for the owner

1. **Sign-off for `AF_REQUEST`** (change 6), only if the probe makes a
   larger window cheap; and the read limit already changed in
   `native-profile.coax` (recorded in `NEEDS_OWNER.md`).
2. **Whether the verdict above is the right bar:** passes level, with
   large, consistent leads on cost and time, on Argon.
