# Paper-Lab Verdict: PMCoder III-E Post-Edit Syntax Grounding (`arXiv:2608.06811-edit-ground`)

- **Verdict:** **`NEEDS-HARDER-PROBES`** — Baseline already passed >=85% with 0 rescues; harden probes before concluding.
- **Claim:** Zero-LLM-cost native JSON and Python AST syntax grounding after Edit/Write adds negligible latency, preserves prompt cache, and catches malformed edits immediately.
- **Axes:** `edit_integrity`
- **Base Env:** `{}`
- **Exp Env:** `{"OA_EXP_EDIT_GROUND": "1"}`
- **Git SHA:** `fb614edecc` | **Date (UTC):** `2026-11-27T05:15:14Z`

## Model: `gemini-4-neon` (`k=1` trial(s)/probe)

**Paired 2×2 Matrix:** **Rescued `0`** | Preserved `1` | **Damaged `0`** | Still Failed `0` (Net `+0`)

| Metric | BASE | EXP | Delta |
|---|---|---|---|
| Pass@1 | 1/1 (100.0%) | 1/1 (100.0%) | **+0.0 pp** |
| Pass^1 (all trials) | 100.0% | 100.0% | +0.0 pp |
| Prompt Cache Hit Rate | 29.3% (3623/12379) | 29.2% (3608/12357) | -0.1 pp |
| Total Tokens (in+out) | 13,007 (12,379in/628out) | 12,989 (12,357in/632out) | -0.1% |
| Wall Time (sum) | 14.5 s | 13.6 s | -5.9% |
| Cost (USD) | $0.0362 | $0.0363 | +0.1% |
| Cost / Solved Task | $0.0362 | $0.0363 | — |
| Turns / Tool Calls | 1 / 2 | 1 / 2 | +0 / +0 |

### Per-Probe Breakdown

| Probe | Axis | BASE | EXP | Delta | BASE wall | EXP wall |
|---|---|---|---|---|---|---|
| `p_ei_01_fast_json_py_edit` | `edit_integrity` | 1/1 | 1/1 | +0 | 14.5 s | 13.6 s |
