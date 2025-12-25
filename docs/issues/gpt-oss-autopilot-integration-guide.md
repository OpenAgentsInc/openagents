# GPT-OSS Autopilot Integration Guide - Issue #2

## Status

**COMPLETE**: GPT-OSS is runnable via `autopilot run --agent gpt-oss` with tool loop support and trajectory capture.

## What Changed

- Added `run_gpt_oss_agent()` in `crates/autopilot/src/main.rs` using `gpt-oss-agent` sessions.
- Added model alias resolution for GPT-OSS (`20b`/`120b`) and default fallback for Claude aliases.
- Added GPT-OSS base URL resolution via `GPT_OSS_URL` (fallback `GPT_OSS_SERVER_URL`).
- Trajectory capture maps GPT-OSS history into `StepType` entries (user, assistant, thinking, tool calls/results).
- `TrajectoryCollector::set_session_id` updates rlog headers and fires callbacks; `set_result` added for non-SDK agents.

## Usage

```bash
# Standard 20b run (short alias)
GPT_OSS_URL=http://localhost:8000 \
autopilot run --agent gpt-oss --model 20b "List files in the repo"

# Explicit model ID
autopilot run --agent gpt-oss --model gpt-oss-120b "Summarize the architecture"

# Legacy env var still supported
GPT_OSS_SERVER_URL=http://localhost:8000 \
autopilot run --agent gpt-oss --model gpt-oss-20b "Explain the task"
```

## Notes / Gaps

- GUI agent selection and local-inference settings UI are still pending (see d-019 Phase 5).
- Autopilot MCP issue tools are not exposed to GPT-OSS yet; GPT-OSS uses its native tool set (browser/python/apply_patch/ui_pane).
- GPT-OSS runs are non-streaming in the CLI; output prints once the completion finishes.

## Testing

- Start `llama-server` with a GPT-OSS GGUF and run one of the commands above.
- Verify `docs/logs/YYYYMMDD/*.rlog` and `*.json` include the GPT-OSS steps and session ID.
