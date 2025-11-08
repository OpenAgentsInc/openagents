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

                UpdatesListView(updates: bridge.updates) { json in
                    selectedJSON = json
                    isJSONSheetPresented = selectedJSON != nil
                }
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
            .sheet(isPresented: $isJSONSheetPresented) { JSONInspectorView(json: selectedJSON ?? "") }
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

// Extracted components live in separate files: UpdatesListView, JSONInspectorView, and MenuSheet.

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
