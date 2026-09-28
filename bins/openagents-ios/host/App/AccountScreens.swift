// The Account tab's own screens: identity keys, this device, and the
// changelog. Rust derives every key form and owns the changelog; these views
// only show and copy what it returns.
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Copy a value. A secret goes to this device's pasteboard only, never to
/// Universal Clipboard, and expires after a minute.
enum KeyCopy {
    static func copy(_ value: String, secret: Bool = false) {
        if secret {
            UIPasteboard.general.setItems(
                [[UTType.plainText.identifier: value]],
                options: [.localOnly: true, .expirationDate: Date().addingTimeInterval(60)])
        } else {
            UIPasteboard.general.string = value
        }
    }
}

/// A key in monospaced type, broken into whole lines by hand so the text
/// system never hyphenates it. A public key copies from its context menu.
private struct KeyText: View {
    let value: String?
    var copyable = true

    private var lines: String {
        guard let value else { return "Not available yet." }
        var rest = Substring(value)
        var lines: [Substring] = []
        while !rest.isEmpty {
            lines.append(rest.prefix(32))
            rest = rest.dropFirst(32)
        }
        return lines.joined(separator: "\n")
    }

    var body: some View {
        Text(verbatim: lines)
            .font(.system(.footnote, design: .monospaced))
            .foregroundStyle(value == nil ? .secondary : .primary)
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityLabel(value ?? "Not available yet.")
            .contextMenu {
                if copyable, let value {
                    Button("Copy", systemImage: "doc.on.doc") { KeyCopy.copy(value) }
                }
            }
    }
}

/// A copy control that confirms briefly.
private struct CopyButton: View {
    let title: String
    let value: String?
    var secret = false
    @State private var copied = false

    var body: some View {
        Button {
            guard let value else { return }
            KeyCopy.copy(value, secret: secret)
            copied = true
            Task {
                try? await Task.sleep(for: .seconds(1.5))
                copied = false
            }
        } label: {
            Label(copied ? "Copied" : title, systemImage: copied ? "checkmark" : "doc.on.doc")
        }
        .disabled(value == nil)
    }
}

/// This device's Nostr identity: the npub first, hex beside it, and the
/// nsec only after an explicit reveal and warning.
struct IdentityKeysScreen: View {
    @ObservedObject var bridge: MobileBridge
    @Environment(\.scenePhase) private var scenePhase
    @State private var account: AccountPacket?
    @State private var nsec: String?
    @State private var warning = false

    var body: some View {
        List {
            Section {
                KeyText(value: account?.npub)
                CopyButton(title: "Copy npub", value: account?.npub)
            } header: {
                Text("Public key")
            } footer: {
                Text("Share your npub freely. It identifies this device.")
            }
            Section("Public key (hex)") {
                KeyText(value: account?.public_hex)
                CopyButton(title: "Copy hex", value: account?.public_hex)
            }
            Section {
                if let nsec {
                    KeyText(value: nsec, copyable: false)
                        .accessibilityIdentifier("identity-nsec")
                    CopyButton(title: "Copy nsec", value: nsec, secret: true)
                    Button("Hide nsec", systemImage: "eye.slash") { self.nsec = nil }
                } else {
                    Text(String(repeating: "•", count: 24))
                        .font(.system(.footnote, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .accessibilityLabel("Hidden")
                    Button("Reveal nsec", systemImage: "eye") { warning = true }
                        .foregroundStyle(.red)
                        .disabled(account == nil)
                        .accessibilityIdentifier("identity-reveal")
                }
            } header: {
                Text("Secret key")
            } footer: {
                Text(account?.origin ?? "")
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
        .background(Color.black.ignoresSafeArea())
        .alert("Reveal your nsec?", isPresented: $warning) {
            Button("Reveal", role: .destructive) {
                bridge.account(reveal: true) { packet in nsec = packet.nsec }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Anyone with your nsec can act as this device on your computers. Never share it, and make sure no one can see your screen.")
        }
        .onAppear {
            bridge.account { account = $0 }
            #if targetEnvironment(simulator)
            // `--identity-script warn` shows the warning and `reveal` answers
            // it, so a simulator check needs no taps.
            let arguments = ProcessInfo.processInfo.arguments
            if let index = arguments.firstIndex(of: "--identity-script"), index + 1 < arguments.count {
                switch arguments[index + 1] {
                case "warn": warning = true
                case "reveal": bridge.account(reveal: true) { packet in nsec = packet.nsec }
                default: break
                }
            }
            #endif
        }
        // The nsec shows only while this screen is open and in front.
        .onDisappear { nsec = nil }
        .onChange(of: scenePhase) { _, phase in if phase != .active { nsec = nil } }
    }
}

/// This device's public key and the app's version.
struct AboutDeviceScreen: View {
    @ObservedObject var bridge: MobileBridge
    @State private var origin: String?

    private var version: String {
        let info = Bundle.main.infoDictionary
        let short = info?["CFBundleShortVersionString"] as? String ?? "?"
        let build = info?["CFBundleVersion"] as? String ?? "?"
        return "\(short) (\(build))"
    }

    var body: some View {
        List {
            Section {
                KeyText(value: bridge.packet?.device_npub)
                KeyText(value: bridge.packet?.device)
            } header: {
                Text("Device public key")
            } footer: {
                Text(origin ?? "")
            }
            Section("App version") {
                Text(version).textSelection(.enabled)
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
        .background(Color.black.ignoresSafeArea())
        .onAppear { bridge.account { origin = $0.origin } }
    }
}

/// What each release brought, newest first.
struct ChangelogScreen: View {
    @ObservedObject var bridge: MobileBridge
    @State private var releases: [Release] = []

    var body: some View {
        List {
            ForEach(releases, id: \.self) { release in
                Section {
                    ForEach(release.items, id: \.self) { item in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(item.title).font(.body.weight(.semibold))
                            Text(item.detail).font(.subheadline).foregroundStyle(.secondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        .padding(.vertical, 2)
                    }
                } header: {
                    HStack(alignment: .firstTextBaseline) {
                        Text(release.version).font(.headline).foregroundStyle(.white)
                        Text(release.title)
                        Spacer()
                    }
                    .textCase(nil)
                }
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
        .background(Color.black.ignoresSafeArea())
        .onAppear { bridge.account { releases = $0.changelog } }
    }
}
