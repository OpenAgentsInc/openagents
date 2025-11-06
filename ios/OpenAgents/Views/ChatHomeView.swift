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
                    Button("Start") { startFlow() }
                        .buttonStyle(.glass)
                        .accessibilityLabel("Start Explore Flow")
                }
            }
            // iOS 26+ only: let the system render the Liquid Glass toolbar background
            .sheet(isPresented: $isMenuPresented) {
                MenuSheet()
            }
        }
    }

    private func startFlow() {
        // Phase 1: ensure a session exists then request index.status
        Task { @MainActor in
            // Reset stream so user sees only new events for this flow
            bridge.updates.removeAll()
            // Create new session
            struct NewResp: Codable { let session_id: ACPSessionId }
            bridge.sendRPC(method: ACPRPC.sessionNew, params: ACP.Agent.SessionNewRequest()) { (resp: NewResp?) in
                guard let sid = resp?.session_id else { return }
                bridge.currentSessionId = sid
                // Minimal control-plane request to trigger a visible stream
                struct StatusParams: Codable { let session_id: ACPSessionId }
                bridge.sendRPC(method: "index.status", params: StatusParams(session_id: sid)) { (_: BridgeManager.EmptyResult?) in }
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
                Text("index.rebuild")
                Spacer()
                Text(upd.status.rawValue)
                    .font(.footnote)
                    .foregroundStyle(upd.status == .error ? .red : .secondary)
            }
        case .agentMessageChunk:
            Text("Agent message")
        case .userMessageChunk:
            Text("User message")
        case .agentThoughtChunk:
            Text("…thinking…")
                .italic()
                .foregroundStyle(.secondary)
        case .plan(let plan):
            Text("Plan: \(plan.entries.count) items")
                .foregroundStyle(.secondary)
        case .availableCommandsUpdate(let ac):
            Text("Available: \(ac.available_commands.count) commands")
                .foregroundStyle(.secondary)
        case .currentModeUpdate(let cur):
            Text("Mode: \(cur.current_mode_id.rawValue)")
                .foregroundStyle(.secondary)
        }
    }
}

#Preview {
    ChatHomeView()
}

#endif
