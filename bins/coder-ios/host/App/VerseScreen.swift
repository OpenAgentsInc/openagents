// Coder-specific controls around the reusable Rust Native surface mount.
import SwiftUI

struct VerseScreen: View {
    @StateObject private var bridge: VerseBridge
    @Environment(\.scenePhase) private var phase
    @State private var joining = false
    @State private var sprint = false
    let selected: Bool

    init(selected: Bool, synthetic: Bool) {
        self.selected = selected
        _bridge = StateObject(wrappedValue: VerseBridge(synthetic: synthetic))
    }

    private var active: Bool { selected && phase == .active }

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 8) {
                if let error = bridge.nativeError ?? bridge.packet?.error {
                    Text(error).font(.callout).textSelection(.enabled).accessibilityIdentifier("verse-error")
                    Button("Retry world renderer") { bridge.retry() }
                }
                if let packet = bridge.packet, let view = packet.view {
                    NativeRenderer(node: view.root, revision: view.revision,
                                   followTarget: nil, followChanged: nil, surface: mount) { _ in }
                    HStack {
                        Text("Frames \(packet.frames_presented)").font(.caption.monospacedDigit())
                            .accessibilityIdentifier("verse-frames")
                        Spacer()
                        Text(packet.position.map { String(format: "%.2f", $0) }.joined(separator: ", "))
                            .font(.caption.monospacedDigit()).accessibilityIdentifier("verse-position")
                    }
                }
                Text("Drag left to move · drag right to look").font(.caption)
                HStack {
                    Button("Jump") { bridge.send(["action": "jump"]) }.accessibilityIdentifier("verse-jump")
                    Toggle("Sprint", isOn: $sprint).fixedSize()
                        .onChange(of: sprint) { _, enabled in
                            if active { bridge.send(["action": "sprint", "enabled": enabled]) }
                        }
                    Spacer()
                    Button("Zoom in", systemImage: "plus.magnifyingglass") { bridge.send(["action": "zoom", "delta": 1]) }
                        .labelStyle(.iconOnly)
                    Button("Zoom out", systemImage: "minus.magnifyingglass") { bridge.send(["action": "zoom", "delta": -1]) }
                        .labelStyle(.iconOnly)
                }
                Text(bridge.packet?.status ?? "Opening world").font(.caption).accessibilityIdentifier("verse-status")
            }
            .padding(.horizontal, 12)
            .padding(.bottom, 8)
            .navigationTitle("Verse")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Button("Join relay") { joining = true }
                        Button("Leave relay") { bridge.send(["action": "disconnect"]) }
                    } label: { Label("World connection", systemImage: "network") }
                }
            }
        }
        .sheet(isPresented: $joining) { VerseRelaySheet(bridge: bridge) }
        .onChange(of: active) { _, enabled in if !enabled { sprint = false } }
    }

    private func mount(resource: String, label: String) -> AnyView {
        guard resource == "verse.world" else {
            return AnyView(Text("This world surface is unavailable."))
        }
        return AnyView(VerseSurface(bridge: bridge, active: active, label: label)
            .frame(maxWidth: .infinity, maxHeight: .infinity))
    }
}

private struct VerseRelaySheet: View {
    @ObservedObject var bridge: VerseBridge
    @Environment(\.dismiss) private var dismiss
    @State private var relay = ""

    var body: some View {
        NavigationStack {
            Form {
                Section("Join a compatible world relay") {
                    Text("Joining publishes this device's world presence and movement. It uses a separate Verse identity, not the chat reader identity.")
                    TextField("wss://relay.example.com", text: $relay)
                        .keyboardType(.URL).textInputAutocapitalization(.never).autocorrectionDisabled()
                        .accessibilityLabel("World relay URL")
                    Button("Join relay") {
                        bridge.send(["action": "connect", "relay": relay])
                        dismiss()
                    }.disabled(relay.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
            .navigationTitle("World connection")
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        }
    }
}
