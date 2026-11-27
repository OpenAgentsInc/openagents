# Paper-Lab Verdict: MemTrace Live State-Consistent Compaction (`arXiv:2610.04838`)

- **Verdict:** **`NEEDS-HARDER-PROBES`** — Baseline already passed >=85% with 0 rescues; harden probes before concluding.
- **Claim:** Re-hashing tracked workspace files and invalidating superseded command verdicts at compaction boundaries prevents stale-file and stale-test-pass hallucinations.
- **Axes:** `stale_state_hard`
- **Base Env:** `{}`
- **Exp Env:** `{"OA_EXP_MEMTRACE": "1"}`
- **Git SHA:** `fb614edecc` | **Date (UTC):** `2026-11-27T05:10:14Z`

## Model: `gemini-4-neon` (`k=1` trial(s)/probe)

**Paired 2×2 Matrix:** **Rescued `0`** | Preserved `2` | **Damaged `0`** | Still Failed `0` (Net `+0`)

| Metric | BASE | EXP | Delta |
|---|---|---|---|
| Pass@1 | 2/2 (100.0%) | 2/2 (100.0%) | **+0.0 pp** |
| Pass^1 (all trials) | 100.0% | 100.0% | +0.0 pp |
| Prompt Cache Hit Rate | 21.4% (6858/32025) | 22.0% (6962/31665) | +0.6 pp |
| Total Tokens (in+out) | 33,584 (32,025in/1,559out) | 32,601 (31,665in/936out) | -2.9% |
| Wall Time (sum) | 32.6 s | 86.0 s | +163.4% |
| Cost (USD) | $0.0999 | $0.0892 | -10.7% |
| Cost / Solved Task | $0.0500 | $0.0446 | — |
| Turns / Tool Calls | 6 / 2 | 6 / 2 | +0 / +0 |

### Per-Probe Breakdown

| Probe | Axis | BASE | EXP | Delta | BASE wall | EXP wall |
|---|---|---|---|---|---|---|
| `p_sth_01_budgeted_stale_verdict` | `stale_state_hard` | 1/1 | 1/1 | +0 | 17.8 s | 40.0 s |
| `p_sth_02_external_config_drift` | `stale_state_hard` | 1/1 | 1/1 | +0 | 14.8 s | 45.9 s |
