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
        .accessibilityLabel("Choose Agent")
        .popover(isPresented: $showPopover, arrowEdge: .bottom) {
            AgentSelectorPopover(
                selected: $bridge.selectedAgent,
                commands: bridge.availableCommands,
                onSelect: { cmd in
                    showPopover = false
                    bridge.selectedAgent = cmd
                    let mode = modeForCommand(cmd)
                    if bridge.currentSessionId == nil { bridge.startNewSession(desiredMode: mode) }
                    else { bridge.setSessionMode(mode) }
                }
            )
            .frame(width: 320, height: 240)
        }
        .keyboardShortcut("k", modifiers: .command)
        .onAppear { initializeSelectionIfNeeded() }
        .onChange(of: bridge.availableCommands) { _, _ in initializeSelectionIfNeeded() }
    }

    private var selectedLabel: String {
        if let a = bridge.selectedAgent { return a.name }
        switch bridge.currentMode { case .claude_code: return "Claude Code"; case .codex: return "Codex"; case .orchestrator: return "Orchestrator"; default: return "Default Agent" }
    }

    private var agentIcon: String {
        if let a = bridge.selectedAgent {
            let n = a.name.lowercased()
            if n.contains("claude") { return "sparkles" }
            if n.contains("codex") { return "chevron.left.slash.chevron.right" }
        }
        switch bridge.currentMode { case .claude_code: return "sparkles"; case .codex: return "chevron.left.slash.chevron.right"; case .orchestrator: return "gear"; default: return "cpu" }
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

    private func modeForCommand(_ cmd: ACP.Client.AvailableCommand) -> ACPSessionModeId {
        let n = cmd.name.lowercased()
        if n.contains("claude") { return .claude_code }
        if n.contains("codex") { return .codex }
        return .default_mode
    }
}

private struct AgentSelectorPopover: View {
    @Binding var selected: ACP.Client.AvailableCommand?
    let commands: [ACP.Client.AvailableCommand]
    let onSelect: (ACP.Client.AvailableCommand) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Select Agent")
                .font(OAFonts.mono(.body, 12))
                .foregroundStyle(OATheme.Colors.textSecondary)
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 4) {
                    if commands.isEmpty {
                        Text("No agents available")
                            .font(OAFonts.mono(.body, 12))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                            .padding()
                    } else {
                        ForEach(commands, id: \.name) { cmd in
                            Button(action: { onSelect(cmd) }) {
                                HStack(spacing: 10) {
                                    Image(systemName: icon(for: cmd))
                                        .foregroundStyle(OATheme.Colors.accent)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(cmd.name)
                                            .font(OAFonts.mono(.body, 12))
                                            .foregroundStyle(OATheme.Colors.textPrimary)
                                        if !cmd.description.isEmpty {
                                            Text(cmd.description)
                                                .font(OAFonts.mono(.caption, 11))
                                                .foregroundStyle(OATheme.Colors.textSecondary)
                                                .lineLimit(2)
                                        }
                                    }
                                    Spacer()
                                    if selected?.name == cmd.name {
                                        Image(systemName: "checkmark")
                                            .foregroundStyle(OATheme.Colors.accent)
                                            .font(.system(size: 12, weight: .semibold))
                                    }
                                }
                                .padding(.horizontal, 12)
                                .padding(.vertical, 8)
                                .background((selected?.name == cmd.name) ? OATheme.Colors.accent.opacity(0.1) : Color.clear)
                                .cornerRadius(6)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
                .padding(.vertical, 8)
            }
            .padding(.horizontal, 4)
        }
        .background(OATheme.Colors.sidebarBackground)
    }
    private func icon(for cmd: ACP.Client.AvailableCommand) -> String {
        let n = cmd.name.lowercased()
        if n.contains("claude") { return "sparkles" }
        if n.contains("codex") { return "chevron.left.slash.chevron.right" }
        return "cpu"
    }
}
#endif
