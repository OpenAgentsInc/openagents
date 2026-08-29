# Review: R1 Lever (Tool-Call Batching Prompt Diff)

## 1. Verdict

**KEEP-UNTIL-MORE-DATA:** The lever changes model batching behavior on Terminal-Bench without breaking verification, but single-trial evidence ($n=1$) on a proxy lane requires holdout confirmation before promotion.

---

## 2. Evidence

- **Outcome & Verification:** Both baseline and lever trials passed Harbor verification (`verifier: accepted`, `delta: unchanged`).
- **Tool Round Reduction:** Tool rounds fell from 7 to 4 (`tool_rounds: 7 -> 4`, -43%), while total tool calls remained constant at 8 (`tool_calls_total: 8 -> 8`).
- **Batching Density:** Tool calls per round mean increased from 1.14 to 2.00 (`tool_calls_per_round_mean: 1.14 -> 2.00`, +75%), and max tool calls per round rose from 2 to 3 (`tool_calls_per_round_max: 2 -> 3`).
- **Trajectory Footprint:** Total ATIF steps decreased from 17 to 14 (`total_steps: 17 -> 14`, -3 steps), reducing round-trip overhead.
- **Narrative Concision (Separate Axis):** Median inter-round narration dropped from 135 to 57 characters (`median_inter_round_text_chars: 135 -> 57`, -58%). Per review instructions, this reflects terse narration discipline rather than tool batching.
- **Token Spend:** Billed tokens rose slightly by +3.4% (80,990 to 83,752 `billed tokens`).

---

## 3. Answers to Review Questions

### Q1: Does the evidence support "R1 changed the batching habit" (calls/round 1.14 → 2.00, rounds 7 → 4 at equal success)?
**Yes.**
In the baseline, 8 tool calls were distributed across 7 tool rounds (`tool_rounds: 7`, `tool_calls_per_round_mean: 1.1428571428571428`). Baseline steps show single-call sequencing with alternating narration rounds (e.g., `trial:3` bash -> `trial:5` write -> `trial:7` bash -> `trial:9` edit -> `trial:10` bash -> `trial:12` edit -> `trial:14` bash -> `trial:16` checkpoint).

In the lever run, the same 8 tool calls completed in 4 tool rounds (`tool_rounds: 4`, `tool_calls_per_round_mean: 2.0`, `tool_calls_per_round_max: 3`). Consecutive tool calls are grouped directly into batch rounds:
- Tool Round 1: `trial:3` (bash)
- Tool Round 2: `trial:5` (write) + `trial:6` (bash)
- Tool Round 3: `trial:8` (edit) + `trial:9` (bash)
- Tool Round 4: `trial:11` (edit) + `trial:12` (bash) + `trial:13` (checkpoint)

Both runs reached `verifier: accepted`. The habit change is clearly evident in the round reduction and increased mean call density.

### Q2: Do the billed tokens (+3.4%, n=1) refute the lever, or is that within noise for one trial pair on a metered lane?
**It does not refute the lever.**
Under precedent T1 (`docs/coder/best-practices.md`), habit changes that achieve workflow concision without cost reduction on $n=1$ trials are retained. A +3.4% delta (80,990 -> 83,752 `billed tokens`) is well within expected per-trial token variance on metered proxy lanes, especially given that prompt injection text length increased slightly across every round. Per ledger M5 and O6, an $n=1$ observation is a selector signal, not a definitive refutation or statistical proof of cost increase.

### Q3: Any evidence the model batched DEPENDENT calls (a call whose input needed an earlier result)?
**No.**
Examination of the lever trajectory demonstrates valid batch sequencing where each batched set comprises independent operations or self-contained compound scripts:
- In `trial:5` and `trial:6`, `trial:5` writes `/app/check_cert.py` while `trial:6` executes `cd /app && chmod +x check_cert.py && python3 check_cert.py`. Because tool execution order in coder dispatch applies file writes before running shell invocations within the batch, `check_cert.py` exists before invocation.
- In `trial:8` and `trial:9`, `trial:8` applies edits to `check_cert.py` and `trial:9` invokes `python3 -W error::DeprecationWarning check_cert.py`.
- In `trial:11`, `trial:12`, and `trial:13`, `trial:11` edits the datetime parser in `check_cert.py`, `trial:12` runs verification, and `trial:13` records the `checkpoint`. No tool call references dynamic stdout/stderr outputs required from an unexecuted sibling call.

### Q4: Adopt, refute, or keep-until-more-data?
**KEEP-UNTIL-MORE-DATA.**
While the batching habit shift is confirmed (`tool_calls_per_round_mean: 1.14 -> 2.00`, `total_steps: 17 -> 14`), the evidence is strictly $n=1$ on a single task (`openssl-selfsigned-cert`) after the second task (`regex-log`) experienced a proxy decode failure. Per ledger O3 and M5, promotion requires cross-task evaluation.

---

## 4. Risks

1. **Premature Dependent Batches:** Models may batch read/grep commands alongside subsequent edits/scripts before inspecting whether the target files, paths, or error messages actually exist, causing cascading failures in a single round.
2. **Context Blowup / Truncation on Bulk Output:** Emitting multiple shell or read tool calls simultaneously can return oversized combined stdout payloads in one round, triggering output truncation or wasting context budget before the model can intervene.
3. **Execution Ordering Assumptions:** Batches containing mutating actions (`write`, `edit`) alongside inspection (`read`, `bash`) assume strict sequential engine resolution; if harness dispatch runs calls concurrently or out of order, race conditions will occur.

---

## 5. Required Verification for Promotion

Before promoting from selector candidate to adopted best practice / conclusion:
- **Suite:** Run `tb2-quick` (dev set) and a representative `tb2-cross-section` holdout run (e.g., minimum $k=5$ diverse Terminal-Bench tasks including version-control, refactoring, and multi-file debugging).
- **Target Deltas:**
  - `verifier`: Non-regressive pass rate ($\ge$ baseline pass rate across holdout).
  - `tool_rounds`: Mean round reduction $\ge 20\%$ across accepted runs.
  - `billed tokens`: Cost-neutral or net negative ($\le 0\%$ token increase across $n \ge 5$ tasks to confirm round savings offset prompt injection overhead).
