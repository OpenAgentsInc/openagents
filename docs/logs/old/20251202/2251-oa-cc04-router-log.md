# 2251 Work Log

Task: oa-cc04
Intent: Add Claude Code router fallback and config; update AGENTS test guidance.

- Added claudeCode config schema defaults and tests.
- Introduced subagent router selecting Claude Code when available with fallback and integrated into orchestrator.
- Extended Claude Code subagent options (permission mode) and added routing tests.
- Updated AGENTS.md startup health guidance.
- Tests: bun test (pass).

