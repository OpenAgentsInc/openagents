# Khala QA trace — ATIF-v1.7 sample (`/login` verify)

This is ONE real, beautiful **agent trace** (not an eval) in **ATIF**
(Agent Trajectory Interchange Format, v1.7), emitted from a real `openagents/khala`
computer-use run against **production** `https://openagents.com/login`.

- `trajectory.json` — the ATIF-v1.7 `Trajectory`. Maps the Khala run
  (`result.json` + `session-trace.json`) to: a `user` goal step, one `agent`
  step per computer-use action (narration as `message`, the decision as
  `reasoning_content`, the action as a `tool_call`, the snapshot as an
  `observation` correlated by `source_call_id`, `metrics.cost_usd = 0` for
  own-infra), and a final `done` verdict step. Validates against the qa-runner
  ATIF validator and matches the harbor golden shape (sequential `step_id` from
  1, `source` enum, tool/observation correlation, ISO timestamps).
- `trace.html` — a self-contained, beautiful viewer (dark, pure black, warm
  off-white `#f1efe8`, Commit Mono, command-surface timeline per
  `apps/openagents.com/DESIGN.md`). Header strip (agent / model / verdict /
  duration / cost $0 / steps), a vertical step timeline (goal → each step with
  narration, collapsible reasoning, the tool call + args, the observation, and
  screenshot thumbnails inline), the embedded + playable video, and final
  metrics. Open it directly in a browser.
- `session.mp4` — the real session recording (referenced by `trace.html`).
- `00-login-page.png` — the screenshot Khala captured (shown inline + linked).
- `trace-screenshot.png` — a full-page render of `trace.html` (proof of render).

Verdict: **PASS** · model `openagents/khala` · cost **$0** (own infra) ·
public-safe (no secrets/tokens/raw provider ids).

Regenerate from a run dir:

```
bun run src/atif-emit.ts --run ./runs/<run> --out ./samples/<name> --session-id <id>
```
