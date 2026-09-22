# Why the two winning Coder One runs were cheap and fast

This analysis takes apart the two best Coder One configurations on the four
panel tasks: the cheapest, **Jev-probe → GPT-6 Luna**, and the fastest,
**Jev-probe → lean Opus 5.5 at low effort**. It explains where their time
and money went, what Jev contributed, and what an upgrade needs to beat
both. The numbers come from three trials per task per arm, 24 trials in all.
Every trial passed. Results: [the results page](README.md).

## The two winners against the baselines

The four-task figures are sums of per-task means. Costs are list-price
figures; see [How to read the columns](README.md#how-to-read-the-columns).

| Configuration | Four-task cost | Four-task agent time | Delegate turns per task (mean) |
| --- | --- | --- | --- |
| Opus 5.5 direct (Claude Code) | $0.6554 | 195.9 s | 7, 17, 5, 3 steps |
| GPT-6 Luna direct (Codex, n = 1) | $0.0336 | 387.4 s | 9, 31, 6, 11 steps |
| **Jev-probe → Luna** (cheapest) | **$0.0219** | 318.1 s | 8.0, 25.7, 7.7, 6.7 |
| **Jev-probe → lean Opus, low effort** (fastest) | $0.3075 | **125.1 s** | 4.0, 7.7, 2.0, 3.0 |

Task order is `fix-git`, `build-cython-ext`, `headless-terminal`,
`fix-code-vulnerability`.

## What happens in a run

Both arms run the same host sequence before any model starts:

1. **Probe battery (0.4 to 2.5 seconds).** The host runs up to twelve
   read-only commands in parallel: a listing, a tree, the README, test
   files, Python and package versions, and on a Git work tree `git status`,
   branches, log, reflog, and stash, plus a listing or head of every
   absolute path the task names.
2. **Jev judges the probes (one request, median 0.4 seconds).** One Noul per
   output: does it hold information the task needs? Outputs at 0.5 or
   higher are kept, up to six and 14,000 characters.
3. **Jev surveys files (0.2 to 2.4 seconds).** Up to 100 candidate files are
   judged in parallel batches of 20, with two Nouls each: relevant, and
   likely to need an edit. The top relevant files' contents are kept.
4. **Brief and delegate.** Code assembles the briefing (2,500 to 12,000
   characters) and hands the task to the delegate with directions to use the
   evidence rather than gather it again.
5. **Close.** One Jev request checks whether the task looks done. It changes
   nothing; it's recorded.

The host's part, Jev included, takes 1 to 5 seconds, and 3.6% to 5.5% of
agent time. **The delegate takes the other 95%.** Everything that follows is
about what the briefing does to the delegate's turns.

## What Jev chose, task by task

| Task | Probes Jev kept (every trial) | Files Jev kept | First delegate action |
| --- | --- | --- | --- |
| `fix-git` | `git log --graph --all`, `git reflog`, `git branch -a`, `git status`, listing | none | `git show --stat c499730 && git merge c499730`: straight to the lost commit the reflog showed |
| `build-cython-ext` | package list, listing | none (the repository is cloned by the task) | `git clone` of the named repository |
| `headless-terminal` | listing, tree, package list | `base_terminal.py` (0.97 relevance) | Opus: write `headless_terminal.py` in one call; Luna: re-read `base_terminal.py`, then write |
| `fix-code-vulnerability` | listing, tree, `git status`, `git log`, `head -200 bottle.py`, branches | `bottle.py` (cut at 8,000 characters) | `git diff bottle.py` and a targeted search for the header-validation code |

**Jev's selections were consistent and task-specific.** On `fix-git` Jev kept
the git history probes and dropped the file listing noise. On
`fix-code-vulnerability` it kept the file with the bug. On `headless-terminal`
it kept the one base class the task extends. Across all 24 trials the kept
set varied by at most one low-ranked probe.

**The briefing removed exploration.** Counting the delegate's commands that
only look around (`ls`, `cat`, `find`, `git log`, `grep`, and so on):

- Luna: **0 exploratory commands in 12 trials.** Its first command was always
  work: the merge, the clone, the targeted search.
- Lean Opus at low effort: 0 to 2 per trial, all targeted (a single `grep`
  across the alias catalogue, or reading the one conflicted file).
- For comparison, Opus direct spent its first 0 to 3 steps looking around
  on the same tasks: git status, reflog, and history on `fix-git`, a listing
  and `setup.py` on `build-cython-ext`, and a listing and `cat` on
  `headless-terminal`. On `fix-code-vulnerability` it started with the diff.

The `fix-git` case shows the effect most clearly. From a plain briefing
without probes, Luna merged the wrong version and failed. From the probe
briefing, which put the reflog in front of it, it passed all three trials.

## Why Jev-probe → Luna is cheap

Its cost is almost entirely Luna's tokens, and Luna is priced at $0.10 per
million input tokens, $0.01 cached, and $0.50 output. Across the 12 trials:

| Share of cost | |
| --- | --- |
| Cached input | 38.6% |
| Uncached input | 31.5% |
| Output | 20.3% |
| **Jev** | **9.6%** |

- **Codex caches well.** 88% to 94% of Luna's input tokens were cache reads
  at a tenth of the price.
- **Uncached input stays small** (7,000 to 42,000 tokens per run), because
  the briefing is compact and Luna rarely reads large files.
- **Jev is now a tenth of the bill.** At $0.042 per million input tokens,
  the probe, survey, and close requests cost about $0.0005 per run, which is
  small against Opus but large against a $0.003 Luna run. The file survey,
  which sends 20 excerpts per request in up to five requests, is most of it.

## Why Jev-probe → lean Opus at low effort is fast

- **Very few turns.** Low effort makes Opus consolidate: one command often
  does what three would. `headless-terminal` took **2 turns** in every trial:
  write the whole file, then check it. `fix-code-vulnerability` took 3.
  `fix-git` took 4.
- **Fast turns.** Each Opus turn took about 3 to 9 seconds, and the lean tool
  set keeps every call's prompt near 7,000 to 11,000 tokens instead of
  16,000 to 20,000.
- **No exploration.** Its first action on each task was already the fix.

**Why it still costs 14 times as much as the Luna arm:** 54.1% of its cost is
cache writes. Claude Code writes its system prompt and the briefing to a
**one-hour** cache on the first call, billed at 2× the input rate ($8 per
million for Opus 5.5), and these runs finish in seconds, so the long cache
lifetime buys nothing. Output is another 35.1% at $20 per million.
[The captured request](claude-code-delegate-prompt/README.md) shows exactly
what goes into that write: about 5,500 characters of system prompt, six tool
definitions, and the briefing, all under `ttl: 1h` markers.

## Where each winner loses to the other

| | Luna arm | Opus arm |
| --- | --- | --- |
| Loses on | **Time**: 2.5 times slower | **Cost**: 14 times more expensive |
| Why | Many turns on build-heavy work (20 to 31 on `build-cython-ext`, each `pip install` or build its own turn), and re-reading files already in the briefing | Cache writes on a one-hour lifetime, and $20-per-million output |
| Worst task | `build-cython-ext`: 185.8 s against 78.0 s | `fix-code-vulnerability`: $0.0642 against $0.0035 |

## How to beat both

The target: a four-task cost below $0.0219 and a four-task time below 125.1
seconds, with every task passing. The cheap model is the only way under
$0.0219; Opus's cache-write floor alone is about $0.03 per task. **So the
upgrade has to make Luna about 2.5 times faster**, mostly by cutting its
turns, while spending less on Jev. Five changes, in the order they matter:

1. **Batch-mode directions.** Tell the delegate to act in few large steps:
   write whole files in one command, chain installs and builds, and run the
   final check once. Low-effort Opus shows the effect: two turns on
   `headless-terminal` against Luna's 5 to 11. Also tell it that files in the
   briefing are complete and current, so it stops re-reading them.
2. **A setup pack chosen by Jev.** Code extracts the setup steps a task names
   (a repository URL and ref to clone, a requirements or project file to
   install). One Jev request asks, per step, whether the task needs it done
   first. The host runs the approved steps in parallel with the probes, and
   the briefing reports their results. On `build-cython-ext`, every trial's
   first two to five Luna turns were exactly these steps.
3. **Complete files for the edit targets.** Raise the per-file cap for files
   Jev scores as likely edits (edit probability 0.8 or higher) so the
   delegate gets them whole, and include the matching test file. This cuts
   read-then-edit turns.
4. **A cheaper Jev front end.** Skip the file survey when the probes already
   name the edit target, and cap the survey pool at 40 files otherwise. This
   halves Jev's share of the cheap arm's cost without losing the selections
   above.
5. **A time-boxed escalation, only if needed.** If the Luna delegate hasn't
   finished within a Jev-sized time budget, start lean Opus at low effort
   with the same briefing plus Luna's progress. This protects the time target
   on build-heavy tasks at a cost that applies only when it fires.

For the Opus track, two further changes are worth measuring separately:
running Claude Code with a five-minute cache instead of one hour
(`CLAUDE_CODE_PROMPT_CACHE_TTL=5m`, confirmed in
[the captured request](claude-code-delegate-prompt/README.md#can-it-be-changed)),
which would cut its cache-write cost by about 37%, and the same batch-mode
directions.

## How to test it

Run the upgraded arm on the four panel tasks and on the four `extended`
tasks the panel does not cover (`cancel-async-tasks`, `git-leak-recovery`,
`log-summary-date-ranges`, `sqlite-db-truncate`), with three trials each.
Compare it against both winners and both direct baselines on the same pins.
It beats both winners only if every task passes, its four-task cost is below
the Luna arm's, and its four-task time is below the Opus arm's. A partial
win, such as cheaper than Luna and faster than Luna, is recorded as exactly
that.

## What the upgrade did

Jev-probe v2 shipped changes 1 through 4 above, plus the 40-file survey
cap, behind `CODER_ONE_PROBE_V2=on` (commit `03401dad74`). It ran three
trials per task on the four panel tasks and on the four `extended` tasks.
Full tables: [the results page](README.md#jev-probe-arms-2026-09-22).

| Configuration | Panel: passed, cost, time | Extended: passed, cost, time |
| --- | --- | --- |
| Opus 5.5 direct | 12/12, $0.6554, 195.9 s | 12/12, $0.4309, 110.5 s |
| GPT-6 Luna direct | 4/4 (n = 1), $0.0336, 387.4 s | 9/12, $0.0095, 190.4 s |
| Jev-probe → Luna | 12/12, $0.0219, 318.1 s | 6/12, $0.0100, 120.1 s |
| **Jev-probe v2 → Luna** | 9/12, $0.0214, 265.3 s | 8/12, $0.0093, 144.9 s |
| Jev-probe → lean Opus, low effort | 12/12, $0.3075, 125.1 s | 12/12, $0.2482, 65.1 s |
| **Jev-probe v2 → lean Opus, low effort** | 12/12, $0.3249, **120.7 s** | 12/12, **$0.2273, 57.2 s** |

**It didn't beat both winners.** What each change did:

- **Batch-mode directions** cut Luna's turns where there was slack
  (`headless-terminal` 7.7 to 4.7, `build-cython-ext` 25.7 to 19.0), and
  Luna's panel time fell 17% on the three tasks it passed. The Opus arm was
  already at two to four turns, so it gained little on the panel; on the
  `extended` tasks its turns fell from 3.7 to 2.7 on average, and time fell
  12%.
- **The setup pack** worked as designed: on `build-cython-ext` Jev
  approved the clone (p = 0.91) and the host ran it before the delegate
  started. It also changed what the survey saw. With the repository
  present, the survey kept its source files and the Opus briefing grew
  from 2,783 to about 9,500 characters, which raised that task's cost 15%.
- **Luna failed `build-cython-ext` in all three v2 trials**, each time on
  a NumPy alias its own checks didn't exercise. The earlier probe arm
  passed all three; its trials searched for doubled replacements
  (`float6464`) before finishing. "Run one final check" appears to have
  cut verification along with turns.
- **Jev's cost fell by a quarter**, $0.00044 to $0.00033 a run, from the
  smaller survey pool.

**Next iteration.** Two changes follow directly from these results:

1. Replace "run one final check" with "run the checks the task names, and
   test every code path you changed", so batch mode keeps the Luna turn
   savings without dropping verification.
2. Run the Opus arm with `CLAUDE_CODE_PROMPT_CACHE_TTL=5m` ([the captured
   request](claude-code-delegate-prompt/README.md#can-it-be-changed)).
   One-hour cache writes are more than half its cost; five-minute writes
   cost 1.25 times the input rate instead of 2 times.

Neither closes the gap alone. The Luna arm's panel time is bound by
`build-cython-ext` (158 seconds with v2), and Opus's cost floor is about
$0.03 a task even with the shorter cache.

## Probe v3

v3 (commit `88f47d67b4`) made both changes above and ran three trials per
task on all eight tasks.

| Configuration | Panel: passed, cost, time | Extended: passed, cost, time |
| --- | --- | --- |
| **Jev-probe v3 → Luna** | 11/12, **$0.0198**, 275.2 s | 8/12, $0.0087, 145.0 s |
| **Jev-probe v3 → lean Opus, low effort, five-minute cache** | 12/12, $0.2853, 162.4 s | 12/12, **$0.2008**, 67.4 s |

- **The checked directions fixed Luna's failure mode.** `build-cython-ext`
  passed 3 of 3, in 163.8 seconds against 185.8 for the original probe
  arm. The one v3 miss was on `headless-terminal`. Across the panel it's
  the cheapest configuration so far.
- **The five-minute cache works as measured.** Every Opus v3 cache write
  was a five-minute write, and the arm's cost fell 12% on both sets.
- **The checked directions cost the Opus arm time.** On
  `build-cython-ext` it took more turns (8.7 against 6.3) and 50% longer.
  Opus didn't need the extra checking: v2's Opus arm passed all 24
  trials without it.

The best configuration per delegate is now different: Luna with v3's
directions, and Opus with v2's directions plus the five-minute cache. The
second hasn't been run yet. Neither closes the gap between the two
winners: the cheapest time on the panel is still an Opus arm, and the
cheapest cost a Luna arm.
