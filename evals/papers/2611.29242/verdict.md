# Paper-Lab Verdict: DepSync Post-Edit Consumer Impact Hint (`arXiv:2611.29242`)

- **Verdict:** **`NEEDS-HARDER-PROBES`** — Baseline already passed >=85% with 0 rescues; harden probes before concluding.
- **Claim:** Appending untouched sibling consumer references after Edit cuts producer-to-consumer propagation omissions
- **Axes:** `dep_sync`
- **Base Env:** `{}`
- **Exp Env:** `{"OA_EXP_DEP_SYNC": "1"}`
- **Git SHA:** `59f705073c` | **Date (UTC):** `2026-11-27T05:53:53Z`

## Model: `gemini-4-neon` (`k=1` trial(s)/probe)

**Paired 2×2 Matrix:** **Rescued `0`** | Preserved `2` | **Damaged `0`** | Still Failed `0` (Net `+0`)

| Metric | BASE | EXP | Delta |
|---|---|---|---|
| Pass@1 | 2/2 (100.0%) | 2/2 (100.0%) | **+0.0 pp** |
| Pass^1 (all trials) | 100.0% | 100.0% | +0.0 pp |
| Prompt Cache Hit Rate | 41.1% (20670/50267) | 34.7% (17254/49759) | -6.4 pp |
| Price-Vector (c=0.1 cloud) | 45,624 u | 47,206 u | +3.5% |
| Price-Vector (c=1.0 local) | 64,227 u | 62,735 u | -2.3% |
| Total Tokens (in+out) | 53,757 (50,267in/3,490out) | 53,003 (49,759in/3,244out) | -1.4% |
| Wall Time (sum) | 71.0 s | 78.6 s | +10.8% |
| Cost (USD) | $0.1442 | $0.1488 | +3.1% |
| Cost / Solved Task | $0.0721 | $0.0744 | — |
| Turns / Tool Calls | 2 / 12 | 2 / 12 | +0 / +0 |

### Per-Probe Breakdown

| Probe | Axis | BASE | EXP | Delta | BASE wall | EXP wall |
|---|---|---|---|---|---|---|
| `p_ds_01_ast_transformer_omission` | `dep_sync` | 1/1 | 1/1 | +0 | 29.0 s | 24.4 s |
| `p_ds_02_serializer_omission` | `dep_sync` | 1/1 | 1/1 | +0 | 42.0 s | 54.2 s |
