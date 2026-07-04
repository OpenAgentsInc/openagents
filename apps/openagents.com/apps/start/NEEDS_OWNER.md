# TS-2a Owner Follow-up

- Staging deploy completed on 2026-07-04 and refreshed for TS-2.
- Staging URL: https://openagents-com-start-staging.openagents.workers.dev
- Worker name: `openagents-com-start-staging`
- Current version ID: `01014344-715c-46f2-a71d-6b6ff5db7587`
- Deploy command: `bun run --cwd apps/openagents.com/apps/start deploy`
- Owner review routes: `/business`, `/docs`, `/docs/api`, `/blog`,
  `/blog/introducing-khala-code`, `/code/download`, `/autopilot`,
  `/autopilot/legal`.
- Discovery surfaces now served from the Start Worker via API Worker helpers:
  `/llms.txt`, `/agents.md`, `/ai.md`, `/skill.md`, `/robots.txt`,
  `/sitemap.xml`, `/.well-known/mcp.json`,
  `/.well-known/mcp/manifest.json`, `/.well-known/ai-catalog.json`.
- Keep `start.openagents.com` custom-domain wiring separate from this scaffold.
