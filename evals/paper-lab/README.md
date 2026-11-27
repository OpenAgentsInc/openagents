# Paper-Lab: Rapid Paper-to-Verdict Eval System

Test any research paper's core mechanism on the real `oa-worker` coding agent in minutes using paired `BASE` vs `EXP` A/B runs on baseline-challenging failure-axis probes.

## Workflow

1. **Pull paper** with the `arxiv` skill (`python3 .agents/skills/arxiv/scripts/arxiv_paper.py fetch <arxiv_id>`).
2. **Gate minimal mechanism** behind an off-by-default `OA_EXP_<NAME>=1` flag in `coder/tui/oa-worker.coax` (or `openagents/core/`).
3. **Calibrate probes (optional)** to confirm baseline actually struggles (target 20–70% BASE pass rate):
   ```bash
   python3 evals/paper-lab/ab_run.py --paper 2610.31710 --axis compaction --models gemini-4-neon --calibrate-base
   ```
4. **Run paired A/B evaluation** (`BASE` vs `EXP` on the exact same compiled `oa-worker` binary):
   ```bash
   python3 evals/paper-lab/ab_run.py \
     --paper 2610.31710 \
     --name "Compact-Bench Entity-Preserving Compaction" \
     --claim "Preserving high-value exact identifiers, completed edits, and tool-output tails across compaction rescues recurring-detail and state-time-disorder probes." \
     --axis compaction \
     --exp-env OA_EXP_COMPACT_PRESERVE=1 \
     --models gemini-4-neon \
     --trials 2 \
     -j 3
   ```
   Pass `--models inkling-2-mini,gemini-4-neon` for dual-model (local 32K + frontier cloud) verdicts.

5. **Inspect permanent verdict** written to `evals/papers/<arxiv_id>/`:
   - `hypothesis.json` — exact claim, axes, flags, models
   - `verdict.json` — paired `Rescued / Preserved / Damaged / Still Failed`, `Pass@1`, `Pass^k`, prompt-cache hit-rate delta, token/cost/wall deltas
   - `verdict.md` — human-readable scorecard (`SHIP` / `LOCAL-ONLY` / `ITERATE` / `KILL` / `NEEDS-HARDER-PROBES`)

## Seeded Failure-Axis Probe Banks (`evals/paper-lab/probes/`)

| Axis | Failure Mode Tested | Probes |
|---|---|---|
| `compaction` | Recurring-detail loss past byte 220, state-time disorder (redoing completed edits), tool-output tail loss across `/compact all` | `p_cmp_01_recurring_details`, `p_cmp_02_state_time_disorder`, `p_cmp_03_deep_error_tail` |
| `stale_state` | Trusting invalidated pre-compaction test passes or stale file reads after workspace mutations | `p_st_01_invalidated_test_pass`, `p_st_02_stale_file_read`, `p_st_03_edit_then_compact_untested` |
| `anti_spiral` | Coupled multi-file invariants where naive symptom patching thrashes | `p_sp_01_coupled_three_file_invariant`, `p_sp_02_trap_ttl_cache` |
