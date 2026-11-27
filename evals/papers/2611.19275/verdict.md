# Paper-Lab Verdict: ToolPulse Deterministic Runtime Cueing (`arXiv:2611.19275`)

- **Verdict:** **`NEEDS-HARDER-PROBES`** — Baseline already passed >=85% with 0 rescues; harden probes before concluding.
- **Claim:** Zero-LLM-cost Blind-Edit and Failure-Inertia verbal reminders preserve pass rate with negligible token/latency tax
- **Axes:** `toolpulse`
- **Base Env:** `{}`
- **Exp Env:** `{"OA_EXP_TOOLPULSE": "1"}`
- **Git SHA:** `59f705073c` | **Date (UTC):** `2026-11-27T05:56:43Z`

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
