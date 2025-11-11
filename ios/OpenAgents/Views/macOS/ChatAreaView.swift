import SwiftUI
import OpenAgentsCore

#if os(macOS)
import AppKit
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
                    HStack { // center the message column
                        Spacer()
                        LazyVStack(alignment: .leading, spacing: 14) {
                            if bridge.updates.isEmpty {
                                // No center placeholder; keep area empty and retain bottom anchor for scroll
                                Color.clear.frame(height: 1).id("bottom")
                            } else {
                                ForEach(Array(bridge.updates.enumerated()), id: \.offset) { (idx, note) in
                                    ChatUpdateRow(note: note, index: idx)
                                        .id(idx)
                                        .padding(.horizontal, 16)
                                }
                                Color.clear.frame(height: 1).id("bottom")
                            }
                        }
                        .frame(maxWidth: 820)
                        Spacer()
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

        bridge.log("ui", "composer send pressed len=\(trimmed.count) hasSession=\(bridge.currentSessionId != nil)")
        bridge.sendPrompt(text: trimmed)
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
    let index: Int
    @EnvironmentObject private var bridge: BridgeManager
    @State private var showDetail = false
    @State private var hovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            switch note.update {
            case .userMessageChunk(let chunk):
                let text = extractText(from: chunk)
                VStack(alignment: .leading, spacing: 6) {
                    bubble(text: text, isUser: true)
                    CopyMarkdownRow(markdown: text, alignRight: true, visible: hovering)
                }
            case .agentMessageChunk(let chunk):
                let text = extractText(from: chunk)
                let isFromOrchestrator = checkIsFromOrchestrator(chunk)
                let _ = print("[ChatUpdateRow] agentMessageChunk idx=\(index): isOrch=\(isFromOrchestrator) textLen=\(text.count) text=\(text.prefix(50))...")

                // Check if this should be aggregated with previous chunk
                let shouldAggregate = shouldAggregateWithPrevious(index: index)

                if shouldAggregate {
                    // Skip this chunk - it will be aggregated into the previous one
                    let _ = print("[ChatUpdateRow] Skipping aggregated chunk at idx=\(index)")
                    EmptyView()
                } else {
                    // Render this chunk (possibly aggregating following chunks)
                    VStack(alignment: .leading, spacing: 6) {
                        if isFromOrchestrator {
                            // Foundation Models orchestrator response - render normally
                            bubble(text: text, isUser: false)
                                .modifier(FadeInOnAppear(duration: 0.10))
                        } else {
                            // Delegated agent response - aggregate with following chunks
                            let aggregatedText = aggregateFollowingChunks(startIndex: index)
                            let taskDesc = inferTaskDescription(from: note)
                            let _ = print("[ChatUpdateRow] Rendering DelegatedAgentCard at idx=\(index) with aggregatedTextLen=\(aggregatedText.count) task=\(taskDesc ?? "nil")")
                            DelegatedAgentCard(
                                text: aggregatedText,
                                provider: inferProvider(from: note),
                                taskDescription: taskDesc
                            )
                            .modifier(FadeInOnAppear(duration: 0.10))
                        }
                        CopyMarkdownRow(markdown: isFromOrchestrator ? text : aggregateFollowingChunks(startIndex: index), alignRight: false, visible: hovering)
                    }
                }
            case .agentThoughtChunk(let chunk):
                bubble(text: extractText(from: chunk), isUser: false, italic: true, secondary: true)
            case .plan(let plan):
                PlanView(plan: plan)
            case .availableCommandsUpdate(let ac):
                Text("Available: \(ac.available_commands.count) commands")
                    .font(OAFonts.mono(.caption, 11))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            case .currentModeUpdate:
                // Hide all mode change indicators
                EmptyView()
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
        // Expand the row to the full column width so hover covers the whole component,
        // not just the bubble, keeping the copy button visible while moving to it.
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
    }

    private func bubble(text: String, isUser: Bool, italic: Bool = false, secondary: Bool = false) -> some View {
        let content = markdownText(text)
            .font(OAFonts.mono(.body, 14))
            .foregroundStyle(secondary ? OATheme.Colors.textSecondary : OATheme.Colors.textPrimary)
            .italic(italic)
            .textSelection(.enabled)
            .lineSpacing(4)
            .padding(isUser ? 12 : 0)
            .background(isUser ? OATheme.Colors.bgQuaternary : Color.clear)
            .cornerRadius(isUser ? 12 : 0)

        return HStack(spacing: 0) {
            if isUser {
                Spacer(minLength: 0)
                content
            } else {
                content
                Spacer(minLength: 0)
            }
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

    private func markdownText(_ text: String) -> Text {
        // Use .inlineOnlyPreservingWhitespace to render inline markdown (bold, italic, code)
        // while preserving ALL whitespace including single newlines.
        //
        // IMPORTANT: .full mode follows standard markdown rules where single newlines
        // are treated as spaces (hard wrapping). Foundation Models generates text with
        // single newlines between sentences/paragraphs, so we need to preserve those.
        let options: AttributedString.MarkdownParsingOptions
        if #available(macOS 14.0, *) {
            options = .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
        } else {
            options = .init(interpretedSyntax: .inlineOnly)
        }

        if let md = try? AttributedString(markdown: text, options: options) {
            return Text(md)
        }
        return Text(text)
    }

    private func mapCall(_ w: ACPToolCallWire) -> ACPToolCall {
        // Map wire args [String: AnyEncodable] → JSONValue for UI rendering
        var jsonObj: [String: JSONValue] = [:]
        if let args = w.arguments {
            for (k, v) in args { jsonObj[k] = v.toJSONValue() }
        }
        return ACPToolCall(id: w.call_id, tool_name: w.name, arguments: .object(jsonObj))
    }

    private func mapResult(_ w: ACPToolCallUpdateWire) -> ACPToolResult {
        let ok: Bool
        switch w.status {
        case .completed: ok = true
        case .error: ok = false
        case .started: ok = false
        }
        return ACPToolResult(call_id: w.call_id, ok: ok)
    }

    private func findResult(for callId: String) -> ACPToolResult? {
        // 1) Completed with explicit output captured
        if let _ = bridge.outputJSONByCallId[callId] { return ACPToolResult(call_id: callId, ok: true) }

        // 2) Heuristic: if an assistant message appears after the last event
        //    referencing this call, treat the call as completed (agent returned
        //    to chat and is waiting for user response).
        // Capture snapshot to avoid concurrent modification
        let updates = bridge.updates

        var lastIdxForCall: Int? = nil
        for (i, note) in updates.enumerated() {
            switch note.update {
            case .toolCall(let w) where w.call_id == callId:
                lastIdxForCall = i
            case .toolCallUpdate(let upd) where upd.call_id == callId:
                lastIdxForCall = i
            default:
                break
            }
        }
        if let idx = lastIdxForCall {
            let sessionId = updates[idx].session_id
            if idx + 1 < updates.count {
                for j in (idx + 1)..<updates.count {
                    let n = updates[j]
                    if n.session_id != sessionId { continue }
                    if case .agentMessageChunk = n.update {
                        return ACPToolResult(call_id: callId, ok: true)
                    }
                }
            }
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

    private func checkIsFromOrchestrator(_ chunk: ACP.Client.ContentChunk) -> Bool {
        guard let sourceValue = chunk._meta?["source"] else { return false }
        if case .string(let sourceStr) = sourceValue.toJSONValue() {
            return sourceStr == "fm_orchestrator"
        }
        return false
    }

    private func inferProvider(from note: ACP.Client.SessionNotificationWire) -> String? {
        // Look backwards in transcript to find provider info
        let sessionId = note.session_id

        // Capture snapshot to avoid concurrent modification
        let updates = bridge.updates

        // Search backwards for mode updates or tool calls
        // Note: For forwarded sub-session chunks, we need to look for delegate.run tool calls
        // with the parent sessionId, and extract provider from arguments
        for i in stride(from: updates.count - 1, through: 0, by: -1) {
            let update = updates[i]

            switch update.update {
            // Check delegate.run tool calls (these are on parent session)
            case .toolCall(let call) where call.name == ToolName.delegate.rawValue && update.session_id == sessionId:
                // Extract provider from arguments
                if let args = call.arguments,
                   let providerArg = args["provider"],
                   case .string(let providerStr) = providerArg.toJSONValue() {
                    if providerStr.lowercased().contains("claude") {
                        return "Claude Code"
                    } else {
                        return "Codex"
                    }
                }
                return "Codex" // Default
            case .toolCall(let call) where call.name == "codex.run" && update.session_id == sessionId:
                return "Codex"
            case .toolCall(let call) where call.name == "claude_code.run" && update.session_id == sessionId:
                return "Claude Code"
            // Check mode updates for ANY session (sub-sessions have their own mode updates)
            case .currentModeUpdate(let mode) where mode.current_mode_id == ACPSessionModeId.codex:
                return "Codex"
            case .currentModeUpdate(let mode) where mode.current_mode_id == ACPSessionModeId.claude_code:
                return "Claude Code"
            default:
                continue
            }
        }
        return nil
    }

    private func inferTaskDescription(from note: ACP.Client.SessionNotificationWire) -> String? {
        // Look backwards to find the delegate.run tool call that started this task
        let sessionId = note.session_id
        let updates = bridge.updates

        for i in stride(from: updates.count - 1, through: 0, by: -1) {
            let update = updates[i]
            guard update.session_id == sessionId else { continue }

            if case .toolCall(let call) = update.update,
               call.name == ToolName.delegate.rawValue,
               let args = call.arguments,
               let userPrompt = args["user_prompt"],
               case .string(let task) = userPrompt.toJSONValue() {
                return task
            }
        }
        return nil
    }

    /// Check if this message chunk should be aggregated with a previous chunk
    /// Returns true if there's ALREADY a card for this session (so skip rendering this chunk)
    private func shouldAggregateWithPrevious(index: Int) -> Bool {
        guard index > 0 else { return false }
        // Capture snapshot to avoid concurrent modification
        let updates = bridge.updates
        guard index < updates.count else { return false }

        let note = updates[index]
        guard case .agentMessageChunk(let chunk) = note.update else { return false }

        // Don't aggregate orchestrator messages
        if checkIsFromOrchestrator(chunk) { return false }

        // Look backwards to see if there's already a delegated agent card for this session
        let sessionId = note.session_id
        for i in stride(from: index - 1, through: 0, by: -1) {
            let prevNote = updates[i]
            guard case .agentMessageChunk(let prevChunk) = prevNote.update else { continue }

            // If we hit an orchestrator message, stop looking back
            if checkIsFromOrchestrator(prevChunk) { break }

            // If we find a previous chunk from the same session, this should aggregate with it
            if prevNote.session_id == sessionId {
                return true
            }
        }

        return false
    }

    /// Aggregate this chunk with ALL following delegated agent chunks from the same session
    /// (even if non-consecutive due to interleaved chunks from other sessions)
    private func aggregateFollowingChunks(startIndex: Int) -> String {
        // Capture snapshot to avoid concurrent modification during iteration
        let updates = bridge.updates

        guard startIndex < updates.count else { return "" }
        let note = updates[startIndex]
        let sessionId = note.session_id
        guard case .agentMessageChunk(let firstChunk) = note.update else {
            return ""
        }

        var texts: [String] = [extractText(from: firstChunk)]

        // Collect ALL following chunks from the same session (even if non-consecutive)
        for i in (startIndex + 1)..<updates.count {
            let nextNote = updates[i]

            // Skip chunks from other sessions
            guard nextNote.session_id == sessionId else { continue }

            guard case .agentMessageChunk(let nextChunk) = nextNote.update else { continue }

            // Stop if we hit an orchestrator message from this session
            if checkIsFromOrchestrator(nextChunk) { break }

            texts.append(extractText(from: nextChunk))
        }

        return texts.joined(separator: "\n\n")
    }
}

// MARK: - Delegated Agent Card
private struct DelegatedAgentCard: View {
    let text: String
    let provider: String?
    let taskDescription: String?

    var body: some View {
        let currentStep = extractCurrentStep(from: text)
        let _ = print("[DelegatedAgentCard] Rendering: provider=\(provider ?? "nil") currentStep=\(currentStep)")

        HStack(spacing: 12) {
            // Provider icon
            if provider != nil {
                Image(systemName: "arrow.right.circle.fill")
                    .foregroundStyle(OATheme.Colors.accent)
                    .font(.system(size: 16))
            }

            // Current reasoning step
            VStack(alignment: .leading, spacing: 2) {
                if let provider = provider {
                    Text(provider)
                        .font(OAFonts.mono(.caption, 11))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }
                Text(currentStep)
                    .font(OAFonts.mono(.body, 13))
                    .foregroundStyle(OATheme.Colors.textPrimary)
            }

            Spacer()

            // Subtle activity indicator
            ProgressView()
                .scaleEffect(0.6)
                .opacity(0.6)
        }
        .padding(12)
        .background(OATheme.Colors.bgTertiary.opacity(0.5))
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(OATheme.Colors.accent.opacity(0.2), lineWidth: 1)
        )
    }

    /// Extract the latest/current reasoning step from the text
    private func extractCurrentStep(from text: String) -> String {
        // Split by lines and find the last non-empty line
        let lines = text.split(separator: "\n", omittingEmptySubsequences: true)

        // Get the last line
        guard let lastLine = lines.last else {
            return taskDescription ?? "Processing..."
        }

        var line = String(lastLine).trimmingCharacters(in: .whitespaces)

        // Remove markdown bold markers if present
        line = line.replacingOccurrences(of: "**", with: "")

        // If it's too long, truncate
        if line.count > 100 {
            line = String(line.prefix(97)) + "..."
        }

        return line.isEmpty ? (taskDescription ?? "Processing...") : line
    }
}

// MARK: - Mode Transition View
private struct ModeTransitionView: View {
    let mode: ACPSessionModeId

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "arrow.right.circle")
                .foregroundStyle(OATheme.Colors.accent)
                .font(.system(size: 14))

            Text("Switched to \(modeName)")
                .font(OAFonts.mono(.caption, 12))
                .foregroundStyle(OATheme.Colors.textSecondary)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(OATheme.Colors.bgTertiary.opacity(0.3))
        .cornerRadius(6)
    }

    private var modeName: String {
        switch mode {
        case .codex:
            return "Codex"
        case .claude_code:
            return "Claude Code"
        default:
            return mode.rawValue
        }
    }
}

// MARK: - Small action row under assistant messages
private struct CopyMarkdownRow: View {
    let markdown: String
    var alignRight: Bool = false
    var visible: Bool = true
    @State private var didCopy = false

    var body: some View {
        HStack(spacing: 8) {
            if alignRight { Spacer(minLength: 0) }
            Button(action: copy) {
                Image(systemName: didCopy ? "checkmark" : "doc.on.doc")
                    .font(.system(size: 13, weight: .semibold))
                    // During feedback, keep the icon white regardless of theme.
                    .foregroundStyle(didCopy ? OATheme.Colors.textPrimary : OATheme.Colors.textSecondary)
            }
            .buttonStyle(.plain)
            .help(didCopy ? "Copied" : "Copy as Markdown")
            if !alignRight { Spacer(minLength: 0) }
        }
        // Keep space reserved to avoid layout shifting; only hide visually.
        // Also, keep the row visible while showing the copy confirmation (didCopy).
        .opacity((visible || didCopy) ? 1 : 0)
        .allowsHitTesting(visible)
        .foregroundStyle(OATheme.Colors.textSecondary)
        .padding(.top, 2)
    }

    private func copy() {
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(markdown, forType: .string)
        withAnimation(.easeInOut(duration: 0.15)) { didCopy = true }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) {
            withAnimation(.easeInOut(duration: 0.15)) { didCopy = false }
        }
    }
}

// Simple fade-in for streaming tokens
private struct FadeInOnAppear: ViewModifier {
    @State private var visible = false
    var duration: Double = 0.12
    func body(content: Content) -> some View {
        content
            .opacity(visible ? 1 : 0)
            .onAppear { withAnimation(.easeIn(duration: duration)) { visible = true } }
    }
}

private struct FadeOnChange<Value: Equatable>: ViewModifier {
    let value: Value
    var from: Double = 0.3
    var to: Double = 1.0
    var duration: Double = 0.12
    @State private var alpha: Double = 1.0
    func body(content: Content) -> some View {
        content
            .opacity(alpha)
            .onAppear { alpha = to }
            .onChange(of: value) { _, _ in
                alpha = from
                withAnimation(.easeIn(duration: duration)) { alpha = to }
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
