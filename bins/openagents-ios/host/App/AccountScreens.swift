// The Account tab's own screens: the trainer card, identity keys, this
// device, and the changelog. Rust derives every key form and owns the changelog; these views
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
            .font(.paper(.footnote))
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
    @State private var displayName = ""

    var body: some View {
        List {
            Section {
                TextField("Shown over your head", text: $displayName)
                    .textInputAutocapitalization(.words)
                    .autocorrectionDisabled()
                    .submitLabel(.done)
                    .onSubmit { bridge.setDisplayName(displayName) { account = $0; displayName = $0.display_name ?? "" } }
                    .accessibilityIdentifier("identity-display-name")
            } header: {
                Text("Display name")
            } footer: {
                Text("Other players in the Grid read this over your avatar. Up to 24 letters, digits, and punctuation.")
            }
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
                        .font(.paper(.footnote))
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
            bridge.account {
                account = $0
                displayName = $0.display_name ?? ""
                UserDefaults.standard.set($0.display_name, forKey: VerseWorld.displayNameKey)
            }
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

/// What each build brought and what to test in it, newest first.
struct ChangelogScreen: View {
    @ObservedObject var bridge: MobileBridge
    @State private var releases: [Release] = []

    var body: some View {
        List {
            ForEach(releases, id: \.self) { release in
                Section {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("What to test").font(.paper(.subheadline, weight: .semibold))
                        Text(release.what_to_test).font(.paper(.subheadline))
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .padding(.vertical, 2)
                    .accessibilityIdentifier("changelog-what-to-test")
                    ForEach(release.items, id: \.self) { item in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(item.title).font(.paper(.body, weight: .semibold))
                            Text(item.detail).font(.paper(.subheadline)).foregroundStyle(.secondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        .padding(.vertical, 2)
                    }
                } header: {
                    HStack(alignment: .firstTextBaseline) {
                        Text("\(release.version) (\(release.build))").font(.paper(.headline)).foregroundStyle(.white)
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

/// The trainer card: the level over this player's head in the Grid, the
/// curve it uses, and the counted awards behind it, derived in Rust from
/// signed NIP-XP awards by the OpenAgents referee. The trainer key is the
/// Verse world key; its nsec shows only after an explicit reveal, to sign a
/// reproduction with on a computer.
struct TrainerScreen: View {
    @ObservedObject var bridge: MobileBridge
    @Environment(\.scenePhase) private var scenePhase
    @State private var card: TrainerPacket?
    @State private var nsec: String?
    @State private var warning = false
    @State private var showWarning = false
    @State private var linking = false
    @State private var linkKey = ""
    @State private var exporting = false
    @State private var export: CardExport?
    @State private var cardFile: ExitFile?
    private let refresh = Timer.publish(every: 2, on: .main, in: .common).autoconnect()

    private var progress: Double {
        guard let card, card.next_level_at > 0 else { return 0 }
        // Share of the way from this level's start to the next level.
        let reached = Double(card.xp)
        let next = Double(card.next_level_at)
        let base = Self.levelStart(card)
        return next > base ? min(1, max(0, (reached - base) / (next - base))) : 0
    }

    /// Cumulative XP at which the card's level starts, under
    /// trainer-curve-v1: ceil(100 · (n - 1)^1.5).
    private static func levelStart(_ card: TrainerPacket) -> Double {
        let n = Double(card.level) - 1
        return n <= 0 ? 0 : (100 * pow(n, 1.5)).rounded(.up)
    }

    var body: some View {
        List {
            if card?.state == "preview" {
                Section {
                    Label("Preview: a labeled fixture of six tutorial reproductions, not real awards.",
                          systemImage: "flask")
                        .font(.paper(.footnote))
                        .foregroundStyle(.yellow)
                }
            }
            Section {
                if let card {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(alignment: .firstTextBaseline) {
                            Text("Level \(card.level)").font(.paper(.largeTitle, weight: .bold))
                            Spacer()
                            Text("\(card.xp) XP").font(.paper(.title3))
                        }
                        ProgressView(value: progress)
                            .tint(.white)
                            .accessibilityIdentifier("trainer-progress")
                        Text("\(card.to_next) XP to level \(card.level + 1) · \(card.curve)")
                            .font(.paper(.footnote))
                            .foregroundStyle(.secondary)
                        Text("Over your head in the Grid: \(card.tag)\(card.xp > 0 && card.profile == "shown" ? " · lv \(card.level)" : "")")
                            .font(.paper(.footnote))
                            .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 4)
                    .accessibilityIdentifier("trainer-card")
                } else {
                    Text("Reading your XP…").foregroundStyle(.secondary)
                }
            } footer: {
                if let card, card.state != "ready", card.state != "preview" {
                    Text("Reading awards from \(card.relay)…")
                }
            }
            if let card {
                Section {
                    if card.profile == "shown" {
                        Label("Shown in the Grid and on boards", systemImage: "eye")
                        Button("Hide my level", systemImage: "eye.slash") {
                            bridge.trainerProfile(shown: false) { self.card = $0 }
                        }
                        .disabled(card.profile_status == "publishing")
                        .accessibilityIdentifier("trainer-hide-level")
                    } else {
                        Label(card.profile == "hidden" ? "Hidden" : "Not shown yet", systemImage: "eye.slash")
                            .foregroundStyle(.secondary)
                        Button("Show my level", systemImage: "eye") { showWarning = true }
                            .disabled(card.profile_status == "publishing")
                            .accessibilityIdentifier("trainer-show-level")
                    }
                    if card.profile_status == "publishing" {
                        HStack { ProgressView(); Text("Publishing…").foregroundStyle(.secondary) }
                    }
                    if let error = card.profile_error {
                        Text(error).font(.paper(.footnote)).foregroundStyle(.red)
                    }
                } header: {
                    Text("Level over your head")
                } footer: {
                    Text("Other players see your level only after you choose to show it. Your XP stays public either way.")
                }
            }
            if let card {
                Section {
                    if let trainer = card.linked_to {
                        Text("This key is linked to the trainer \(String(trainer.prefix(16)))…, so its XP counts there.")
                            .font(.paper(.footnote))
                            .foregroundStyle(.secondary)
                    }
                    ForEach(card.linked_keys, id: \.self) { key in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(String(key.npub.prefix(20)) + "…").font(.paper(.body))
                                Text(key.status == "linked" ? "Linked both ways: its XP counts here"
                                                            : "Waiting for this key to link back")
                                    .font(.paper(.caption))
                                    .foregroundStyle(key.status == "linked" ? .green : .secondary)
                            }
                            Spacer()
                            Button("Remove", role: .destructive) {
                                bridge.trainerLink(remove: key.public_hex) { self.card = $0 }
                            }
                            .buttonStyle(.borderless)
                            .disabled(card.profile_status == "publishing")
                        }
                    }
                    Button("Link a key", systemImage: "link") { linkKey = ""; linking = true }
                        .disabled(card.profile_status == "publishing")
                        .accessibilityIdentifier("trainer-link-key")
                } header: {
                    Text("Linked keys")
                } footer: {
                    Text("Sign work on a computer with its own key and have it count here, without moving your trainer key. Enter that key's npub, then on the computer run: microcoder xp link --relay \(card.relay) --trainer \(card.npub)")
                        .textSelection(.enabled)
                }
            }
            if let card {
                Section {
                    Button("Export card", systemImage: "square.and.arrow.up") { exporting = true }
                        .accessibilityIdentifier("trainer-export")
                    if let export {
                        if let error = export.error {
                            Text(error).font(.paper(.footnote)).foregroundStyle(.red)
                        }
                        if let link = export.link, let url = URL(string: link) {
                            ShareLink(item: url) { Label("Share link", systemImage: "link") }
                            Text(link).font(.paper(.caption)).foregroundStyle(.secondary)
                                .textSelection(.enabled)
                        }
                        if let json = export.json, let name = export.file_name {
                            Button("Save card JSON", systemImage: "doc") {
                                cardFile = ExitFile(name: name, text: json)
                            }
                        }
                        Text(export.preview ? "Preview: signed, not published."
                             : card.card_status == "published" ? "Published to \(card.relay)."
                             : card.card_status == "failed" ? "The relay didn't take the card. Export again."
                             : "Publishing…")
                            .font(.paper(.footnote))
                            .foregroundStyle(card.card_status == "failed" ? .red : .secondary)
                    }
                } header: {
                    Text("Trainer card")
                } footer: {
                    Text("A signed summary of your level, keys, and counted awards. Anyone can check it with openagents xp verify-card.")
                }
            }
            if let card, !card.titles.isEmpty {
                Section("Titles") {
                    Text(card.titles.joined(separator: ", "))
                }
            }
            if let card {
                Section {
                    if card.awards.isEmpty {
                        Text("No awards yet. Reproduce a published pass from its recipe to earn your first; a tutorial quest is worth 50 XP.")
                            .font(.paper(.subheadline))
                            .foregroundStyle(.secondary)
                    }
                    ForEach(card.awards, id: \.self) { award in
                        if let url = URL(string: award.link) {
                            Link(destination: url) {
                                VStack(alignment: .leading, spacing: 2) {
                                    HStack(alignment: .firstTextBaseline) {
                                        Text(award.title).font(.paper(.body, weight: .semibold))
                                        Spacer()
                                        Text("+\(award.xp) XP").font(.paper(.subheadline))
                                    }
                                    Text("\(award.quest) · \(award.role) · \(award.season)")
                                        .font(.paper(.caption))
                                        .foregroundStyle(.secondary)
                                }
                            }
                            .foregroundStyle(.white)
                        }
                    }
                    if let guide = URL(string: "https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/tutorial-quests.md") {
                        Link(destination: guide) {
                            Label(card.open_quests == 1 ? "1 open tutorial quest" : "\(card.open_quests) open quests",
                                  systemImage: "flag.checkered")
                        }
                    }
                } header: {
                    Text("Counted awards")
                } footer: {
                    Text("Counted by OpenAgents. \(card.note)")
                }
                Section {
                    KeyText(value: card.npub)
                    CopyButton(title: "Copy npub", value: card.npub)
                    if let nsec {
                        KeyText(value: nsec, copyable: false)
                        CopyButton(title: "Copy nsec", value: nsec, secret: true)
                        Button("Hide nsec", systemImage: "eye.slash") { self.nsec = nil }
                    } else {
                        Button("Reveal nsec", systemImage: "eye") { warning = true }
                            .foregroundStyle(.red)
                            .accessibilityIdentifier("trainer-reveal")
                    }
                } header: {
                    Text("Trainer key")
                } footer: {
                    Text("Your trainer key is your Verse world key: the one over your head in the Grid. Sign a reproduction with it on your computer (`microcoder xp reproduce --key`), and the award shows here and in the Grid.")
                }
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
        .background(Color.black.ignoresSafeArea())
        .fileExporter(isPresented: Binding(get: { cardFile != nil }, set: { if !$0 { cardFile = nil } }),
                      document: cardFile, contentType: .json,
                      defaultFilename: cardFile?.name ?? "trainer-card.json") { _ in cardFile = nil }
        .alert("Export your trainer card?", isPresented: $exporting) {
            Button("Export") {
                bridge.trainerExport { export = $0 }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This signs your card with your trainer key and publishes it to relay.openagents.com, so its link opens a public page. It lists your level, your linked keys, and your counted awards.")
        }
        .alert("Link a key", isPresented: $linking) {
            TextField("npub1…", text: $linkKey)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
            Button("Link") {
                bridge.trainerLink(add: linkKey) { self.card = $0 }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This publishes your trainer profile listing the key. Its XP counts here only after that key signs a link back to you.")
        }
        .alert("Show your level?", isPresented: $showWarning) {
            Button("Show") {
                bridge.trainerProfile(shown: true) { self.card = $0 }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This publishes a trainer profile signed by your trainer key to relay.openagents.com. Your level then shows over your head in the Grid and on boards. You can hide it again at any time.")
        }
        .alert("Reveal your trainer nsec?", isPresented: $warning) {
            Button("Reveal", role: .destructive) {
                bridge.trainer(reveal: true) { packet in nsec = packet.nsec }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Anyone with this nsec can act as you in Verse and sign work in your name. It can't reach your computers or your wallet. Never share it, and make sure no one can see your screen.")
        }
        .onAppear { bridge.trainer { card = $0 } }
        .onReceive(refresh) { _ in
            guard scenePhase == .active, card?.state != "preview" else { return }
            bridge.trainer { card = $0 }
        }
        .onDisappear { nsec = nil }
        .onChange(of: scenePhase) { _, phase in if phase != .active { nsec = nil } }
    }
}

/// Account > Your keys (BYOK, #10176): the person's own OpenRouter, Vercel
/// AI Gateway, and TypeSafe keys. Rust tests each key and writes every row;
/// this screen collects a key in a secure field (never shown, never kept in
/// a draft) and never draws one, only its last four characters.
struct YourKeysScreen: View {
    @ObservedObject var bridge: MobileBridge
    @State private var adding: ProviderKeysState.Row?
    @State private var removing: ProviderKeysState.Row?

    private var state: ProviderKeysState? { bridge.packet?.provider_keys }

    var body: some View {
        List {
            if let state {
                Section {
                    Text(state.status)
                        .accessibilityIdentifier("keys-status")
                    Toggle("Use my keys for everything", isOn: Binding(
                        get: { state.mine },
                        set: { bridge.providerKeysMine($0) }))
                        .disabled(!state.mine && state.mine_blocked != nil)
                        .accessibilityIdentifier("keys-mine")
                } footer: {
                    Text(state.mine_blocked.map { state.mine ? "" : $0 } ?? "Chat replies, Jev, and search run on your own provider accounts. Nothing falls back to OpenAgents.")
                }
                ForEach(state.rows) { row in
                    Section {
                        HStack {
                            Text(row.last_four.map { "Ends in \($0)" } ?? "Not added")
                                .foregroundStyle(row.last_four == nil ? .secondary : .primary)
                            Spacer()
                            if row.checking {
                                ProgressView()
                            } else if let word = row.state {
                                Text(word).foregroundStyle(word == "works" ? .green : .orange)
                            }
                        }
                        if let line = row.line {
                            Text(line).font(.paper(.footnote)).foregroundStyle(.secondary)
                        }
                        Button(row.last_four == nil ? "Add key" : "Replace key", systemImage: "key") { adding = row }
                            .accessibilityIdentifier("keys-\(row.provider)-add")
                        if row.last_four != nil {
                            Button("Test", systemImage: "checkmark.circle") { bridge.providerKeyTest(row.provider) }
                                .disabled(row.checking)
                                .accessibilityIdentifier("keys-\(row.provider)-test")
                            Button("Remove", systemImage: "trash", role: .destructive) { removing = row }
                                .accessibilityIdentifier("keys-\(row.provider)-remove")
                        }
                        if let url = URL(string: row.page) {
                            Link("Make a key at \(row.name)", destination: url).font(.paper(.footnote))
                        }
                    } header: {
                        Text(row.name)
                    }
                }
                if let notice = bridge.providerKeyError ?? state.notice {
                    Section { Text(notice).font(.paper(.footnote)).accessibilityIdentifier("keys-notice") }
                }
            } else {
                Text("Your keys load in a moment.").foregroundStyle(.secondary)
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
        .background(Color.black.ignoresSafeArea())
        .sheet(item: $adding) { row in
            ProviderKeyEntry(row: row) { key in bridge.providerKeyAdd(row.provider, key: key) }
        }
        .confirmationDialog("Remove your \(removing?.name ?? "") key?", isPresented: Binding(
            get: { removing != nil }, set: { if !$0 { removing = nil } }), titleVisibility: .visible) {
            Button("Remove", role: .destructive) {
                if let row = removing { bridge.providerKeyRemove(row.provider) }
                removing = nil
            }
        }
        .alert("Use your keys for everything?", isPresented: $bridge.askMine) {
            Button("Use my keys") { bridge.providerKeysMine(true) }
            Button("Not now", role: .cancel) {}
        } message: {
            Text("Chat replies, Jev, and search will run on your own provider accounts, never on OpenAgents.")
        }
    }
}

/// The secure field for one provider's key (`provider_key`): it never shows
/// what was typed or pasted, offers no autofill or suggestions, and forgets
/// the value when it closes.
private struct ProviderKeyEntry: View {
    let row: ProviderKeysState.Row
    let submit: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var key = ""

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    SecureField(row.input.label, text: $key)
                        .textContentType(.password)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                        .privacySensitive()
                        .accessibilityIdentifier("keys-entry")
                } footer: {
                    Text(row.input.prompt)
                }
            }
            .navigationTitle(row.name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { key = ""; dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") {
                        submit(key)
                        key = ""
                        dismiss()
                    }
                    .disabled(key.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                              || key.utf8.count > row.input.max_bytes)
                }
            }
        }
        .onDisappear { key = "" }
    }
}
