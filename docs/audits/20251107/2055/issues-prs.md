# GitHub Issues & PRs Snapshot

Captured via `gh` CLI at audit time.

## Open issues

- #1424 — P0: Neutralize BridgeConfig.defaultHost; use last-known/simulator loopback — https://github.com/OpenAgentsInc/openagents/issues/1424
- #1423 — P0: Centralize logging (os.Logger), replace print, add DEBUG gating — https://github.com/OpenAgentsInc/openagents/issues/1423

## Recently closed issues (last 10)

- #1433 — Refactor: Implement Agent Registration System for Extensible Agent Management — closed 2025-11-08T01:09:26Z — https://github.com/OpenAgentsInc/openagents/issues/1433
- #1432 — Tool calls not rendering in iOS simplified timeline view — closed 2025-11-07T21:49:38Z — https://github.com/OpenAgentsInc/openagents/issues/1432
- #1431 — Refactor DesktopWebSocketServer into Modular Components (Tinyvex‑first) — closed 2025-11-07T22:45:06Z — https://github.com/OpenAgentsInc/openagents/issues/1431
- #1430 — Tinyvex History — Recent Sessions Drawer + Load Messages — closed 2025-11-07T20:19:31Z — https://github.com/OpenAgentsInc/openagents/issues/1430
- #1429 — Tinyvex Phase 1 — ACP‑First Local Sync Server (SwiftNIO + GRDB) — closed 2025-11-07T19:59:03Z — https://github.com/OpenAgentsInc/openagents/issues/1429
- #1428 — P0: Replace provider 'global tailer' with immediate ACP streaming (stdout) + session-scoped tail (optional) — closed 2025-11-07T20:02:14Z — https://github.com/OpenAgentsInc/openagents/issues/1428
- #1427 — P0: Convert NewChatView into ACP-compliant chat timeline (messages, thoughts, tools, plans) — closed 2025-11-08T01:28:50Z — https://github.com/OpenAgentsInc/openagents/issues/1427
- #1426 — P0: Add CI (build + tests) and SwiftLint/SwiftFormat — closed 2025-11-08T01:19:44Z — https://github.com/OpenAgentsInc/openagents/issues/1426
- #1425 — P0: Update docs/ios-bridge to JSON-RPC initialize (align with ADR-0004) — closed 2025-11-08T01:44:16Z — https://github.com/OpenAgentsInc/openagents/issues/1425
- #1422 — P0: Extract prettyShellCommand & command parsing to OpenAgentsCore; deduplicate UI use — closed 2025-11-08T02:46:43Z — https://github.com/OpenAgentsInc/openagents/issues/1422

## Open PRs

- None at the time of audit

## Recently closed PRs (last 10)

- #1414 — BREAKING: Delete Expo, Tauri, Rust - Swift-only v0.3 — merged 2025-11-06T04:29:14Z — https://github.com/OpenAgentsInc/openagents/pull/1414
- #1408 — Swift: add OpenAgentsCore history scanners for Codex/Claude + tests — merged 2025-11-04T02:09:50Z — https://github.com/OpenAgentsInc/openagents/pull/1408
- #1405 — Fix WebKit TDZ in BridgeProvider send() — merged 2025-11-03T15:27:41Z — https://github.com/OpenAgentsInc/openagents/pull/1405
- #1403 — Core imports + TS stabilization (Expo typecheck clean) — merged 2025-11-03T12:45:13Z — https://github.com/OpenAgentsInc/openagents/pull/1403
- #1402 — Desktop: Load Expo Web in Tauri (dev/prod) — merged 2025-11-03T02:54:59Z — https://github.com/OpenAgentsInc/openagents/pull/1402
- #1400 — Tauri: Sidebar + desktop‑managed bridge (start/stop/status) with Settings/Connection UI — merged 2025-11-03T02:16:47Z — https://github.com/OpenAgentsInc/openagents/pull/1400
- #1398 — Core package: shared chat UI for Expo + Tauri — merged 2025-11-03T01:25:25Z — https://github.com/OpenAgentsInc/openagents/pull/1398
- #1396 — Tauri: reuse Expo ThreadListItem in sidebar via RN-web shims — merged 2025-11-02T22:21:08Z — https://github.com/OpenAgentsInc/openagents/pull/1396
- #1395 — Tauri: bridge connect panel + auto token/port/connect, chat view, sidebar (recent chats + raw), full-height/layout fixes — merged 2025-11-02T21:53:43Z — https://github.com/OpenAgentsInc/openagents/pull/1395
- #1393 — Phase 3: integrate tinyvex/react in app; add Metro + TS aliases — merged 2025-11-02T21:01:30Z — https://github.com/OpenAgentsInc/openagents/pull/1393

Notes:

- The repo transitioned to Swift‑only in #1414; ensure docs and CI reflect this (issue #1426 indicates CI + lint adoption now closed).
- Open issues #1423/#1424 align with audit findings (central logging, neutral default host).

