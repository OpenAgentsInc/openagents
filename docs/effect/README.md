# OpenAgents Effect v4 + `@effect/ui` Specifications

Design and execution specs for building OpenAgents' web frontend and cloud control plane with **Effect v4** and **`@effect/ui`** (no React) while keeping the compiled **Coax coder (`oa-worker` + `openagents/core/`)** as the single source of truth for agent execution across Web, Desktop, and Mobile.

## Documents

| Document | Scope |
| --- | --- |
| [`half-day-sprint.md`](half-day-sprint.md) | Time-boxed 4-hour feasibility sprint and Go/No-Go gate: wire a live `@effect/ui` streaming chat UI (`VirtualList` + `Markdown` + `Query.StreamService` + `DevTools` MCP) directly to the compiled `oa-worker` binary. |
| [`full-architecture.md`](full-architecture.md) | Target production architecture: Effect v4 control plane, long-lived Linux `oa-worker` sandbox bridge, canonical `openagents.chat.event.v2` / `mcworker-wire` schema mapping, `@effect/ui` frontend decomposition, deployment, phased rollout, and optional future Coax-to-WASM kernel. |

## Core Decisions

1. **Keep Coax `oa-worker` as the sole agent engine.** Do not port the ~30k LOC of `#authority` permissions, compaction, prompt caching, sandboxing, subagents, or local Psionic integration into TypeScript. Run the compiled Linux `oa-worker` binary in sandboxes (and locally on Desktop) behind one framed wire contract.
2. **Use `@effect/ui` + `@effect/ui-kit` + Tailwind v4 for Web (no React).** Share one Effect v4 programming model (`Schema`, `Layer`, `Effect`, `Stream`, `HttpApi`, `Query.StreamService`) across server and browser, with deterministic `Story`/`Play` tests and agent-drivable MCP DevTools.
3. **Keep native Desktop (`apps/openagents-desktop/`) and iOS (`apps/openagents-ios/`).** Do not replace shipped native clients with `@effect/ui-native` (too early) or `expo/exact` (pre-1.0 with daily breaking changes). Web, Desktop, and iOS converge on the same cloud session protocol and event log.
4. **Defer Coax Core → WASM until earned.** A coding agent needs a Linux sandbox for `Bash`, `git`, and file edits anyway. Only extract `oa-worker`'s turn loop into a sans-I/O `wasm32` kernel if we later need zero-sandbox serverless turns.

## Pinned Baseline (as of 2026-11-28)

- **Effect:** `effect` `^4.1.6`
- **Effect UI:** [`Effect-TS/ui`](https://github.com/Effect-TS/ui) snapshot pinned to commit **`9d672abe`** via `pkg.pr.new` (not yet published to npm; never track floating `@main` outside throwaway spikes).
- **Build & Styling:** Vite `^8`, Tailwind CSS v4 (`@tailwindcss/vite`), TypeScript `strict: true` + `exactOptionalPropertyTypes: true`.
- **Coax Coder Runtime:** `openagents/runtime/oa-worker.coax` (`mcworker-wire.coax` + `openagents.session.v1`) and `coax/web/api/account-chat-cloud-wire.coax` (`openagents.chat.event.v2`).
