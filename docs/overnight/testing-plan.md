# Orchestration — Testing Plan (Stub)

- Compressed Orchestration cycle (1–3 minute intervals) using the runner directly
- Multi-agent execution (Claude Code + Codex) with mocks
- FM decision quality (skip if FM unavailable)
- Error recovery (agent crash, timeout)
- Draft PR creation (skipped if `gh` not authenticated)

Post-demo: expand with end-to-end scheduler tests and UI monitoring flows.
