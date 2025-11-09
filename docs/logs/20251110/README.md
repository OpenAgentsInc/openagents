# Log — 2025-11-10: Orchestration + Tests + CI

- Added SchedulerService actor; wired RPCs:
  - orchestrate/scheduler.status reports running and next_wake_time
  - orchestrate/scheduler.reload starts/restarts background scheduler
  - orchestrate/scheduler.run_now triggers immediate run
- Extended LocalJsonRpcClient to support orchestration config set/activate, scheduler status/run_now, and Tinyvex title clear.
- Added tests:
  - OrchestrationSchedulerTests (programmatic control, ACP session/update observed)
  - TinyvexTitleTests (set/get/clear via local RPC)
  - ExportFormattingTests (JSON/Markdown helper)
  - HistoryApiTests (empty DB, invalid id, DB not attached)
- Extracted transcript export logic to a shared helper
- CI workflow added for macOS build/test
- Claude CLI test now soft-skips if CLI cannot execute in env

