# Autopilot Coder — Claude Lane

Dated audits and planning records for Claude support in Pylon: the Claude
Agent bridge (`@anthropic-ai/claude-agent-sdk`), the `claude_agent_task` work
class, Fable (`claude-fable-5`) as a model profile on that lane, and the
parity program that brings the local supervised daily-driver surface up to
the level the Codex lane reached in #4839-#4842.

- `2026-06-12-pylon-claude-codex-parity-audit.md`: full description of the
  current Claude vs Codex system in Pylon and the Autopilot worker API, the
  asymmetry table (assignment-lane parity and Claude dual-capability default
  vs the Codex-only composer/danger/dev surface), the CL1-CL4 suggested issue
  set plus amendments to #4838/#4842/#4843, end states E1-E5, and the
  decision to consume — not expand — the terminal-agent-systems packs
  (cite Pack A/B/C; no new pack).

Canonical lane docs live with the implementation:

- `apps/pylon/docs/claude-agent-bridge.md`
- `apps/pylon/docs/claude-agent-task-smoke.md`

Prior design/leverage audits in the parent directory:

- `../2026-06-10-claude-agent-sdk-local-claude-pylon-audit.md`
- `../2026-06-10-claude-agent-bridge-promise-leverage-audit.md`
- `../2026-06-12-pylon-codex-day-to-day-readiness-audit.md`
