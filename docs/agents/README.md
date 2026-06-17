# Agent Runtime Audits

This directory consolidates the current OpenAgents evaluations of agent
frameworks and runtimes.

The common decision across these docs is that OpenAgents remains the authority
layer for identity, workspace membership, GitHub write grants, billing, payout,
accepted work, and public projections. Agent frameworks may own bounded runtime
state, connector sessions, drafts, and realtime coordination only after trusted
OpenAgents code has selected the user, workspace, repository, issue, credential
grant, and instance key.

## Contents

| Document | Subject | Current Recommendation |
| --- | --- | --- |
| [Flue Framework Audit](./2026-06-16-flue-framework-openagents-audit.md) | Flue as a connector and agent-harness sidecar for provider events and durable sessions. | Worth a GitHub-first sidecar pilot, not a replacement for `openagents.com` authority. |
| [Flue GitHub Connector Roadmap](./2026-06-16-flue-github-web-ui-integration-roadmap.md) | Ordered GitHub connector and OpenAgents web UI integration plan. | Keep GitHub first, make status visible in the web UI, and close the loop through OpenAgents-owned writeback. |
| [Rivet Effect SDK Audit](./2026-06-17-rivet-effect-sdk-openagents-audit.md) | Rivet Actors and `@rivetkit/effect` as an Effect-native actor substrate. | Good pilot candidate for one non-authoritative actor surface; watch beta/raw-context boundaries. |
| [Cloudflare Agents SDK Audit](./2026-06-17-cloudflare-agents-sdk-openagents-audit.md) | Cloudflare Durable Object-backed Agents SDK as a native workroom/agent runtime. | Leading Cloudflare-native runtime candidate, but only behind OpenAgents auth and authority. |

## Runtime Map

| Runtime | Best Role | Do Not Move Into It |
| --- | --- | --- |
| Flue | Third-party connector ingress and agent sidecar sessions. | Login, team membership, billing, GitHub credentials, accepted-work records, public projections. |
| Rivet | Effect-native actors for bounded agent/session/workroom state and typed actions. | OpenAgents product authority, payment authority, raw private data, broad provider credentials. |
| Cloudflare Agents SDK | Cloudflare-native Durable Object agents, realtime state, durable fibers, Workflows, MCP, React hooks. | Generic public `/agents/...` routing, raw transcript storage, provider mutation authority, payment/writeback authority. |

## Near-Term Order

1. Keep the existing Flue GitHub connector roadmap as the connector lane.
2. Use the Cloudflare Agents SDK as the first Cloudflare-native runtime spike
   for one authenticated, non-authoritative workroom/session agent.
3. Keep Rivet as an Effect-native actor reference and possible alternate pilot
   where typed action/state contracts are more valuable than Cloudflare-native
   deployment.
4. Before any production agent memory or transcript persistence, define the
   redaction and retention invariant: no raw secrets, raw private repo content,
   raw provider payloads, raw prompts, or raw runner logs in durable agent
   history by default.
