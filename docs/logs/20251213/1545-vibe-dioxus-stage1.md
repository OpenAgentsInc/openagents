# Vibe + OANIX — Dioxus Stage 1

## What we had
- Vibe UI plan (`docs/logs/old/20251210/2119-vibe-ui-plan.md`) outlining tabbed surface (Projects, Editor, Database, Deploy) with OANIX integration.
- Dioxus adoption plan (`docs/logs/20251213/0020-dioxus-wgpui-plan.md`) to replace GPUI/Zed stacks.

## What I built
- New Dioxus Vibe surface with Bloomberg-esque theming and tab switcher.
- Mocked data and types for OANIX-flavored resources (namespaces, mounts, deployments, analytics).
- Stub panels aligned to the plan:
  - Projects: grid of namespaces/projects + template picker.
  - Editor: File tree showing OANIX mounts, code preview, terminal log, agent feed, preview placeholder.
  - Database: table browser + schema view stub.
  - Deploy: deployment list, domain manager, analytics metrics.
- App toggle between Vibe and existing MechaCoder so both flows stay accessible.

## Next implementation steps
1. Wire real data sources:
   - Pull live OANIX mounts (Namespace) into FileTree; stream LogsFs into TerminalPanel.
   - Surface Scheduler jobs and RunResult summaries in AgentPanel.
2. Hook actions:
   - Add buttons to launch WASI jobs via OanixEnv + Scheduler.
   - Trigger deploy actions and show status updates.
3. Bring wgpui canvas for rich previews (code/graph) and chat rendering when ready.
4. Add routing (Dioxus router) and server functions for data fetch + mutations.
