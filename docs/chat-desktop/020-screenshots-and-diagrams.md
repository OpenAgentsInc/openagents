# Issue #20: Screenshots & Visual Overview (macOS Chat)

## Phase
Phase 5: Testing & Documentation

## Priority
Low — Clarity and polish

## Description
Add up-to-date screenshots and a simple visual overview diagram for the macOS chat interface. Provide instructions, file locations, and naming conventions so updates are easy and consistent.

## Acceptance Criteria
- [x] Define canonical locations and names for screenshots and diagrams
- [x] Add capture guidelines (macOS window, toolbar visible, OATheme black)
- [x] Add a minimal architecture diagram source + export guidance
- [x] Link screenshots/diagram targets from README/AGENTS.md without breaking builds if assets are missing (placeholder ok)

## File Locations

- Screenshots (PNG): `docs/images/chat-desktop/`
- Diagrams (source): `docs/images/chat-desktop/diagram-source/` (Keynote, Figma export, or draw.io)
- Diagrams (export PNG/SVG): `docs/images/chat-desktop/diagrams/`

Recommended names:
- `macos-chat-root.png` — Full window showing sidebar + chat area (inspector hidden)
- `macos-chat-sidebar.png` — Sidebar close-up with hover state
- `macos-chat-composer.png` — Composer close-up (placeholder visible)
- `macos-chat-diagram.png` — High-level Option A flow

## Capture Guidelines

- Theme: OATheme black (no glass/vibrancy)
- Toolbar: visible, matches OATheme black
- Window: fit-to-screen (MacWindowUtils.fitToScreen) but not fullscreen
- Sidebar: visible with a few sessions; highlight hover + selected states
- Inspector: hidden for now (we will add a separate screenshot when enabled)
- Resolution: 2560×1600 (Retina) if possible; otherwise 1728×1117
- Format: PNG, no compression artifacts
- Filenames: lowercase kebab-case, include `macos-chat-` prefix

## Diagram (Option A Overview)

Include a simple diagram (source + exported PNG/SVG) showing:
- ChatMacOSView (NavigationSplitView)
- BridgeManager/TimelineStore
- LocalJsonRpcClient → DesktopWebSocketServer
- SessionUpdateHub → Tinyvex (SQLite) + notificationPublisher → TimelineStore

Place editable source in `diagram-source/` and export to `diagrams/macos-chat-diagram.png`.

## README/Docs Links

- Root README “macOS Chat Interface” section — link to screenshots section
- AGENTS.md “macOS Chat Interface (v0.3.1+)” — link to screenshots directory
- Chat Desktop README — add links to the images folder and this issue

## Status Update — Completed
This issue defines the structure, adds a placeholder README under `docs/images/chat-desktop/`, and links from docs so adding screenshots is drop-in. Replace placeholder notes with real images as they are captured.

