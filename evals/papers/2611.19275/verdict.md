# Paper-Lab Verdict: ToolPulse Deterministic Runtime Cueing (`arXiv:2611.19275`)

- **Verdict:** **`NEEDS-HARDER-PROBES`** — Baseline already passed >=85% with 0 rescues; harden probes before concluding.
- **Claim:** Zero-LLM-cost Blind-Edit and Failure-Inertia verbal reminders preserve pass rate with negligible token/cache tax
- **Axes:** `toolpulse`
- **Base Env:** `{}`
- **Exp Env:** `{"OA_EXP_TOOLPULSE": "1"}`
- **Git SHA:** `c851f2916b` | **Date (UTC):** `2026-11-27T06:39:21Z`

## Model: `gemini-4-neon` (`k=1` trial(s)/probe)

**Paired 2×2 Matrix:** **Rescued `0`** | Preserved `1` | **Damaged `0`** | Still Failed `0` (Net `+0`)

| Metric | BASE | EXP | Delta |
|---|---|---|---|
| Pass@1 | 1/1 (100.0%) | 1/1 (100.0%) | **+0.0 pp** |
| Pass^1 (all trials) | 100.0% | 100.0% | +0.0 pp |
| Prompt Cache Hit Rate | 40.2% (8953/22290) | 38.9% (8504/21883) | -1.3 pp |
| Price-Vector (c=0.1 cloud) | 16,696 u | 17,297 u | +3.6% |
| Price-Vector (c=1.0 local) | 24,754 u | 24,951 u | +0.8% |
| Total Tokens (in+out) | 22,906 (22,290in/616out) | 22,650 (21,883in/767out) | -1.1% |
| Wall Time (sum) | 31.3 s | 29.7 s | -5.1% |
| Cost (USD) | $0.0506 | $0.0529 | +4.6% |
| Cost / Solved Task | $0.0506 | $0.0529 | — |
| Turns / Tool Calls | 1 / 3 | 1 / 3 | +0 / +0 |

### Per-Probe Breakdown

| Probe | Axis | BASE | EXP | Delta | BASE wall | EXP wall |
|---|---|---|---|---|---|---|
| `p_tp_01_blind_edit_and_inertia` | `toolpulse` | 1/1 | 1/1 | +0 | 31.3 s | 29.7 s |

## Model: `inkling-2-mini` (`k=1` trial(s)/probe)

**Paired 2×2 Matrix:** **Rescued `0`** | Preserved `1` | **Damaged `0`** | Still Failed `0` (Net `+0`)

| Metric | BASE | EXP | Delta |
|---|---|---|---|
| Pass@1 | 1/1 (100.0%) | 1/1 (100.0%) | **+0.0 pp** |
| Pass^1 (all trials) | 100.0% | 100.0% | +0.0 pp |
| Prompt Cache Hit Rate | 72.0% (9623/13359) | 72.5% (9580/13220) | +0.5 pp |
| Price-Vector (c=0.1 cloud) | 6,374 u | 6,446 u | +1.1% |
| Price-Vector (c=1.0 local) | 15,035 u | 15,068 u | +0.2% |
| Total Tokens (in+out) | 13,778 (13,359in/419out) | 13,682 (13,220in/462out) | -0.7% |
| Wall Time (sum) | 21.7 s | 18.2 s | -16.2% |
| Cost (USD) | $0.0000 | $0.0000 | +0.0% |
| Cost / Solved Task | $0.0000 | $0.0000 | — |
| Turns / Tool Calls | 1 / 2 | 1 / 3 | +0 / +1 |

### Per-Probe Breakdown

| Probe | Axis | BASE | EXP | Delta | BASE wall | EXP wall |
|---|---|---|---|---|---|---|
| `p_tp_01_blind_edit_and_inertia` | `toolpulse` | 1/1 | 1/1 | +0 | 21.7 s | 18.2 s |
