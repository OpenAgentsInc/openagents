import SwiftUI
import Combine
import OpenAgentsCore

#if os(iOS)
/// Fresh screen showcasing the new top toolbar header for iOS 26+.
/// This screen intentionally does not auto-load a conversation thread.
struct ChatHomeView: View {
    @Environment(\.colorScheme) private var colorScheme
    @State private var isMenuPresented = false
    @EnvironmentObject private var bridge: BridgeManager
    // Transient "working" indicator shown between orchestration RPC start and the first streamed update
    @State private var isWorking = false
    @State private var workingStartedAt: Date? = nil
    @State private var workingSeconds: Int = 0
    @State private var workingTimer = Timer.publish(every: 1.0, on: .main, in: .common).autoconnect()
    // Raw JSON inspector state
    @State private var isJSONSheetPresented = false
    @State private var selectedJSON: String? = nil

    var body: some View {
        if Features.simplifiedIOSUI {
            SimplifiedNavigationView()
        } else {
            fullChatView
        }
    }

    private var fullChatView: some View {
        NavigationStack {
            // Main content placeholder
            VStack(spacing: 12) {
                // Banner with connection + updates info
                HStack(spacing: 8) {
                    switch bridge.status {
                    case .connected(let host, let port):
                        Text("ACP OK (0.2.2)")
                            .font(.headline)
                        Text("· \(host):\(port, format: .number.grouping(.never))")
                            .foregroundStyle(.secondary)
                    case .handshaking(let host, let port):
                        Text("Handshaking… \(host):\(port, format: .number.grouping(.never))")
                    case .connecting(let host, let port):
                        Text("Connecting… \(host):\(port, format: .number.grouping(.never))")
                    case .discovering:
                        Text("Discovering desktop…")
                    case .advertising(let port):
                        Text("Advertising on :\(port, format: .number.grouping(.never))")
                    case .idle:
                        Text("Idle")
                    case .error(let msg):
                        Text("Error: \(msg)")
                            .foregroundStyle(.red)
                    }
                    Spacer()
                    Text("updates: \(bridge.updates.count)")
                        .foregroundStyle(.secondary)
                        .font(.subheadline)
                }
                .padding(.horizontal)

                // Show a brief top status while orchestration is starting (until first update arrives)
                if isWorking {
                    HStack(spacing: 8) {
                        ProgressView().progressViewStyle(.circular)
                            .tint(colorScheme == .dark ? .white : OATheme.Colors.textSecondary)
                        Text("Working (\(workingSeconds)s)")
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        Spacer()
                    }
                    .padding(.horizontal)
                }

                // Sticky plan header: show latest plan once, with per-step status icons
                if let (plan, stepStatuses) = latestPlanFromUpdates(bridge.updates) {
                    PlanStateView(state: plan, stepStatuses: stepStatuses)
                        .padding(.horizontal)
                }

                // Inline status for FM analysis while summary is being prepared
                if let fmStatus = fmAnalysisStatus(bridge.updates), fmStatus.inProgress {
                    HStack(spacing: 8) {
                        ProgressView().progressViewStyle(.circular)
                            .tint(colorScheme == .dark ? .white : OATheme.Colors.textSecondary)
                        Text("Analyzing intent…")
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        if let pct = fmStatus.progressPercent {
                            Text(pct)
                                .foregroundStyle(OATheme.Colors.textTertiary)
                        }
                        Spacer()
                    }
                    .padding(.horizontal)
                }

                // Streamed ACP updates as a simple list (excluding plan updates; shown in header)
                let visibleIndices = bridge.updates.indices.filter { idx in
                    if case .plan = bridge.updates[idx].update { return false } else { return true }
                }
                List(visibleIndices, id: \.self) { idx in
                    let note = bridge.updates[idx]
                    UpdateRow(note: note) { tapped in
                        // Prefer showing the latest `output` JSON for this call_id if available
                        if let callId = callId(from: tapped), let out = bridge.outputJSONByCallId[callId] {
                            selectedJSON = out
                        } else if let json = toPrettyJSON(tapped) {
                            selectedJSON = json
                        } else {
                            selectedJSON = nil
                        }
                        isJSONSheetPresented = selectedJSON != nil
                    }
                }
                .listStyle(.plain)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .contentShape(Rectangle())
            .navigationTitle("")
            .toolbarTitleDisplayMode(.inline)
            .toolbar {
                ChatHeaderToolbar(
                    title: "Home",
                    onToggleMenu: { isMenuPresented.toggle() },
                    onNewChat: { /* hook up compose/present flow here */ }
                )
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Start") { startExploreFlow() }
                        .buttonStyle(.glass)
                        .tint(colorScheme == .dark ? .white : OATheme.Colors.accent)
                        .foregroundStyle(colorScheme == .dark ? Color.white : OATheme.Colors.textPrimary)
                        .accessibilityLabel("Start Workspace Exploration")
                }
            }
            // iOS 26+ only: let the system render the Liquid Glass toolbar background
            .sheet(isPresented: $isMenuPresented) {
                MenuSheet()
            }
            // Timer to update the "Working (Ns)" indicator
            .onReceive(workingTimer) { _ in
                if isWorking, let t0 = workingStartedAt {
                    workingSeconds = max(0, Int(Date().timeIntervalSince(t0).rounded()))
                }
            }
            // Hide working indicator as soon as the first streamed update arrives
            .onChange(of: bridge.updates.count) { newCount in
                if isWorking && newCount > 0 { isWorking = false }
            }
            // Present raw JSON inspector when available
            .sheet(isPresented: $isJSONSheetPresented) {
                JSONInspectorView(json: selectedJSON ?? "")
            }
        }
    }

    private func startExploreFlow() {
        // Phase 2: Start workspace exploration using on-device FM orchestrator
        Task { @MainActor in
            // Reset stream so user sees only new events for this flow
            bridge.updates.removeAll()
            // Start working indicator until the first update arrives
            isWorking = true
            workingStartedAt = Date()
            workingSeconds = 0

            // IMPORTANT: The orchestrator runs on macOS, not iOS!
            // We need to send the macOS workspace path, not an iOS sandbox path.
            // Default to the user's home directory on macOS.
            let workspacePath = ProcessInfo.processInfo.environment["TEST_WORKSPACE_ROOT"] ?? "/Users/christopherdavid/code/openagents"

            // Goals for exploration - triggers session history analysis
            let goals = [
                "Find recent conversations about this project",
                "Identify most frequently modified files from conversation history"
            ]

            // Start orchestration
            OpenAgentsLog.ui.info("ChatHome Starting workspace exploration: \(workspacePath, privacy: .private)")
            bridge.orchestrateExploreStart(root: workspacePath, goals: goals) { response in
                if let response = response {
                    OpenAgentsLog.ui.info("ChatHome Orchestration started: \(response.plan_id)")
                } else {
                    OpenAgentsLog.ui.error("ChatHome Orchestration failed to start")
                }
            }
        }
    }
}

// MARK: - JSON helpers and inspector
private func toPrettyJSON(_ note: ACP.Client.SessionNotificationWire) -> String? {
    let enc = JSONEncoder()
    enc.outputFormatting = [.prettyPrinted, .sortedKeys]
    guard let data = try? enc.encode(note) else { return nil }
    return String(data: data, encoding: .utf8)
}

private struct JSONInspectorView: View {
    let json: String
    var body: some View {
        NavigationStack {
            ScrollView {
                Text(json)
                    .font(.system(.footnote, design: .monospaced))
                    .textSelection(.enabled)
                    .padding()
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .background(OATheme.Colors.background)
            .navigationTitle("Tool Call JSON")
            .toolbarTitleDisplayMode(.inline)
        }
    }
}

private func callId(from note: ACP.Client.SessionNotificationWire) -> String? {
    switch note.update {
    case .toolCall(let c): return c.call_id
    case .toolCallUpdate(let u): return u.call_id
    default: return nil
    }
}

private struct MenuSheet: View {
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                Section("Navigation") {
                    Label("Home", systemImage: "house")
                    Label("Recent", systemImage: "clock")
                    Label("Settings", systemImage: "gear")
                }
            }
            .navigationTitle("Menu")
            .toolbarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done", action: { dismiss() })
                        .buttonStyle(.glass)
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}

private struct UpdateRow: View {
    let note: ACP.Client.SessionNotificationWire
    var onInspect: ((ACP.Client.SessionNotificationWire) -> Void)? = nil
    @EnvironmentObject private var bridge: BridgeManager
    @Environment(\.colorScheme) private var colorScheme
    var body: some View {
        switch note.update {
        case .toolCall(let call):
            HStack {
                Image(systemName: "hammer")
                Text(call.name)
                Spacer()
                Text("call_id: \(call.call_id.prefix(8))…")
                    .foregroundStyle(.secondary)
                    .font(.footnote)
            }
            .contentShape(Rectangle())
            .onTapGesture { onInspect?(note) }
        case .toolCallUpdate(let upd):
            HStack(alignment: .firstTextBaseline) {
                let hasOutput = bridge.outputJSONByCallId[upd.call_id] != nil
                // Extract numeric progress if available
                let progressValue: Double? = {
                    if let meta = upd._meta, let p = meta["progress"]?.toJSONValue() {
                        switch p {
                        case .number(let d): return d
                        case .string(let s):
                            if s.hasSuffix("%"), let v = Double(s.dropLast()) { return v / 100.0 }
                            if let v = Double(s) { return v }
                            return nil
                        default: return nil
                        }
                    }
                    return nil
                }()
                let isCompleteish = hasOutput || upd.status == .completed || ((progressValue ?? 0.0) >= 0.999)
                // Leading status icon/spinner sized like SF Symbol
                if upd.status == .error {
                    Image(systemName: "xmark.octagon")
                        .foregroundStyle(OATheme.Colors.danger)
                } else if isCompleteish {
                    Image(systemName: "checkmark.circle")
                } else {
                    ProgressView().progressViewStyle(.circular)
                        .tint(colorScheme == .dark ? .white : OATheme.Colors.textSecondary)
                        .frame(width: 14, height: 14)
                        .alignmentGuide(.firstTextBaseline) { d in d[VerticalAlignment.center] }
                }
                // Show the original tool name if known; fall back to the call_id
                let name = bridge.toolCallNames[upd.call_id] ?? "call \(upd.call_id.prefix(8))…"
                if let pv = progressValue, !isCompleteish {
                    Text("\(name) (\(Int((pv * 100).rounded()))%)")
                } else {
                    Text(name)
                }
                Spacer()
                let statusText = isCompleteish ? ACPToolCallUpdateWire.Status.completed.rawValue : upd.status.rawValue
                Text(statusText)
                    .font(.footnote)
                    .foregroundStyle(upd.status == .error ? .red : .secondary)
            }
            .contentShape(Rectangle())
            .onTapGesture { onInspect?(note) }
        case .agentMessageChunk(let chunk):
            // Render agent message content without extra header label/icon
            VStack(alignment: .leading, spacing: 4) {
                markdownText(extractText(from: chunk))
                    .font(.body)
            }
        case .userMessageChunk(let chunk):
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Image(systemName: "person")
                        .foregroundStyle(.green)
                    Text("User")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Text(extractText(from: chunk))
                    .font(.body)
            }
        case .agentThoughtChunk(let chunk):
            HStack {
                Image(systemName: "brain")
                    .foregroundStyle(.purple)
                Text(extractText(from: chunk))
                    .italic()
                    .foregroundStyle(.secondary)
            }
        case .plan:
            // Hide inline plan rows; plan is rendered once at the top
            EmptyView()
        case .availableCommandsUpdate(let ac):
            Text("Available: \(ac.available_commands.count) commands")
                .foregroundStyle(.secondary)
        case .currentModeUpdate(let cur):
            Text("Mode: \(cur.current_mode_id.rawValue)")
                .foregroundStyle(.secondary)
        }
    }

    /// Extract text content from a ContentChunk
    private func extractText(from chunk: ACP.Client.ContentChunk) -> String {
        switch chunk.content {
        case .text(let textContent):
            return textContent.text
        case .image:
            return "[Image]"
        case .audio:
            return "[Audio]"
        case .resource_link(let link):
            return "Resource: \(link.uri)"
        case .resource(let embeddedResource):
            // EmbeddedResource has a nested resource field which is an enum
            switch embeddedResource.resource {
            case .text(let textResource):
                return "Embedded text: \(textResource.uri)"
            case .blob(let blobResource):
                return "Embedded blob: \(blobResource.uri)"
            }
        }
    }

    /// Render markdown text
    private func markdownText(_ text: String) -> Text {
        if let md = try? AttributedString(markdown: text, options: AttributedString.MarkdownParsingOptions(interpretedSyntax: .inlineOnlyPreservingWhitespace)) {
            return Text(md)
        }
        return Text(text)
    }
}

// Compute latest plan state from streamed updates
private func latestPlanFromUpdates(_ updates: [ACP.Client.SessionNotificationWire]) -> (ACPPlanState, [String: ACPPlanEntryStatus])? {
    // Find the last plan update and convert to ACPPlanState with de-duplicated steps and per-step status
    guard let last = updates.last(where: { if case .plan = $0.update { return true } else { return false } }) else { return nil }
    guard case let .plan(p) = last.update else { return nil }

    // Deduplicate by content while preserving order, and compute best status per step
    var seen = Set<String>()
    var orderedSteps: [String] = []
    var statusByStep: [String: ACPPlanEntryStatus] = [:]

    func better(_ a: ACPPlanEntryStatus?, _ b: ACPPlanEntryStatus) -> ACPPlanEntryStatus {
        // completed > in_progress > pending
        switch (a ?? .pending, b) {
        case (_, .completed): return .completed
        case (.completed, _): return .completed
        case (_, .in_progress): return .in_progress
        case (.in_progress, _): return .in_progress
        default: return .pending
        }
    }

    for e in p.entries {
        let key = e.content
        if !seen.contains(key) {
            seen.insert(key)
            orderedSteps.append(key)
            statusByStep[key] = e.status
        } else {
            statusByStep[key] = better(statusByStep[key], e.status)
        }
    }

    let allCompleted = orderedSteps.allSatisfy { statusByStep[$0] == .completed }
    let anyInProgress = orderedSteps.contains { statusByStep[$0] == .in_progress }
    let status: ACPPlanStatus = allCompleted ? .completed : (anyInProgress ? .running : .running)
    let plan = ACPPlanState(status: status, summary: nil, steps: orderedSteps, ts: Int64(Date().timeIntervalSince1970 * 1000))
    return (plan, statusByStep)
}

// Derive fm.analysis progress from streamed updates (started → completed)
private func fmAnalysisStatus(_ updates: [ACP.Client.SessionNotificationWire]) -> (inProgress: Bool, progressPercent: String?)? {
    // Find last tool_call for fm.analysis
    var lastCallId: String?
    for note in updates.reversed() {
        if case let .toolCall(call) = note.update, call.name == "fm.analysis" {
            lastCallId = call.call_id
            break
        }
    }
    guard let callId = lastCallId else { return nil }
    // Find last update for that callId
    for note in updates.reversed() {
        if case let .toolCallUpdate(upd) = note.update, upd.call_id == callId {
            switch upd.status {
            case .completed, .error:
                return (false, nil)
            case .started:
                // Try extract progress meta
                if let meta = upd._meta, let j = meta["progress"]?.toJSONValue() {
                    switch j {
                    case .number(let d): return (true, "\(Int((d * 100).rounded()))%")
                    case .string(let s): return (true, s)
                    default: break
                    }
                }
                return (true, nil)
            }
        }
    }
    return nil
}

#Preview {
    ChatHomeView()
}

#endif
