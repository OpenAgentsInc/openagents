# Issue #7: Add Agent/Model Selector to Toolbar/Header

## Phase
Phase 2: Main UI Components

## Priority
Medium - User-facing feature for agent selection

## Description
Add a dropdown or menu in the toolbar/header to select between different agents, commands, or configurations, similar to ChatGPT's model selector.

## Current State
- iOS has agent selection UI in `NewChatView` (dropdown in header)
- `BridgeManager` tracks `availableCommands` from desktop
- No agent selector UI on macOS desktop

## Target State
- Toolbar button/dropdown showing current agent/command
- Click to open popover with available agents/commands
- Display agent name, description, and icon
- Persist selected agent per session
- Integrate with `BridgeManager.availableCommands`
- Visual indication of active agent

## Status
Completed (implemented on main)

What shipped
- AgentSelectorView at top-left of the sidebar (above New Chat).
- Heuristic mapping of `availableCommands` → modes (Claude Code, Codex) with fallback.
- Selecting agent sets session mode: starts a new session with desired mode or updates mode on the current session.
- Visual indicator of current mode and checkmark in popover; ⌘K opens selector.
- `BridgeManager.selectedAgent` used to keep UI label consistent.

## Acceptance Criteria
- [x] Add button showing current agent
- [x] Popover menu lists available agents (by mode)
- [x] Select agent updates `BridgeManager.selectedAgent`
- [x] Visual feedback for selected agent (checkmark)
- [x] Fallback to "Default Agent" when none available
- [x] Keyboard shortcut ⌘K to open agent selector
- [x] Selected agent persists in session (via mode)

## Technical Details

### File Structure
```swift
// ios/OpenAgents/Views/macOS/AgentSelectorView.swift
struct AgentSelectorView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    @State private var showPopover = false

    var body: some View {
        Button(action: { showPopover.toggle() }) {
            HStack(spacing: 6) {
                Image(systemName: agentIcon)
                    .font(.system(size: 14))

                Text(selectedAgentName)
                    .font(OAFonts.mono(size: 13))
                    .lineLimit(1)

                Image(systemName: "chevron.down")
                    .font(.system(size: 10))
                    .foregroundColor(OATheme.Colors.textSecondary)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(OATheme.Colors.background.opacity(0.5))
            .cornerRadius(8)
        }
        .buttonStyle(.plain)
        .popover(isPresented: $showPopover, arrowEdge: .bottom) {
            AgentSelectorPopover(
                selectedAgent: $bridgeManager.selectedAgent,
                availableCommands: bridgeManager.availableCommands,
                onSelect: { agent in
                    bridgeManager.selectedAgent = agent
                    showPopover = false
                }
            )
        }
        .keyboardShortcut("k", modifiers: .command)
    }

    private var selectedAgentName: String {
        bridgeManager.selectedAgent?.name ?? "Default Agent"
    }

    private var agentIcon: String {
        // Heuristic: map known command names to SF Symbols; fallback to cpu
        switch bridgeManager.selectedAgent?.name {
        case .some(let n) where n.localizedCaseInsensitiveContains("claude"): return "sparkles"
        case .some(let n) where n.localizedCaseInsensitiveContains("codex"): return "chevron.left.slash.chevron.right"
        default: return "cpu"
        }
    }
}

struct AgentSelectorPopover: View {
    @Binding var selectedAgent: ACP.Client.AvailableCommand?
    let availableCommands: [ACP.Client.AvailableCommand]
    let onSelect: (ACP.Client.AvailableCommand) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header
            Text("Select Agent")
                .font(OAFonts.mono(size: 12, weight: .semibold))
                .foregroundColor(OATheme.Colors.textSecondary)
                .padding(.horizontal, 16)
                .padding(.vertical, 12)

            Divider()

            // Agent list
            ScrollView {
                VStack(alignment: .leading, spacing: 4) {
                    if availableCommands.isEmpty {
                        EmptyAgentListView()
                    } else {
                        ForEach(availableCommands, id: \.name) { command in
                            AgentRowView(
                                command: command,
                                isSelected: command.name == selectedAgent?.name,
                                onSelect: { onSelect(command) }
                            )
                        }
                    }
                }
                .padding(.vertical, 8)
            }
            .frame(width: 300, maxHeight: 400)
        }
        .background(Material.ultraThin)
    }
}

struct AgentRowView: View {
    let command: ACP.Client.AvailableCommand
    let isSelected: Bool
    let onSelect: () -> Void

    var body: some View {
        Button(action: onSelect) {
            HStack(spacing: 12) {
                // Icon
                Image(systemName: iconFor(command))
                    .font(.system(size: 18))
                    .foregroundColor(OATheme.Colors.accent)
                    .frame(width: 24, height: 24)

                // Name and description
                VStack(alignment: .leading, spacing: 2) {
                    Text(command.name)
                        .font(OAFonts.mono(size: 13, weight: .medium))
                        .foregroundColor(OATheme.Colors.textPrimary)

                    Text(command.description)
                        .font(OAFonts.mono(size: 11))
                        .foregroundColor(OATheme.Colors.textSecondary)
                        .lineLimit(2)
                }

                Spacer()

                // Checkmark if selected
                if isSelected {
                    Image(systemName: "checkmark")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundColor(OATheme.Colors.accent)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .background(isSelected ? OATheme.Colors.accent.opacity(0.1) : Color.clear)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
    private func iconFor(_ cmd: ACP.Client.AvailableCommand) -> String {
        let n = cmd.name.lowercased()
        if n.contains("claude") { return "sparkles" }
        if n.contains("codex") { return "chevron.left.slash.chevron.right" }
        return "cpu"
    }
}

struct EmptyAgentListView: View {
    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 32))
                .foregroundColor(OATheme.Colors.textSecondary)

            Text("No agents available")
                .font(OAFonts.mono(size: 13))
                .foregroundColor(OATheme.Colors.textPrimary)

            Text("Connect to desktop bridge to see available agents")
                .font(OAFonts.mono(size: 11))
                .foregroundColor(OATheme.Colors.textSecondary)
                .multilineTextAlignment(.center)
        }
        .padding(32)
        .frame(maxWidth: .infinity)
    }
}
```

### Integration with Toolbar
```swift
// In ChatMacOSView or main view
.toolbar {
    ToolbarItem(placement: .navigation) {
        AgentSelectorView()
    }

    ToolbarItem(placement: .automatic) {
        Button(action: openSettings) {
            Image(systemName: "gear")
        }
    }
}
```

### BridgeManager Extension
Add `selectedAgent` property:
```swift
@Published var selectedAgent: ACP.Client.AvailableCommand?
```

### ACP.Client.AvailableCommand Model
This is already defined in `OpenAgentsCore/AgentClientProtocol/client.swift` as:
```swift
public struct AvailableCommand: Codable, Equatable {
    public var name: String
    public var description: String
    public var input: AvailableCommandInput?
}
```

### Sending with Selected Agent
When sending a message, use selected agent:
```swift
func sendMessage(_ text: String) async {
    guard let sessionId = currentSessionId else { return }

    let command = selectedAgent?.name ?? "default"

    // Send via shared dispatcher; set desired mode/command if applicable
    bridgeManager.sendPrompt(text: text)
}
```

## Dependencies
- Issue #2 (BridgeManager chat state - provides `availableCommands`)

## Blocked By
- Issue #2

## Blocks
None - Enhancement feature

## Estimated Complexity
Medium (3-4 hours)

## Testing Requirements
- [ ] Build succeeds on macOS target
- [ ] Toolbar button shows current agent
- [ ] Popover opens on click
- [ ] Selecting agent updates state
- [ ] Selected agent persists in session
- [ ] Empty state shows when no agents available
- [ ] Keyboard shortcut ⌘K works
- [ ] Agent selection affects message sending

## References
- iOS agent selector: `ios/OpenAgents/Views/NewChatView.swift` (header dropdown)
- ACP commands: Check `AgentClientProtocol/` for command definitions
- ChatGPT desktop app for UX reference
