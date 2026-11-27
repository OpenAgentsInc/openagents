# Paper-Lab Verdict: The Complexity Trap: Epoch-Aligned Observation Masking (`arXiv:2508.21433`)

- **Verdict:** **`KILL-CLOUD`** — Prompt-cache trap on cloud: total tokens fell -41.0% and wall -60.4%, but cache hit rate dropped -26.0 pp (83.2% -> 57.2%), raising USD cost +22.3% ($0.4740 -> $0.5798) with 1 damaged case (budget cap hit earlier). Keep off on cloud; candidate for local 32K Inkling only.
- **Claim:** Batch-masking older verbose tool observations outside a rolling window while preserving user intent, assistant reasoning, tool args, and extracted anchors cuts input tokens and cost without losing accuracy.
- **Axes:** `long_obs`
- **Base Env:** `{}`
- **Exp Env:** `{"OA_EXP_OBS_MASK": "1"}`
- **Git SHA:** `fb614edecc` | **Date (UTC):** `2026-11-27T05:13:48Z`

## Model: `gemini-4-neon` (`k=1` trial(s)/probe)

**Paired 2×2 Matrix:** **Rescued `0`** | Preserved `1` | **Damaged `1`** | Still Failed `0` (Net `-1`)

| Metric | BASE | EXP | Delta |
|---|---|---|---|
| Pass@1 | 2/2 (100.0%) | 1/2 (50.0%) | **-50.0 pp** |
| Pass^1 (all trials) | 100.0% | 50.0% | -50.0 pp |
| Prompt Cache Hit Rate | 83.2% (562443/675945) | 57.2% (227751/398151) | -26.0 pp |
| Total Tokens (in+out) | 679,218 (675,945in/3,273out) | 400,444 (398,151in/2,293out) | -41.0% |
| Wall Time (sum) | 194.9 s | 77.2 s | -60.4% |
| Cost (USD) | $0.4740 | $0.5798 | +22.3% |
| Cost / Solved Task | $0.2370 | $0.5798 | — |
| Turns / Tool Calls | 11 / 5 | 10 / 2 | -1 / -3 |

### Per-Probe Breakdown

| Probe | Axis | BASE | EXP | Delta | BASE wall | EXP wall |
|---|---|---|---|---|---|---|
| `p_lo_01_multi_log_audit` | `long_obs` | 1/1 | 0/1 | -1 | 131.7 s | 41.6 s |
| `p_lo_02_noisy_trace_audit` | `long_obs` | 1/1 | 1/1 | +0 | 63.2 s | 35.6 s |
