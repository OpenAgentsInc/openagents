# GPT-OSS autopilot runner integration

- added `run_gpt_oss_agent` in `crates/autopilot/src/main.rs` with tool loop support
- resolved GPT-OSS model aliases (`20b`/`120b`) and `GPT_OSS_URL` fallback handling
- mapped GPT-OSS session history to trajectory steps (user/assistant/thinking/tool call/result)
- updated `TrajectoryCollector` to update rlog headers on `set_session_id` and allow `set_result`
- refreshed GPT-OSS autopilot docs + directive status
