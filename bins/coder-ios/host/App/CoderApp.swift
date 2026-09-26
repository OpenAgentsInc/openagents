// Coder's native shell. The rendered feature tree and domain actions come from Rust.
import SwiftUI

@main
struct CoderApp: App {
    @State private var selectedTab = "chats"
    @StateObject private var bridge = MobileBridge(
        synthetic: ProcessInfo.processInfo.arguments.contains("--synthetic"))

    var body: some Scene {
        WindowGroup {
            TabView(selection: $selectedTab) {
                ReaderScreen(bridge: bridge, selected: selectedTab == "chats")
                    .tabItem { Label("Chats", systemImage: "text.bubble") }.tag("chats")
                VerseScreen(selected: selectedTab == "verse",
                            synthetic: ProcessInfo.processInfo.arguments.contains("--synthetic"))
                    .tabItem { Label("Verse", systemImage: "globe") }.tag("verse")
            }.tint(Color(red: 1, green: 176.0 / 255.0, blue: 0))
        }
    }
}

private struct ReaderScreen: View {
    @ObservedObject var bridge: MobileBridge
    let selected: Bool
    @Environment(\.scenePhase) private var phase
    @State private var pairing = false
    @State private var disconnecting = false
    private let timer = Timer.publish(every: 5, on: .main, in: .common).autoconnect()

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 8) {
                if let error = bridge.nativeError ?? bridge.packet?.error {
                    Text(error).font(.callout).textSelection(.enabled)
                        .accessibilityIdentifier("reader-error")
                }
                if let view = bridge.packet?.view {
                    NativeRenderer(node: view.root, revision: view.revision,
                                   followTarget: bridge.packet?.follow_target,
                                   followChanged: followAction(page: bridge.packet?.follow_page)) { node in
                        bridge.activate(view: view, node: node)
                    }
                } else {
                    ContentUnavailableView("Reader is opening", systemImage: "text.bubble",
                                           description: Text("Saved chats appear after protected local state opens."))
                }
            }
            .padding(.horizontal, 12)
            .navigationTitle("Coder")
            .navigationBarTitleDisplayMode(.inline)
            .safeAreaInset(edge: .bottom) {
                HStack {
                    Text(bridge.packet?.status ?? "Opening local state").font(.caption)
                    Spacer()
                    if bridge.busy { ProgressView().accessibilityLabel("Refreshing reader") }
                    Text("Read-only").font(.caption).accessibilityIdentifier("reader-read-only")
                }.padding(8).background(.bar)
            }
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Connect") { pairing = true }.disabled(bridge.busy)
                }
                ToolbarItemGroup(placement: .topBarTrailing) {
                    Button("Refresh", systemImage: "arrow.clockwise") { bridge.refresh(force: true) }
                        .disabled(bridge.busy)
                    Menu {
                        Button("Disconnect and erase cached chats", role: .destructive) { disconnecting = true }
                    } label: { Image(systemName: "ellipsis.circle").accessibilityLabel("Reader options") }
                        .disabled(bridge.busy)
                }
            }
        }
        .sheet(isPresented: $pairing) { PairingSheet(bridge: bridge) }
        .confirmationDialog("Disconnect and erase cached chats on this device?", isPresented: $disconnecting,
                            titleVisibility: .visible) {
            Button("Disconnect and erase", role: .destructive) { bridge.disconnect() }
        } message: { Text("The computer's conversations remain unchanged. This device keeps its Keychain identity.") }
        .onChange(of: phase) { _, current in bridge.setForeground(selected && current == .active) }
        .onChange(of: selected) { _, current in bridge.setForeground(current && phase == .active) }
        .onAppear { bridge.setForeground(selected && phase == .active) }
        .onDisappear { bridge.setForeground(false) }
        .onReceive(timer) { _ in if selected && phase == .active { bridge.refresh() } }
    }

    private func followAction(page: String?) -> ((Bool) -> Void)? {
        guard let page else { return nil }
        return { enabled in bridge.setFollowing(enabled, page: page) }
    }
}

private struct PairingSheet: View {
    @ObservedObject var bridge: MobileBridge
    @Environment(\.dismiss) private var dismiss
    @State private var code = ""

    var body: some View {
        NavigationStack {
            Form {
                Section("This device") {
                    Text("Give this public key to the computer's reader host.")
                    Text(bridge.packet?.public_key ?? "Unavailable")
                        .font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                        .accessibilityIdentifier("reader-public-key")
                    if let key = bridge.packet?.public_key {
                        ShareLink("Share public key", item: key)
                    }
                }
                Section("Connection code") {
                    Text("Paste the code generated by your computer. Connecting requests read-only access.")
                    TextEditor(text: $code).frame(minHeight: 120)
                        .autocorrectionDisabled().textInputAutocapitalization(.never)
                        .accessibilityLabel("Host connection code")
                    Button("Connect to computer") {
                        bridge.connect(code)
                        code = ""
                        dismiss()
                    }.disabled(code.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || bridge.busy)
                }
            }
            .navigationTitle("Connect Coder")
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        }
    }
}
