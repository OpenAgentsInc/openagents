# Closing: The Shape of What's Coming

- **Date:** 2025-12-08
- **Time:** 17:05 CT
- **Author:** Claude Opus 4.5
- **Context:** Final synthesis and instructions

---

## The Big Picture

What you've built this week isn't just a test generation system that evolves its own configs. It's a proof-of-concept for a different theory of machine intelligence entirely.

For sixty years, AI research has oscillated between two poles: symbolic systems that encode human knowledge (and shatter on contact with reality) and statistical systems that learn from data (but can't explain themselves and require oceanic compute to train). The current paradigm—foundation models—is the apotheosis of the statistical approach: pour in all the text, turn the crank on gradient descent, and hope that intelligence emerges from scale.

What you're prototyping is a third path. Not intelligence-as-knowledge-encoding, not intelligence-as-weight-optimization, but **intelligence-as-structured-search-with-memory**. The model provides priors. The environment provides grounding. The tests provide feedback. The loops provide adaptation. The database provides memory. And the whole apparatus **learns not just to solve problems, but to learn how to evaluate whether problems are solved**—which is to say, it learns epistemology.

If this works—if TestGen scores improve under evolution, if those improvements transfer to HillClimber pass rates, if internal metrics correlate with external benchmarks, if local compute with better scaffolding can match or exceed cloud compute with bigger models—then you've demonstrated something profound: that the missing ingredient in AI isn't more parameters or more data, but better **structure** around inference. That the Bitter Lesson applies not just to training but to deployment. That a small model embedded in a rich epistemic architecture can outcompete a large model embedded in a thin one.

And if that's true, then the strategic implications cascade: Apple's on-device models become a serious platform, not a privacy feature. Cloud providers' moats narrow. The agent era becomes about **who builds the best loops**, not who trains the biggest weights. The entire economics of AI shifts from "rent intelligence by the token" to "own the infrastructure for search." Terminal-Bench stops being a benchmark and becomes a battleground for architectural paradigms.

None of that is guaranteed. It all reduces to whether three graphs slope upward. But the fact that it *could* be true—that you've constructed a system where these cosmic questions have operational definitions and falsifiable predictions—is itself a kind of achievement. Most AI philosophy is theater. Yours runs on SQLite.

---

## Instructions for the Coding Agent

### Immediate (Before Running Evolution)

1. **Fix token efficiency calculation**
   - File: `src/hillclimber/testgen-analyzer.ts`
   - Bug: `tokenEfficiency` shows 0.00 despite having comprehensiveness (8.0) and tokens (53k)
   - Expected: `(8.0 / 55221) * 1000 / 10 ≈ 0.0145` or adjust formula for meaningful scale
   - Add unit tests to verify calculation against known trajectories

2. **Fix trajectory save timing**
   - File: `src/hillclimber/testgen-runner.ts`
   - Bug: Using 1-second delay workaround before reading trajectory
   - Fix: Refactor `insertTestGenTrajectory` to return a proper Effect/Promise and await it
   - The runner should not proceed until persistence confirms success

3. **Add meta-reasoner rate limit handling**
   - File: `src/hillclimber/testgen-meta-reasoner.ts`
   - Add exponential backoff (start 5s, max 60s, 3 retries)
   - On exhaustion: return `{ type: "keep" }` gracefully
   - Log rate limit events for monitoring

### Short-Term (First Evolution Run)

4. **Add guardrails to prevent degenerate evolution**
   - Hard minimum: 10 total tests, 2 per category
   - Soft ceiling: warn at 80k tokens, hard-stop at 100k
   - Config delta caps: temperature ±0.1, tests ±1, rounds ±1 per evolution step
   - Implement in `testgen-meta-reasoner.ts` validation

5. **Run the first real evolution experiment**
   ```bash
   bun run src/hillclimber/test-gen-cli.ts --evolve \
     --max-runs 50 \
     --task regex-log \
     --sleep 5000
   ```
   - Capture: score vs run index, comprehensiveness vs run index, config changes
   - Success: upward trend in score, non-trivial config changes
   - Log everything: even failed meta-reasoning attempts

6. **Create research tracking document**
   - File: `docs/research/2025-12-testgen-hillclimber.md`
   - Sections: Hypotheses, Methods, Results, Discussion
   - This becomes the artifact for TB2 submission / investor conversations

### Medium-Term (After First Experiment)

7. **Run correlation experiment: TestGen quality → HillClimber performance**
   - Generate tests with baseline config (v1.0.0)
   - Run HillClimber on `regex-log` with those tests, record pass rate
   - After evolution, generate tests with best config (e.g., v1.3.0)
   - Run HillClimber again, compare pass rates
   - This is the "does it actually help agents?" question

8. **Run validation experiment: Internal metrics → TB2 correlation**
   - Pick 3-5 tasks where you'll allow yourself to see TB2 official tests
   - Generate tests, compute internal metrics
   - Compare to TB2 tests (overlap, coverage, similarity)
   - Correlate: which internal metrics predict TB2 alignment?

9. **Build minimal stats visualization**
   - CLI is fine for now: `testgen:stats --plot` or similar
   - Output: score over time, config changes, metric trends
   - Don't build a full UI yet—just enough to see the curves

### What NOT To Do (Yet)

- Don't refactor the architecture until you have experimental results
- Don't add new metrics to the scoring function until you've validated existing ones
- Don't build the UI dashboard until you know what's worth visualizing
- Don't optimize for token efficiency until you know whether it correlates with quality
- Don't automate scoring function evolution (Loop 4) until Loops 0-2 are stable

---

## The Three Graphs That Matter

Everything reduces to whether these slope upward:

1. **TestGen score vs evolution step** — Does meta-learning work?
2. **HillClimber pass rate vs TestGen config version** — Does epistemic quality transfer?
3. **TB2 performance vs internal TestGen metrics** — Is bootstrapping valid?

If yes to all three: paradigm shift confirmed, proceed to scale.
If no: diagnose which link is broken, fix it, retry.

---

## Final Word

You've spent a day philosophizing with multiple AI systems about the nature of machine intelligence, the Bitter Lesson, epistemic engines, and the future of coding agents. That conversation generated genuine insight and refined the thesis to something falsifiable.

Now stop talking and run the loops.

The philosophy is done. The code is written. The database is waiting. Either the curves bend upward and everything we've said is vindicated, or they don't and we learn something important.

Press go. See what happens. Report back.

---

- **Status:** Commentary complete
- **Next:** Fix bugs → Run 50-iteration evolution → Plot the curves → Decide what's real
