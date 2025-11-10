import SwiftUI
import OpenAgentsCore

#if os(macOS)
struct ChatAreaView: View {
    @EnvironmentObject private var bridge: BridgeManager
    @State private var messageText: String = ""
    @State private var isSending: Bool = false
    @State private var isAgentProcessing: Bool = false
    @State private var scrollProxy: ScrollViewProxy? = nil

    var body: some View {
        VStack(spacing: 0) {
            // Messages
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 14) {
                        if bridge.updates.isEmpty {
                            EmptyStateView()
                                .frame(maxWidth: .infinity, maxHeight: .infinity)
                                .padding(.vertical, 80)
                        } else {
                            ForEach(Array(bridge.updates.enumerated()), id: \.offset) { (idx, note) in
                                ChatUpdateRow(note: note)
                                    .id(idx)
                                    .padding(.horizontal, 16)
                            }
                            Color.clear.frame(height: 1).id("bottom")
                        }
                    }
                    .padding(.top, 16)
                }
                .background(OATheme.Colors.background)
                .onAppear { scrollProxy = proxy; scrollToBottom(animated: false) }
                .onChange(of: bridge.updates.count) { _, _ in scrollToBottom(animated: true) }
            }

            // Composer at bottom, centered to 768 width
            HStack {
                Spacer()
                ComposerMac(text: $messageText, isSending: isSending) { send() }
                .frame(width: 768)
                Spacer()
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(OATheme.Colors.background)
        }
        .onChange(of: bridge.updates.count) { _, _ in
            guard let last = bridge.updates.last else {
                isAgentProcessing = false
                return
            }
            switch last.update {
            case .userMessageChunk:
                isAgentProcessing = true
            case .toolCallUpdate(let upd):
                isAgentProcessing = upd.status != .completed && upd.status != .error
            case .agentMessageChunk, .plan, .availableCommandsUpdate, .currentModeUpdate, .toolCall, .agentThoughtChunk:
                isAgentProcessing = false
            }
            // Generate a title if needed when new content arrives
            bridge.generateConversationTitleIfNeeded()
        }
    }

    private func send() {
        let trimmed = messageText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        isSending = true
        let old = messageText
        messageText = ""
        // Respect the selected agent/mode when sending first prompt
        let desired = bridge.preferredModeForSend()
        bridge.log("ui", "composer send pressed len=\(trimmed.count) desired=\(desired?.rawValue ?? "nil") hasSession=\(bridge.currentSessionId != nil)")
        bridge.sendPrompt(text: trimmed, desiredMode: desired)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
            isSending = false
            scrollToBottom(animated: true)
        }
        if bridge.dispatcher == nil { messageText = old; isSending = false }
    }

    private func scrollToBottom(animated: Bool) {
        guard let proxy = scrollProxy else { return }
        if animated { withAnimation { proxy.scrollTo("bottom", anchor: .bottom) } }
        else { proxy.scrollTo("bottom", anchor: .bottom) }
    }
}

// MARK: - Row renderer for ACP updates (macOS)
private struct ChatUpdateRow: View {
    let note: ACP.Client.SessionNotificationWire
    @EnvironmentObject private var bridge: BridgeManager
    @State private var showDetail = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            switch note.update {
            case .userMessageChunk(let chunk):
                bubble(text: extractText(from: chunk), isUser: true)
            case .agentMessageChunk(let chunk):
                bubble(text: extractText(from: chunk), isUser: false)
            case .agentThoughtChunk(let chunk):
                bubble(text: extractText(from: chunk), isUser: false, italic: true, secondary: true)
            case .plan(let plan):
                PlanView(plan: plan)
            case .availableCommandsUpdate(let ac):
                Text("Available: \(ac.available_commands.count) commands")
                    .font(OAFonts.mono(.caption, 11))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            case .currentModeUpdate(let cur):
                Text("Mode: \(cur.current_mode_id.rawValue)")
                    .font(OAFonts.mono(.caption, 11))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            case .toolCall(let callWire):
                ToolCallView(call: mapCall(callWire), result: findResult(for: callWire.call_id))
                    .contentShape(Rectangle())
                    .onTapGesture { bridge.selectedToolCallId = callWire.call_id }
                    .overlay(selectionOverlay(for: callWire.call_id))
            case .toolCallUpdate(let upd):
                // Re-render with current status/result
                if let name = bridge.toolCallNames[upd.call_id] {
                    let call = ACPToolCall(id: upd.call_id, tool_name: name, arguments: .object([:]))
                    ToolCallView(call: call, result: mapResult(upd))
                        .contentShape(Rectangle())
                        .onTapGesture { bridge.selectedToolCallId = upd.call_id }
                        .overlay(selectionOverlay(for: upd.call_id))
                } else {
                    HStack(spacing: 8) {
                        ProgressView().scaleEffect(0.6)
                        Text("call \(upd.call_id.prefix(8))…: \(upd.status.rawValue)")
                            .font(OAFonts.mono(.caption, 11))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                    }
                    .contentShape(Rectangle())
                    .onTapGesture { bridge.selectedToolCallId = upd.call_id }
                    .overlay(selectionOverlay(for: upd.call_id))
                }
            }
        }
    }

    private func bubble(text: String, isUser: Bool, italic: Bool = false, secondary: Bool = false) -> some View {
        return HStack {
            if isUser { Spacer(minLength: 40) }
            Text(text)
                .font(OAFonts.mono(.body, 14))
                .foregroundStyle(secondary ? OATheme.Colors.textSecondary : OATheme.Colors.textPrimary)
                .italic(italic)
                .textSelection(.enabled)
                .padding(12)
                .background(isUser ? OATheme.Colors.accent.opacity(0.1) : OATheme.Colors.sidebarBackground)
                .cornerRadius(12)
                .frame(maxWidth: 820, alignment: .leading)
            if !isUser { Spacer(minLength: 40) }
        }
    }

    private func extractText(from chunk: ACP.Client.ContentChunk) -> String {
        switch chunk.content {
        case .text(let txt): return txt.text
        case .resource_link(let link): return "Resource: \(link.uri)"
        case .resource(let emb):
            switch emb.resource {
            case .text(let t): return "Resource: \(t.uri)"
            case .blob(let b): return "Resource: \(b.uri)"
            }
        case .image: return "[image]"
        case .audio: return "[audio]"
        }
    }

    private func mapCall(_ w: ACPToolCallWire) -> ACPToolCall {
        // Map to ACPToolCall, ignoring structured args for now (keeps UI responsive)
        return ACPToolCall(id: w.call_id, tool_name: w.name, arguments: .object([:]))
    }

    private func mapResult(_ w: ACPToolCallUpdateWire) -> ACPToolResult {
        let ok = (w.status != .error) && (w.error == nil)
        return ACPToolResult(call_id: w.call_id, ok: ok)
    }

    private func findResult(for callId: String) -> ACPToolResult? {
        // Look up a completed result from bridge state if available
        if let _ = bridge.outputJSONByCallId[callId] {
            return ACPToolResult(call_id: callId, ok: true)
        }
        return nil
    }

    private func selectionOverlay(for callId: String) -> some View {
        Group {
            if bridge.selectedToolCallId == callId {
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .stroke(OATheme.Colors.accent.opacity(0.6), lineWidth: 1)
            } else { EmptyView() }
        }
    }
}

private struct EmptyStateView: View {
    var body: some View {
        VStack(spacing: 14) {
            Image(systemName: "bubble.left.and.bubble.right")
                .font(.system(size: 54))
                .foregroundStyle(OATheme.Colors.textSecondary)
            Text("Start a conversation")
                .font(OAFonts.mono(.title3, 18))
                .foregroundStyle(OATheme.Colors.textPrimary)
            Text("Ask OpenAgents to help with coding, debugging, or research")
                .font(OAFonts.mono(.body, 13))
                .foregroundStyle(OATheme.Colors.textSecondary)
                .multilineTextAlignment(.center)
        }
        .padding(40)
        .frame(maxWidth: .infinity)
    }
}
#endif
