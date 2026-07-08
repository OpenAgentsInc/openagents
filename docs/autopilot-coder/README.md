# Autopilot Coder

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


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
  labor market), and the phased productize → unify → market roadmap. It now
  carries the Pack A timing overlay for proof/supervision operationalization:
  #4813 tracks the parent, #4814-#4823 track the child subsystem issues, and
  Pack A affects proof/claim closure without pausing unrelated rungs. It now
  also carries the Pack B readiness update: #4824 tracks account,
  credential, and policy hardening, with #4825-#4830 covering credentials,
  settings, security review, telemetry/privacy, retention/deletion, and
  minimal managed policy. Pack C is now implemented as #4831-#4835, covering
  repository/worktree identity, change capture, file/shell/workspace authority,
  and delivery/PR readiness receipts. The public freshness/order-book hygiene
  pair is closed by #4836/#4837; the next pack is intentionally not filed until
  the remaining MVP proof gates (#4768/#4772) have consumed Pack C evidence or
  been explicitly narrowed.
- `2026-06-12-pylon-codex-day-to-day-readiness-audit.md`: immediate
  day-to-day readiness audit for switching owner coding to Pylon with Codex as
  the primary workhorse and Fable via the Claude Agent lane. Verdict:
  source-checkout daily-driver dogfood is close enough to prove with one
  retained owner-watched run; full packaged replacement is still blocked by
  v0.3 packaging, retained proof, delivery ergonomics, and unattended/public
  readiness gates. `pylon work submit` commit pinning and explicit
  Codex/Fable adapter intent are implemented in source.
  It also proposes a local owner-only Pylon Dev Mode for improving Pylon from
  inside Pylon with Codex fixes, optional Fable review, targeted checks, and
  safe reloads.
- `2026-06-21-autopilot-verse-coding-agent-pane-overlay-audit.md`: current
  audit for bringing coding agents into the Autopilot Desktop Verse surface as
  an explicit code-mode overlay. Covers prior pane systems, the existing
  Desktop pane manager and Pylon/Codex/Claude Agent runtime, R3F/Drei `Html`
  lessons, the recommended `three-effect` DOM overlay host boundary, input
  ownership, required coding panes, and the regression smokes needed before
  calling Verse coding mode ready. It now carries the Codex-first sequential
  `VCODE-01` through `VCODE-16` issue ladder, including multiple Codex
  accounts, account/session sync, approvals, diffs, logs, diagnostics, and the
  reusable integration smoke.
- `2026-06-21-opencode-desktop-harvest-for-verse-coding-overlay-audit.md`:
  companion audit of the local `projects/repos/opencode` desktop app. Extracts
  the reusable host-boundary, sidecar-readiness, scoped-command, dock-stack,
  timeline-projection, permission, diff/review, terminal, and diagnostics
  patterns that fit the Autopilot Verse coding overlay plan, and maps those
  patterns onto the shared `VCODE` issue ladder.
- `pylon-multi-session-agent-runbook.md`: operator handoff for a fresh coding
  agent using Pylon as the local multi-account coding orchestrator: inspect
  connected Codex/ChatGPT and Claude accounts, run batch `multi-session`
  subagents, drive live `session.spawn/list/events/cancel` over the Pylon
  control server, and close out with public-safe proof artifacts.
- `claude/2026-06-12-pylon-claude-codex-parity-audit.md`: full audit of
  current Claude support in Pylon measured against the Codex lane — the
  assignment-spine parity and Claude dual-capability default versus the
  Codex-only local supervised surface (composer backend, dangerous mode, dev
  doctor execution mode, TUI labels) — with the CL1-CL4 suggested issue set,
  amendments to #4838/#4842/#4843, end states, and the decision to cite the
  closed Pack A/B/C contracts rather than file a new operationalization pack.
- `terminal-agent-systems/2026-06-11-open-issue-delegation-plan.md`: delegation
  plan for the current open issue set, assigning the MVP ladder, Pack A, Pack
  B, market, and W3 evaluation work across eight named agent worktrees with
  status-comment, rebase/merge, deferred E2E, and closeout rules.
- `implementation-log.md`: running issue-by-issue implementation notes for the
  Autopilot coder backlog.
- `terminal-agent-systems/`: dated OpenAgents-native audits for 62 terminal
  agent subsystems plus the broader systems index, including the runtime
  kernel, workspace materialization, conversation/query, tools, file and shell
  authority, permission approval, sandbox boundaries, task supervision,
  context assembly, compaction, providers, memory, retrieval, LSP diagnostics,
  TUI, command, review, notification, session navigation, MCP, plugin, skill,
  hook, settings, authentication, Git/GitHub, editor, and browser/desktop
  integration systems, through collaboration, operations, release, evaluation,
  security, retention, onboarding, accessibility, localization, and managed
  policy. Start with
  `terminal-agent-systems/2026-06-11-terminal-agent-systems-operationalization-roadmap.md`
  when deciding which audits to operationalize during the MVP sprint.
- `no-spend-e2e-smoke.md`: documented command and retained-evidence checks for
  the public no-spend Autopilot Coder smoke.
- `paid-e2e-smoke.md`: documented command and retained-evidence checks for the
  CI-safe paid Autopilot Coder route smoke.
- `paid-l402-boundary.md`: current signed L402 retry contract and remaining
  live verifier gap for paid Autopilot Coder work.
