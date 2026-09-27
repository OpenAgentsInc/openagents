// Native input and presentation at the world's computer. Rust validates pairing
// and projects every history row; entering this panel grants no task authority.
import SwiftUI
import UIKit

struct ComputerPanel: View {
    @ObservedObject var reader: MobileBridge
    let active: Bool
    let close: () -> Void
    let worldAction: ([String: Any]) -> Void
    let worldConnection: VerseConnection?
    let worldStorageError: String?
    @Binding var pairing: Bool
    @State private var pasting = false
    @State private var scanning = false
    @State private var code = ""
    @State private var inputError: String?
    @State private var disconnecting = false
    @State private var copied = false
    @State private var relay = ""
    @State private var settings = false
    @State private var computers = false
    @State private var computersScanning = false
    @State private var computersValue = ""
    private let command = "./pair"
    private var paired: Bool { reader.packet?.paired == true }
    private var reading: Bool { reader.packet?.reading == true && !pairing }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(settings ? "Settings" : computers ? "Computers" : "Chats").font(.headline)
                Spacer()
                if !settings && !computers && paired && !pairing { refreshButton }
                Button {
                    if settings {
                        settings = false
                        computers = false
                        pairing = false
                    } else {
                        settings = true
                    }
                    scanning = false
                    computersScanning = false
                } label: {
                    Image(systemName: settings ? "chevron.left" : "ellipsis")
                }
                .accessibilityLabel(settings ? "Back to chats" : "Computer settings")
                .accessibilityIdentifier("computer-settings")
                Button("Back to world", systemImage: "xmark") { close() }
                    .labelStyle(.iconOnly).accessibilityIdentifier("computer-close")
            }
            if let error = inputError ?? reader.nativeError ?? reader.packet?.error {
                Text(error).font(.caption).textSelection(.enabled).accessibilityIdentifier("reader-error")
            }
            if reader.busy {
                HStack { ProgressView(); Text("Loading…").font(.caption) }
                    .accessibilityIdentifier("computer-busy")
            }
            if settings {
                settingsContent
            } else if computers {
                computersContent
            } else if !paired || pairing {
                pairingControls
            } else if let view = reader.packet?.view {
                NativeRenderer(node: view.root, revision: view.revision,
                               followTarget: reader.packet?.follow_target,
                               followChanged: followAction(page: reader.packet?.follow_page)) { node in
                    reader.activate(view: view, node: node)
                }
            } else {
                Text("Loading chats…").font(.caption)
            }
        }
        .padding(12)
        .background(Color(red: 0.025, green: 0.02, blue: 0).opacity(0.97), in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(.tint.opacity(0.7), lineWidth: 1))
        .onAppear {
            relay = worldConnection?.relay ?? "wss://relay.openagents.com"
            reader.setForeground(active)
        }
        .onDisappear { scanning = false; computersScanning = false; computersValue = ""; reader.setForeground(false) }
        .onChange(of: worldConnection?.relay) { _, value in
            relay = value ?? "wss://relay.openagents.com"
        }
        .onChange(of: reader.packet?.computers_exit) { _, exit in
            if exit == true { computers = false; computersScanning = false }
        }
        .onChange(of: active) { _, current in
            if !current { computersValue = "" }
            reader.setForeground(current)
        }
        .task(id: active && paired && !pairing && !scanning && !computers && !settings) {
            guard active && paired && !pairing && !scanning && !computers && !settings else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(5))
                if !Task.isCancelled { reader.refresh() }
            }
        }
        .task(id: computers && active && !settings) {
            guard computers && active && !settings else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(3))
                if !Task.isCancelled, !computersScanning { reader.pollComputers() }
            }
        }
    }

    private var settingsContent: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Button("Pair computer", systemImage: "qrcode") {
                    settings = false; computers = false; pairing = true
                }.accessibilityIdentifier("computer-pair")
                Button(computers ? "Chats" : "Computers", systemImage: "desktopcomputer") {
                    settings = false; computers.toggle()
                    if computers { reader.refreshComputers() }
                }.accessibilityIdentifier("computers-toggle")
                Divider()
                VStack(alignment: .leading, spacing: 8) {
                    Text("World connection").font(.headline)
                    Text(worldConnection?.label ?? "Offline")
                        .font(.caption).accessibilityIdentifier("world-connection-status")
                    if let error = worldStorageError ?? worldConnection?.error {
                        Text(error).font(.caption).textSelection(.enabled)
                            .accessibilityIdentifier("world-connection-error")
                    }
                    TextField("Relay URL", text: $relay)
                        .keyboardType(.URL).textInputAutocapitalization(.never).autocorrectionDisabled()
                        .accessibilityLabel("World relay URL").accessibilityIdentifier("world-relay")
                    HStack {
                        Button(worldConnection?.relay == nil ? "Join" : "Reconnect") {
                            worldAction(["action": "connect", "relay": relay])
                        }
                        .disabled(relay.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !active)
                        .accessibilityIdentifier("world-join")
                        if worldConnection?.relay != nil {
                            Button("Leave") { worldAction(["action": "disconnect"]) }
                                .accessibilityIdentifier("world-leave")
                        }
                    }
                    Text("Shares your avatar and movement.").font(.caption2).foregroundStyle(.secondary)
                }
                Divider()
                DisclosureGroup("Device details") {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(reader.packet?.public_key ?? "Unavailable")
                            .font(.system(.caption2, design: .monospaced)).textSelection(.enabled)
                            .accessibilityIdentifier("reader-public-key")
                        Text(reader.packet?.status ?? "Opening…")
                            .font(.caption2).accessibilityIdentifier("reader-status")
                        if paired {
                            if disconnecting {
                                Text("Erase cached chats on this phone?").font(.caption)
                                HStack {
                                    Button("Disconnect and erase", role: .destructive) {
                                        reader.disconnect(); disconnecting = false; settings = false
                                    }
                                    Button("Cancel") { disconnecting = false }
                                }
                            } else {
                                Button("Disconnect computer", role: .destructive) { disconnecting = true }
                            }
                        }
                    }
                }.font(.caption)
            }.frame(maxWidth: .infinity, alignment: .leading)
        }
        .scrollDismissesKeyboard(.interactively)
    }

    private var refreshButton: some View {
        Button("Refresh", systemImage: "arrow.clockwise") { reader.refresh(force: true) }
            .labelStyle(.iconOnly).disabled(reader.busy).accessibilityIdentifier("reader-refresh")
    }

    /// The Rust Native Computers tree, plus the one native field or scanner
    /// its current input request names. Rust validates every value.
    private var computersContent: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                if let view = reader.packet?.computers {
                    NativeRenderer(node: view.root, revision: view.revision,
                                   followTarget: nil, followChanged: nil) { node in
                        reader.activateComputers(view: view, node: node)
                    }
                } else {
                    Text("Computers are unavailable on this device.")
                }
                if let qr = reader.packet?.computers_qr {
                    InvitationQRCode(qr: qr)
                }
                if let input = reader.packet?.computers_input {
                    computersInput(input).id(input.token)
                }
            }.frame(maxWidth: .infinity, alignment: .leading)
        }
        .scrollDismissesKeyboard(.interactively)
    }

    private func computersInput(_ input: ComputersInput) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(input.prompt).font(.caption)
            if input.scan {
                if computersScanning {
                    InlineQRScanner { value in
                        computersScanning = false
                        submitComputers(input, value)
                    }
                    Button("Stop scanning") { computersScanning = false }
                } else {
                    Button("Scan QR code", systemImage: "qrcode.viewfinder") { computersScanning = true }
                        .disabled(reader.busy || !active)
                        .accessibilityIdentifier("computers-scan")
                }
            }
            Group {
                if input.secret {
                    SecureField(input.label, text: $computersValue)
                        .textContentType(nil)
                        .privacySensitive()
                } else {
                    TextField(input.label, text: $computersValue, axis: .vertical)
                        .lineLimit(1...4)
                }
            }
            .autocorrectionDisabled().textInputAutocapitalization(.never)
            .accessibilityLabel(input.label).accessibilityIdentifier("computers-input")
            HStack {
                Button("Submit") { submitComputers(input, computersValue) }
                    .disabled(reader.busy || computersValue.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityIdentifier("computers-submit")
                Button("Cancel") {
                    computersValue = ""; computersScanning = false; inputError = nil
                    reader.cancelComputers(token: input.token)
                }
                .disabled(reader.busy)
                .accessibilityIdentifier("computers-cancel")
            }
        }
        .onAppear { computersValue = ""; computersScanning = input.scan && active }
        .onDisappear { computersValue = ""; computersScanning = false }
    }

    private func submitComputers(_ input: ComputersInput, _ value: String) {
        computersValue = ""; computersScanning = false
        guard value.utf8.count <= input.max_bytes else {
            inputError = "That's too long. Copy it again."
            return
        }
        inputError = nil
        reader.submitComputers(token: input.token, value: value)
    }

    private var pairingControls: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("In your OpenAgents folder:")
                Text(command).font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                    .accessibilityIdentifier("computer-command")
                Button(copied ? "Copied" : "Copy command", systemImage: "doc.on.doc") {
                    UIPasteboard.general.string = command
                    copied = true
                }
                Text("Then scan the QR code.").font(.caption)
                if scanning {
                    InlineQRScanner { value in
                        scanning = false
                        submit(value)
                    }
                    HStack {
                        Button("Stop scanning") { scanning = false }
                        Button("Paste code") { scanning = false; pasting = true }
                            .accessibilityIdentifier("computer-paste")
                    }
                } else {
                    HStack {
                        Button("Scan QR code", systemImage: "qrcode.viewfinder") { scanning = true; pasting = false }
                            .disabled(reader.busy || !active).accessibilityIdentifier("computer-scan")
                        Button("Paste code", systemImage: "doc.on.clipboard") { pasting = true }
                            .disabled(reader.busy).accessibilityIdentifier("computer-paste")
                    }
                }
                if pasting {
                    TextEditor(text: $code).frame(minHeight: 85, maxHeight: 130)
                        .autocorrectionDisabled().textInputAutocapitalization(.never)
                        .accessibilityLabel("Computer invitation").accessibilityIdentifier("computer-code")
                    Button("Connect") { submit(code); code = "" }
                        .disabled(reader.busy || code.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        .accessibilityIdentifier("computer-connect")
                }
                Text("Read-only chat access.")
                    .font(.caption).foregroundStyle(.secondary)
                DisclosureGroup("Setup help") {
                    Text("Update this checkout first. With Coder installed, run coder pair instead. Keep the command running while you read chats.")
                        .font(.caption).textSelection(.enabled)
                }.font(.caption)
                if paired {
                    Button("Back to chats") { pairing = false; scanning = false; pasting = false }
                        .accessibilityIdentifier("computer-chats")
                }
            }.frame(maxWidth: .infinity, alignment: .leading)
        }.scrollDismissesKeyboard(.interactively)
    }

    private func submit(_ value: String) {
        guard value.utf8.count <= 65_536 else {
            inputError = "Code too long. Copy a fresh invitation."
            return
        }
        inputError = nil
        reader.connect(value) { succeeded in
            if succeeded { pairing = false; pasting = false; scanning = false }
        }
    }

    private func followAction(page: String?) -> ((Bool) -> Void)? {
        guard let page else { return nil }
        return { enabled in reader.setFollowing(enabled, page: page) }
    }
}
