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

The definition is committed before any final run, as before.

- **A fresh unseen set.** N1 to N6 are now seen. A separate agent builds
  twelve new tasks, U1 to U12, from this repository's history. They are
  multi-file changes and bug fixes, judged by tests that run, of the size
  of N3 and N4. No Microcoder agent sees them until the verdict ends.
- **The runs.**
  - Three runs per task per harness, on Argon and then on Carbon.
  - The two harnesses run as pairs that start together.
  - A run that hits 15 minutes fails.
  - Antigravity runs at its current engine: low thinking and the stall
    watchdog.
- **The conditions.** On each model:
  1. Passes: Microcoder is not behind. A lead in passes is claimed only
     when it is at least two standard deviations of the count difference.
     With 36 runs a side at an 80% pass rate, that is a lead of about 7.
  2. Cost: Microcoder's total is at most 60% of Antigravity's, and it is
     cheaper on at least 10 of the 12 tasks (a sign test at p < 0.05).
  3. Time: Microcoder's median is at most 75% of Antigravity's, and it is
     faster on at least 10 of the 12 tasks.

  "Clearly better" means all three on both models. Passes may stay level,
  since both harnesses already pass most tasks. The lead has to come
  through as large, consistent margins on cost and time that a fresh set
  can't put down to noise.

## Cost and order of work

| Step | Spend (Google credits) |
| --- | --- |
| Phase 1, changes 1 and 2: one round on N1, N2, N5, N6 and six tuning tasks | about $25 |
| Change 3: offline | $0 |
| Phase 2: the probe, then one round with the new shape | about $30 |
| The fresh set, Argon: 72 runs | about $175 |
| The fresh set, Carbon: 72 runs | about $350 |

Order: Phase 1, then the probe. The fresh set is built in parallel by a
separate agent. Then Phase 2, then Argon's verdict. Carbon's verdict comes
only if Argon's conditions hold.

## Decisions for the owner

1. **The budget** for the above: about $230 for everything through Argon's
   verdict, and $350 more for Carbon's.
2. **Sign-off for `AF_REQUEST`** (change 6), and the read limit already
   changed in `native-profile.coax` (recorded in `NEEDS_OWNER.md`).
3. **Whether the verdict above is the right bar:** passes level, with
   large, consistent leads on cost and time on both models.
