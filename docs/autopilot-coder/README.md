# Autopilot Coder

This directory records the product and implementation audit trail for the
Autopilot coder direction: agent-first coding-work delegation, Probe/Pylon
fanout, Autopilot Sites, MDK/L402 buyer payment, worker settlement gates, and
Forum reporting.

- `2026-06-09-probe-autopilot-sites-agent-api-audit.md`: audit of the current
  Probe, Autopilot, Autopilot Sites, Pylon, payment, and Forum-reporting
  systems against the target "do this on Autopilot" delegated coding-work
  endpoint.
- `2026-06-09-autopilot-coder-current-status-gap-audit.md`: current status and
  gap audit after the P0 issue flow, including the distinction between route
  harness proof and a full live paid coding-agent flow.
- `2026-06-10-autopilot-coder-full-flow-audit.md`: full-flow audit against the
  owner target "through my Pylon, ask my agent to do coding work and it gets
  done ASAP" — closed/open issue map, the #4633 live production smoke result
  and its caveat, promise statuses, and the remaining unowned gaps.
- `2026-06-10-claude-agent-sdk-local-claude-pylon-audit.md`: design audit for
  "Pylon can talk to your local Claude" — the Claude Agent SDK (TypeScript) as
  the requester-Pylon coding execution lane, exact worker-loop seams, option
  mapping, BYOK/branding/redaction boundaries, and the companion promise
  `pylon.local_claude_agent_bridge.v1`. Implemented same-day via epic #4717
  (#4718/#4719/#4720).
- `2026-06-10-claude-agent-bridge-promise-leverage-audit.md`: leverage audit of
  the shipped Claude Agent bridge against all 39 outstanding registry promises
  (registry `2026-06-10.24`) — the three supercharged clusters (compliant
  labor stream, coding-runtime successor, Artanis evolution loop) and the top
  three next moves (first paid local-Claude labor job, real-repo work class +
  `pylon work` entry, Artanis coding tick action).
- `2026-06-11-autopilot-unified-audit-roadmap.md`: the unified audit and
  roadmap — full inventory of the live `/autopilot` web product (chat
  workrooms, goals, SHC container execution, provider-account lease routing,
  billing/metering, token accounting, the coding-autopilot record layer)
  measured against the six wedge problems, the two-stacks finding (the web
  product and the work-order/labor spine don't know each other exist), the
  three-lane placement model anchored by Pylons (hosted SHC / owner Pylon /
  labor market), and the phased productize → unify → market roadmap.
- `implementation-log.md`: running issue-by-issue implementation notes for the
  Autopilot coder backlog.
- `no-spend-e2e-smoke.md`: documented command and retained-evidence checks for
  the public no-spend Autopilot Coder smoke.
- `paid-e2e-smoke.md`: documented command and retained-evidence checks for the
  CI-safe paid Autopilot Coder route smoke.
- `paid-l402-boundary.md`: current signed L402 retry contract and remaining
  live verifier gap for paid Autopilot Coder work.
