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

/// Computers: a native list of your computers, then Coder's shared screens
/// for one computer, adding one, and activity. Rust builds both and checks
/// every choice.
struct ComputersTab: View {
    @ObservedObject var bridge: MobileBridge

    private var home: ComputersHome? { bridge.packet?.computers_home }

    var body: some View {
        VStack(spacing: 0) {
            ForEach(bridge.packet?.notices ?? [], id: \.self) { notice in
                Text(notice).font(.caption).foregroundStyle(.secondary).padding(.horizontal)
            }
            if let failure = bridge.failure {
                Text(failure).font(.caption).padding(.horizontal)
            }
            if let home {
                ComputersList(home: home, bridge: bridge)
            } else {
                SurfaceView(view: bridge.packet?.computers) { view, node in
                    bridge.activate("computers", view: view, node: node)
                }
                if let qr = bridge.packet?.computers_qr {
                    InvitationQR(qr: qr).frame(width: 220, height: 220).padding()
                        .accessibilityLabel("Invitation QR code")
                }
            }
            if let input = bridge.packet?.computers_input {
                InputBar(input: input, busy: bridge.busy,
                         submit: { bridge.submit("computers", token: input.token, value: $0) },
                         cancel: { bridge.cancel("computers", token: input.token) })
                    .id(input.token)
            }
        }
        .background(Color.black.ignoresSafeArea())
        .navigationTitle(home == nil ? "" : "Computers")
        .navigationBarTitleDisplayMode(.inline)
        // Past the list, back returns to it rather than to Account.
        .navigationBarBackButtonHidden(home == nil)
        .toolbar {
            if let home {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Button("Activity", systemImage: "list.bullet") { bridge.computersGo("activity") }
                        Button("Refresh", systemImage: "arrow.clockwise") { bridge.computersGo("refresh") }
                        if home.owner_key {
                            Button("Enter owner key", systemImage: "key") { bridge.computersGo("owner_key") }
                        }
                        if home.keep_directory {
                            Button("Keep this device's version", systemImage: "checkmark.circle") {
                                bridge.computersGo("keep_directory")
                            }
                        }
                    } label: {
                        Image(systemName: "ellipsis")
                    }
                    .accessibilityLabel("More")
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button { bridge.computersGo("add") } label: { Image(systemName: "plus") }
                        .accessibilityLabel("Add a computer")
                }
            } else {
                ToolbarItem(placement: .topBarLeading) {
                    Button { bridge.computersGo("home") } label: { Image(systemName: "chevron.left") }
                        .accessibilityLabel("Computers")
                }
            }
        }
        .onAppear {
            bridge.computersGo("home")
            #if targetEnvironment(simulator)
            // `--computers-script open|add|activity` opens the first computer,
            // adding a computer, or activity, so a simulator check needs no taps.
            let arguments = ProcessInfo.processInfo.arguments
            if let index = arguments.firstIndex(of: "--computers-script"), index + 1 < arguments.count {
                let step = arguments[index + 1]
                Task {
                    try? await Task.sleep(for: .seconds(1))
                    if step == "open", let first = bridge.packet?.computers_home?.rows.first {
                        bridge.openComputer(first.host)
                    } else if step == "add" || step == "activity" {
                        bridge.computersGo(step)
                    }
                }
            }
            #endif
        }
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

/// One row per computer: a status dot, its name, and a short status. A tap
/// opens it; its menu holds the rest.
private struct ComputersList: View {
    let home: ComputersHome
    @ObservedObject var bridge: MobileBridge
    @State private var confirming: (row: ComputersHome.Row, item: ComputersHome.Item)?

    var body: some View {
        List {
            if let empty = home.empty {
                Section {
                    VStack(spacing: 12) {
                        Image(systemName: "desktopcomputer")
                            .font(.system(size: 40, weight: .light))
                            .foregroundStyle(.secondary)
                        Text(empty)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                        Button("Add a computer") { bridge.computersGo("add") }
                            .buttonStyle(.borderedProminent)
                            .tint(.white)
                            .foregroundStyle(.black)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 24)
                }
            } else {
                Section {
                    ForEach(home.rows) { row in
                        Button { bridge.openComputer(row.host) } label: { ComputerRow(row: row) }
                            .accessibilityIdentifier("computer-\(row.name)")
                            .contextMenu {
                                ForEach(row.menu, id: \.self) { item in
                                    Button(item.label, role: item.confirm == nil ? nil : .destructive) {
                                        choose(row, item)
                                    }
                                }
                            }
                            .swipeActions {
                                if let forget = row.menu.first(where: { $0.choice == "forget" }) {
                                    Button("Forget", systemImage: "trash", role: .destructive) {
                                        choose(row, forget)
                                    }
                                }
                            }
                    }
                } footer: {
                    if let notice = home.notice { Text(notice) }
                }
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
        .background(Color.black.ignoresSafeArea())
        .confirmationDialog(confirming?.item.confirm ?? "", isPresented: Binding(
            get: { confirming != nil }, set: { if !$0 { confirming = nil } }),
            titleVisibility: .visible) {
            if let confirming {
                Button(confirming.item.label, role: .destructive) {
                    bridge.chooseComputer(confirming.row.host, confirming.item.choice)
                }
            }
        }
    }

    private func choose(_ row: ComputersHome.Row, _ item: ComputersHome.Item) {
        if item.confirm == nil {
            bridge.chooseComputer(row.host, item.choice)
        } else {
            confirming = (row, item)
        }
    }
}

private struct ComputerRow: View {
    let row: ComputersHome.Row

    private var tone: Color {
        switch row.tone {
        case "online": .green
        case "pending": .yellow
        case "alert": .red
        default: Color(white: 0.45)
        }
    }

    var body: some View {
        HStack(spacing: 12) {
            Circle().fill(tone).frame(width: 8, height: 8)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(row.name).foregroundStyle(.white)
                Text(row.status).font(.subheadline).foregroundStyle(.secondary)
            }
            Spacer(minLength: 8)
            Image(systemName: "chevron.right")
                .font(.footnote.weight(.semibold))
                .foregroundStyle(.tertiary)
        }
        .padding(.vertical, 2)
        .contentShape(Rectangle())
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
        .task { await CoderLaunchTaps.run(bridge) }
        // Task status and a running chat's transcript move on their own:
        // every second while the open chat changes, else every three.
        .task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(bridge.packet?.coder_live == true ? 1 : 3))
                if !Task.isCancelled && !bridge.busy { bridge.refreshComputers() }
            }
        }
    }
}

/// Simulator checks: `--coder-tap KEY[,KEY...]` taps Coder nodes in order
/// once the surface shows them. A key ending in `*` taps the first node whose
/// key starts with the rest, such as `task-*` for the first chat.
enum CoderLaunchTaps {
    @MainActor static func run(_ bridge: MobileBridge) async {
        #if DEBUG || targetEnvironment(simulator)
        let arguments = ProcessInfo.processInfo.arguments
        guard let index = arguments.firstIndex(of: "--coder-tap"), index + 1 < arguments.count else { return }
        for key in arguments[index + 1].split(separator: ",").map(String.init) {
            for _ in 0..<20 {
                try? await Task.sleep(for: .milliseconds(500))
                guard let view = bridge.packet?.coder, let node = find(key, in: view.root) else { continue }
                bridge.activate("coder", view: view, node: node)
                break
            }
        }
        #endif
    }

    private static func find(_ key: String, in node: NativeNode) -> String? {
        let matches = key.hasSuffix("*") ? node.key.hasPrefix(String(key.dropLast())) : node.key == key
        if matches { return node.key }
        let children: [NativeNode]
        switch node.element {
        case let .stack(_, nodes), let .list(_, nodes), let .transcript(_, nodes, _): children = nodes
        default: children = []
        }
        for child in children { if let found = find(key, in: child) { return found } }
        return nil
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
