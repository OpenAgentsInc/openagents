import SwiftUI
import OpenAgentsCore

#if os(iOS)
/// Fresh screen showcasing the new top toolbar header for iOS 26+.
/// This screen intentionally does not auto-load a conversation thread.
struct ChatHomeView: View {
    @State private var isMenuPresented = false
    @EnvironmentObject private var bridge: BridgeManager

    var body: some View {
        NavigationStack {
            // Main content placeholder
            VStack(spacing: 12) {
                // Banner with connection + updates info
                HStack(spacing: 8) {
                    switch bridge.status {
                    case .connected(let host, let port):
                        Text("ACP OK (0.2.2)")
                            .font(.headline)
                        Text("· \(host):\(port)")
                            .foregroundStyle(.secondary)
                    case .handshaking(let host, let port):
                        Text("Handshaking… \(host):\(port)")
                    case .connecting(let host, let port):
                        Text("Connecting… \(host):\(port)")
                    case .discovering:
                        Text("Discovering desktop…")
                    case .advertising(let port):
                        Text("Advertising on :\(port)")
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

                // Streamed ACP updates as a simple list
                List(bridge.updates.indices, id: \.self) { idx in
                    let note = bridge.updates[idx]
                    UpdateRow(note: note)
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
                        .accessibilityLabel("Start Workspace Exploration")
                }
            }
            // iOS 26+ only: let the system render the Liquid Glass toolbar background
            .sheet(isPresented: $isMenuPresented) {
                MenuSheet()
            }
        }
    }

    private func startExploreFlow() {
        // Phase 2: Start workspace exploration using on-device FM orchestrator
        Task { @MainActor in
            // Reset stream so user sees only new events for this flow
            bridge.updates.removeAll()

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
            print("[ChatHome] Starting workspace exploration: \(workspacePath)")
            bridge.orchestrateExploreStart(root: workspacePath, goals: goals) { response in
                if let response = response {
                    print("[ChatHome] Orchestration started: \(response.plan_id)")
                } else {
                    print("[ChatHome] Orchestration failed to start")
                }
            }
        }
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
    @EnvironmentObject private var bridge: BridgeManager
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
        case .toolCallUpdate(let upd):
            HStack(alignment: .firstTextBaseline) {
                Image(systemName: upd.status == .completed ? "checkmark.circle" : (upd.status == .started ? "circle.dashed" : "xmark.octagon"))
                // Show the original tool name if known; fall back to the call_id
                let name = bridge.toolCallNames[upd.call_id] ?? "call \(upd.call_id.prefix(8))…"
                Text(name)
                Spacer()
                Text(upd.status.rawValue)
                    .font(.footnote)
                    .foregroundStyle(upd.status == .error ? .red : .secondary)
            }
        case .agentMessageChunk(let chunk):
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Image(systemName: "sparkles")
                        .foregroundStyle(.blue)
                    Text("Agent")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
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
        case .plan(let plan):
            // Convert ACPPlan to ACPPlanState for rendering
            let planState = ACPPlanState(
                status: .running,
                summary: nil,
                steps: plan.entries.map { $0.content },
                ts: nil
            )
            PlanStateView(state: planState)
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

#Preview {
    ChatHomeView()
}

#endif
