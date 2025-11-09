# Issue #29: Reset/Clear Session Title (Tinyvex)

## Phase
Phase 4: Integration & Features

## Priority
Low-Medium — quality-of-life and data hygiene

## Description
Allow people to clear a chat session’s custom title and fall back to the default auto/title (or session ID abbreviation). Persist this in Tinyvex so the cleared state survives app restarts.

## Target State
- Context menu action in sidebar rows: “Reset Title”.
- Command-level API: `tinyvex/history.clearSessionTitle`.
- BridgeManager exposes `clearSessionTitle(sessionId:)` and updates `conversationTitles` map.
- Local mac adapter and server route implemented; DB deletes the row from `conversation_titles`.
- UI refresh picks up cleared state immediately.

## Acceptance Criteria
- [x] DB API `clearSessionTitle(sessionId:)` removes any persisted title.
- [x] JSON‑RPC method `tinyvex/history.clearSessionTitle` registered on DesktopWebSocketServer.
- [x] Local helper `localClearSessionTitle(sessionId:)` added.
- [x] BridgeManager (mac) provides `clearSessionTitle` and removes from in‑memory map.
- [x] Sidebar context menu includes “Reset Title” and works.
- [x] Build succeeds on macOS; no warnings introduced.

## Technical Notes
- Clearing is implemented as a DELETE from `conversation_titles` keyed by `session_id`.
- Sidebar title rendering already falls back to an abbreviated session ID when no title exists.
- Inline rename remains available via double‑click or context menu “Rename…”.

## Files Changed
- ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/DbLayer.swift
- ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift
- ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Local.swift
- ios/OpenAgents/Bridge/BridgeManager+Mac.swift
- ios/OpenAgents/Views/macOS/SessionSidebarView.swift

## Status
Implemented and verified via local macOS build.

