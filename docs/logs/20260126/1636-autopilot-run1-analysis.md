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

## Update 1 (implementation)
- Added Full Auto decision diagnostics, guardrail audit fields, and raw prediction parsing to `full_auto.rs`.
- Added run metadata creation + run events (`run_started`, decision events, pause/stop) and decision sequence IDs.
- Emitted `fullauto/decision_raw` alongside `fullauto/decision` with raw prediction + parse diagnostics.
- Added dedicated Full Auto decision logs (`fullauto-decisions.jsonl`, `fullauto-decisions-raw.jsonl`) and run metadata files.

## Update 2 (logging reliability)
- Switched app-server event logging to stream events directly to disk (no completion-gated flush).
- Added `OPENAGENTS_EVENT_LOG_DIR` override and `OPENAGENTS_APP_SERVER_LOG_STREAMING` toggle.

## Update 3 (bundles + visibility)
- Added `export_full_auto_trace_bundle` command to bundle logs + config snapshot + decision summary.
- Added log path broadcast (`app/log_paths`) and a Logs section in the sidebar to surface effective log directories.

## Update 4 (2026-01-26) Detailed implementation log
### Files added
- `apps/autopilot-desktop/src-tauri/src/full_auto_logging.rs`
  - Full Auto log paths, run metadata writer, decision logs, raw logs, run events, and log path snapshot.
  - Env overrides: `OPENAGENTS_FULL_AUTO_LOG_DIR`, `OPENAGENTS_TRACE_BUNDLE_DIR`.
- `apps/autopilot-desktop/src-tauri/src/diagnostics.rs`
  - `export_full_auto_trace_bundle` command to collect logs + run metadata + summaries.

### Full Auto decision capture improvements
- `apps/autopilot-desktop/src-tauri/src/full_auto.rs`
  - Added decision diagnostics + guardrail audit structs.
  - Return type is now `FullAutoDecisionResult` with parsed decision + parse diagnostics.
  - Parsing now accepts numeric strings for `confidence` and reports parse errors.
  - Guardrail decisions now include `rule`, `original_action`, and confidence deltas.
  - Added run metadata creation (`run_id`, config snapshot) on Full Auto enable.
  - Added `run_started` event, decision sequence IDs, and run event appends.

### App-server decision event improvements
- `apps/autopilot-desktop/src-tauri/src/backend/app_server.rs`
  - Emits `fullauto/decision` with `runId`, `sequenceId`, `eventTs`, and `guardrail` payload.
  - Emits `fullauto/decision_raw` with raw prediction + parse diagnostics + summary snapshot.
  - Writes JSONL logs for decisions + raw decisions.
  - Appends run events (`decision`, `run_paused`) to run event log.
  - Emits `app/log_paths` on `codex/connected` so UI can show effective log dirs.

### App-server event logging reliability
- `apps/autopilot-desktop/src-tauri/src/file_logger.rs`
  - App-server events are now streamed directly to disk (no completion-gated flush).
  - Added `OPENAGENTS_EVENT_LOG_DIR` override.
  - Added `OPENAGENTS_APP_SERVER_LOG_STREAMING` toggle (default on).

### UI visibility
- `apps/autopilot-desktop/src/components/status-dashboard/component.ts`
  - Added a Logs section in the sidebar showing app-server, Full Auto, and trace bundle paths.
  - Handles `app/log_paths` events to populate log directory fields.

### Trace bundle tooling
- `apps/autopilot-desktop/src-tauri/src/diagnostics.rs`
  - `export_full_auto_trace_bundle` includes:
    - latest app-server + ACP logs
    - fullauto decision logs + raw logs
    - run metadata files
    - decision summary JSON + markdown
    - config snapshot JSON

### Runtime wiring
- `apps/autopilot-desktop/src-tauri/src/lib.rs`
  - Registered `export_full_auto_trace_bundle` command.
  - Added module imports for logging and diagnostics.

### Misc
- `apps/autopilot-desktop/src-tauri/src/codex.rs`
  - Full Auto disable now appends a `run_disabled` event before clearing state.

### Commit reference
- Commit: `feat: add full auto decision logging and trace bundles`
