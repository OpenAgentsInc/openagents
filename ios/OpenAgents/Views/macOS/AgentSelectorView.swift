import SwiftUI
import OpenAgentsCore

#if os(macOS)
struct AgentSelectorView: View {
    @EnvironmentObject private var bridge: BridgeManager
    @State private var showPopover = false

    var body: some View {
        Button(action: { showPopover.toggle() }) {
            HStack(spacing: 6) {
                Image(systemName: agentIcon)
                    .font(.system(size: 14))
                Text(selectedLabel)
                    .font(OAFonts.mono(.body, 12))
                    .foregroundStyle(OATheme.Colors.textPrimary)
                    .lineLimit(1)
                Image(systemName: "chevron.down")
                    .font(.system(size: 10))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(OATheme.Colors.background)
            )
        }
        .buttonStyle(.plain)
        .popover(isPresented: $showPopover, arrowEdge: .bottom) {
            AgentSelectorPopover(
                available: availableOptions,
                currentMode: bridge.currentMode,
                onSelectMode: { mode in
                    showPopover = false
                    // Update selection state for UI persistence
                    bridge.selectedAgent = bridge.availableCommands.first { cmd in
                        let n = cmd.name.lowercased()
                        return (mode == .claude_code && n.contains("claude")) || (mode == .codex && n.contains("codex"))
                    }
                    if bridge.currentSessionId == nil { bridge.startNewSession(desiredMode: mode) }
                    else { bridge.setSessionMode(mode) }
                }
            )
            .frame(width: 280, height: 200)
        }
        .keyboardShortcut("k", modifiers: .command)
        .onAppear { initializeSelectionIfNeeded() }
        .onChange(of: bridge.availableCommands) { _, _ in initializeSelectionIfNeeded() }
    }

    private var availableOptions: [ACPSessionModeId] {
        // Map availableCommands to modes when possible; fallback to common modes
        let names = bridge.availableCommands.map { $0.name.lowercased() }
        var modes: [ACPSessionModeId] = []
        if names.contains(where: { $0.contains("claude") }) { modes.append(.claude_code) }
        if names.contains(where: { $0.contains("codex") }) { modes.append(.codex) }
        if modes.isEmpty { modes = [.claude_code, .codex] }
        return modes
    }

    private var selectedLabel: String {
        switch bridge.currentMode {
        case .claude_code: return "Claude Code"
        case .codex: return "Codex"
        case .orchestrator: return "Orchestrator"
        default: return "Default Agent"
        }
    }

    private var agentIcon: String {
        switch bridge.currentMode {
        case .claude_code: return "sparkles"
        case .codex: return "chevron.left.slash.chevron.right"
        case .orchestrator: return "gear"
        default: return "cpu"
        }
    }

    private func initializeSelectionIfNeeded() {
        // If there is no explicit selectedAgent, seed based on currentMode or first available
        if bridge.selectedAgent == nil {
            if let seeded = bridge.availableCommands.first(where: { cmd in
                let n = cmd.name.lowercased()
                return (bridge.currentMode == .claude_code && n.contains("claude")) || (bridge.currentMode == .codex && n.contains("codex"))
            }) {
                bridge.selectedAgent = seeded
            } else {
                bridge.selectedAgent = bridge.availableCommands.first
            }
        }
    }
}

private struct AgentSelectorPopover: View {
    let available: [ACPSessionModeId]
    let currentMode: ACPSessionModeId
    let onSelectMode: (ACPSessionModeId) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Select Agent")
                .font(OAFonts.mono(.body, 12))
                .foregroundStyle(OATheme.Colors.textSecondary)
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(available, id: \.rawValue) { mode in
                        Button(action: { onSelectMode(mode) }) {
                            HStack(spacing: 10) {
                                Image(systemName: icon(for: mode))
                                    .foregroundStyle(OATheme.Colors.accent)
                                Text(label(for: mode))
                                    .font(OAFonts.mono(.body, 12))
                                    .foregroundStyle(OATheme.Colors.textPrimary)
                                Spacer()
                                if mode == currentMode {
                                    Image(systemName: "checkmark")
                                        .foregroundStyle(OATheme.Colors.accent)
                                        .font(.system(size: 12, weight: .semibold))
                                }
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 8)
                            .background(mode == currentMode ? OATheme.Colors.accent.opacity(0.1) : Color.clear)
                            .cornerRadius(6)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.vertical, 8)
            }
            .padding(.horizontal, 4)
        }
        .background(OATheme.Colors.sidebarBackground)
    }

    private func label(for mode: ACPSessionModeId) -> String {
        switch mode {
        case .claude_code: return "Claude Code"
        case .codex: return "Codex"
        case .orchestrator: return "Orchestrator"
        default: return "Default Agent"
        }
    }
    private func icon(for mode: ACPSessionModeId) -> String {
        switch mode {
        case .claude_code: return "sparkles"
        case .codex: return "chevron.left.slash.chevron.right"
        case .orchestrator: return "gear"
        default: return "cpu"
        }
    }
}
#endif
