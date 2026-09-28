// Rust builds every screen; this host decodes and renders them, and supplies
// the pieces a Rust Native tree cannot: tabs, the camera, keyboards, and the
// terminal's keyboard target.
import SwiftUI

@main
struct OpenAgentsApp: App {
    @StateObject private var bridge = MobileBridge()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            HomeScreen(bridge: bridge)
                .onChange(of: scenePhase, initial: true) { _, phase in
                    if phase != .inactive { bridge.lifecycle(phase == .active) }
                }
                .nativeFixture()
        }
    }
}

struct HomeScreen: View {
    @ObservedObject var bridge: MobileBridge

    var body: some View {
        AppTabs(bridge: bridge)
            .tint(.white)
            .preferredColorScheme(.dark)
            .fullScreenCover(isPresented: Binding(
                get: { bridge.packet?.terminal == true },
                set: { _ in })) {
                TerminalScreen(bridge: bridge)
            }
    }
}

/// One Rust Native surface on black, scrolling when it is taller than the
/// screen.
struct SurfaceView: View {
    let view: NativeView?
    let activate: (NativeView, String) -> Void

    var body: some View {
        ScrollView {
            if let view {
                NativeRenderer(node: view.root, revision: view.revision, followTarget: nil,
                               followChanged: nil, activate: { node in activate(view, node) })
                    .frame(maxWidth: .infinity, alignment: .topLeading)
            }
        }
        .background(Color.black.ignoresSafeArea())
    }
}

struct ComputersTab: View {
    @ObservedObject var bridge: MobileBridge

    var body: some View {
        VStack(spacing: 0) {
            ForEach(bridge.packet?.notices ?? [], id: \.self) { notice in
                Text(notice).font(.caption).foregroundStyle(.secondary).padding(.horizontal)
            }
            if let failure = bridge.failure {
                Text(failure).font(.caption).padding(.horizontal)
            }
            SurfaceView(view: bridge.packet?.computers) { view, node in
                bridge.activate("computers", view: view, node: node)
            }
            if let qr = bridge.packet?.computers_qr {
                InvitationQR(qr: qr).frame(width: 220, height: 220).padding()
                    .accessibilityLabel("Invitation QR code")
            }
            if let input = bridge.packet?.computers_input {
                InputBar(input: input, busy: bridge.busy,
                         submit: { bridge.submit("computers", token: input.token, value: $0) },
                         cancel: { bridge.cancel("computers", token: input.token) })
                    .id(input.token)
            }
        }
        .background(Color.black.ignoresSafeArea())
        // Host status moves on its own; poll while no value is being entered.
        .task(id: bridge.packet?.computers_input == nil) {
            guard bridge.packet?.computers_input == nil else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(3))
                if !Task.isCancelled && !bridge.busy { bridge.refreshComputers() }
            }
        }
    }
}

/// Chats with Coder: a new chat is a task on the chosen computer.
struct CoderTab: View {
    @ObservedObject var bridge: MobileBridge

    var body: some View {
        Group {
            if let view = bridge.packet?.coder {
                NativeRenderer(node: view.root, revision: view.revision, followTarget: nil,
                               followChanged: nil,
                               submit: { token, text in bridge.submit("coder", token: token, value: text) },
                               activate: { node in bridge.activate("coder", view: view, node: node) })
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            } else {
                Color.clear
            }
        }
        .background(Color.black.ignoresSafeArea())
        // Task status and a running chat's transcript move on their own.
        .task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(3))
                if !Task.isCancelled && !bridge.busy { bridge.refreshComputers() }
            }
        }
    }
}

/// Chats from every computer paired for reading. The surface holds its own
/// lists, so it does not scroll as a whole.
struct ChatsTab: View {
    @ObservedObject var bridge: MobileBridge

    var body: some View {
        VStack(spacing: 0) {
            ZStack(alignment: .topTrailing) {
                if let view = bridge.packet?.chats {
                    NativeRenderer(node: view.root, revision: view.revision,
                                   followTarget: nil, followChanged: nil,
                                   activate: { node in bridge.activate("chats", view: view, node: node) })
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                } else {
                    Color.clear
                }
                if bridge.packet?.chats_loading == true {
                    ProgressView().padding().accessibilityLabel("Loading chats")
                }
            }
            if let input = bridge.packet?.chats_input {
                InputBar(input: input, busy: bridge.busy,
                         submit: { bridge.submit("chats", token: input.token, value: $0) },
                         cancel: { bridge.cancel("chats", token: input.token) })
                    .id(input.token)
            }
        }
        .background(Color.black.ignoresSafeArea())
        // Reads finish in the background; poll until they do.
        .task(id: bridge.packet?.chats_loading == true) {
            guard bridge.packet?.chats_loading == true else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                if !Task.isCancelled { bridge.snapshot() }
            }
        }
    }
}

struct TailnetTab: View {
    @ObservedObject var bridge: MobileBridge

    var body: some View {
        ZStack(alignment: .topTrailing) {
            Color.black.ignoresSafeArea()
            // The device list is its own scrolling list; a surrounding
            // scroll view would collapse it.
            if let view = bridge.packet?.tailnet {
                NativeRenderer(node: view.root, revision: view.revision, followTarget: nil,
                               followChanged: nil,
                               activate: { node in bridge.activate("tailnet", view: view, node: node) })
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
            if bridge.packet?.tailnet_loading == true {
                ProgressView().padding().accessibilityLabel("Checking your tailnet")
            }
        }
        // A background read finishes on its own; poll until it does.
        .task(id: bridge.packet?.tailnet_loading == true) {
            guard bridge.packet?.tailnet_loading == true else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                if !Task.isCancelled { bridge.snapshot() }
            }
        }
    }
}

/// The keyboard or camera for the current Computers input request. Rust
/// validates every value; nothing here is kept.
struct InputBar: View {
    let input: ComputersInput
    let busy: Bool
    let submit: (String) -> Void
    let cancel: () -> Void
    @State private var value = ""
    @State private var scanning = false
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(input.label).font(.headline)
            Text(input.prompt).font(.caption)
            if let error { Text(error).font(.caption) }
            if scanning {
                InlineQRScanner { send($0) }
                Button("Type instead") { scanning = false }
            } else if input.secret {
                SecretInputField(label: input.label, value: $value) { if !busy { send(value) } }
            } else {
                TextField(input.label, text: $value, axis: .vertical)
                    .lineLimit(1...6)
                    .autocorrectionDisabled().textInputAutocapitalization(.never)
                    .accessibilityIdentifier("computers-input")
            }
            HStack {
                if !scanning {
                    Button("Submit") { send(value) }
                        .disabled(busy || value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    if input.scan { Button("Scan QR code", systemImage: "qrcode.viewfinder") { scanning = true } }
                }
                Spacer()
                Button("Cancel") { value = ""; cancel() }
            }
        }
        .padding(12)
        .background(Color(white: 0.08), in: RoundedRectangle(cornerRadius: 16))
        .padding(8)
        .onAppear { scanning = input.scan }
    }

    private func send(_ text: String) {
        guard text.utf8.count <= input.max_bytes else {
            error = "That's too long. Copy it again."
            return
        }
        error = nil
        value = ""
        scanning = false
        submit(text)
    }
}

/// Draws the modules Rust rendered; the invitation never leaves the device.
struct InvitationQR: View {
    let qr: ComputersQR

    var body: some View {
        Canvas { context, size in
            let cell = min(size.width, size.height) / CGFloat(max(qr.size, 1))
            context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(.white))
            for (y, row) in qr.rows.enumerated() {
                for (x, module) in row.enumerated() where module == "1" {
                    context.fill(Path(CGRect(x: CGFloat(x) * cell, y: CGFloat(y) * cell,
                                             width: cell, height: cell)), with: .color(.black))
                }
            }
        }
    }
}
