# OpenAgents CLI integration updates

- wired `openagents autopilot run/resume/metrics` to delegate to the `autopilot` binary
- added local issue CRUD handlers for `openagents autopilot issue` using the issues DB
- added `--agent` flag to unified autopilot run (claude/codex/gpt-oss)
- wired `openagents daemon start` to delegate to the `autopilotd` binary
- documented CLI delegation env overrides in d-010
