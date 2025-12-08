Alright, this is the fun part: you’ve crossed the “talking about it” -> “it actually runs, twice, and wrote to SQLite” boundary. So let me be the boring-but-useful voice that takes all this philosophy and turns it into a small, ruthless research program.

I’ll keep it practical.

---

## 1. You now have *three* things, not one

From all of that:

1. **A working TestGen engine**

   * Iterative, category-aware, reflection-aware.
   * Proven to generate ~21 tests with 0.89 balance and 8.0/10 self-rated comprehensiveness for `regex-log`.
   * Fully persisted as trajectories.

2. **A working TestGen HillClimber evolution loop (minus meta)**

   * Runs, scores, writes to `testgen_runs` and `testgen_configs`.
   * CLI behaves, migrations behave, analyzer behaves (modulo the token efficiency bug).

3. **A very concrete thesis**

   > “Evolving test generation configs and using better epistemic tooling will measurably improve both TestGen quality and downstream agent performance, enough to compensate for smaller/local models.”

You don’t need more conceptual machinery. You need verdicts.

---

## 2. Lock the architecture, change only *two* things

For the next chunk of work, I’d treat the architecture as **frozen** (minus bugfixes), and give yourself permission to change only:

1. **Config values** (via evolution loop + occasional human edits)
2. **Scoring weights** (slowly, with explicit versioning)

Everything else—file layout, message schema, tables, HUD messages—should be considered “off limits” until you’ve run enough experiments to know what’s actually working.

That gives you a clean separation:

* *Implementation substrate* is stable.
* *Learned knobs* are allowed to move.

---

## 3. Shrink the research plan to 3 decisive questions

You have a dozen possible experiments. I’d collapse them into three, each with a binary-ish outcome.

### Q1. Does evolution improve testgen scores over time?

**Design:**

* Fix the current config as `v1.0.0`.

* Fix the scoring function as `score_v1` (what you have now).

* Get meta-reasoner unblocked (paid model / backoff / local FM).

* Run:

  ```bash
  bun run src/hillclimber/test-gen-cli.ts --evolve \
    --max-runs 50 \
    --task regex-log \
    --sleep 2000
  ```

* Plot:

  * score vs run index
  * comprehensiveness vs run index
  * tests-per-category vs run index

**Success =** clear upward trajectory in score (and/or comprehensiveness) over 50 runs; ideally with configs actually changing in non-trivial ways.

**Failure =** scores bounce around noise level or stagnate, configs oscillate, or evolution has no effect.

This alone answers: *“Is TestGen HillClimber doing anything real?”*

---

### Q2. Do better TestGen configs actually help HillClimber solve tasks?

This is the “epistemic engine ties back to agents” question.

**Design:**

For a given TB task (or a small set of them):

1. Generate tests with **baseline** TestGen config (v1.0.0).
2. Run HillClimber on that task using those tests.
3. Record:

   * pass rate
   * median turns to success
   * maybe some “hint usage” stats.

Then:

4. After evolution, pick a **new** TestGen config (say v1.3.0, the best one by score).
5. Generate tests again for the same task(s).
6. Run HillClimber again with these new tests.
7. Compare.

**Success =**

* Higher pass rate and/or fewer turns with evolved tests **without** obviously trivializing the test suite (e.g., number of tests didn’t collapse to 3 nonsense checks).

**Failure =**

* No improvement, or worse: pass rate improves only because the evolved tests got easier / fewer.

You’re literally asking: *“Does better testgen score → better agent behavior?”*

---

### Q3. Do internal metrics correlate with an external benchmark (TB2)?

You can’t do this for all tasks without sacrificing blindness, but you can do it for a small, quarantined subset.

**Design (development mode):**

1. Pick N tasks where you allow yourself to inspect the official TB2 tests.

2. For each task:

   * Run TestGen with a given config.
   * Compute internal metrics (score, comprehensiveness, balance, anti-cheat coverage, etc.).
   * Compare generated tests to TB2 tests:

     * Overlap in input patterns.
     * Overlap in covered edge cases.
     * Manual or automated “similarity” rating (even rough).

3. Correlate:

   * internal score vs “TB2 similarity”
   * individual sub-metrics (e.g., balance, anti-cheat) vs TB2 similarity.

**Success =** some internal metrics clearly move in the same direction as “looks like TB2-quality tests.”

**Failure =** internal metrics go up while TB2 similarity is flat or random; or no discernible relationship.

This answers: *“Is our scoring function pointed in roughly the same direction as what TB2 actually cares about?”*

---

## 4. Bugfixes that matter *before* those 3 questions

Out of your bug list, I’d say three are blocking for serious experimentation:

1. **Token efficiency = 0.00**

   * This is a free field in your scoring vector. Don’t waste it.
   * Fix the calc, then aggressively **log and histogram** it. You may discover token usage variation that you can later trade off or constrain.

2. **Trajectory save timing**

   * Replace the 1s sleep with a real `await` on the insert Effect.
   * You *really* don’t want races here; experiment results depending on “whether the DB was fast enough today” is nightmare territory.

3. **Meta-reasoner rate limits**

   * Simple exponential backoff + fallback to “no change” is fine.
   * For early experiments, you can even require meta to succeed, and if it 429s, just skip that iteration and don’t log it at all.

Everything else (UI dashboard, retention, etc.) can wait until you’ve got those three big questions answered.

---

## 5. Guardrails that will save you pain later

While you’re running 50-run evolutions, I’d clamp a few things up-front:

* **Hard min tests per category** (e.g., ≥ 2) and **hard min total tests** (e.g., ≥ 10).
  This prevents the obvious “just generate 3 tests” collapse.

* **Ceiling on tokens per run** (soft limit, warn at say 80k, hard-stop at 100k).
  This keeps evolution from discovering “just sample 15 rounds at temp 1.0 forever.”

* **Config delta caps**
  Don’t let a single meta step change:

  * temperature by > 0.1
  * min/max tests by > 1
  * max rounds by > 1
    This keeps evolution smooth enough to reason about.

* **Log config + score pair every single run**
  Even if meta fails, “(config, score)” is your core dataset.

---

## 6. Treat this like a small ML paper, not a feature

You’re not just shipping a widget; you’re running a mini research program. I’d literally give yourself a doc like:

> `docs/research/2025-12-testgen-hillclimber.md`

with sections:

* Hypotheses (the three questions above)
* Methods (commands you run, task sets)
* Results (plots, tables)
* Discussion (what worked / didn’t)
* Next changes to the system justified by those results

The win isn’t just “hehe it works,” it’s being able to say to yourself (and later to TB2 judges / investors):

> “Here are the curves.
> Here’s how evolution improved TestGen scores.
> Here’s how that translated to HillClimber.
> Here’s where local + loops got us compared to cloud + one-shot.”

You already almost have that doc in your logs; it just needs to be consolidated into something you can revisit when you’re six months and 50 experiments further in.

---

## 7. My one-sentence synthesis

Right now, you don’t need more theory; you need **three clean graphs**:

1. TestGen score vs evolution step
2. HillClimber pass rate vs TestGen config version
3. TB2 performance vs internal TestGen metrics

Everything else—the Bitter Lesson, the epistemic engine, the multi-loop elegance—either cashes out into those curves or it doesn’t.

You’ve done the hard architectural work. Now it’s just: fix the couple of bugs, unblock meta, press “go” on `--evolve`, and see if the world matches the story you’ve been telling.
