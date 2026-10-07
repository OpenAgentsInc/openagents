// The world computer's Chats page: the read-only reader, its pairing, and the
// world connection. The Computers screens draw in the world HUD instead. Rust
// validates pairing and projects every history row; this page grants no task
// authority.
import SwiftUI
import UIKit

struct ComputerPanel: View {
    @ObservedObject var reader: MobileBridge
    let active: Bool
    let close: () -> Void
    /// Return to the Computers screens in the world HUD.
    let computers: () -> Void
    let worldAction: ([String: Any]) -> Void
    let worldConnection: VerseConnection?
    let worldStorageError: String?
    let worldCredits: String?
    @Binding var pairing: Bool
    @State private var pasting = false
    @State private var scanning = false
    @State private var code = ""
    @State private var inputError: String?
    @State private var disconnecting = false
    @State private var copied = false
    @State private var relay = ""
    @State private var settings = false
    @State private var aboutVerse = false
    private let command = "openagents pair"
    private var paired: Bool { reader.packet?.paired == true }
    private var reading: Bool { reader.packet?.reading == true && !pairing }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(settings ? "Settings" : "Chats").font(.paper(.headline))
                Spacer()
                if !settings && paired && !pairing { refreshButton }
                Button("Computers", systemImage: "desktopcomputer") { computers() }
                    .labelStyle(.iconOnly).accessibilityIdentifier("computers-toggle")
                Button {
                    if settings {
                        settings = false
                        pairing = false
                    } else {
                        settings = true
                    }
                    scanning = false
                } label: {
                    Image(systemName: settings ? "chevron.left" : "ellipsis")
                }
                .accessibilityLabel(settings ? "Back to chats" : "Computer settings")
                .accessibilityIdentifier("computer-settings")
                Button("Back to world", systemImage: "xmark") { close() }
                    .labelStyle(.iconOnly).accessibilityIdentifier("computer-close")
            }
            if let error = inputError ?? reader.nativeError ?? reader.packet?.error {
                Text(error).font(.paper(.caption)).textSelection(.enabled).accessibilityIdentifier("reader-error")
            }
            if reader.busy {
                HStack { ProgressView(); Text("Loading…").font(.paper(.caption)) }
                    .accessibilityIdentifier("computer-busy")
            }
            if settings {
                settingsContent
            } else if !paired || pairing {
                pairingControls
            } else if let view = reader.packet?.view {
                NativeRenderer(node: view.root, revision: view.revision,
                               followTarget: reader.packet?.follow_target,
                               followChanged: followAction(page: reader.packet?.follow_page)) { node in
                    reader.activate(view: view, node: node)
                }
            } else {
                Text("Loading chats…").font(.paper(.caption))
            }
        }
        .padding(12)
        .background(Color(red: 0.025, green: 0.02, blue: 0).opacity(0.97), in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(.tint.opacity(0.7), lineWidth: 1))
        .onAppear {
            relay = worldConnection?.relay ?? "wss://relay.openagents.com"
            reader.setForeground(active)
        }
        .onDisappear { scanning = false; reader.setForeground(false) }
        .onChange(of: worldConnection?.relay) { _, value in
            relay = value ?? "wss://relay.openagents.com"
        }
        .onChange(of: active) { _, current in reader.setForeground(current) }
        .task(id: active && paired && !pairing && !scanning && !settings) {
            guard active && paired && !pairing && !scanning && !settings else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(5))
                if !Task.isCancelled { reader.refresh() }
            }
        }
    }

    private var settingsContent: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Button("Pair computer", systemImage: "qrcode") {
                    settings = false; pairing = true
                }.accessibilityIdentifier("computer-pair")
                Divider()
                VStack(alignment: .leading, spacing: 8) {
                    Text("World connection").font(.paper(.headline))
                    Text(worldConnection?.label ?? "Offline")
                        .font(.paper(.caption)).accessibilityIdentifier("world-connection-status")
                    if let error = worldStorageError ?? worldConnection?.error {
                        Text(error).font(.paper(.caption)).textSelection(.enabled)
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
                    Text("Shares your avatar and movement.").font(.paper(.caption2)).foregroundStyle(.secondary)
                }
                Divider()
                DisclosureGroup(isExpanded: $aboutVerse) {
                    Text(worldCredits ?? "Loading notices…")
                        .font(.paper(.caption2)).textSelection(.enabled)
                        .accessibilityIdentifier("verse-credits")
                } label: {
                    Text("About Verse").accessibilityIdentifier("verse-about")
                }
                .onChange(of: aboutVerse) { _, expanded in
                    if expanded && worldCredits == nil { worldAction(["action": "zone_credits"]) }
                }
                Divider()
                DisclosureGroup("Device details") {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(reader.packet?.public_key ?? "Unavailable")
                            .font(.paper(.caption2)).textSelection(.enabled)
                            .accessibilityIdentifier("reader-public-key")
                        Text(reader.packet?.status ?? "Opening…")
                            .font(.paper(.caption2)).accessibilityIdentifier("reader-status")
                        if let wakes = reader.pushStatus {
                            Text(wakes).font(.paper(.caption2)).foregroundStyle(.secondary)
                                .accessibilityIdentifier("push-status")
                        }
                        if paired {
                            if disconnecting {
                                Text("Erase cached chats on this phone?").font(.paper(.caption))
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
                }.font(.paper(.caption))
            }.frame(maxWidth: .infinity, alignment: .leading)
        }
        .scrollDismissesKeyboard(.interactively)
    }

    private var refreshButton: some View {
        Button("Refresh", systemImage: "arrow.clockwise") { reader.refresh(force: true) }
            .labelStyle(.iconOnly).disabled(reader.busy).accessibilityIdentifier("reader-refresh")
    }

    private var pairingControls: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("In a terminal on the computer:")
                Text(command).font(.paper(.caption)).textSelection(.enabled)
                    .accessibilityIdentifier("computer-command")
                Button(copied ? "Copied" : "Copy command", systemImage: "doc.on.doc") {
                    UIPasteboard.general.string = command
                    copied = true
                }
                Text("Then scan the QR code.").font(.paper(.caption))
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
                    .font(.paper(.caption)).foregroundStyle(.secondary)
                DisclosureGroup("Setup help") {
                    Text("Install the openagents command on the computer first. Keep the command running while you read chats.")
                        .font(.paper(.caption)).textSelection(.enabled)
                }.font(.paper(.caption))
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
