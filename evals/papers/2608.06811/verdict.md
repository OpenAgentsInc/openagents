# Paper-Lab Verdict: PMCoder Anti-Spiral Phase Grounding (`arXiv:2608.06811`)

- **Verdict:** **`NEEDS-HARDER-PROBES`** — Baseline already passed >=85% with 0 rescues; harden probes before concluding.
- **Claim:** Cache-safe in-distribution BACKTRACK and VERIFICATION reminders on repeated failure/churn rescue coupled multi-file bugs without breaking prompt cache.
- **Axes:** `anti_spiral`
- **Base Env:** `{}`
- **Exp Env:** `{"OA_EXP_PMCODER": "1"}`
- **Git SHA:** `fb614edecc` | **Date (UTC):** `2026-11-27T05:03:27Z`

## Model: `gemini-4-neon` (`k=1` trial(s)/probe)

**Paired 2×2 Matrix:** **Rescued `0`** | Preserved `2` | **Damaged `0`** | Still Failed `0` (Net `+0`)

| Metric | BASE | EXP | Delta |
|---|---|---|---|
| Pass@1 | 2/2 (100.0%) | 2/2 (100.0%) | **+0.0 pp** |
| Pass^1 (all trials) | 100.0% | 100.0% | +0.0 pp |
| Prompt Cache Hit Rate | 50.4% (23455/46531) | 47.2% (22016/46631) | -3.2 pp |
| Total Tokens (in+out) | 49,558 (46,531in/3,027out) | 49,785 (46,631in/3,154out) | +0.5% |
| Wall Time (sum) | 45.5 s | 54.8 s | +20.5% |
| Cost (USD) | $0.1182 | $0.1245 | +5.3% |
| Turns / Tool Calls | 2 / 10 | 2 / 10 | +0 / +0 |

### Per-Probe Breakdown

| Probe | Axis | BASE | EXP | Delta | BASE wall | EXP wall |
|---|---|---|---|---|---|---|
| `p_sp_01_coupled_three_file_invariant` | `anti_spiral` | 1/1 | 1/1 | +0 | 25.5 s | 26.0 s |
| `p_sp_02_trap_ttl_cache` | `anti_spiral` | 1/1 | 1/1 | +0 | 20.0 s | 28.8 s |
