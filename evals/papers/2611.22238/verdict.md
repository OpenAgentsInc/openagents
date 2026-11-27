# Paper-Lab Verdict: Break-Even Observation Masking (c=0.1 cloud gate) (`arXiv:2611.22238`)

- **Verdict:** **`SHIP-EFFICIENCY`** — Preserved pass rate (1/1, 0 damaged) while cutting tokens -28.0% and cost +31.1%.
- **Claim:** Gating observation masking with R*c*B > (1-c)*S eliminates the cloud prompt-cache cost trap while preserving pass rate
- **Axes:** `long_obs`
- **Base Env:** `{}`
- **Exp Env:** `{"OA_EXP_OBS_MASK": "1"}`
- **Git SHA:** `59f705073c` | **Date (UTC):** `2026-11-27T06:02:51Z`

## Model: `gemini-4-neon` (`k=1` trial(s)/probe)

**Paired 2×2 Matrix:** **Rescued `0`** | Preserved `1` | **Damaged `0`** | Still Failed `0` (Net `+0`)

| Metric | BASE | EXP | Delta |
|---|---|---|---|
| Pass@1 | 1/1 (100.0%) | 1/1 (100.0%) | **+0.0 pp** |
| Pass^1 (all trials) | 100.0% | 100.0% | +0.0 pp |
| Prompt Cache Hit Rate | 83.5% (244275/292538) | 64.7% (136032/210159) | -18.8 pp |
| Price-Vector (c=0.1 cloud) | 77,654 u | 92,926 u | +19.7% |
| Price-Vector (c=1.0 local) | 297,502 u | 215,355 u | -27.6% |
| Total Tokens (in+out) | 293,779 (292,538in/1,241out) | 211,458 (210,159in/1,299out) | -28.0% |
| Wall Time (sum) | 102.3 s | 53.5 s | -47.7% |
| Cost (USD) | $0.2000 | $0.2623 | +31.1% |
| Cost / Solved Task | $0.2000 | $0.2623 | — |
| Turns / Tool Calls | 5 / 3 | 5 / 3 | +0 / +0 |

### Per-Probe Breakdown

| Probe | Axis | BASE | EXP | Delta | BASE wall | EXP wall |
|---|---|---|---|---|---|---|
| `p_lo_02_noisy_trace_audit` | `long_obs` | 1/1 | 1/1 | +0 | 102.3 s | 53.5 s |
