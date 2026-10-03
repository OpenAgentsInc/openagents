# Calibration: what TypeSafe means by it, and why we aren't really doing it

Date: 2026-10-03. Sources:
- the three transcripts in this folder;
- every page of <https://docs.typesafe.ai/> that uses "calibrate", "calibrated" or "calibration": all 111 sitemap pages were read through `llms-full.txt`, plus the `/confidence` page, which those pages point to;
- this repository at `origin/main`.

## Summary

TypeSafe sells Jev on one property: **its probabilities are calibrated against outcomes**, so software can act on them through thresholds. TypeSafe also says, on every page that shows a threshold, that the threshold itself is not calibrated. A production threshold has to come from your own labelled outcomes and the cost of being wrong.

We take Jev's probabilities and gate about 40 product decisions on them, mostly in the router, delegation and background rules. The router, delegation and background thresholds were all chosen by hand on raw numbers. We measure reliability offline for two router questions, and we keep even that map switched off when serving. We don't record the probabilities behind live decisions, so we can't check them against outcomes. One exception does it properly: the stall detector's thresholds (`crates/coder-one/src/stall.rs:757`).

**We aren't really doing calibration. We have the tools, the data sources and the vocabulary, but no loop.**

## What TypeSafe means by calibration

### The model property

- "System One models are trained for calibrated decisions: their probabilities are optimized against outcomes to reflect uncertainty. Calibration is measured across groups of predictions; it does not guarantee that an individual answer is correct." ([concepts/system-one](https://docs.typesafe.ai/concepts/system-one))
- "Higher probability should correspond to a greater chance that the answer is correct… Outcomes assigned a probability of `0.2` should occur about 20% of the time. Outcomes assigned a probability of `0.8` should occur about 80% of the time… These rates describe groups of predictions, not a guarantee about any single answer." ([introduction/machine-learning-primer](https://docs.typesafe.ai/introduction/machine-learning-primer))
- RLCD, "Reinforcement learning for calibrated decisions", is set against RLHF, which "can also reward sycophancy and confident-sounding hallucinations". The primer says production automation "needs a different training objective—one centered on constrained decisions and calibrated uncertainty". ([primer](https://docs.typesafe.ai/introduction/machine-learning-primer); and [how-to-build-with-system-one](https://docs.typesafe.ai/concepts/how-to-build-with-system-one): "communicates uncertainty through calibrated probabilities instead of tending toward overconfidence")
- Diogo Almeida in the talks:
  - RLHF "optimizing for human preference", RLVR "for log error rates of pure correctness", and TypeSafe "a third thing that is optimized for calibrated decision-making" ([AI Engineer, 16:22–16:34](2026-07-31-ai-engineer-diogo-almeida-transcript.md));
  - models are "so encouraged to make plausible-looking answers, which is not what you want if you want to make calibrated decisions" ([AI Council, 19:50–19:56](2026-06-19-ai-council-diogo-almeida-transcript.md));
  - for automation you want the model "to just do the task correctly in a calibrated way" ([AI Engineer, 07:58–08:04](2026-07-31-ai-engineer-diogo-almeida-transcript.md)).

### Limits they state

- "`jev-1.13`'s score levels are weak in numerical calibration": you may threshold a Score's expectation, but not interpolate magnitudes ([model-jaggedness/jev-1.13](https://docs.typesafe.ai/model-jaggedness/jev-1.13)).
- "vague questions give mushy, uncalibrated scores". Questions should be narrow, grounded, per-field, with "bad = TRUE" framing ([cookbooks/sde_cascade](https://docs.typesafe.ai/cookbooks/sde_cascade)).
- Model aliases move: "If you have tuned confidence thresholds against a specific version, pin that version's ID" ([models](https://docs.typesafe.ai/models)).

### Thresholds are the application's job, and need your outcomes

- "The threshold is an illustrative application policy, not a calibrated guarantee… Choose production thresholds using labeled examples and the cost of incorrect actions and human review." ([cookbooks/consistency_choice_cookbook](https://docs.typesafe.ai/cookbooks/consistency_choice_cookbook))
- "The band is illustrative; it is neither a calibrated guarantee nor an optimized threshold. Set production boundaries from labeled examples and from the cost of incorrect decisions and of review." ([cookbooks/consistency_noul_cookbook](https://docs.typesafe.ai/cookbooks/consistency_noul_cookbook))
- "The correct threshold values depend on your domain and the performance of the model for your use case. Start with conservative thresholds, test with your own data, and adjust as you observe results." ([confidence](https://docs.typesafe.ai/confidence))

### Patterns that depend on it

- **Three bands:** act automatically, proceed with caution, or don't act. "Thresholds scale with risk": a 0.5 floor routes to a human, and a destructive action needs more than 0.9 ([confidence](https://docs.typesafe.ai/confidence)).
- **Measuring confidence:** `confidence` is derived from `probabilities`, using |2p−1| for a Noul and a normalized p_max for a Choice. The page suggests p_max and the top-to-second ratio as alternatives worth trying ([confidence](https://docs.typesafe.ai/confidence)).
- **A calibrated verifier makes a cascade work:** it is "high on real errors and low on correct ones, so a single threshold cleanly splits accept vs escalate" ([sde_cascade](https://docs.typesafe.ai/cookbooks/sde_cascade)).
- **Recalibration downstream:** when a downstream model's output is probabilistic, recalibrate it ([cookbooks/autoresearch_feature_discovery](https://docs.typesafe.ai/cookbooks/autoresearch_feature_discovery)).

## What we do today

### We have the machinery

- `crates/gym/src/calibrate.rs`: binned reliability maps (`Map::fit`, `fit_auto`, `fit_banded`) and `score()`, which reports accuracy, ECE, Brier, NLL and confident errors. Records name the door, the suite partition and the gate.
- `crates/tenancy/src/admission.rs:402`: Gym admission guards on ECE, Brier and NLL. `crates/tenancy/src/training.rs:4` defines a `calibration` partition.
- `docs/lev/calibration.md`: Lev, our own System One door, refuses to report a probability without a fitted map. Its status is "proposed. There are no Lev numbers."

### The router measures two of its questions offline, and keeps the map off

`crates/coder/src/router/calibration.rs:1-21` fits one map each for `route` and `answer` on the labelled eval's calibration partition, and scores them on held-out rows. Result: `crates/coder/fixtures/chat-router/calibration-v2.json`, set `chat-router-v5`, 408 rows fitted, 2026-10-02.

| Question | Held-out n | Accuracy | Raw ECE / Brier | Mapped ECE / Brier | Gate |
|---|---|---|---|---|---|
| `route` | 254 | 0.886 | 0.050 / 0.071 | 0.029 / 0.072 | failed |
| `answer` | 182 | 0.599 | **0.131** / 0.161 | 0.062 / 0.137 | passed |

- `answer` is overconfident: 13 points of ECE on a question that is right 60% of the time.
- The `answer` map passed the gate. Serving it is still `off` by default (`calibration.rs:34`, `CODER_WORKER_ROUTER_CALIBRATION`), "because the policy's thresholds were tuned on raw probabilities".
- Every router measurement since 2026-09-29 repeats "serving keeps calibration off", for example `docs/coder/measurements/2026-09-30-product-kb-connect-needs.md:56`.

### About 40 thresholds were set by hand

- **The router:** `crates/coder/src/router/policy.rs:109-198` holds about 30 constants. Examples:
  - `ROUTE_CONFIDENCE` 0.80 and `ANSWER_CONFIDENCE` 0.80;
  - `DISPATCH_LANE` 0.75, `RISK_REFUSE` 0.85 and `CLARIFY_WINS` 0.40;
  - `ENGINE_CONFIDENCE`, `FANOUT_CONFIDENCE` and `READ_ONLY_CONFIDENCE`, each 0.70.

  Each has a one-line rationale ("a wrong one costs an ignored card"). None was derived from outcomes or a cost model. Most read questions with no map at all: `lane`, `risk`, `engine`, `fanout`, `read_only`, `capability`, `cli`, `cli_group`, `standing.rule`, `deck`, `tool`.
- **Delegation:**
  - `HARD_AT` = 0.8 (`crates/coder-delegate/src/recipe.rs:71`) was raised from 0.5 in the "task-class recalibration" (#10245, `docs/cost/2026-10-02-shadow-baseline-measurement.md:118-135`). That fit used 105 runs over **7 tasks, all labelled `change`**, with no hard task, so the threshold was moved without a single positive example.
  - `CHECK_KEEP` 0.7 (`recipe.rs:95`) has no recorded measurement.
- **Two values marked "unmeasured development value" in their own doc comments:** `SELECT` 0.5 (`crates/coder-delegate/src/system.rs:47`) and evidence `YES` 0.5 (`crates/coder-delegate/src/component/evidence.rs:19`).
- **Gate flags:**
  - `DEPENDS_FLAG` 0.5 and `PLAIN_FLAG` 0.2 (`crates/coder-delegate/src/issue.rs:905, 1378`).
  - `PLAIN_FLAG` was set from three readings: 0.04, 0.06 and 0.38.
- **Background rules:**
  - `Condition::Judgment` acts when `p ≥ threshold%`, where the threshold comes from the drafted rule (`crates/background/src/engine.rs:310-330`, `rule.rs:206`).
  - Nobody ever checks whether a rule that fired was right.

### The one place we do it properly

The stall detector (`crates/coder-one/src/stall.rs:757-783`) froze its thresholds "from the calibration tasks before the evaluation labels were read". It used 327 labelled checkpoints and the "highest recall with precision of at least 0.80". That is exactly TypeSafe's recipe: labelled examples, the cost of errors, and a held-out evaluation.

### Live decisions aren't recorded in a form we could calibrate from

- **Route records leave out the readings.** A route record (`~/.openagents/routes/*.jsonl`, schema `openagents.route.admission-snapshot.v1`) stores the route family, a hash of the result, the policy id and the effects. It does not store the probabilities that drove the decision, or the eventual outcome. Routed runs are marked `unchecked` (`openagents efficiency`, 5e22f2af96).
- **Run outcomes are recorded:** independent checks, cost and time in the standing study (`bench/efficiency/`), shadow baselines (`coder.shadow`), and issue-flow results.
- **The two are never joined.** No record pairs Jev's `hard`, `engine`, `lane` or `read_only` reading with what then happened.

## The gap, plainly

1. **No live outcome join.** We can't draw a reliability curve for any production decision, because the reading and the outcome are never stored together.
2. **Thresholds without labels.** About 40 cut-offs were chosen by hand. Three call themselves unmeasured, and one (`HARD_AT`) was "recalibrated" on data with no positive class.
3. **Measured but not served.** The one map that passed its gate (`answer`, ECE 0.131 → 0.062) stays off because the thresholds were tuned to raw numbers. So the uncalibrated thresholds are what keeps the calibrated map off.
4. **No cost model.** TypeSafe's rule is that thresholds follow the cost of a wrong action against the cost of review or escalation. We don't write down either cost per decision.
5. **No drift loop.** Jev aliases move ([models](https://docs.typesafe.ai/models)). We don't pin the version per measured threshold, and we don't re-measure on a schedule.
6. **The efficiency page claims cost and time, not decision quality.** Nothing on openagents.com/efficiency says how often the router's or recipe's decisions were right.

## Plan

### What to calibrate first, in order of decisions per day × cost of being wrong

1. **Router `route` and `answer`**, which every chat message passes through. We already have the maps and 400+ labelled rows.
   - Re-derive `ROUTE_CONFIDENCE` and `ANSWER_CONFIDENCE` on calibrated probabilities. The cost of a wrong prepared answer is a wrong reply; the cost of falling through is one model call.
   - Then turn `CODER_WORKER_ROUTER_CALIBRATION` on.
2. **Dispatch questions:** `lane`, `engine`, `fanout`, `read_only`. A wrong one wastes a delegation, minutes and money, or edits when it should have only read. Outcome labels come from the run itself, joined to its route reading:
   - did the delegated run pass its independent check;
   - did a read-only run write anything;
   - did a fan-out produce independent results?
3. **Recipe `hard` and `asks_only`**, plus `CHECK_KEEP`. Outcome: steps or time to a checked result, and whether a kept check actually separated before and after. This needs labelled hard tasks, which the standing study task set lacks; add them.
4. **Background `Judgment` rules.** Outcome: the user's reaction to the notice (kept, undone, or rule paused soon after), plus periodic spot labels.
5. **Gate flags** (`DEPENDS_FLAG`, `PLAIN_FLAG`) against fix-round outcomes.

### Metric

- **What to report:** for each question and each family (for example "chat-router route" or "delegate engine"):
  - a reliability curve, as the existing `gym::calibrate` binned table, which shows the count per bin;
  - ECE, Brier, NLL, accuracy and n;
  - confident errors, meaning p ≥ 0.9 and wrong.
- **Rules for thin data:** no number is reported for a bin under 20, and every report names the Jev model ID it was measured on.

### Data we already have

- The router eval set: labelled, with calibration and held-out partitions (`crates/coder/tests/router_eval.rs`).
- The standing study's 84 runs per study, and the shadow-baseline 126 runs, both with independent checks.
- Issue-flow results: landed, failed or fix rounds, with the gate outcome.
- Stall detector checkpoints: 327, labelled.

What's missing is the join: the readings at decision time.

### The loop

1. **Record:** extend the route admission snapshot (and the delegation record) with the raw probabilities of each question read, the calibrated value if a map applied, the threshold, the decision, and the Jev model ID.
2. **Label:**
   - attach the outcome when it's known: run check result, user correction, a rerouted follow-up, or a rule undone;
   - sample 1% of decisions with no automatic outcome for a human or Claude-judge label, recorded as such.
3. **Fit:** each night, per question, `gym::calibrate::Map::fit_auto` on the last 30 days' calibration partition, scored on a held-out split, judged by the existing gate.
4. **Re-derive thresholds:** pick each threshold from calibrated probabilities and a written cost pair, for example C(false act) and C(escalate). Act when p·C(false act) < (1−p)·C(escalate) does not hold. The three-band layout follows TypeSafe's `/confidence`. The result is a dated record, not an edit to `policy.rs`.
5. **Serve:** the worker loads the newest admitted map and threshold record. It refuses one fitted on another question set or model ID (`Calibration::check` already does this for the router).
6. **Drift:** pin the Jev model ID per record. Re-run on alias change, and weekly. Raise a background notice when ECE rises by more than 0.05 or a bin's error rate moves outside its interval.

### Abstention from calibrated scores

Every gated decision gets an explicit middle band: clarify, offer instead of act, or ask first, sized from the cost pair. It shouldn't come from a hand-picked 0.6 to 0.8. Destructive or spending actions (writes, payouts, deletes in background rules) get the higher bar `/confidence` recommends.

### On openagents.com/efficiency

- **A new "Decisions" section:** for the router and the recipe, the reliability curve, ECE and n per question, the threshold in force, and the share of decisions that abstained.
- **The headline claim** becomes "cost per checked result at a measured decision accuracy", not cost alone.

## Steps (issue-sized)

1. **Record readings:** add the raw and calibrated probabilities, threshold, decision and Jev model ID of every question read to route admission snapshots and delegation records.
2. **Join outcomes:** attach run check results, user corrections and rule undo events to the route record by request id. Add an `openagents efficiency decisions` report with a reliability table per question.
3. **Router thresholds:** re-derive `ROUTE_CONFIDENCE` and `ANSWER_CONFIDENCE` from the `calibration-v2` maps with a written cost pair, then turn `CODER_WORKER_ROUTER_CALIBRATION` on and re-measure.
4. **Dispatch maps:** fit maps for `lane`, `engine`, `fanout` and `read_only` from the joined data, once n ≥ 200 per question.
5. **Recipe labels:** add labelled hard tasks to the standing study, then re-fit `HARD_AT`, `asks_only` and `CHECK_KEEP` with a positive class present.
6. **Unmeasured values:** replace the three "unmeasured development value" thresholds (`SELECT`, evidence `YES`, `PLAIN_FLAG`) with measured ones, or remove the gate.
7. **Background rules:** add outcome capture to `Judgment` rules, using kept, undone and spot-label outcomes.
8. **Recalibration:** a nightly job, plus re-runs on Jev model change, with a drift notice in the watchers view.
9. **Public page:** add the "Decisions" section to openagents.com/efficiency.
10. **Lev:** fit the first real calibration record, so `docs/lev/calibration.md` stops saying "There are no Lev numbers."
