// Native input and presentation at the world's computer. Rust validates pairing
// and projects every history row; entering this panel grants no task authority.
import SwiftUI
import UIKit

struct ComputerPanel: View {
    @ObservedObject var reader: MobileBridge
    let active: Bool
    let close: () -> Void
    let worldAction: ([String: Any]) -> Void
    @Binding var pairing: Bool
    @State private var pasting = false
    @State private var scanning = false
    @State private var code = ""
    @State private var inputError: String?
    @State private var disconnecting = false
    @State private var copied = false
    @State private var relay = ""
    @State private var computers = false
    @State private var computersScanning = false
    @State private var computersValue = ""
    private let command = "cargo run --release -p coder-connect -- connect"
    private let timer = Timer.publish(every: 5, on: .main, in: .common).autoconnect()
    private var paired: Bool { reader.packet?.paired == true }
    private var reading: Bool { reader.packet?.reading == true && !pairing }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Label("Computer", systemImage: "desktopcomputer").font(.headline)
                Spacer()
                if reading && !computers { refreshButton }
                if !reading || computers {
                    Button(computers ? "Chats" : "Computers") {
                        computers.toggle()
                        computersScanning = false
                        scanning = false
                        if computers { reader.refreshComputers() }
                    }
                    .disabled(reader.busy)
                    .accessibilityIdentifier("computers-toggle")
                }
                Button("Back to world", systemImage: "xmark") { close() }
                    .labelStyle(.iconOnly).accessibilityIdentifier("computer-close")
            }
            if let error = inputError ?? reader.nativeError ?? reader.packet?.error {
                Text(error).font(.callout).textSelection(.enabled).accessibilityIdentifier("reader-error")
            }
            if reader.busy {
                HStack { ProgressView(); Text("Connecting or updating…").font(.caption) }
                    .accessibilityIdentifier("computer-busy")
            }
            if computers {
                computersContent
            } else if !paired || pairing {
                pairingControls
            } else {
                if !reading { HStack {
                    Text("Read-only").font(.caption).accessibilityIdentifier("reader-read-only")
                    Spacer()
                    Button("Connect computer") { pairing = true }
                        .accessibilityIdentifier("computer-pair")
                    refreshButton
                } }
                if let view = reader.packet?.view {
                    NativeRenderer(node: view.root, revision: view.revision,
                                   followTarget: reader.packet?.follow_target,
                                   followChanged: followAction(page: reader.packet?.follow_page)) { node in
                        reader.activate(view: view, node: node)
                    }
                } else {
                    Text("Saved chats appear when the computer finishes connecting.")
                }
                if !reading { DisclosureGroup("Device details") {
                    Text(reader.packet?.public_key ?? "Unavailable")
                        .font(.system(.caption2, design: .monospaced)).textSelection(.enabled)
                        .accessibilityIdentifier("reader-public-key")
                    if disconnecting {
                        Text("Erase cached chats on this phone? The computer's files stay unchanged.")
                            .font(.caption)
                        HStack {
                            Button("Disconnect and erase", role: .destructive) { reader.disconnect(); disconnecting = false }
                            Button("Keep connection") { disconnecting = false }
                        }
                    } else {
                        Button("Disconnect this computer", role: .destructive) { disconnecting = true }
                    }
                }.font(.caption) }
            }
            if !reading && !computers { DisclosureGroup("World connection") {
                Text("Joining publishes this device's world presence and movement with a separate Verse identity.")
                    .font(.caption)
                TextField("wss://relay.example.com", text: $relay)
                    .keyboardType(.URL).textInputAutocapitalization(.never).autocorrectionDisabled()
                    .accessibilityLabel("World relay URL")
                HStack {
                    Button("Join relay") { worldAction(["action": "connect", "relay": relay]) }
                        .disabled(relay.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !active)
                    Button("Leave relay") { worldAction(["action": "disconnect"]) }
                }
            }.font(.caption) }
            Text(reader.packet?.status ?? "Opening protected local state")
                .font(.caption2).foregroundStyle(.secondary).accessibilityIdentifier("reader-status")
        }
        .padding(14)
        .background(Color(red: 0.025, green: 0.02, blue: 0).opacity(0.97), in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(.tint.opacity(0.7), lineWidth: 1))
        .onAppear { reader.setForeground(active) }
        .onDisappear { scanning = false; computersScanning = false; reader.setForeground(false) }
        .onChange(of: reader.packet?.computers_exit) { _, exit in
            // First run finished: return to the existing pairing onboarding.
            if exit == true { computers = false; computersScanning = false }
        }
        .onChange(of: active) { _, current in
            reader.setForeground(current)
        }
        .onReceive(timer) { _ in
            if active, paired, !pairing, !scanning, !computers { reader.refresh() }
        }
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
            TextField(input.label, text: $computersValue, axis: .vertical)
                .lineLimit(1...4)
                .autocorrectionDisabled().textInputAutocapitalization(.never)
                .accessibilityLabel(input.label).accessibilityIdentifier("computers-input")
            Button("Submit") { submitComputers(input, computersValue) }
                .disabled(reader.busy || computersValue.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .accessibilityIdentifier("computers-submit")
        }
        .onAppear { computersValue = ""; computersScanning = input.scan && active }
    }

    private func submitComputers(_ input: ComputersInput, _ value: String) {
        guard value.utf8.count <= input.max_bytes else {
            inputError = "That's too long. Copy it again."
            return
        }
        inputError = nil
        reader.submitComputers(token: input.token, value: value)
        computersValue = ""
    }

    private var pairingControls: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("Connect your computer").font(.headline)
                Text("Run this in the OpenAgents repository on your computer, then scan its QR invitation.")
                Text(command).font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                    .accessibilityIdentifier("computer-command")
                Button(copied ? "Command copied" : "Copy command", systemImage: "doc.on.doc") {
                    UIPasteboard.general.string = command
                    copied = true
                }
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
                    Button("Connect to computer") { submit(code); code = "" }
                        .disabled(reader.busy || code.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        .accessibilityIdentifier("computer-connect")
                }
                Text("This connection can read saved chats. It cannot run commands or approve work.")
                    .font(.caption).foregroundStyle(.secondary)
                if paired {
                    Button("Back to chats") { pairing = false; scanning = false; pasting = false }
                        .accessibilityIdentifier("computer-chats")
                }
            }.frame(maxWidth: .infinity, alignment: .leading)
        }.scrollDismissesKeyboard(.interactively)
    }

    private func submit(_ value: String) {
        guard value.utf8.count <= 65_536 else {
            inputError = "The connection code is too large. Copy a fresh invitation from the computer."
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
