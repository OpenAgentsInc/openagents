# OpenAgents Documentation Index

This directory tracks the active OpenAgents cross-platform stack.
Current product surfaces: web control plane (`apps/openagents.com/`), Elixir runtime (`apps/openagents-runtime/`), mobile (`apps/mobile`), Rust desktop Codex app (`apps/autopilot-desktop/`), and Electron desktop Lightning app (`apps/desktop/`).

## Start Here

- Product and mission: `MANIFESTO.md`
- End-to-end system architecture: `ARCHITECTURE.md`
- Progressive disclosure map: `AGENT_MAP.md`
- Terminology: `GLOSSARY.md`
- Repository map (current): `PROJECT_OVERVIEW.md`
- Current roadmap: `ROADMAP.md`
- Agent contract and engineering rules: `../AGENTS.md`

## Control-Plane and Runtime Docs

- Runtime architecture plan: `plans/active/elixir-agent-runtime-gcp-implementation-plan.md`
- Runtime internal API contract: `../apps/openagents-runtime/docs/RUNTIME_CONTRACT.md`
- Runtime operations runbook: `../apps/openagents-runtime/docs/OPERATIONS.md`
- DS-Elixir runtime contract: `../apps/openagents-runtime/docs/DS_ELIXIR_RUNTIME_CONTRACT.md`
- DS-Elixir operations runbook: `../apps/openagents-runtime/docs/DS_ELIXIR_OPERATIONS.md`

## Codex Architecture

- Canonical unified plan: `codex/unified-runtime-desktop-plan.md`
- Convex sync-layer plan: `plans/active/convex-self-hosting-runtime-sync-plan.md`
- Master execution roadmap: `plans/active/convex-runtime-codex-master-roadmap.md`
- Codex docs index: `codex/README.md`

## Contracts (Canonical Specs)

- Execution artifacts + replay: `execution/`
- Protocol surface: `protocol/`
- DSE/compiler contracts: `dse/`
- Proto generation policy + verification command: `../proto/README.md` (`./scripts/verify-proto-generate.sh`)

## Plans

- Repo-wide plans hub: `plans/`

## Product Surfaces

- Web app runbooks: `autopilot/`
- Web production E2E testing: `autopilot/testing/PROD_E2E_TESTING.md`
- Web stream testing: `autopilot/testing/STREAM_TESTING.md`
- Web trace retrieval and debugging: `autopilot/testing/TRACE_RETRIEVAL.md`
- Pane system docs: `autopilot/reference/EFFUSE_PANES.md`
- Mobile app source: `../apps/mobile/`
- Rust desktop Codex app source: `../apps/autopilot-desktop/`
- Electron desktop Lightning app source: `../apps/desktop/`

## Local Development

- Laravel web (control plane): `../apps/openagents.com/` — see `plans/active/laravel-rebuild.md`; typically `cd apps/openagents.com && composer run dev`.
- Elixir runtime (execution): `../apps/openagents-runtime/` — `cd apps/openagents-runtime && mix phx.server`.
- Local CI policy + hooks: `LOCAL_CI.md`
- Mobile local dev: `../apps/mobile/README.md`
- Rust desktop Codex local dev: `../apps/autopilot-desktop/` — typically `cargo run -p autopilot-desktop`.
- Electron desktop Lightning local dev: `../apps/desktop/README.md`
- Storybook and component docs: `STORYBOOK.md`

## Audits

- Architecture and technical audits: `audits/`

## Historical Archive

- Deprecated docs were moved to backroom archives.
