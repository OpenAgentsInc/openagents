# OpenAgents Desktop (EP212 Shell)

This app is the desktop execution boundary for Lightning flows in EP212.

## Purpose

- Desktop hosts wallet/payment execution boundaries.
- `openagents.com` remains the chat/orchestration UI.
- Convex is the command/result bus between web and desktop.

## Current Scope

- Electron app shell with Effect service graph.
- Renderer is fully Effuse-based and mounted in Effuse panes (same pane system family as web).
- Auth linkage strategy: sign in with the same email used on `openagents.com` to map to the same OpenAgents user id.
- Connectivity probes for OpenAgents API and Convex.
- Background executor loop skeleton (`queued -> running -> completed|failed`) via a demo task provider.

## Commands

```bash
cd apps/desktop
npm run dev
npm run typecheck
npm test
```

## Environment

Optional environment variables:

- `OA_DESKTOP_OPENAGENTS_BASE_URL` (default: `https://openagents.com`)
- `OA_DESKTOP_CONVEX_URL` (default: `https://aware-caterpillar-962.convex.cloud`)
- `OA_DESKTOP_EXECUTOR_TICK_MS` (default: `2000`)

## Security Notes

- Renderer runs with `contextIsolation: true`, `sandbox: true`, `nodeIntegration: false`.
- Auth token state is memory-only in this phase.
- No Lightning key material is handled in the current implementation.
