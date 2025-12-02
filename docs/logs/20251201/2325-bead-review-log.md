## Bead Review — 2025-12-01 23:25 CT

- Two top-priority items: epic `openagents-42j` (Autonomous Coding Agent Infrastructure) and in-progress `openagents-42j.7` (Agent loop/tool lifecycle). Everything else is P2/P3 and waiting on the core loop.
- No obvious duplicates; all tasks hang off the epic and depend on the core loop/tool schema work. Several provider integrations and remaining tools are blocked implicitly by `openagents-42j.7`.
- Staleness: all beads were created/updated today; no stale items. One bead `openagents-42j.7` is already `in_progress`; avoid parallel edits unless coordinating.

### Details and suggested order

1) `openagents-42j` (epic, P1): umbrella for the coding agent infra. Leave open; track sub-tasks below.
2) `openagents-42j.7` (P1, in_progress): implement agent loop and tool execution lifecycle. This unblocks most other work. Confirm ownership before proceeding to avoid contention.
3) Schema/tooling: `openagents-42j.15` (Convert tool schemas to Effect Schema, P2). We already ported the edit tool; extend pattern to the rest once the loop is stable.
4) Tool ports (P2/P3): read (`.1`), bash (`.2`), write (`.3`), grep/find/ls (`.4-.6`). These depend on the loop and schema decisions; prioritize read/bash/write first.
5) Provider abstraction + providers: `openagents-42j.11` (Unified provider abstraction), OpenAI/Anthropic/Gemini (`.8-.10`), token accounting (`.13`), streaming partial tool args (`.12`), session persistence (`.14`). Sequence: abstraction → OpenRouter already present → add OpenAI/Anthropic/Gemini after loop/tools; token accounting and session persistence follow once loop is working.

### Blockers / notes
- Core blocker is `openagents-42j.7` completion. All other items can be staged but rely on the loop’s shape and the final tool schema contract.
- No duplicate/obsolete beads detected. If scope widens (e.g., more providers), add new beads linked to the epic rather than stretching existing ones.
