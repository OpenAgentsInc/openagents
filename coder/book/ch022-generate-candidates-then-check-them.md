# 22. Generate candidates, then check them

> **Status: partly implemented.** OpenAgents implements two production
> `#authority` two-arm baseline-vs-candidate evaluation and promotion gates—for
> local conversational capability authoring (`coax/lib/agent/desktop/capability-gate.coax`,
> `capability-author.coax`, `capability-eval.coax`, adopted directly by
> `openagents/runtime/oa-worker.coax`) and for gBench Inkling serving/LoRA hillclimbing
> (`coax/lib/gbench/policy.coax`, `coax/lib/gbench/hillclimb.coax`). Multi-candidate
> best-of-N coding patch repair loops under a shared wall/token budget remain
> open.

Generating a candidate—whether a newly authored local Skill (`SKILL.md`), a
serving/LoRA configuration, or a candidate code patch—is only half the job. A
candidate earns promotion into the live system only after a two-arm evaluation
against the baseline passes a fail-closed `#authority` promotion gate with
held-out coverage and a verified ATIF-v1.8 trace digest.

OpenAgents ships two `#authority` promotion gates in production:

1. **Local Capability Authoring Promotion Gate
   (`coax/lib/agent/desktop/capability-gate.coax`, wired into
   `coax/lib/agent/desktop/capability-author.coax`,
   `coax/lib/agent/desktop/capability-eval.coax`, and
   `openagents/runtime/oa-worker.coax`, `#13896`, `#14059`):**
   - `cap_valid_slug` validates capability slugs (`[a-z0-9-]`, 2..48 chars, no
     leading/trailing/double dashes, no path traversal).
   - `cap_admit_promotion` compares baseline vs. candidate across positive and
     held-out/negative cases and refuses promotion if there are zero cases, zero
     held-out cases, an unverified ATIF-v1.8 digest, zero candidate passes,
     overall regression, held-out regression, or cost/latency budget overruns.
   - Passing capabilities promote directly into `<skills_dir>/<slug>/` with
     `eval.atif.jsonl` and `evidence.json`; rejected candidates preserve their
     rejection evidence without touching the live Skills repertoire.
2. **gBench Two-Arm Hillclimb Promotion Gate (`coax/lib/gbench/policy.coax` &
   `coax/lib/gbench/hillclimb.coax`):**
   - `gb_admit_hillclimb` compares a challenger Inkling variation or serving
     policy (reasoning effort, LoRA overlay, quantization, speculative lookup)
     against the current champion.
   - Promotes on strict score improvement OR equal score with a verified
     `>= 10%` TTFT/TPOT latency speedup; quarantines ties with no strict gain
     (`QUARANTINE_HELD`); and rejects scaffold mismatches, unverified ATIF
     digests, harness errors, golden-set regressions, score regressions, CV
     instability, or cost-cap overruns.

## Profile

This chapter's profile is `openagents-candidate-promotion-v1 (capability-gate.coax + gbench/policy.coax), bounded CPU`:

- Validates safe slugs and rejects path escapes.
- Promotes passing local capabilities while rejecting held-out regressions,
  unverified ATIF traces, and latency-budget overruns.
- Promotes gBench challengers on strict score gain or `>= 10%` TPOT speedup,
  quarantines flat ties (`QUARANTINE_HELD`), and rejects scaffold mismatches
  and golden-set regressions.

## Worked example

The runnable example [candidate-eval.coax](../examples/candidate-eval.coax)
drives both `coax/lib/agent/desktop/capability-gate.coax` and
`coax/lib/gbench/policy.coax` directly.

<!-- source: coder/examples/candidate-eval.coax example -->
```jai
main :: () {
  // Part A: Local capability promotion gate (`capability-gate.coax`, used by oa-worker)
  print("slug check: fast-lint=% ../escape=%\n",
    cap_valid_slug("fast-lint"), cap_valid_slug("../escape"));

  cand_ok := CapEvalSummary.{
    total_cases = 4, heldout_cases = 2,
    baseline_passed = 2, candidate_passed = 3,
    baseline_heldout_passed = 1, candidate_heldout_passed = 2,
    candidate_cost_micros = 400, max_cost_micros = 500,
    candidate_latency_ms = 900, max_latency_ms = 1200,
    atif_verified = true,
  };
  cand_heldout_reg := cand_ok;
  cand_heldout_reg.candidate_heldout_passed = 0;
  cand_no_atif := cand_ok;
  cand_no_atif.atif_verified = false;
  cand_slow := cand_ok;
  cand_slow.candidate_latency_ms = 1500;

  print("cap gate ok: %\n", cap_verdict_label(cap_admit_promotion(cand_ok)));
  print("cap gate heldout reg: %\n", cap_verdict_label(cap_admit_promotion(cand_heldout_reg)));
  print("cap gate no atif: %\n", cap_verdict_label(cap_admit_promotion(cand_no_atif)));
  print("cap gate latency: %\n", cap_verdict_label(cap_admit_promotion(cand_slow)));

  // Part B: gBench two-arm hillclimb gate (`coax/lib/gbench/policy.coax`)
  base := GbHillclimbEval.{
    scaffolds_match = true, atif_verified = true, golden_regressed = false,
    cand_harness_errors = 0, graded_total = 30,
    champ_score_bps = 7000, cand_score_bps = 7500,
    champ_ttft_us = 200000, cand_ttft_us = 200000,
    champ_tpot_us = 10000, cand_tpot_us = 10000,
    cand_cv_bps = 200, max_cv_bps = 500,
    cand_cost_micros = 0, max_cost_micros = 0,
  };
  v_gain := gb_admit_hillclimb(base);

  // Equal score with >= 10% decode latency (TPOT) speedup promotes; equal with no gain holds
  speedup := base;
  speedup.cand_score_bps = 7000;
  speedup.cand_tpot_us = 8800;
  v_speed := gb_admit_hillclimb(speedup);

  flat := base;
  flat.cand_score_bps = 7000;
  v_flat := gb_admit_hillclimb(flat);

  // Scaffold mismatch or golden regression is rejected even with higher score
  mismatch := base;
  mismatch.scaffolds_match = false;
  v_mis := gb_admit_hillclimb(mismatch);

  golden_reg := base;
  golden_reg.golden_regressed = true;
  v_gold := gb_admit_hillclimb(golden_reg);

  print("hillclimb score gain: % (%)\n", gb_gate_status(v_gain), gb_gate_reason(v_gain));
  print("hillclimb tpot speedup: % (%)\n", gb_gate_status(v_speed), gb_gate_reason(v_speed));
  print("hillclimb no gain: % (%)\n", gb_gate_status(v_flat), gb_gate_reason(v_flat));
  print("hillclimb scaffold mismatch: % (%)\n", gb_gate_status(v_mis), gb_gate_reason(v_mis));
  print("hillclimb golden regression: % (%)\n", gb_gate_status(v_gold), gb_gate_reason(v_gold));
}
```

Run it in debug or release:

```shell
coax/bin/coax run coder/examples/candidate-eval.coax
```

```text
slug check: fast-lint=true ../escape=false
cap gate ok: PROMOTED: candidate passed two-arm eval and held-out gate
cap gate heldout reg: REJECTED: candidate regressed on held-out cases
cap gate no atif: REJECTED: ATIF-v1.8 trace digest unverified
cap gate latency: REJECTED: candidate exceeded latency budget
hillclimb score gain: PROMOTED (promoted_gain_verified)
hillclimb tpot speedup: PROMOTED (promoted_gain_verified)
hillclimb no gain: QUARANTINE_HELD (quarantine_held_no_strict_gain)
hillclimb scaffold mismatch: REJECTED (rejected_scaffold_mismatch)
hillclimb golden regression: REJECTED (rejected_golden_regressed)
```

The checked fixture is [candidate-eval.out](../examples/candidate-eval.out).

## Live self-test

To run the real two-arm shell execution, ATIF-v1.8 logging, promotion, and
rejection evidence pipeline end-to-end:

```shell
coax/web/tool.sh coax/lib/agent/desktop/capability-eval.coax selftest
```

## What remains

- Multi-candidate best-of-N coding patch generation and bounded generate-run-repair
  loops inside `oa-worker` under a unified token and wall-clock budget.

## Done means

- [x] `capability-gate.coax`, `capability-author.coax`, `capability-eval.coax`,
  and `gbench/policy.coax` enforce `#authority` two-arm promotion with held-out
  coverage and verified ATIF-v1.8 digests.
- [x] `coder/examples/candidate-eval.coax` matches
  `coder/examples/candidate-eval.out` in debug and release.
- [ ] Bounded multi-candidate code-patch repair loops are wired into `oa-worker`
  and measured on held-out repository tasks.
