---
title: Tricoder Architecture & Performance Notes
---

# Tricoder Architecture & Performance Notes

This repo implements the Tricoder “mobile command center” as two cooperating layers:

1. **Expo app (`expo/`)** – Presents agent sessions, history, and controls. It consumes structured Codex JSONL events and renders them as streaming UI rows and cards.
2. **OpenAgents bridge (`crates/oa-bridge/`)** – An Axum-based WebSocket service that launches `codex exec --json`, forwards stdout/stderr to clients, and relays user prompts via stdin.

Understanding how these pieces interact is critical for maintaining responsiveness and reliability, especially when updates try to squeeze more throughput out of the same architecture.

## High-Level Flow

- The app connects to the bridge at `ws://<host>:8787/ws` (configurable in Settings).
- Each prompt the user submits in the Session screen travels over the WebSocket `"control": "prompt"` payload to the bridge.
- The bridge pipes the text to `codex exec` on stdin, then **immediately closes stdin** so Codex interprets the prompt as complete.
- Codex streams JSONL events (`reason`, `exec_begin`, `file_change`, etc.) back. The bridge broadcasts every line to all connected clients and mirrors them to STDERR so the operator still sees a CLI feed.
- The app parses each line with `expo/lib/codex-events.ts`, updates the local AsyncStorage-backed log stores, and renders UI elements while keeping command deltas out of the scroll view.

## Expo App Overview

- **Routing (Expo Router)** – Primary routes live in `expo/app/`:
  - `/session` handles the live feed and composer, `/session/[id]` replays stored threads.
  - Drawer routes provide History, Projects, Library, and Settings.
- **Event Parsing** – `expo/lib/codex-events.ts` normalizes JSONL into typed items. Session screens ignore `exec_command_output_delta` noise and gracefully skip partial JSON envelopes to avoid UI jank while commands stream.
- **State & Persistence**:
  - `expo/lib/log-store.ts` keeps streamed entries in a persisted Zustand store (`@openagents/logs-v2`) so History and message drill-down screens hydrate instantly.
  - `expo/providers/ws.tsx` manages the WebSocket lifecycle and exposes toggles (`readOnly`, `networkEnabled`, `approvals`, `attachPreface`). Connection status feeds the header indicator.
  - Projects and Skills providers mirror bridge broadcasts so the drawer can switch projects with zero extra round-trips.
- **Rendering** – Components under `expo/components/jsonl/` translate parsed items into cards/rows. The Session screen keeps a manual `ScrollView` index, auto-scroll state, and command deduping map to limit repeated work during bursts.
- **Performance Considerations**:
  - Large command output deltas are **never appended**; only summarized completion messages render.
  - Async UI state (queued follow-ups, working timers) stays in small React state hooks to avoid serializing into AsyncStorage on every keystroke.
  - The composer routes prompts through `pickProjectFromUtterance` so the correct project context is chosen once per send rather than on every incoming event.

## Codex Bridge Overview

- **Binary Launch** – `crates/oa-bridge/src/main.rs` spawns Codex with enforced flags:
  - `--dangerously-bypass-approvals-and-sandbox`, `-s danger-full-access`, `-m gpt-5`, plus `-c model_reasoning_effort=high`.
  - A repo root heuristic ensures the child process starts from the project root (needed for file edits and commands).
- **WebSocket Fanout** – Axum routes `/ws` connections into a broadcast channel so every client shares a single Codex child. New clients receive the replay buffer (`MAX_HISTORY_LINES` default 2000) before live streaming begins.
- **Child Resilience**:
  - Once stdin is exhausted, Codex exits; the bridge respawns the process for the next prompt so consecutive messages always start fresh.
  - Interrupts (`{"control":"interrupt"}`) send a SIGINT-equivalent via a tracked PID if the child is busy.
- **History & Metadata**:
  - History queries hit `~/.codex/sessions` using `HistoryCache`, which rate-limits scans to keep UI fetches cheap.
  - Projects/skills controls round-trip through `~/.openagents/projects` and `~/.openagents/skills`, respecting JSON Schema validation defined in `crates/oa-bridge/schemas/`.
  - `watch_skills_and_broadcast` monitors skill directories so updates appear in-app without manual refresh.
- **Performance Protections**:
  - `summarize_exec_delta_for_log` collapses huge `exec_command_output_delta` arrays before logging, keeping STDIN watchers and TTY output responsive.
  - Lines containing sandbox metadata mark traces with `info!` to help operators spot configuration drift without flooding the client.
  - Broadcast history truncates older lines when it exceeds `MAX_HISTORY_LINES`, preventing unbounded memory use.

## Reliability & Tuning Checklist

- **Adding new event kinds** – Update both `expo/lib/codex-events.ts` and the JSONL renderer components. Ensure the Session screen’s skip logic handles the new format to avoid breaking auto-scroll.
- **Modifying WebSocket payloads** – Mirror changes in `ws.tsx`, the Session screen, and any provider using the message bus. Keep payloads JSON serializable; binary frames are not yet supported.
- **Long-running commands** – Prefer summarizing at the bridge (as done for `exec_command_output_delta`) rather than filtering in the UI alone. Anything that touches stderr should consider the replay buffer size.
- **AsyncStorage keys** – When migrating persisted data, bump the version in stores to avoid hydration errors after OTA updates.
- **OTA builds** – The Expo layer uses EAS Update (`bun run update:ios`) with runtime version `0.2.0`. Validate type-check (`bun run typecheck`) and lint before publishing to prevent breaking remote users.

Keeping these details in mind ensures future agents can modify either layer without regressing stream performance or destabilizing the Codex lifecycle.
