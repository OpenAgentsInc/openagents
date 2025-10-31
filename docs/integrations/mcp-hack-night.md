# World Wild Web MCP Hack Night × Tricoder

This note collects practical ways to combine the WWW AI & MCP Hack Night stack (mcp-lite on Cloudflare Workers + AI Agent template) with OpenAgents Tricoder (mobile command center + Rust bridge that streams Codex CLI JSONL).

Goal: ship a compelling demo fast — your MCP server runs on Cloudflare, your agent exercises it, and Tricoder renders the full live stream (reasoning, commands, file changes, and MCP tool calls) on mobile and desktop.

## What you’re building at Hack Night
- MCP server: built with mcp-lite and deployed to Cloudflare Workers (wrangler). Prefer no‑auth for easiest pairing.
- Optional agent app: Cloudflare Agents Starter based “AI agent template” with UI, tools, and MCP client support.

## What Tricoder gives you
- Live stream UI: Tricoder parses Codex CLI JSONL into rich rows, including MCP items.
  - JSONL `item.*` → UI rows; `mcp_tool_call` renders with server/tool/status.
- Bridge: `oa-bridge` Axum WebSocket server spawns `codex exec --json`, forwards stdout/stderr to all clients. For remote access, use LAN/VPN; public tunnels are out of scope here.
- Projects & Skills: Markdown+frontmatter records your repos and reusable procedures; validated against bundled JSON Schemas and synced to the app.

## Quick path to a demo
1) Stand up an MCP server on Cloudflare
- pnpm create mcp-lite@latest (select Cloudflare template)
- pnpm dev, then pnpm run deploy (wrangler login first time)

2) Exercise it with a client
- MCP Inspector, Claude Desktop, or your AI agent template
- Keep tool names and outputs small and descriptive for nice UI rendering

3) Run Tricoder locally
- From this repo: cargo bridge (or npx tricoder for a one‑command setup)
- App default WS: ws://localhost:8787/ws (configurable in Settings)

4) Open the Tricoder app (mobile or desktop)
- Send a prompt that causes your agent to call your MCP tools
- Watch for MCP rows: the stream includes items like:
  - {"type":"mcp_tool_call","server":"my-server","tool":"do_thing","status":"in_progress"}

Tip: If your agent stack lets you configure MCP servers, point it at your Worker’s SSE endpoint. Tricoder already renders MCP events — you just need your agent to emit them while Codex runs.

## Integration ideas (pick 1–2 for Hack Night)
- Mobile operator for your MCP server
  - Use Tricoder as the operator console for your Worker tools. Great for demos where you trigger tools (Notion, GitHub, KV, email) and the stream shows MCP calls alongside reasoning and command output.
- “Project Concierge” on the go
  - Create a Project for your hack repo and a few Skills (checklist or scripted flows). Use MCP tools to fetch issues, triage PRs, or post updates; keep everything observable in Tricoder.
- CF Workers DevOps via MCP
  - Build tools that deploy, tail logs, or flip feature flags (KV/Queues/Durable Objects). Tricoder shows each MCP invocation so a stakeholder can follow along.
- Personal automation agent
  - Language tutor, CRM nudger, or playlist builder. Use mcp-lite tools for domain actions; Tricoder provides a clean feed to explain what’s happening.
- “Tunnel control” tool
  - A small MCP tool that hits a simple endpoint you host to toggle Tricoder tunnels or report bridge health, so your agent can announce a public demo URL in‑stream.
- Session journaling
  - MCP tool writes a summary of the current Tricoder stream to Cloudflare KV or Notion for later retrieval; you demo end‑to‑end capture from phone.

## Mapping reference (so UI shows what you expect)
- Tricoder consumes Codex JSONL. Relevant kinds:
  - mcp_tool_call → rendered as MCP row (server, tool, status)
  - command_execution → command + aggregated output
  - file_change → add/update/delete cards
  - reasoning, agent_message → narrative text
- Keep tool outputs concise; prefer returning compact JSON or short text. Tricoder summarizes very large deltas in logs.

## Packaging your Hack Night project for Tricoder
- Create a Project file so your hack is selectable in the app:
  - Path: ~/.openagents/projects/<id>.project.md
  - Required frontmatter: name, workingDir (see docs/projects-and-skills-schema.md)
  - Optional: repo metadata, instructions block with a short operator guide
- Add Skills for repeatable flows:
  - Path: ~/.openagents/skills/<skill-id>/SKILL.md
  - Include Instructions and Workflow sections; list any MCP tools the flow uses

## Runbook (local demo)
- Bridge: cargo bridge (binds 0.0.0.0:8787)
- App: cd expo && bun install && bun run start
- Mobile: set WS to ws://localhost:8787/ws in Settings
- Prompt: ask for a task that triggers your MCP tools; observe MCP rows and statuses

## Notes and constraints
- Bridge control is WebSocket‑only; don’t add REST endpoints
- The Rust bridge injects permissive sandbox/approvals for smooth demos
- If your agent stack emits MCP events, Tricoder will render them — no extra wiring in the app needed

## Pitches you can demo in 3–5 minutes
- “GitHub triage bot on a phone” — tools: search issues, label, comment (Hono APIs via Codegen); UI: MCP rows show each action
- “KV as a scratchpad” — tools: save_note, list_notes, get_note; show persistence round‑trip
- “Screenshift‑style UI manipulations” — tool drives site transforms; Tricoder narrates calls and results
- “Playlist while coding” — Spotify tool creates/updates playlist; show tool chain and final link

## Links
- mcp-lite: https://github.com/fiberplane/mcp-lite
- Cloudflare Workers: https://developers.cloudflare.com/workers/
- Cloudflare Agents Starter (template base): https://github.com/cloudflare/agents-starter
- Tricoder repo: https://github.com/OpenAgentsInc/openagents
- Codex JSONL schema in this repo: docs/exec-jsonl-schema.md
