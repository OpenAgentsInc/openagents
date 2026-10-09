// Report a problem, My reports, and Account > Playtest. Rust fills in and
// checks every report, decides whether a screenshot may be offered, seals
// the report to the triage key, and keeps the playtest log; these views only
// collect what the tester writes and chooses, and show the exact screenshot
// and log before anything is sent.
import SwiftUI
import UIKit

/// Where the tester is, as Rust names tabs and screens.
@MainActor
final class PlaytestPlace: ObservableObject {
    @Published var tab: AppTab = AppTabLaunch.tab
    @Published var accountRoute: AccountRoute?

    // The account surface counts as the Chat tab for reports.
    var tabName: String { tab == .link ? AppTab.coder.rawValue : tab.rawValue }

    var routeName: String {
        guard tab == .account, let route = accountRoute else { return "home" }
        return route.rawValue
    }
}

/// The device facts a report carries.
enum ReportDevice {
    static var version: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0"
    }
    static var build: String { Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "0" }
    static var os: String { UIDevice.current.systemVersion }
    static var model: String {
        if let simulated = ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"] {
            return simulated
        }
        var system = utsname()
        uname(&system)
        return withUnsafeBytes(of: &system.machine) { bytes in
            String(decoding: bytes.prefix(while: { $0 != 0 }), as: UTF8.self)
        }
    }
}

/// One open Report a problem sheet: Rust's form and, when Rust allows one,
/// the screen as it was when the tester asked.
struct ReportSession: Identifiable {
    let id = UUID()
    var draft: ReportDraft
    let image: UIImage?
}

/// Opens the Report a problem sheet from Account or a long press on the
/// tab bar.
@MainActor
final class ReportCoordinator: ObservableObject {
    @Published var session: ReportSession?
    /// Give feedback on selected text, from the selection menu (#10127).
    @Published var feedback: FeedbackRequest?

    func start(bridge: MobileBridge, place: PlaytestPlace) {
        bridge.reportDraft(tab: place.tabName, route: place.routeName) { draft in
            // Rust says whether this screen may be captured; the Wallet and
            // key screens never are.
            let image = draft.screenshot_allowed ? Self.capture() : nil
            self.session = ReportSession(draft: draft, image: image)
        }
    }

    private static func capture() -> UIImage? {
        guard let window = UIApplication.shared.connectedScenes
            .compactMap({ $0 as? UIWindowScene })
            .flatMap(\.windows)
            .first(where: \.isKeyWindow) else { return nil }
        let renderer = UIGraphicsImageRenderer(bounds: window.bounds)
        return renderer.image { _ in
            window.drawHierarchy(in: window.bounds, afterScreenUpdates: false)
        }
    }
}

/// The screenshot, cropped at the top and bottom, as the JPEG that would be
/// sent. Nil when it can't be made small enough to send.
enum ReportImage {
    static func cropped(_ image: UIImage, top: Double, bottom: Double) -> UIImage? {
        guard let cg = image.cgImage else { return nil }
        let height = Double(cg.height)
        let from = (height * top).rounded()
        let to = (height * (1 - bottom)).rounded()
        guard to - from >= 16,
              let part = cg.cropping(to: CGRect(x: 0, y: from, width: Double(cg.width), height: to - from))
        else { return nil }
        return UIImage(cgImage: part)
    }

    static func jpeg(_ image: UIImage) -> (data: Data, width: Int, height: Int)? {
        for (width, quality) in [(360.0, 0.55), (300.0, 0.4), (240.0, 0.3)] {
            let scale = min(1, width / image.size.width)
            let size = CGSize(width: (image.size.width * scale).rounded(),
                              height: (image.size.height * scale).rounded())
            let format = UIGraphicsImageRendererFormat()
            format.scale = 1
            let small = UIGraphicsImageRenderer(size: size, format: format).image { _ in
                image.draw(in: CGRect(origin: .zero, size: size))
            }
            if let data = small.jpegData(compressionQuality: quality), data.count <= 24_000 {
                return (data, Int(size.width), Int(size.height))
            }
        }
        return nil
    }
}

/// Report a problem: what happened, what you expected, the steps, and a
/// kind. Everything that goes with it is shown before it is sent.
struct ReportSheet: View {
    @State var session: ReportSession
    @ObservedObject var bridge: MobileBridge
    @Environment(\.dismiss) private var dismiss
    @State private var kind = "bug"
    @State private var happened = ""
    @State private var expected = ""
    @State private var steps = ""
    @State private var quote = false
    @State private var includeTask = false
    @State private var includeShot = false
    @State private var includeLog = true
    @State private var includeChat = false
    @State private var cropTop = 0.0
    @State private var cropBottom = 0.0
    @State private var sending = false
    @State private var error: String?
    @State private var sent: ReportRow?

    private var draft: ReportDraft { session.draft }
    private var hint: String { draft.kinds.first { $0.value == kind }?.hint ?? "" }
    private var shot: UIImage? {
        guard includeShot, let image = session.image else { return nil }
        return ReportImage.cropped(image, top: cropTop, bottom: cropBottom)
    }

    var body: some View {
        NavigationStack {
            if let sent {
                receipt(sent)
            } else {
                form
            }
        }
        .preferredColorScheme(.dark)
    }

    private var form: some View {
        Form {
            Section {
                Picker("Kind", selection: $kind) {
                    ForEach(draft.kinds, id: \.self) { Text($0.label).tag($0.value) }
                }
                .pickerStyle(.segmented)
                .accessibilityIdentifier("report-kind")
                Text(hint).font(.paper(.footnote)).foregroundStyle(.secondary)
            }
            Section("What happened") {
                TextEditor(text: $happened).frame(minHeight: 80)
                    .accessibilityIdentifier("report-happened")
            }
            Section("What you expected") {
                TextEditor(text: $expected).frame(minHeight: 60)
            }
            Section("Steps") {
                TextEditor(text: $steps).frame(minHeight: 60)
            }
            if let task = draft.task {
                Section {
                    Toggle("Attach this chat's task ID", isOn: $includeTask)
                } footer: {
                    Text(task).font(.paper(.caption))
                }
            }
            screenshotSection
            if let lines = draft.chat_lines, !lines.isEmpty {
                Section {
                    Toggle("Share this chat (\(lines.count) messages)", isOn: $includeChat)
                        .accessibilityIdentifier("report-share-chat")
                    DisclosureGroup("Show the whole chat") {
                        ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                            Text(line).font(.paper(.caption))
                        }
                    }
                } footer: {
                    Text("Off by default. Helps us improve our answers: exactly these messages, sent only to the triage team.")
                }
            }
            if draft.logging {
                Section {
                    Toggle("Attach the playtest log (\(draft.log_lines.count) events)", isOn: $includeLog)
                    DisclosureGroup("Show the whole log") {
                        ForEach(Array(draft.log_lines.enumerated()), id: \.offset) { _, line in
                            Text(line).font(.paper(.caption))
                        }
                    }
                } footer: {
                    Text("Tab, screen, event, and time only: exactly these lines.")
                }
            }
            Section {
                Toggle("You may quote my words in a public issue", isOn: $quote)
            } footer: {
                Text(draft.privacy)
            }
            Section("Sent with the report") {
                Text("\(ReportDevice.version) (\(ReportDevice.build)) · \(draft.tab)/\(draft.route) · \(ReportDevice.model) · iOS \(ReportDevice.os)")
                    .font(.paper(.footnote))
                    .foregroundStyle(.secondary)
            }
            if !draft.triage_ready {
                Section {
                    Text("This build can't send reports yet. Yours is saved on this phone and is sent by a later build. To report now, use the GitHub form.")
                        .font(.paper(.footnote))
                    if let url = URL(string: draft.fallback) {
                        Link("Open the Playtest report form", destination: url)
                    }
                }
            }
            if let error {
                Section { Text(error).foregroundStyle(.red).accessibilityIdentifier("report-error") }
            }
        }
        .scrollContentBackground(.hidden)
        .background(Color.black.ignoresSafeArea())
        .navigationTitle("Report a problem")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
            ToolbarItem(placement: .confirmationAction) {
                Button(draft.triage_ready ? "Send" : "Save") { send() }
                    .disabled(sending || happened.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityIdentifier("report-send")
            }
        }
        .onAppear { includeLog = draft.logging }
    }

    @ViewBuilder private var screenshotSection: some View {
        if !draft.screenshot_allowed {
            Section {
                Text("No screenshot from the Wallet or a key screen. Describe it in words.")
                    .font(.paper(.footnote)).foregroundStyle(.secondary)
            }
        } else if session.image != nil {
            Section {
                Toggle("Attach a screenshot", isOn: $includeShot)
                    .accessibilityIdentifier("report-screenshot")
                if includeShot {
                    if let shot {
                        Image(uiImage: shot).resizable().scaledToFit().frame(maxHeight: 260)
                            .frame(maxWidth: .infinity)
                    }
                    LabeledContent("Crop top") { Slider(value: $cropTop, in: 0...0.45) }
                    LabeledContent("Crop bottom") { Slider(value: $cropBottom, in: 0...0.45) }
                }
            } footer: {
                Text("Off by default. The picture shown is exactly what's sent, made smaller.")
            }
        }
    }

    private func receipt(_ row: ReportRow) -> some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Text(row.status == "waiting" ? "Saved on this phone" : "Report filed")
                        .font(.paper(.title2, weight: .bold))
                    if let code = row.code {
                        Text(code).font(.code(.title3)).textSelection(.enabled)
                            .accessibilityIdentifier("report-code")
                    }
                    Text(row.status == "waiting"
                         ? "It's sent by a later build. You'll find it in Account, My reports."
                         : "Quote this code if you talk to us about it. You'll find it in Account, My reports.")
                        .font(.paper(.subheadline)).foregroundStyle(.secondary)
                }
                .padding(.vertical, 4)
            }
        }
        .scrollContentBackground(.hidden)
        .background(Color.black.ignoresSafeArea())
        .navigationTitle("Report a problem")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
        }
    }

    private func send() {
        var form: [String: Any] = [
            "app_version": ReportDevice.version, "build": ReportDevice.build,
            "device": ReportDevice.model, "os_version": ReportDevice.os,
            "tab": draft.tab, "route": draft.route, "kind": kind,
            "happened": happened, "expected": expected, "steps": steps, "quote": quote,
            "include_task": includeTask, "include_log": includeLog && draft.logging,
            "log_digest": draft.log_digest,
            "include_chat": includeChat && !(draft.chat_lines ?? []).isEmpty,
            "chat_digest": draft.chat_digest ?? "",
        ]
        if draft.screenshot_allowed, let shot, let jpeg = ReportImage.jpeg(shot) {
            form["screenshot"] = ["jpeg_base64": jpeg.data.base64EncodedString(),
                                  "width": jpeg.width, "height": jpeg.height]
        }
        sending = true
        error = nil
        bridge.sendReport(form) { packet in
            sending = false
            if let row = packet.sent {
                sent = row
            } else {
                error = packet.error ?? "The report couldn't be filed."
                // The log or the chat moved on: show the current one before sending.
                if error?.contains("log changed") == true || error?.contains("chat changed") == true {
                    bridge.reportDraft(tab: draft.tab, route: draft.route) { session.draft = $0 }
                }
            }
        }
    }
}

/// Text selected in a transcript and the row it starts in, for Give
/// feedback (#10127).
struct FeedbackRequest: Identifiable {
    let id = UUID()
    let text: String
    let row: String
}

/// Give feedback: the selected text, quoted, and a comment. Send files it
/// as a report Rust seals to the triage key.
struct FeedbackSheet: View {
    let request: FeedbackRequest
    let tab: String
    let route: String
    @ObservedObject var bridge: MobileBridge
    @Environment(\.dismiss) private var dismiss
    @State private var comment = ""
    @State private var sending = false
    @State private var error: String?
    @State private var done: String?

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Text(request.text)
                        .font(.paper(.callout))
                        .foregroundStyle(.secondary)
                        .lineLimit(8)
                }
                Section {
                    ZStack(alignment: .topLeading) {
                        if comment.isEmpty {
                            Text("What's wrong or what should change?")
                                .foregroundStyle(.tertiary)
                                .padding(.top, 8)
                                .padding(.leading, 5)
                        }
                        TextEditor(text: $comment).frame(minHeight: 100)
                            .accessibilityIdentifier("feedback-comment")
                            .disabled(done != nil)
                    }
                }
                if let done {
                    Section { Text(done).accessibilityIdentifier("feedback-sent") }
                }
                if let error {
                    Section { Text(error).foregroundStyle(.red) }
                }
            }
            .scrollContentBackground(.hidden)
            .background(Color.black.ignoresSafeArea())
            .navigationTitle("Give feedback")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(done == nil ? "Cancel" : "Close") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Send") { send() }
                        .disabled(sending || done != nil
                                  || comment.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        .accessibilityIdentifier("feedback-send")
                }
            }
        }
        .preferredColorScheme(.dark)
    }

    private func send() {
        let form: [String: Any] = [
            "app_version": ReportDevice.version, "build": ReportDevice.build,
            "device": ReportDevice.model, "os_version": ReportDevice.os,
            "tab": tab, "route": route, "text": request.text, "comment": comment,
            "row": request.row,
        ]
        sending = true
        error = nil
        bridge.sendFeedback(form) { packet in
            sending = false
            if let said = packet.feedback {
                done = said
            } else {
                error = packet.error ?? "The feedback couldn't be sent."
            }
        }
    }
}

/// The reports this phone filed, newest first.
struct MyReportsScreen: View {
    @ObservedObject var bridge: MobileBridge
    @EnvironmentObject private var reporter: ReportCoordinator
    @EnvironmentObject private var place: PlaytestPlace
    @State private var packet: ReportsPacket?
    private let refresh = Timer.publish(every: 2, on: .main, in: .common).autoconnect()

    var body: some View {
        List {
            if let packet, packet.reports.isEmpty {
                Section {
                    Text("No reports yet. Long-press the tab bar on any screen, or use Report a problem in Account.")
                        .font(.paper(.subheadline)).foregroundStyle(.secondary)
                }
            }
            ForEach(packet?.reports ?? [], id: \.self) { row in
                VStack(alignment: .leading, spacing: 3) {
                    HStack(alignment: .firstTextBaseline) {
                        Text(row.code ?? "No code yet").font(.code(.body, weight: .semibold))
                        Spacer()
                        Text(row.status_label).font(.paper(.caption))
                            .foregroundStyle(row.status == "sent" ? .secondary : Color.yellow)
                    }
                    Text(row.summary).font(.paper(.subheadline)).lineLimit(2)
                    Text("\(row.kind_label) · \(row.place) · \(row.build)\(row.screenshot ? " · screenshot" : "")\(row.log ? " · playtest log" : "")\(row.published == true ? " · public record" : "")")
                        .font(.paper(.caption)).foregroundStyle(.secondary)
                    if let error = row.error {
                        Text(error).font(.paper(.caption)).foregroundStyle(.red)
                    }
                }
                .padding(.vertical, 2)
            }
            Section {
                Button("Report a problem", systemImage: "exclamationmark.bubble") {
                    reporter.start(bridge: bridge, place: place)
                }
            } footer: {
                if let packet, !packet.triage_ready {
                    Text("This build can't send reports yet; they wait here and are sent by a later build.")
                } else {
                    Text("Reports go privately to the OpenAgents triage team. Accepted ones become public GitHub issues that we write.")
                }
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
        .background(Color.black.ignoresSafeArea())
        .onAppear { bridge.reports { packet = $0 } }
        .onReceive(refresh) { _ in
            guard packet?.reports.contains(where: { $0.status == "sending" }) == true else { return }
            bridge.reports { packet = $0 }
        }
    }
}

/// Account > Playtest: playtest logging, reports, and how testing works.
/// Playtest logging has no switch here; the build sets it.
struct PlaytestScreen: View {
    @ObservedObject var bridge: MobileBridge
    @EnvironmentObject private var reporter: ReportCoordinator
    @EnvironmentObject private var place: PlaytestPlace
    @State private var packet: ReportsPacket?
    @State private var confirmClear = false
    @Environment(\.scenePhase) private var scenePhase
    @State private var card: TrainerPacket?
    private let refresh = Timer.publish(every: 3, on: .main, in: .common).autoconnect()

    var body: some View {
        List {
            // The playtest card shows only once the playtest referee's awards
            // are read: while its key is unpublished there are no real
            // numbers to show, so there is no card.
            if let card, PlaytestCardSection.shown(card) {
                PlaytestCardSection(card: card, reports: packet)
            }
            if let log = packet?.log {
                Section {
                    Text(log.note)
                        .accessibilityIdentifier("playtest-logging")
                    if log.events > 0 {
                        DisclosureGroup(log.events == 1 ? "1 event" : "\(log.events) events") {
                            ForEach(Array(log.lines.enumerated()), id: \.offset) { _, line in
                                Text(line).font(.paper(.caption))
                            }
                        }
                        Button("Delete the log", role: .destructive) { confirmClear = true }
                    }
                } header: {
                    Text("Playtest logging")
                } footer: {
                    if log.on {
                        Text("This phone notes which tab and screen you're on, error codes, and when. Never messages, prompts, keys, recovery words, invoices, addresses, or amounts. The log stays on this phone and goes only in a report you preview.")
                    }
                }
            }
            Section {
                Button("Report a problem", systemImage: "exclamationmark.bubble") {
                    reporter.start(bridge: bridge, place: place)
                }
                .accessibilityIdentifier("playtest-report")
                NavigationLink(value: AccountRoute.reports) {
                    Label("My reports", systemImage: "tray.full")
                }
            } footer: {
                Text("Tip: long-press the tab bar on any screen to report what's on it.")
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
        .background(Color.black.ignoresSafeArea())
        .confirmationDialog("Delete the playtest log?", isPresented: $confirmClear) {
            Button("Delete", role: .destructive) { bridge.playtestClear { packet = $0 } }
        }
        .onAppear { bridge.reports { packet = $0 } }
        .onAppear { bridge.trainer { card = $0 } }
        .onReceive(refresh) { _ in
            guard scenePhase == .active,
                  card?.playtest.state != "preview",
                  card?.playtest.state != "unpublished" else { return }
            bridge.trainer { card = $0 }
        }
    }
}

/// The playtest card at the top of Account > Playtest: sessions, accepted
/// reports, fixes verified, playtest XP beside (never inside) the trainer
/// level, and titles. Rust reads them from the playtest referee; the card
/// shows only once they are read.
struct PlaytestCardSection: View {
    let card: TrainerPacket?
    let reports: ReportsPacket?

    /// Whether the card has real numbers to show: the referee's awards are
    /// read, or (debug builds only) the labeled preview.
    static func shown(_ card: TrainerPacket) -> Bool {
        card.playtest.state == "ready" || card.playtest.state == "preview"
    }

    var body: some View {
        Section {
            VStack(alignment: .leading, spacing: 10) {
                if card?.playtest.state == "preview" {
                    Label("Preview: a labeled fixture, not real awards.", systemImage: "flask")
                        .font(.paper(.footnote)).foregroundStyle(.yellow)
                }
                HStack(alignment: .firstTextBaseline) {
                    Text("\(card?.playtest.xp ?? 0) playtest XP").font(.paper(.title2, weight: .bold))
                        .accessibilityIdentifier("playtest-xp")
                    Spacer()
                    if let card {
                        Text("Trainer: \(card.xp) XP · lv \(card.level)")
                            .font(.paper(.footnote)).foregroundStyle(.secondary)
                    }
                }
                HStack(spacing: 0) {
                    stat(card?.playtest.sessions ?? 0, "sessions")
                    stat(card?.playtest.accepted_reports ?? 0, "accepted reports")
                    stat(card?.playtest.fixes_verified ?? 0, "fixes verified")
                }
                if let titles = card?.playtest.titles, !titles.isEmpty {
                    Text(titles.map { $0.uppercased() }.joined(separator: " · "))
                        .font(.paper(.footnote, weight: .semibold))
                }
                Text(status).font(.paper(.footnote)).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.vertical, 4)
            .accessibilityIdentifier("playtest-card")
            ForEach(card?.playtest.awards ?? [], id: \.self) { award in
                if let url = URL(string: award.link) {
                    Link(destination: url) {
                        HStack(alignment: .firstTextBaseline) {
                            Text(award.title).font(.paper(.subheadline))
                            Spacer()
                            Text("+\(award.xp) XP").font(.paper(.subheadline))
                        }
                    }
                    .foregroundStyle(.white)
                }
            }
        } header: {
            Text("Playtest card")
        } footer: {
            Text(card?.playtest.note ?? "")
        }
    }

    private var status: String {
        let filed = reports?.reports.count ?? 0
        let phone = "\(filed) report\(filed == 1 ? "" : "s") filed from this phone."
        switch card?.playtest.state {
        case "connecting", "reading":
            return "\(phone) Reading playtest awards…"
        default:
            return phone
        }
    }

    private func stat(_ value: Int, _ label: String) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("\(value)").font(.paper(.title3))
            Text(label).font(.paper(.caption)).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// A long press on the tab bar opens Report a problem for the screen on
/// view. SwiftUI's tab bar is a UIKit tab bar, so this finds it and adds
/// the recognizer once.
struct TabBarLongPress: UIViewRepresentable {
    let action: () -> Void

    func makeUIView(context: Context) -> Probe {
        let probe = Probe()
        probe.action = action
        probe.isUserInteractionEnabled = false
        return probe
    }

    func updateUIView(_ probe: Probe, context: Context) { probe.action = action }

    final class Probe: UIView {
        var action: (() -> Void)?
        private weak var bar: UITabBar?

        override func didMoveToWindow() {
            super.didMoveToWindow()
            attach(tries: 5)
        }

        private func attach(tries: Int) {
            guard bar == nil, let window else { return }
            if let found = Self.find(in: window) {
                let press = UILongPressGestureRecognizer(target: self, action: #selector(pressed(_:)))
                press.minimumPressDuration = 0.6
                press.name = "openagents-report"
                found.addGestureRecognizer(press)
                bar = found
            } else if tries > 0 {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self.attach(tries: tries - 1) }
            }
        }

        private static func find(in view: UIView) -> UITabBar? {
            if let bar = view as? UITabBar { return bar }
            for child in view.subviews {
                if let bar = find(in: child) { return bar }
            }
            return nil
        }

        @objc private func pressed(_ press: UILongPressGestureRecognizer) {
            if press.state == .began {
                UIImpactFeedbackGenerator(style: .medium).impactOccurred()
                action?()
            }
        }
    }
}
