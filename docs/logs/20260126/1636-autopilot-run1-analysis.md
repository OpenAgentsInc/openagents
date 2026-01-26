# Autopilot Run 1 Analysis (2026-01-26 16:36)

## Observed outcome
- Run ended with UI message: `PAUSE (0.00) — Low confidence (0.00) decision; pausing Full Auto.`
- This message is produced from the `fullauto/decision` event handler in the sidebar and indicates the Full Auto guardrail forced a pause.

## Evidence checks
- Searched local app-server logs in `apps/autopilot-desktop/tmp/app-server-events_20260126_*.jsonl` for `fullauto/decision`, `Low confidence`, and `pausing Full Auto` but found no matching entries.
- This suggests either:
  - the `fullauto/decision` event was not flushed to file (buffered under a thread id that never received a completion event), or
  - the decision event was emitted but not recorded due to a logging gap.

## Likely root cause (code path)
The message text comes from the Full Auto guardrails in:
- `apps/autopilot-desktop/src-tauri/src/full_auto.rs`
  - `FullAutoState::enforce_guardrails` sets `action = Pause` when `decision.confidence < min_confidence`.
  - `min_confidence` defaults to **0.55** (`FullAutoConfig::default`).
  - `read_prediction_f32` parses the LLM output via `as_f64().unwrap_or(0.0)`.

Given the UI showed **confidence 0.00**, the most probable causes are:
1. **Missing or non-numeric `confidence` output** from `FullAutoDecisionSignature`.
   - If the model returns confidence as a string (e.g., `"0.62"`), `as_f64()` yields `None` → `0.0`.
2. **Incomplete/empty signature output**, causing defaults:
   - Unknown action strings map to `Pause`, and a missing confidence maps to `0.0`.

Either case triggers the low-confidence guardrail and pauses Full Auto.

## Why this matters
- The pause was not caused by an explicit error or failure in the run; it was a **format/typing mismatch** in the decision output.
- This can silently halt Full Auto even when the decision model is otherwise “reasonable.”

## Suggested follow-ups (for future fix)
1. **Improve parsing tolerance**
   - Accept numeric strings for `confidence` (e.g., parse `Value::as_str()` to `f32`).
2. **Add decision-output validation logging**
   - Log raw decision output before guardrails to diagnose schema mismatches.
3. **Ensure `fullauto/decision` events always flush**
   - Consider a direct flush after emitting `fullauto/decision` to avoid losing the event when no completion event follows.

## Open questions
- Which exact thread/turn produced the decision? The current log files do not capture the `fullauto/decision` event, so the thread id and decision payload are unknown.
- If you can provide the thread id or exact log file, I can correlate the run timeline in more detail.

## Recommendations: improve logging for reliable run analysis

### 1) Make Full Auto decisions a first-class, durable log record
- **Always flush `fullauto/decision` immediately** after emission (do not wait for a completion event).
- Write to a dedicated JSONL log: `~/.openagents/autopilot-desktop/logs/fullauto-decisions.jsonl`.
- Include: timestamp, workspace_id, thread_id, turn_id, action, confidence, reason, next_input_preview, model name, latency_ms.

### 2) Capture raw model output and parse diagnostics
- Log the **raw prediction payload** from `FullAutoDecisionSignature` (before parsing).
- Record a parse report:
  - `confidence_raw`, `confidence_parsed`, `confidence_parse_error` (if any)
  - `action_raw`, `action_parsed`, `action_parse_error`
- This lets us detect schema mismatch immediately.

### 3) Add a “decision envelope” to app-server events
- Emit an event like `fullauto/decision` **and** a `fullauto/decision_raw` event that includes:
  - full decision summary (parsed fields)
  - parse diagnostics
  - guardrail outcome (which rule caused pause/stop)

### 4) Persist event logs per thread/turn, not per “completion” heuristics
- Current buffering depends on “completion detection,” which can skip flushing.
- Write events to disk **streaming as they arrive**, then optionally maintain a “session index” for grouping.
- Consider rotating files by thread id and day: `app-server-events_<date>_<thread>.jsonl`.

### 5) Introduce a run metadata file for each Full Auto run
- On `fullauto/enable`, create a run record:
  - `run_id`, workspace_id, thread_id, started_at, model, config thresholds.
- Append `decisions`, `turns`, and `termination_reason` as the run progresses.

### 6) Promote log paths to config + show in UI
- Allow log locations to be configured in `PlanModeConfig`/`FullAutoConfig` or a global config file.
- Surface the active log paths in the UI (status sidebar or diagnostics panel).

### 7) Add a log indexer to summarize runs
- Create a small tool/command that reads:
  - fullauto decision logs
  - app-server event logs
  - token usage logs
- Outputs a “Run Summary” (JSON + markdown) with:
  - total turns, last decision, reason for stop/pause, confidence stats.

### 8) Guardrail audit trail
- When guardrails override a decision, emit:
  - `guardrail_triggered: true`
  - `guardrail_rule: low_confidence | no_progress | max_turns | max_tokens | interrupted | failed`
  - original decision vs enforced decision

### 9) Consistent timestamps + monotonic ordering
- Include `event_ts` and `sequence_id` in all logs to make merges reliable.
- Optionally include `monotonic_ms` since app start.

### 10) Add a “trace bundle” export
- Provide a single export command that bundles:
  - app-server logs
  - fullauto decision logs
  - ACP logs
  - config snapshot
- Store under `docs/logs/<date>/` for durable analysis.
