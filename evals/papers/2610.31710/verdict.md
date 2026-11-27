# Paper-Lab Verdict: Compact-Bench Entity & State-Preserving Compaction (`arXiv:2610.31710`)

- **Verdict:** **`SHIP`** — Clean win (+2 rescued, 0 damaged, wall -12.2%, cache +0.1 pp).
- **Claim:** Preserving high-value exact identifiers, completed edits, and tool-output tails across compaction rescues recurring-detail and state-time-disorder failures without hurting latency or prompt cache.
- **Axes:** `compaction`
- **Base Env:** `{}`
- **Exp Env:** `{"OA_EXP_COMPACT_PRESERVE": "1"}`
- **Git SHA:** `fb614edecc` | **Date (UTC):** `2026-11-27T04:54:45Z`

## Model: `gemini-4-neon` (`k=1` trial(s)/probe)

**Paired 2×2 Matrix:** **Rescued `2`** | Preserved `1` | **Damaged `0`** | Still Failed `0` (Net `+2`)

| Metric | BASE | EXP | Delta |
|---|---|---|---|
| Pass@1 | 1/3 (33.3%) | 3/3 (100.0%) | **+66.7 pp** |
| Pass^1 (all trials) | 33.3% | 100.0% | +66.7 pp |
| Prompt Cache Hit Rate | 41.0% (54495/132811) | 41.1% (50842/123570) | +0.1 pp |
| Total Tokens (in+out) | 136,529 (132,811in/3,718out) | 127,107 (123,570in/3,537out) | -6.9% |
| Wall Time (sum) | 154.8 s | 135.9 s | -12.2% |
| Cost (USD) | $0.2989 | $0.2789 | -6.7% |
| Turns / Tool Calls | 12 / 16 | 12 / 16 | +0 / +0 |

### Per-Probe Breakdown

| Probe | Axis | BASE | EXP | Delta | BASE wall | EXP wall |
|---|---|---|---|---|---|---|
| `p_cmp_01_recurring_details` | `compaction` | 0/1 | 1/1 | +1 | 32.0 s | 43.0 s |
| `p_cmp_02_state_time_disorder` | `compaction` | 0/1 | 1/1 | +1 | 73.5 s | 53.1 s |
| `p_cmp_03_deep_error_tail` | `compaction` | 1/1 | 1/1 | +0 | 49.2 s | 39.8 s |
