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
    private let command = "cargo run --release -p coder-connect -- connect"
    private let timer = Timer.publish(every: 5, on: .main, in: .common).autoconnect()
    private var paired: Bool { reader.packet?.paired == true }
    private var reading: Bool { reader.packet?.reading == true && !pairing }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Label("Computer", systemImage: "desktopcomputer").font(.headline)
                Spacer()
                if reading { refreshButton }
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
            if !paired || pairing {
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
            if !reading { DisclosureGroup("World connection") {
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
        .onDisappear { scanning = false; reader.setForeground(false) }
        .onChange(of: active) { _, current in
            reader.setForeground(current)
        }
        .onReceive(timer) { _ in
            if active, paired, !pairing, !scanning { reader.refresh() }
        }
    }

    private var refreshButton: some View {
        Button("Refresh", systemImage: "arrow.clockwise") { reader.refresh(force: true) }
            .labelStyle(.iconOnly).disabled(reader.busy).accessibilityIdentifier("reader-refresh")
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
