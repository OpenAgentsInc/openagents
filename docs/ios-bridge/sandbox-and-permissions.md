# iOS Bridge: Sandbox and Permissions

**Last Updated**: 2025-11-06

This document explains the sandbox and permissions model for the OpenAgents iOS/macOS app, particularly as it relates to the desktop WebSocket bridge.

---

## Table of Contents

1. [Overview](#overview)
2. [Platform Differences](#platform-differences)
3. [Entitlements Files](#entitlements-files)
4. [Sandbox Impact on External Processes](#sandbox-impact-on-external-processes)
5. [Why macOS Needs Sandbox Disabled](#why-macos-needs-sandbox-disabled)
6. [File Access Permissions](#file-access-permissions)
7. [Common Issues](#common-issues)
8. [Troubleshooting](#troubleshooting)

---

## Overview

The OpenAgents app runs on both iOS and macOS, but has very different security requirements:

- **iOS**: Runs on user's phone/iPad as a client, connecting to remote desktop via WebSocket
- **macOS**: Runs on desktop as a **server**, needs to launch external CLI tools (claude, codex)

The app uses **Apple's App Sandbox** to restrict what it can access and do. However, the sandbox requirements differ dramatically between platforms.

---

## Platform Differences

### iOS (Client Mode)

- **Sandboxed**: ✅ YES (required by App Store)
- **Launches processes**: ❌ NO (never needs to)
- **Role**: WebSocket client connecting to desktop
- **Permissions needed**:
  - Network access (WebSocket connections)
  - File access (none - reads from WebSocket)

### macOS (Server Mode)

- **Sandboxed**: ❌ NO (disabled for development tools)
- **Launches processes**: ✅ YES (claude CLI, shell commands)
- **Role**: WebSocket server running agent sessions
- **Permissions needed**:
  - Network listening (WebSocket server on port 8787)
  - File access (~/.claude, ~/.codex session directories)
  - **Process execution** (launch claude CLI, shells)

---

## Entitlements Files

Entitlements files control what capabilities an app has.

### `OpenAgents.entitlements` (Production/Release)

**Location**: `ios/OpenAgents/OpenAgents.entitlements`

**Current Configuration** (as of 2025-11-06):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- macOS: disable sandbox to allow running claude CLI and other external tools -->
    <!-- Sandbox is critical for iOS but unnecessary for macOS desktop app -->
    <key>com.apple.security.get-task-allow</key>
    <true/>
</dict>
</plist>
```

**Key Points**:
- **No app-sandbox key** = sandbox is DISABLED
- `com.apple.security.get-task-allow` allows debugging/development features
- This configuration is for macOS ONLY (iOS builds use separate entitlements via build config)

### `OpenAgentsDebug.entitlements` (Debug Builds)

**Location**: `ios/OpenAgents/OpenAgentsDebug.entitlements`

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Debug build: sandbox disabled to allow unrestricted file reads for discovery. -->
    <key>com.apple.security.get-task-allow</key>
    <true/>
</dict>
</plist>
```

**Key Points**:
- Identical to production (both disable sandbox)
- Used for local development/testing

### Historical Configuration (BROKEN)

**Before 2025-11-06**, `OpenAgents.entitlements` had:

```xml
<key>com.apple.security.app-sandbox</key>
<true/>
<key>com.apple.security.files.absolute-path.read-only</key>
<array>
    <string>/Users/christopherdavid/.codex/sessions</string>
    <string>/Users/christopherdavid/.claude</string>
    ...
</array>
```

**Why This Was Broken**:
- Sandbox enabled (`app-sandbox` = true)
- File access granted for specific paths
- **BUT**: Sandboxed apps **cannot execute external processes**
- Server couldn't launch claude CLI or shells
- All prompts failed with "claude CLI not found"

---

## Sandbox Impact on External Processes

### What the Sandbox Blocks

When `com.apple.security.app-sandbox` is enabled, the app **CANNOT**:

1. ❌ Execute external binaries (claude, codex, node, npm, etc.)
2. ❌ Launch shells (/bin/zsh, /bin/bash)
3. ❌ Use Process() to spawn child processes
4. ❌ Run scripts or tools
5. ❌ Fork/exec system calls

### What Happens When Blocked

The symptoms:
- `Process()` throws an error or silently fails
- `which claude` never finds the CLI
- Login shells (`/bin/zsh -l -c "which claude"`) fail
- No debug logs from subprocess attempts
- Server responds with "claude CLI not found" error

### Why This Matters for OpenAgents

The macOS server MUST:
1. Find the claude CLI (often via login shell PATH)
2. Launch claude as a subprocess with user's prompt
3. Monitor claude's output
4. Read session JSONL files claude writes

None of this works with sandbox enabled.

---

## Why macOS Needs Sandbox Disabled

### The Desktop Development Tool Model

macOS apps that manage development tools typically:
- Run as **helper apps** or **menu bar utilities**
- Launch external CLIs and tools
- Integrate with user's shell environment
- Access project directories

Examples: VS Code, Xcode, Terminal, iTerm2, GitHub Desktop

**None of these are sandboxed** because they need to execute arbitrary commands and tools.

### OpenAgents is a Development Tool Manager

The macOS OpenAgents app:
- Is a **WebSocket server** for managing agent sessions
- Launches **Claude Code CLI** to run AI coding agents
- Reads/writes session files in user directories
- Executes shell commands on behalf of agents

This is **fundamentally incompatible with sandboxing**.

### Security Trade-offs

**With Sandbox Disabled**:
- ✅ Can launch claude CLI
- ✅ Can access session files
- ✅ Works as intended
- ⚠️ Has full user permissions (same as Terminal)

**With Sandbox Enabled**:
- ❌ Cannot launch processes
- ❌ App is completely non-functional
- ✅ Theoretically more secure (but useless)

Since the app is:
- Not distributed via App Store (development tool)
- Designed to run local development commands
- User-facing (not a service/daemon)

**Disabling the sandbox is the correct choice**.

---

## File Access Permissions

### Session Directories

The server needs read access to:
- `~/.claude/` - Claude Code session files
- `~/.codex/sessions/` - Codex session files (legacy)

**With sandbox disabled**, these are accessible by default (user's home directory).

**If we re-enabled sandbox**, we'd need:
```xml
<key>com.apple.security.files.absolute-path.read-only</key>
<array>
    <string>$(HOME)/.claude</string>
    <string>$(HOME)/.codex</string>
</array>
```

But this still wouldn't allow process execution!

### Working Directory Access

When launching claude CLI, the process needs:
- Access to current working directory
- Access to project directories
- Write access for session files

These are automatic with sandbox disabled.

---

## Common Issues

### Issue 1: "claude CLI not found"

**Symptoms**:
- Server logs: `[Bridge][server] ERROR: claude CLI not found`
- iOS shows: "❌ Error: Claude CLI not found. Please install Claude Code CLI."

**Causes**:
1. **Sandbox is enabled** (most common before 2025-11-06 fix)
2. claude CLI not installed (`npm install -g @anthropic-ai/claude-cli`)
3. claude not in PATH (fnm/nvm shell-specific paths)

**Fix**:
1. Verify entitlements: `OpenAgents.entitlements` should NOT have `app-sandbox` key
2. Rebuild app completely (Xcode → Product → Clean Build Folder)
3. Quit and restart app
4. Check installation: `which claude` in terminal

### Issue 2: Server Can't Launch Shells

**Symptoms**:
- No error, but login shell attempts (`/bin/zsh -l`) fail silently
- Process() calls succeed but produce no output

**Cause**: Sandbox is still enabled (cached build)

**Fix**:
1. Clean build folder
2. Verify entitlements are correct
3. Rebuild from scratch

### Issue 3: fnm/nvm Paths Not Found

**Symptoms**:
- `which claude` works in terminal
- Server can't find claude even without sandbox

**Cause**: fnm/nvm add paths dynamically in shell startup scripts, not available to GUI apps

**Fix** (implemented in code):
- Recursive search in `~/.local/state/fnm_multishells/`
- Login shell execution: `/bin/zsh -l -c "which claude"`
- Fallback to common paths

---

## Troubleshooting

### Verify Sandbox Status

Check your current entitlements:

```bash
codesign -d --entitlements - /path/to/OpenAgents.app
```

Look for:
- ✅ Should NOT see: `<key>com.apple.security.app-sandbox</key>`
- ✅ Should see: `<key>com.apple.security.get-task-allow</key>`

### Test Process Execution

Create a test:

```swift
let process = Process()
process.executableURL = URL(fileURLWithPath: "/bin/echo")
process.arguments = ["test"]
try? process.run()
process.waitUntilExit()
print("Status: \(process.terminationStatus)")
```

- If status = 0: ✅ Not sandboxed, can execute processes
- If throws/fails: ❌ Still sandboxed

### Enable Verbose Logging

Server already logs extensively:
```
[Bridge][server] === STARTING CLAUDE CLI SEARCH ===
[Bridge][server] searching fnm path: ...
[Bridge][server] checking: /usr/local/bin/claude
[Bridge][server] trying shell: /bin/zsh
[Bridge][server] ✓ found claude at: ...
```

If you don't see these logs, the code isn't running (stale binary).

### Force Clean Build

```bash
cd ios
rm -rf build/
xcodebuild clean -scheme OpenAgents
xcodebuild build -scheme OpenAgents -sdk macosx
```

Then fully quit and restart the app.

---

## Summary

- **iOS**: Keep sandbox enabled (App Store requirement)
- **macOS**: Sandbox MUST be disabled for server functionality
- **Entitlements**: `OpenAgents.entitlements` should NOT have `app-sandbox` key
- **Process Execution**: Only works with sandbox disabled
- **File Access**: Automatic with sandbox disabled
- **Claude CLI**: Found via recursive fnm search + login shells

This is **by design** for a desktop development tool manager.
