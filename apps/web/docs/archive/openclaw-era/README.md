# OpenClaw-era documentation (archived)

These docs describe the **previous** openagents.com product direction: full OpenClaw integration (Hatchery, managed runtime, Cloudflare Agents SDK agent worker, zero-to-OpenClaw flow). That direction is not being pursued in favor of a lighter-weight spec.

**Archived 2026-02-04.** Kept for historical context only.

## Contents

| Doc | Purpose |
|-----|---------|
| **MVP_SPEC.md** | Roll-up spec: Early Access → Full Flow, OpenClaw + Hatchery |
| **openclaw-on-openagents-com.md** | Canonical Cloudflare + OpenClaw roadmap |
| **openclaw-hatchery-architecture.md** | Hatchery ↔ OpenClaw instance/runtime architecture |
| **openclaw-full-flow.md** | End-to-end flow: Hatchery UI → Convex → API → runtime |
| **zero-to-openclaw-30s.md** | “Zero to OpenClaw in 30 seconds” flow and current state |
| **split-plan.md** | Autopilot (Agents SDK) + OpenClaw runtime split |
| **cloudflare-agents-sdk-openagents-com.md** | Background: Agents SDK + agent worker |
| **agent-login.md** | Agent signup/login design (Principal, API keys, scopes) |
| **agent-capabilities-on-openagents-com.md** | What agents can/cannot do on the site (OpenClaw API, Hatchery, etc.) |
| **openclaw-rust-api-dependency-analysis.md** | Decision: keep Rust API for OpenClaw release |
| **flow-conversion-plan.md** | Flow UI: SVG graph system (Unkey flow reference) |
| **CODING_AGENT_MISSION.md** | Agent mission: implement web MVP (verification loop, MVP_SPEC) |
| **HANDOFF-NEXT-AGENT.md** | Handoff: OpenClaw “Only HTML” tool fix |
| **chat-only-html-error-report.md** | Report: “Only HTML” error in OpenClaw tool flow |
| **pi-plugins-support-plan.md** | Pi plugins + OpenClaw runtime integration |

## Current docs (still active)

- **E2E / testing:** `../e2e-browser-testing-plan.md` (at docs root) — Playwright E2E plan.
- **Product / feed:** `../nostr-grid-visualization-plan.md` (at docs root) — Nostr grid visualization for feed/communities.

For repo-wide product and protocol docs, see repo root `docs/` and `ROADMAP.md`, `docs/open-protocols/`.
