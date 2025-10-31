---
title: Tricoder Architecture & Performance Notes
---

# Tricoder Architecture & Performance Notes

This repo implements the Tricoder “mobile command center” as two cooperating layers:

1. **Expo app (`expo/`)** – Presents agent sessions, history, and controls. It consumes ACP-compliant session updates and Tinyvex typed rows (threads/messages/tool calls) over WebSocket and renders them via typed components.
2. **OpenAgents bridge (`crates/oa-bridge/`)** – An Axum-based WebSocket service that mediates ACP sessions and Tinyvex streams, forwards typed snapshots/updates to clients, and relays user prompts via control messages.

Understanding how these pieces interact is critical for maintaining responsiveness and reliability, especially when updates try to squeeze more throughput out of the same architecture.

## High-Level Flow

- The app connects to the bridge at `ws://<host>:8787/ws` (configurable in Settings).
- The Session composer sends prompts via `tvx.query`/`tvx.subscribe` and ACP control messages; the bridge manages provider lifecycles and sessions.
- The bridge emits typed Tinyvex snapshots/updates (`threads`, `messages`, `toolCalls`) and ACP `bridge.acp` notifications. No JSONL parsing is required on the client.
- The app updates local stores from these typed envelopes and renders streaming UI with ACP components.

## Expo App Overview

- **Routing (Expo Router)** – Primary routes live in `expo/app/`:
  - `/session` handles the live feed and composer, `/session/[id]` replays stored threads.
  - Drawer routes provide History, Projects, Library, and Settings.
- **Typed Streams** – `expo/providers/tinyvex.tsx` and `expo/providers/acp.tsx` consume Tinyvex snapshots/updates and ACP notifications using generated bridge types in `expo/types/bridge/*`.
- **State & Persistence**:
  - `expo/lib/log-store.ts` keeps streamed entries in a persisted Zustand store (`@openagents/logs-v2`) so History and message drill-down screens hydrate instantly.
  - `expo/providers/ws.tsx` manages the WebSocket lifecycle and exposes toggles (`readOnly`, `networkEnabled`, `approvals`, `attachPreface`). Connection status feeds the header indicator.
  - Projects and Skills providers mirror bridge broadcasts so the drawer can switch projects with zero extra round-trips.
- **Rendering** – ACP renderers under `expo/components/acp/` translate typed updates into cards/rows. The Session screen keeps a manual `ScrollView` index, auto-scroll state, and deduping to limit repeated work during bursts.
- **Performance Considerations**:
  - Large command/terminal outputs are summarized at the source; UI consumes concise, typed updates.
  - Async UI state (queued follow-ups, working timers) stays in small React state hooks to avoid serializing into AsyncStorage on every keystroke.
  - The composer routes prompts through `pickProjectFromUtterance` so the correct project context is chosen once per send rather than on every incoming event.

## Bridge Overview

- **Typed WS contract** – The bridge emits canonical, snake_case rows and status structs (e.g., `ThreadSummaryTs`, `MessageRowTs`, `ToolCallRowTs`, `SyncStatusTs`) and typed envelopes (`tinyvex.snapshot`, `tinyvex.query_result`). Types are exported to TS via `ts-rs` into `expo/types/bridge/*`.
- **Session/Provider management** – The bridge manages ACP sessions and maps provider outputs into canonical types. Repo root heuristics still apply for file operations.
- **History & Metadata** – Projects/skills live under `~/.openagents/*` and are validated via schemas under `crates/oa-bridge/schemas/`.
- **Performance Protections** – The bridge summarizes oversized outputs and batches updates to keep UI responsive.

## Reliability & Tuning Checklist

- **Adding new kinds** – Update bridge types/mapping and ACP/Tinyvex renderers. Ensure the Session screen skip logic remains efficient for bursty updates.
- **Modifying WebSocket payloads** – Mirror changes in `ws.tsx`, the Session screen, and any provider using the message bus. Keep payloads JSON serializable; binary frames are not yet supported.
- **Long-running commands** – Prefer summarizing at the bridge (as done for `exec_command_output_delta`) rather than filtering in the UI alone. Anything that touches stderr should consider the replay buffer size.
- **AsyncStorage keys** – When migrating persisted data, bump the version in stores to avoid hydration errors after OTA updates.
- **OTA builds** – The Expo layer uses EAS Update (`bun run update:ios`) with runtime version `0.3.0`. Validate type-check (`bun run typecheck`) and lint before publishing to prevent breaking remote users.

Keeping these details in mind ensures future agents can modify either layer without regressing stream performance or destabilizing the Codex lifecycle.
