# Issue #19: macOS Chat — Quickstart & Troubleshooting

## Phase
Phase 5: Testing & Documentation

## Priority
Low-Medium — Improve onboarding and support

## Description
Provide a concise quickstart and troubleshooting guide for the new macOS chat interface so contributors/users can get productive quickly and self‑serve common fixes.

## Acceptance Criteria
- [x] Quickstart covers build/run, first launch, working directory, starting chats
- [x] Keyboard shortcuts included
- [x] Troubleshooting covers Tinyvex DB, logs, and common pitfalls
- [x] Links to relevant docs (ADR‑0007, AGENTS.md, chat-desktop README)

## Quickstart (macOS)

### Build & Run
1. Open the workspace: `cd ios && open OpenAgents.xcworkspace`
2. Select the `OpenAgents` scheme and target “My Mac”.
3. Run (⌘R). The window auto-fits the screen on first appear.

### First Launch
- The app shows the chat interface: left sidebar (sessions) + center chat area.
- The inspector (right) is hidden for now.
- Toolbar: gear opens Settings (⌘,), wrench opens Developer tools (⌥⌘D).

### Working Directory
- Open Settings (⌘,) → Workspace → Change… and select your working directory.
- The desktop server broadcasts `session/update` and persists chat history to Tinyvex.

### Start a Chat
- Click “New Chat” in the sidebar (or press ⌘N).
- Type in the composer (Return = send; Shift+Return = newline).
- Select an agent with ⌘K if multiple are available.

### Keyboard Shortcuts
- ⌘N: New chat
- ⌘K: Agent selector
- ⌘B: Toggle sidebar
- ⌘,: Settings
- ⌥⌘D: Developer tools
- ⌘/: Keyboard shortcuts help
- Return: Send
- Shift+Return: New line
- Delete: Delete selected session (with confirmation)

## Troubleshooting

### Tinyvex Database Path
- Default path: `~/Library/Application Support/OpenAgents/tinyvex.sqlite`
- If history looks empty, confirm the DB file exists and is writable.

### Logs
- Server/app logs: Console.app (subsystem: OpenAgents) and Developer → Logs.
- You can open the Logs folder via Developer menu in-app.

### “No such module 'OpenAgentsCore'”
- Ensure you opened `OpenAgents.xcworkspace`, not the `.xcodeproj` directly.
- Clean build folder (⌘⇧K) and retry.

### Toolbar/Sidebar Colors
- The UI uses OATheme black surfaces. If you see gray overlays, rebuild and verify you’re on the latest `main` with Issue 014 applied.

### Notifications Not Appearing
- The mac app uses a local JSON‑RPC adapter to call the server (`DesktopWebSocketServer`) and subscribes to `session/update` via Combine.
- Verify the app log shows “DesktopWebSocketServer on ws://0.0.0.0:9099”.
- Check that `BridgeManager+Mac.start()` ran (see init/appear logs).

## References
- ADR‑0007: docs/adr/0007-macos-chat-interface-architecture.md
- AGENTS.md: macOS chat interface and repo map
- Chat Desktop Roadmap: docs/chat-desktop/README.md

## Status Update — Completed
Added this quickstart/troubleshooting guide and linked it from the chat-desktop README index.

