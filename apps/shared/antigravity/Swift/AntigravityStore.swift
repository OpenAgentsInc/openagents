// Firm reason: 6 (cross-Apple @MainActor SwiftUI store orchestrating TailscaleClient, LocalSocketClient, MobileDeviceStore, real-time push frames, Antigravity sessions, subagents, approvals, and Workspace Git/PR controls).
import Foundation
import SwiftUI

typealias AppStore = AntigravityStore

@MainActor
final class AntigravityStore: ObservableObject {
    let store: MobileDeviceStore
    let client: TailscaleClient

    @Published var computers: [ComputerEntry] = []
    @Published var activeComputerID: String?
    @Published var sessions: [ChatSessionSummary] = []
    @Published var activeSessionID: String?
    @Published var items: [TranscriptItem] = []

    @Published var workspaceName: String = ""
    @Published var folders: [String] = []
    @Published var selectedFolder: String = ""

    @Published var availableModels: [AntigravityModelOption] = AntigravityModelOption.fallbackCatalog
    @Published var selectedModelID: String = "gemini-4-neon"
    @Published var selectedEffort: ThinkingEffort = .medium
    @Published var reviewMode: ReviewMode = .auto

    @Published var draftText: String = ""
    @Published var queuedDraft: String?
    @Published var pendingAttachments: [FileAttachment] = []
    @Published var searchText: String = ""

    @Published var isLoadingSessions: Bool = false
    @Published var isLoadingTranscript: Bool = false
    @Published var isSending: Bool = false
    @Published var isAwaitingTurnStart: Bool = false
    @Published var isPairingHostID: String?
    @Published var bannerError: String?
    @Published var statusToast: String?

    @Published var isComputersPresented: Bool = false
    @Published var isWorkspacePresented: Bool = false
    @Published var isSidebarOpen: Bool = false

    // Subagent inspector
    @Published var activeSubagentItem: TranscriptItem?
    @Published var subagentItems: [TranscriptItem] = []
    @Published var isLoadingSubagent: Bool = false

    // Workspace Git / Diff / Branch / Worktree / PR review
    @Published var workspaceSnapshot: WorkspaceSnapshot?
    @Published var isLoadingWorkspace: Bool = false
    @Published var workspaceActionBusy: Bool = false
    @Published var workspaceError: String?

    // Real-time stream state
    @Published var livePushConnected: Bool = false
    @Published var dynamicSkills: [SlashCommandItem] = []

    /// Test-only switch enabled strictly via `-OAE2ELoopbackAllowed` launch argument for local Simulator E2E harnesses.
    var allowLoopbackForTests: Bool = false

    /// Zero-secret notification callback (`kind`: "completed" | "approval", `title`: generic non-secret status).
    var onTurnNotificationEvent: ((_ kind: String, _ title: String) -> Void)?

    private var pollTask: Task<Void, Never>?
    private var pushDebounceTask: Task<Void, Never>?
    private var lastPushReceivedAt: Date = .distantPast
    private var fastPollUntil: Date = .distantPast
    private var reachabilityByID: [String: ReachabilityState] = [:]
    private var legacyNoModeHosts: Set<String> = []
    private var wasActiveSessionBusy = false
    private var lastNotifiedInteractionID: String = ""
    private var pendingOptimisticUserItem: TranscriptItem?
    private var pendingOptimisticBaselineUserCount: Int = 0
    private var sentImagesByName: [String: Data] = [:]

    init(store: MobileDeviceStore = MobileDeviceStore()) {
        self.store = store
        self.client = TailscaleClient(book: store.tailnetBook)
        let args = ProcessInfo.processInfo.arguments
        if args.contains("-OAResetStateForTests") {
            store.resetForTests()
        }
        if args.contains("-OANoLocalNetworkBrowse") {
            self.client.browsesLocalNetwork = false
        }
        if args.contains("-OAE2ELoopbackAllowed") {
            self.allowLoopbackForTests = true
        }
        self.client.onDiscoveredChange = { [weak self] in
            self?.reloadComputers()
        }
        self.client.onReachabilityChange = { [weak self] host, state in
            self?.reachabilityByID[host] = state
            if state == .reachable {
                self?.livePushConnected = true
            }
            self?.reloadComputers()
        }
        self.client.onPushEvent = { [weak self] host, event in
            self?.handlePushEvent(host: host, event: event)
        }
        reloadComputers()
        if let activeID = activeComputerID {
            loadPreferences(for: activeID)
            restoreOfflineCache(for: activeID)
        }
    }

    var devicePublicKeyHex: String {
        store.devicePublicKeyHex() ?? ""
    }

    var activeComputer: ComputerEntry? {
        guard let id = activeComputerID else { return computers.first }
        return computers.first(where: { $0.id == id }) ?? computers.first
    }

    var activeSession: ChatSessionSummary? {
        guard let sid = activeSessionID else { return nil }
        return sessions.first(where: { $0.id == sid })
    }

    var pendingInteraction: PendingInteraction? {
        guard let session = activeSession, !session.interactionID.isEmpty else { return nil }
        let hint = items.last(where: { $0.role == .notice || $0.role == .tool })?.text ?? "Antigravity is waiting for your approval to continue."
        return PendingInteraction(id: session.interactionID, sessionID: session.id, prompt: hint)
    }

    var workingSessions: [ChatSessionSummary] {
        filteredSessions.filter(\.busy)
    }

    var recentSessions: [ChatSessionSummary] {
        filteredSessions.filter { !$0.busy }
    }

    private var filteredSessions: [ChatSessionSummary] {
        let q = searchText.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !q.isEmpty else { return sessions }
        return sessions.filter {
            $0.displayTitle.lowercased().contains(q)
                || $0.model.lowercased().contains(q)
                || $0.id.lowercased().contains(q)
        }
    }

    var selectedModelOption: AntigravityModelOption {
        availableModels.first(where: { $0.id == selectedModelID })
            ?? availableModels.first
            ?? AntigravityModelOption.fallbackCatalog[0]
    }

    var supportedEffortsForSelectedModel: [ThinkingEffort] {
        selectedModelOption.supportedEfforts
    }

    var matchingSlashCommands: [SlashCommandItem] {
        let trimmed = draftText.trimmingCharacters(in: .whitespaces)
        guard trimmed.hasPrefix("/") else { return [] }
        let query = trimmed.lowercased()
        let catalog = SlashCommandItem.defaults + dynamicSkills
        if query == "/" { return Array(catalog.prefix(10)) }
        return Array(catalog.filter { $0.command.lowercased().hasPrefix(query) }.prefix(10))
    }

    /// `@` workspace folder & changed-file mention autocomplete using host-reported `folders` and Workspace Git status.
    var matchingFolders: [String] {
        guard let lastWord = draftText.split(separator: " ").last, lastWord.hasPrefix("@") else {
            return []
        }
        var pool = folders
        if let changed = workspaceSnapshot?.changedFiles {
            for f in changed where !pool.contains(f.path) {
                pool.append(f.path)
            }
        }
        guard !pool.isEmpty else { return [] }
        let q = String(lastWord.dropFirst()).lowercased()
        if q.isEmpty { return Array(pool.prefix(8)) }
        return Array(pool.filter { $0.lowercased().contains(q) }.prefix(8))
    }

    /// Monotonic fingerprint that changes on every streamed thinking/assistant chunk so views auto-scroll smoothly.
    var streamingContentFingerprint: String {
        let last = items.last
        return "\(items.count):\(last?.id ?? ""):\(last?.text.count ?? 0):\(last?.complete == true):\(activeSession?.busy == true):\(isAwaitingTurnStart)"
    }

    /// Desktop-parity live status word (`Sending…` -> `Thinking…` -> `Working…`).
    var streamingStatusLabel: String? {
        if isSending { return "Sending…" }
        if isAwaitingTurnStart && activeSession?.busy != true { return "Thinking…" }
        guard activeSession?.busy == true else { return nil }
        if let last = items.last {
            if last.role == .thought && !last.complete { return nil } // Thought row itself shows live Thinking header
            if last.role == .tool && !last.complete { return "Working…" }
        }
        return "Thinking…"
    }

    func localImageData(for name: String) -> Data? {
        sentImagesByName[name]
    }

    /// Handles `openagents://ask?prompt=...` and `openagents://computer?address=...&port=...` deep links.
    func handleIncomingURL(_ url: URL) {
        guard url.scheme?.lowercased() == "openagents",
              let comps = URLComponents(url: url, resolvingAgainstBaseURL: false) else { return }
        let host = (url.host ?? "").lowercased()
        let queryItems = comps.queryItems ?? []
        if host == "ask" || host == "chat" {
            if let prompt = queryItems.first(where: { $0.name == "prompt" || $0.name == "text" })?.value, !prompt.isEmpty {
                draftText = draftText.isEmpty ? prompt : "\(draftText)\n\(prompt)"
            }
        } else if host == "computer" || host == "pair" {
            if let address = queryItems.first(where: { $0.name == "address" || $0.name == "host" })?.value, !address.isEmpty {
                let name = queryItems.first(where: { $0.name == "name" })?.value ?? ""
                let portStr = queryItems.first(where: { $0.name == "port" })?.value ?? ""
                let port = UInt16(portStr) ?? AppTailnet.port
                isComputersPresented = true
                Task { _ = await addManualComputer(address: address, name: name, port: port, autoPair: true) }
            } else {
                isComputersPresented = true
            }
        }
    }

    /// Discovers dynamic Antigravity skills (`SKILL.md`) from local workspace / ~/.gemini directories.
    func refreshDynamicSkills(workspaceRoot: String? = nil) {
        var roots: [URL] = []
        if let ws = workspaceRoot, !ws.isEmpty {
            let wsURL = URL(fileURLWithPath: ws)
            roots.append(wsURL.appendingPathComponent(".gemini/skills", isDirectory: true))
            roots.append(wsURL.appendingPathComponent(".agents/skills", isDirectory: true))
        }
        #if os(macOS)
        let home = FileManager.default.homeDirectoryForCurrentUser
        roots.append(home.appendingPathComponent(".gemini/antigravity/skills", isDirectory: true))
        roots.append(home.appendingPathComponent(".gemini/skills", isDirectory: true))
        #endif

        var found: [SlashCommandItem] = []
        var seen = Set(SlashCommandItem.defaults.map { $0.command.lowercased() })
        let fm = FileManager.default
        for root in roots {
            guard let entries = try? fm.contentsOfDirectory(at: root, includingPropertiesForKeys: nil) else { continue }
            for entry in entries.sorted(by: { $0.lastPathComponent < $1.lastPathComponent }) {
                let skillMD = entry.appendingPathComponent("SKILL.md")
                guard fm.fileExists(atPath: skillMD.path) else { continue }
                let slug = entry.lastPathComponent.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !slug.isEmpty else { continue }
                let cmd = "/\(slug)"
                guard seen.insert(cmd.lowercased()).inserted else { continue }
                var desc = "Run \(slug) skill"
                if let raw = try? String(contentsOf: skillMD, encoding: .utf8) {
                    for line in raw.split(separator: "\n").prefix(20) {
                        let s = line.trimmingCharacters(in: .whitespaces)
                        if s.lowercased().hasPrefix("description:") {
                            let val = s.dropFirst(12).trimmingCharacters(in: CharacterSet(charactersIn: " \"'"))
                            if !val.isEmpty { desc = String(val.prefix(80)); break }
                        }
                    }
                }
                found.append(SlashCommandItem(id: "skill-\(slug)", command: cmd, subtitle: desc, template: "\(cmd) "))
            }
        }
        dynamicSkills = found
    }

    func applyFolderMention(_ folder: String) {
        if let range = draftText.range(of: "@[^\\s]*$", options: .regularExpression) {
            draftText.replaceSubrange(range, with: "@\(folder) ")
        } else {
            draftText += "@\(folder) "
        }
        if activeSessionID == nil && MobileCore.isValidFolder(folder) {
            selectedFolder = folder
        }
    }

    // MARK: - Lifecycle

    func onForeground() {
        client.resume()
        reloadComputers()
        subscribeActiveHost()
        #if os(macOS)
        LocalSocketClient.shared.startWatchingRuns { [weak self] in
            Task { @MainActor [weak self] in
                guard let self, self.activeComputer?.isLocalMac == true else { return }
                self.schedulePushRefresh(sessionID: self.activeSessionID ?? "")
            }
        }
        #endif
        startFallbackPolling()
        Task { await refreshActiveComputer() }
    }

    func onBackground() {
        pollTask?.cancel()
        pollTask = nil
        pushDebounceTask?.cancel()
        pushDebounceTask = nil
        #if os(macOS)
        LocalSocketClient.shared.stopWatchingRuns()
        #endif
        client.suspend()
    }

    // MARK: - Computer management & Relay-free Tailscale Pairing

    func reloadComputers() {
        let paired = store.pairedComputers
        let tailnet = store.tailnetBook.computers

        var byID: [String: ComputerEntry] = [:]

        #if os(macOS)
        let localReady = LocalSocketClient.shared.isSocketAvailable
        byID[ComputerEntry.localMacID] = ComputerEntry(
            id: ComputerEntry.localMacID,
            hostKey: nil,
            name: Host.current().localizedName ?? "This Mac",
            ip: "",
            dns: "",
            port: AppTailnet.port,
            paired: nil,
            reachability: localReady ? .reachable : .unreachable("Start local host daemon"),
            isLocalMac: true
        )
        #endif

        for p in paired {
            let tn = tailnet.first(where: { $0.host == p.host })
            byID[p.host] = ComputerEntry(
                id: p.host,
                hostKey: p.host,
                name: p.name.isEmpty ? (tn?.name ?? "My Mac") : p.name,
                ip: tn?.ip ?? "",
                dns: tn?.dns ?? "",
                port: tn?.port ?? AppTailnet.port,
                paired: p,
                reachability: reachabilityByID[p.host] ?? .unknown,
                isLocalMac: false
            )
        }
        for tn in tailnet {
            if var existing = byID[tn.host] {
                existing.ip = tn.ip
                existing.dns = tn.dns
                existing.port = tn.port
                if existing.name.isEmpty { existing.name = tn.name }
                byID[tn.host] = existing
            } else {
                byID[tn.host] = ComputerEntry(
                    id: tn.host,
                    hostKey: MobileCore.isValidHostKey(tn.host) ? tn.host : nil,
                    name: tn.name,
                    ip: tn.ip,
                    dns: tn.dns,
                    port: tn.port,
                    paired: nil,
                    reachability: reachabilityByID[tn.host] ?? .unknown,
                    isLocalMac: false
                )
            }
        }

        let sorted = byID.values.sorted { a, b in
            if a.isLocalMac != b.isLocalMac { return a.isLocalMac }
            if a.canSendRPC != b.canSendRPC { return a.canSendRPC }
            return a.name.localizedCaseInsensitiveCompare(b.name) == .orderedAscending
        }
        computers = sorted

        if let current = activeComputerID, sorted.contains(where: { $0.id == current }) {
            return
        }
        if let saved = store.lastActiveHost, sorted.contains(where: { $0.id == saved }) {
            activeComputerID = saved
        } else {
            activeComputerID = sorted.first?.id
        }
    }

    func selectComputer(_ computer: ComputerEntry) {
        activeComputerID = computer.id
        store.lastActiveHost = computer.id
        activeSessionID = nil
        items = []
        sessions = []
        workspaceName = ""
        folders = []
        selectedFolder = ""
        pendingOptimisticUserItem = nil
        isAwaitingTurnStart = false
        fastPollUntil = .distantPast
        wasActiveSessionBusy = false
        workspaceSnapshot = nil
        activeSubagentItem = nil
        subagentItems = []
        bannerError = nil
        loadPreferences(for: computer.id)
        restoreOfflineCache(for: computer.id)
        subscribeActiveHost()
        Task { await refreshActiveComputer() }
    }

    func forgetComputer(_ computer: ComputerEntry) {
        guard !computer.isLocalMac else { return }
        if let key = computer.hostKey {
            store.removePaired(host: key)
            client.unsubscribe(host: key)
        }
        store.tailnetBook.forget(computer.id)
        reachabilityByID.removeValue(forKey: computer.id)
        if activeComputerID == computer.id {
            activeComputerID = nil
            activeSessionID = nil
            sessions = []
            items = []
            workspaceSnapshot = nil
        }
        reloadComputers()
    }

    /// Requests a signed grant directly over Tailscale TCP port `computer.port` (zero public relays!).
    @discardableResult
    func pairOverTailscale(_ computer: ComputerEntry) async -> String? {
        guard !computer.isLocalMac else { return nil }
        let target = computer.ip.isEmpty ? computer.dns : computer.ip
        guard !target.isEmpty else {
            return "No Tailscale IP or MagicDNS address available for this computer."
        }
        guard let pubkey = store.devicePublicKeyHex(), !pubkey.isEmpty else {
            return "Device key unavailable."
        }

        isPairingHostID = computer.id
        reachabilityByID[computer.id] = .checking
        reloadComputers()
        defer { isPairingHostID = nil }

        let result = await client.requestDirectGrant(
            address: target,
            port: computer.port,
            devicePubkey: pubkey,
            clientLabel: store.localClientLabel
        )
        switch result {
        case .failure(let err):
            if case .refused(let code) = err {
                reachabilityByID[computer.id] = .refused(code)
            } else {
                reachabilityByID[computer.id] = .unreachable(err.localizedDescription)
            }
            reloadComputers()
            return err.localizedDescription

        case .success(let answer):
            guard let grant = answer.grant, !grant.isEmpty else {
                return "Host did not return a grant."
            }
            let displayName = answer.label.isEmpty ? computer.name : answer.label
            let paired = PairedComputer(
                host: answer.host,
                relay: MobileCore.directRelayLabel,
                grant: grant,
                epoch: answer.epoch,
                expires: answer.expires,
                name: displayName
            )
            store.upsertPaired(paired)

            // Replace placeholder manual ID with real 64-hex host key in tailnetBook
            if computer.id != answer.host {
                store.tailnetBook.forget(computer.id)
                reachabilityByID.removeValue(forKey: computer.id)
            }
            let updatedEndpoint = AppTailnetComputer(
                host: answer.host,
                ip: computer.ip,
                dns: computer.dns,
                port: computer.port,
                name: displayName,
                seen: Int64(Date().timeIntervalSince1970)
            )
            store.tailnetBook.remember(updatedEndpoint)
            reachabilityByID[answer.host] = .reachable
            activeComputerID = answer.host
            store.lastActiveHost = answer.host
            bannerError = nil
            statusToast = "Paired with \(displayName) over Tailscale"
            reloadComputers()
            subscribeActiveHost()
            await refreshActiveComputer()
            return nil
        }
    }

    /// Validates and saves a manually entered Tailscale IP (`100.x.y.z`) or MagicDNS hostname,
    /// probes reachability, and optionally requests a relay-free direct Tailscale grant immediately.
    func addManualComputer(address: String, name: String, port: UInt16 = AppTailnet.port, autoPair: Bool = true) async -> String? {
        let trimmed = address.trimmingCharacters(in: .whitespacesAndNewlines)
        let isLoopbackTest = allowLoopbackForTests && (trimmed == "127.0.0.1" || trimmed == "localhost")
        let isIP = AppTailnet.isTailnet(trimmed) || isLoopbackTest
        let dns = isIP ? "" : (AppTailnet.dnsName(trimmed) ?? "")
        guard isIP || !dns.isEmpty else {
            return "Enter a Tailscale IP (100.64.0.0/10) or MagicDNS name (for example, mac.tailnet.ts.net)."
        }

        let cleanName = AppTailnetComputer.name(name.isEmpty ? (dns.isEmpty ? trimmed : dns) : name)
        let endpointID = "manual:\(isIP ? AppTailnet.plain(trimmed) : dns):\(port)"
        let entry = AppTailnetComputer(
            host: endpointID,
            ip: isIP ? AppTailnet.plain(trimmed) : "",
            dns: dns,
            port: port,
            name: cleanName.isEmpty ? "My Mac" : cleanName,
            seen: Int64(Date().timeIntervalSince1970)
        )
        store.tailnetBook.remember(entry)
        reachabilityByID[endpointID] = .checking
        reloadComputers()

        if autoPair, let added = computers.first(where: { $0.id == endpointID }) {
            let pairErr = await pairOverTailscale(added)
            if pairErr == nil { return nil }
            // Fall back to keeping the entry saved with its reachability state visible
            return pairErr
        }

        let probeAddress = isIP ? entry.ip : entry.dns
        let state = await client.probe(address: probeAddress, port: port)
        reachabilityByID[endpointID] = state
        reloadComputers()
        return nil
    }

    func probeComputer(_ computer: ComputerEntry) async {
        if computer.isLocalMac {
            reloadComputers()
            return
        }
        let target = computer.ip.isEmpty ? computer.dns : computer.ip
        guard !target.isEmpty else { return }
        reachabilityByID[computer.id] = .checking
        reloadComputers()
        let state = await client.probe(address: target, port: computer.port)
        reachabilityByID[computer.id] = state
        reloadComputers()
    }

    // MARK: - Antigravity preferences & composer helpers

    private func loadPreferences(for hostID: String) {
        let model = store.selectedModel(for: hostID)
        selectedModelID = model
        let effortRaw = store.selectedEffort(for: hostID, model: model)
        selectedEffort = ThinkingEffort(rawValue: effortRaw) ?? .medium
        reviewMode = store.selectedReviewMode(for: hostID)
    }

    func chooseModel(_ modelID: String) {
        let normalized = MobileCore.normalizeAntigravityModel(modelID)
        selectedModelID = normalized
        if let hostID = activeComputer?.id {
            store.setSelectedModel(normalized, for: hostID)
        }
        let clampedEffort = MobileCore.normalizeAntigravityEffort(model: normalized, want: selectedEffort.rawValue)
        if let effort = ThinkingEffort(rawValue: clampedEffort) {
            selectedEffort = effort
            if let hostID = activeComputer?.id {
                store.setSelectedEffort(effort.rawValue, for: hostID, model: normalized)
            }
        }
    }

    func chooseEffort(_ effort: ThinkingEffort) {
        let clamped = MobileCore.normalizeAntigravityEffort(model: selectedModelID, want: effort.rawValue)
        selectedEffort = ThinkingEffort(rawValue: clamped) ?? .medium
        if let hostID = activeComputer?.id {
            store.setSelectedEffort(selectedEffort.rawValue, for: hostID, model: selectedModelID)
        }
    }

    func chooseReviewMode(_ mode: ReviewMode) {
        reviewMode = mode
        if let hostID = activeComputer?.id {
            store.setSelectedReviewMode(mode, for: hostID)
        }
    }

    func applySlashCommand(_ item: SlashCommandItem) {
        draftText = item.template
    }

    func addAttachment(_ attachment: FileAttachment) {
        guard pendingAttachments.count < 4 else { return }
        pendingAttachments.append(attachment)
    }

    func removeAttachment(_ attachment: FileAttachment) {
        pendingAttachments.removeAll { $0.id == attachment.id }
    }

    func queueCurrentDraft() {
        let trimmed = draftText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        queuedDraft = trimmed
        draftText = ""
    }

    func clearQueuedDraft() {
        queuedDraft = nil
    }

    // MARK: - Chat actions

    func startNewChat() {
        activeSessionID = nil
        items = []
        pendingOptimisticUserItem = nil
        isAwaitingTurnStart = false
        fastPollUntil = .distantPast
        wasActiveSessionBusy = false
        activeSubagentItem = nil
        subagentItems = []
        bannerError = nil
        isSidebarOpen = false
        subscribeActiveHost()
    }

    func selectSession(_ session: ChatSessionSummary) {
        activeSessionID = session.id
        pendingOptimisticUserItem = nil
        isAwaitingTurnStart = false
        fastPollUntil = .distantPast
        activeSubagentItem = nil
        subagentItems = []
        bannerError = nil
        isSidebarOpen = false
        subscribeActiveHost()
        Task { await refreshActiveTranscript() }
    }

    func refreshActiveComputer() async {
        guard let computer = activeComputer else { return }
        guard computer.canSendRPC else {
            if !computer.isLocalMac && (!computer.ip.isEmpty || !computer.dns.isEmpty) {
                await probeComputer(computer)
            }
            return
        }
        isLoadingSessions = sessions.isEmpty
        defer { isLoadingSessions = false }

        if let descView = await performEnvironmentRPC(computer: computer, method: "DescribeEnvironment") {
            guard activeComputerID == computer.id else { return }
            applyMetadata(from: descView)
        }

        var mergedByID: [String: ChatSessionSummary] = [:]
        if let listView = await performEnvironmentRPC(computer: computer, method: "ListSessions") {
            guard activeComputerID == computer.id else { return }
            applyMetadata(from: listView)
            for s in listView.sessions { mergedByID[s.id] = s }
        }
        // Also fetch native ~/.gemini Antigravity sessions (`host-agy.coax`) and merge cleanly
        if let agyView = await performEnvironmentRPC(computer: computer, method: "ListSessions", model: "antigravity", quietErrors: true) {
            guard activeComputerID == computer.id else { return }
            for s in agyView.sessions where mergedByID[s.id] == nil {
                mergedByID[s.id] = s
            }
        }

        guard activeComputerID == computer.id else { return }
        if !mergedByID.isEmpty || bannerError == nil {
            sessions = mergedByID.values.sorted { $0.updatedAtMs > $1.updatedAtMs }
            if let sid = activeSessionID, !sessions.contains(where: { $0.id == sid }) {
                activeSessionID = nil
                items = []
            }
            persistOfflineCache(for: computer.id)
        }

        if activeSessionID != nil {
            await refreshActiveTranscript()
        }
    }

    /// Applies server-projected transcript items while keeping any in-flight optimistic user message
    /// pinned at the bottom until the host actually echoes the new turn (eliminating the 3-4s vanish gap).
    private func applyServerItems(_ serverItems: [TranscriptItem], nowBusy: Bool) {
        if let pending = pendingOptimisticUserItem {
            let serverUsers = serverItems.filter { $0.role == .user }.count
            let pendingNeedle = String(ParsedUserMessage.parse(pending.text).text.trimmingCharacters(in: .whitespacesAndNewlines).prefix(80))
            let lastServerUser = serverItems.last(where: { $0.role == .user })?.text ?? ""
            let echoedByText = !pendingNeedle.isEmpty && lastServerUser.contains(pendingNeedle)
            if serverUsers > pendingOptimisticBaselineUserCount || echoedByText {
                pendingOptimisticUserItem = nil
                items = serverItems
                isAwaitingTurnStart = false
            } else {
                var merged = serverItems
                merged.append(pending)
                items = merged
            }
        } else if !serverItems.isEmpty {
            items = serverItems
            isAwaitingTurnStart = false
        } else if !nowBusy && Date() > fastPollUntil {
            isAwaitingTurnStart = false
        }
    }

    func refreshActiveTranscript() async {
        guard let sid = activeSessionID,
              let computer = activeComputer,
              computer.canSendRPC else { return }

        isLoadingTranscript = items.isEmpty
        defer { isLoadingTranscript = false }

        if let view = await performEnvironmentRPC(computer: computer, method: "ReadSession", session: sid) {
            guard activeComputerID == computer.id, activeSessionID == sid else { return }
            var nowBusy = false
            var currentInteraction = ""
            if let updated = view.sessions.first(where: { $0.id == sid }) {
                nowBusy = updated.busy
                currentInteraction = updated.interactionID
                if let idx = sessions.firstIndex(where: { $0.id == sid }) {
                    sessions[idx] = updated
                } else {
                    sessions.insert(updated, at: 0)
                }
            }
            applyServerItems(view.items, nowBusy: nowBusy)
            persistOfflineCache(for: computer.id)

            // Fire zero-secret completion / approval notification hooks for backgrounded clients.
            if !currentInteraction.isEmpty && currentInteraction != lastNotifiedInteractionID {
                lastNotifiedInteractionID = currentInteraction
                onTurnNotificationEvent?("approval", "Antigravity requested approval")
            } else if currentInteraction.isEmpty {
                lastNotifiedInteractionID = ""
            }
            if wasActiveSessionBusy && !nowBusy && pendingOptimisticUserItem == nil {
                onTurnNotificationEvent?("completed", "Antigravity finished your turn")
            }

            // Auto-dispatch queued follow-up the moment the active turn finishes
            if wasActiveSessionBusy && !nowBusy, let nextPrompt = queuedDraft, !nextPrompt.isEmpty {
                queuedDraft = nil
                draftText = nextPrompt
                wasActiveSessionBusy = false
                await sendMessage()
                return
            }
            wasActiveSessionBusy = nowBusy
        }
    }

    func sendMessage() async {
        let baseText = draftText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !baseText.isEmpty || !pendingAttachments.isEmpty else { return }
        guard let computer = activeComputer, computer.canSendRPC else {
            isComputersPresented = true
            return
        }

        let capturedDraft = draftText
        let capturedAttachments = pendingAttachments
        var composed = baseText
        for att in capturedAttachments {
            if let img = att.imageData {
                sentImagesByName[att.name] = img
            }
            composed += att.renderedPromptBlock()
        }

        // If viewing a read-only ended desktop chat, smoothly continue in a fresh interactive session
        // seeded with a compact context header instead of hitting a host "conflict" refusal.
        let continueFromReadOnly = activeSession?.isObserveOnly == true
        if continueFromReadOnly, let oldTitle = activeSession?.displayTitle {
            let lastReply = items.last(where: { $0.role == .assistant })?.text.prefix(1200) ?? ""
            var seed = "[Continuing from earlier session \"\(oldTitle)\"]\n"
            if !lastReply.isEmpty {
                seed += "[Last reply summary]: \(lastReply)\n\n"
            }
            composed = seed + composed
        }

        // Legacy 16-key openagents-environment hosts reject EaRequest.mode with "unsupported".
        // Always omit "mode" for the default (.auto) and for hosts known to be v1-only.
        let hostSupportsMode = !legacyNoModeHosts.contains(computer.id)
        var modeWire = (reviewMode == .auto || !hostSupportsMode) ? "" : reviewMode.wireValue
        if reviewMode == .plan && modeWire.isEmpty && !composed.hasPrefix("/plan") && !composed.hasPrefix("[Plan mode") {
            composed = "[Plan mode — outline a plan before editing files]\n\n" + composed
        }
        let payloadText = String(composed.prefix(15_500))

        // Clear composer & display the optimistic user bubble (including real photo thumbnails!) at 0ms.
        let commandID = MobileCore.randomHex(bytes: 16)
        let optimisticID = "pending-\(commandID)"
        let previousItems = items
        if continueFromReadOnly {
            activeSessionID = nil
            items = []
        }
        let baselineUsers = items.filter { $0.role == .user }.count
        let optimisticBubbleText = capturedAttachments.contains(where: \.isImage) ? composed : (baseText.isEmpty ? composed : baseText)
        let optimisticItem = TranscriptItem(
            id: optimisticID,
            role: .user,
            text: optimisticBubbleText,
            toolName: "",
            complete: true,
            startedAtMs: Int64(Date().timeIntervalSince1970 * 1000),
            subagentCalls: 0,
            subagentNow: "",
            path: "",
            diff: "",
            diffCut: false,
            noticeKind: ""
        )
        pendingOptimisticBaselineUserCount = baselineUsers
        pendingOptimisticUserItem = optimisticItem
        items.append(optimisticItem)
        draftText = ""
        pendingAttachments = []

        isSending = true
        isAwaitingTurnStart = true
        bannerError = nil
        statusToast = nil
        defer { isSending = false }

        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        let expiresMs = nowMs + 300_000
        let model = MobileCore.normalizeAntigravityModel(selectedModelID)
        let effort = MobileCore.normalizeAntigravityEffort(model: model, want: selectedEffort.rawValue)

        func dispatchSend(modeArg: String, textArg: String, cmdArg: String) async -> ParsedEnvironmentView? {
            if let current = activeSession, !continueFromReadOnly {
                return await performEnvironmentRPC(
                    computer: computer,
                    method: "SendMessage",
                    session: current.id,
                    command: cmdArg,
                    generation: max(1, current.generation),
                    revision: current.revision,
                    text: textArg,
                    issuedMs: nowMs,
                    expiresMs: expiresMs,
                    model: model,
                    effort: effort,
                    mode: modeArg,
                    quietErrors: !modeArg.isEmpty
                )
            } else {
                // Host `es_dispatch` requires a 32-char lowercase hex session_id on OpenSession.
                let newSessionID = MobileCore.randomHex(bytes: 16)
                let folder = selectedFolder.isEmpty || !MobileCore.isValidFolder(selectedFolder) ? "" : selectedFolder
                return await performEnvironmentRPC(
                    computer: computer,
                    method: "OpenSession",
                    session: newSessionID,
                    command: cmdArg,
                    generation: 1,
                    revision: 0,
                    text: textArg,
                    issuedMs: nowMs,
                    expiresMs: expiresMs,
                    model: model,
                    effort: effort,
                    folder: folder,
                    mode: modeArg,
                    quietErrors: !modeArg.isEmpty
                )
            }
        }

        var view = await dispatchSend(modeArg: modeWire, textArg: payloadText, cmdArg: commandID)

        // Automatic transparent downgrade if an older Mac host rejects optional "mode" with "unsupported".
        if let v = view, v.error == "unsupported", !modeWire.isEmpty {
            legacyNoModeHosts.insert(computer.id)
            modeWire = ""
            var fallbackText = payloadText
            if reviewMode == .plan && !fallbackText.hasPrefix("/plan") && !fallbackText.hasPrefix("[Plan mode") {
                fallbackText = String(("[Plan mode — outline a plan before editing files]\n\n" + fallbackText).prefix(15_500))
            }
            view = await dispatchSend(modeArg: "", textArg: fallbackText, cmdArg: MobileCore.randomHex(bytes: 16))
        }

        guard let finalView = view, finalView.error.isEmpty else {
            if let err = view?.error, !err.isEmpty {
                bannerError = HostErrorFormatter.describe(err, method: activeSession == nil ? "OpenSession" : "SendMessage")
            }
            pendingOptimisticUserItem = nil
            items = previousItems
            draftText = capturedDraft
            pendingAttachments = capturedAttachments
            isAwaitingTurnStart = false
            return
        }

        fastPollUntil = Date().addingTimeInterval(30.0)
        if let opened = finalView.sessions.first {
            activeSessionID = opened.id
            wasActiveSessionBusy = opened.busy || true
            if let idx = sessions.firstIndex(where: { $0.id == opened.id }) {
                sessions[idx] = opened
            } else {
                sessions.insert(opened, at: 0)
            }
            subscribeActiveHost()
        }
        if !finalView.items.isEmpty {
            applyServerItems(finalView.items, nowBusy: finalView.sessions.first?.busy ?? true)
        }
        persistOfflineCache(for: computer.id)
        await refreshActiveTranscript()
    }

    func stopActiveTurn() async {
        guard let current = activeSession,
              let computer = activeComputer,
              computer.canSendRPC else { return }

        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        _ = await performEnvironmentRPC(
            computer: computer,
            method: "StopTurn",
            session: current.id,
            command: MobileCore.randomHex(bytes: 16),
            generation: max(1, current.generation),
            revision: current.revision,
            issuedMs: nowMs,
            expiresMs: nowMs + 300_000
        )
        await refreshActiveTranscript()
    }

    // MARK: - Interactive Approvals & Questions (`RespondInteraction`)

    func respondToInteraction(allow: Bool, note: String = "") async {
        guard let current = activeSession,
              !current.interactionID.isEmpty,
              let computer = activeComputer,
              computer.canSendRPC else { return }

        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        _ = await performEnvironmentRPC(
            computer: computer,
            method: "RespondInteraction",
            session: current.id,
            command: MobileCore.randomHex(bytes: 16),
            generation: max(1, current.generation),
            revision: current.revision,
            text: note,
            interaction: current.interactionID,
            allow: allow,
            issuedMs: nowMs,
            expiresMs: nowMs + 300_000
        )
        await refreshActiveTranscript()
    }

    // MARK: - Subagent Inspector (`ReadSubagent` & `StopSubagent`)

    func openSubagent(_ item: TranscriptItem) {
        activeSubagentItem = item
        subagentItems = []
        Task { await refreshSubagent() }
    }

    func closeSubagent() {
        activeSubagentItem = nil
        subagentItems = []
    }

    func refreshSubagent() async {
        guard let sub = activeSubagentItem,
              let sid = activeSessionID,
              let computer = activeComputer,
              computer.canSendRPC else { return }

        isLoadingSubagent = true
        defer { isLoadingSubagent = false }

        if let view = await performEnvironmentRPC(
            computer: computer,
            method: "ReadSubagent",
            session: sid,
            item: sub.id,
            quietErrors: true
        ) {
            subagentItems = view.items
        }
    }

    func stopSubagent(_ item: TranscriptItem) async {
        guard let current = activeSession,
              let computer = activeComputer,
              computer.canSendRPC else { return }

        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        _ = await performEnvironmentRPC(
            computer: computer,
            method: "StopSubagent",
            session: current.id,
            command: MobileCore.randomHex(bytes: 16),
            generation: max(1, current.generation),
            revision: current.revision,
            issuedMs: nowMs,
            expiresMs: nowMs + 300_000,
            item: item.id
        )
        await refreshSubagent()
        await refreshActiveTranscript()
    }

    // MARK: - Workspace Git / Diff / Branch / Worktree / PR Controls (`WsRequest`)

    func refreshWorkspace() async {
        guard let computer = activeComputer, computer.canSendRPC else { return }
        isLoadingWorkspace = true
        workspaceError = nil
        defer { isLoadingWorkspace = false }

        if let snap = await performWorkspaceRPC(computer: computer, method: "ReadWorkspace") {
            workspaceSnapshot = snap
            if !snap.path.isEmpty {
                refreshDynamicSkills(workspaceRoot: snap.path)
            }
            if !snap.error.isEmpty {
                workspaceError = HostErrorFormatter.describe(snap.error, method: "ReadWorkspace")
            }
        }
    }

    func switchBranch(_ branch: String) async {
        guard let computer = activeComputer, computer.canSendRPC,
              let rev = workspaceSnapshot?.revision, !rev.isEmpty else { return }
        await runWorkspaceMutation(computer: computer, method: "SwitchBranch", branch: branch, expectedRevision: rev)
    }

    func createBranch(name: String, base: String = "") async {
        guard let computer = activeComputer, computer.canSendRPC,
              let rev = workspaceSnapshot?.revision, !rev.isEmpty else { return }
        await runWorkspaceMutation(computer: computer, method: "CreateBranch", branch: name, base: base, expectedRevision: rev)
    }

    func createWorktree(branch: String, base: String = "") async {
        guard let computer = activeComputer, computer.canSendRPC,
              let rev = workspaceSnapshot?.revision, !rev.isEmpty else { return }
        await runWorkspaceMutation(computer: computer, method: "CreateWorktree", branch: branch, base: base, expectedRevision: rev)
    }

    func removeWorktree(branch: String) async {
        guard let computer = activeComputer, computer.canSendRPC,
              let rev = workspaceSnapshot?.revision, !rev.isEmpty else { return }
        await runWorkspaceMutation(computer: computer, method: "RemoveWorktree", branch: branch, expectedRevision: rev)
    }

    func pushCurrentBranch() async {
        guard let computer = activeComputer, computer.canSendRPC,
              let snap = workspaceSnapshot, !snap.branch.isEmpty, !snap.revision.isEmpty else { return }
        await runWorkspaceMutation(computer: computer, method: "PushBranch", branch: snap.branch, expectedRevision: snap.revision)
    }

    func createPullRequest(title: String, body: String, base: String) async {
        guard let computer = activeComputer, computer.canSendRPC,
              let snap = workspaceSnapshot, !snap.branch.isEmpty, !snap.revision.isEmpty else { return }
        await runWorkspaceMutation(
            computer: computer,
            method: "CreatePullRequest",
            branch: snap.branch,
            base: base,
            title: title,
            body: body,
            expectedRevision: snap.revision
        )
    }

    private func runWorkspaceMutation(
        computer: ComputerEntry,
        method: String,
        branch: String = "",
        base: String = "",
        title: String = "",
        body: String = "",
        expectedRevision: String
    ) async {
        workspaceActionBusy = true
        workspaceError = nil
        defer { workspaceActionBusy = false }

        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        if let snap = await performWorkspaceRPC(
            computer: computer,
            method: method,
            branch: branch,
            base: base,
            title: title,
            body: body,
            expectedRevision: expectedRevision,
            command: MobileCore.randomHex(bytes: 16),
            issuedMs: nowMs,
            expiresMs: nowMs + 120_000
        ) {
            workspaceSnapshot = snap
            if !snap.error.isEmpty {
                workspaceError = HostErrorFormatter.describe(snap.error, method: method)
            } else if !snap.githubError.isEmpty {
                workspaceError = snap.githubError
            }
        }
    }

    // MARK: - Real-time push & fallback polling

    private func subscribeActiveHost() {
        guard let computer = activeComputer, !computer.isLocalMac,
              let endpoint = endpointFor(computer),
              computer.canSendRPC else { return }
        client.subscribe(endpoint: endpoint, session: activeSessionID ?? "")
    }

    private func handlePushEvent(host: String, event: AppTailnet.PushEvent) {
        guard activeComputer?.id == host else { return }
        lastPushReceivedAt = Date()
        livePushConnected = true
        schedulePushRefresh(sessionID: event.session)
    }

    private func schedulePushRefresh(sessionID: String) {
        pushDebounceTask?.cancel()
        pushDebounceTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(60))
            guard !Task.isCancelled, let self else { return }
            if !sessionID.isEmpty, sessionID == self.activeSessionID {
                await self.refreshActiveTranscript()
                if self.activeSubagentItem != nil {
                    await self.refreshSubagent()
                }
            } else if self.activeSessionID != nil {
                await self.refreshActiveTranscript()
            } else {
                await self.refreshActiveComputer()
            }
        }
    }

    private func startFallbackPolling() {
        pollTask?.cancel()
        pollTask = Task { [weak self] in
            while !Task.isCancelled {
                let isStreaming = await MainActor.run {
                    guard let self else { return false }
                    return self.activeSession?.busy == true
                        || self.isAwaitingTurnStart
                        || self.pendingOptimisticUserItem != nil
                        || Date() < self.fastPollUntil
                }
                // Stream thinking & reply tokens smoothly at ~4.5 Hz during active turns when no push arrived.
                try? await Task.sleep(for: .milliseconds(isStreaming ? 220 : 2500))
                guard !Task.isCancelled, let self else { break }
                guard let computer = self.activeComputer, computer.canSendRPC else { continue }
                let recentPush = Date().timeIntervalSince(self.lastPushReceivedAt) < 1.2
                if isStreaming && !recentPush && self.activeSessionID != nil {
                    await self.refreshActiveTranscript()
                }
            }
        }
    }

    // MARK: - Offline snapshot cache

    private func restoreOfflineCache(for hostID: String) {
        guard let cached = store.loadCache(for: hostID) else { return }
        if workspaceName.isEmpty { workspaceName = cached.workspaceName }
        if sessions.isEmpty { sessions = cached.sessions }
        if activeSessionID == nil, let lastSID = cached.lastSessionID,
           cached.sessions.contains(where: { $0.id == lastSID }) {
            activeSessionID = lastSID
            items = cached.lastItems
        }
    }

    private func persistOfflineCache(for hostID: String) {
        let snapshot = CachedHostSnapshot(
            hostID: hostID,
            workspaceName: workspaceName,
            sessions: Array(sessions.prefix(40)),
            lastSessionID: activeSessionID,
            lastItems: Array(items.suffix(80)),
            savedAt: Int64(Date().timeIntervalSince1970)
        )
        store.saveCache(snapshot)
    }

    // MARK: - Internal RPC dispatch (Local Unix Socket on macOS OR NIP-44 Tailscale TCP)

    private func endpointFor(_ computer: ComputerEntry) -> AppTailnetComputer? {
        guard let hostKey = computer.hostKey else { return nil }
        if let saved = store.tailnetBook.computers.first(where: { $0.host == hostKey }) {
            return saved
        }
        guard !computer.ip.isEmpty || !computer.dns.isEmpty else { return nil }
        return AppTailnetComputer(
            host: hostKey,
            ip: computer.ip,
            dns: computer.dns,
            port: computer.port,
            name: computer.name
        )
    }

    private func applyMetadata(from view: ParsedEnvironmentView) {
        if !view.workspaceName.isEmpty {
            workspaceName = view.workspaceName
        }
        if !view.folders.isEmpty {
            folders = view.folders
        }
        if !view.antigravityModels.isEmpty {
            availableModels = view.antigravityModels
            if !availableModels.contains(where: { $0.id == selectedModelID }) {
                selectedModelID = availableModels[0].id
            }
        }
    }

    private func performEnvironmentRPC(
        computer: ComputerEntry,
        method: String,
        session: String = "",
        command: String = "",
        generation: Int64 = 0,
        revision: Int64 = 0,
        text: String = "",
        interaction: String = "",
        allow: Bool = false,
        before: Int64 = 0,
        issuedMs: Int64 = 0,
        expiresMs: Int64 = 0,
        model: String = "",
        effort: String = "",
        folder: String = "",
        item: String = "",
        mode: String = "",
        quietErrors: Bool = false
    ) async -> ParsedEnvironmentView? {
        guard let work = MobileCoreWork(),
              let params = MobileCore.encodeEnvironmentRequest(
                work: work,
                method: method,
                session: session,
                command: command,
                generation: generation,
                revision: revision,
                text: text,
                interaction: interaction,
                allow: allow,
                before: before,
                issuedMs: issuedMs,
                expiresMs: expiresMs,
                model: model,
                effort: effort,
                folder: folder,
                item: item,
                mode: mode
              ) else {
            if !quietErrors { bannerError = "Could not build Antigravity request." }
            return nil
        }

        let result: Result<Data, TailscaleRPCError>
        if computer.isLocalMac {
            result = await LocalSocketClient.shared.exchangeLine(params)
        } else {
            guard let paired = computer.paired, !paired.isExpired,
                  let endpoint = endpointFor(computer),
                  let secret = store.loadOrCreateDeviceSecret() else {
                if !quietErrors { bannerError = "Computer is not paired over Tailscale." }
                return nil
            }
            result = await client.sendRPC(
                paired: paired,
                endpoint: endpoint,
                secret: secret,
                method: method,
                params: params,
                isWorkspace: false
            )
        }

        switch result {
        case .failure(let error):
            if !quietErrors { bannerError = error.localizedDescription }
            return nil
        case .success(let data):
            guard let parsed = ParsedEnvironmentView.parse(data), parsed.method == method else {
                if !quietErrors { bannerError = "Malformed response from computer." }
                return nil
            }
            if !parsed.error.isEmpty {
                if !quietErrors { bannerError = HostErrorFormatter.describe(parsed.error, method: method) }
            } else if !quietErrors {
                bannerError = nil
            }
            return parsed
        }
    }

    private func performWorkspaceRPC(
        computer: ComputerEntry,
        method: String,
        branch: String = "",
        base: String = "",
        title: String = "",
        body: String = "",
        expectedRevision: String = "",
        command: String = "",
        issuedMs: Int64 = 0,
        expiresMs: Int64 = 0
    ) async -> WorkspaceSnapshot? {
        guard let work = MobileCoreWork(),
              let params = MobileCore.encodeWorkspaceRequest(
                work: work,
                method: method,
                workspace: "default",
                branch: branch,
                base: base,
                title: title,
                body: body,
                expectedRevision: expectedRevision,
                command: command,
                issuedMs: issuedMs,
                expiresMs: expiresMs
              ) else {
            workspaceError = "Could not encode workspace request."
            return nil
        }

        let result: Result<Data, TailscaleRPCError>
        if computer.isLocalMac {
            result = await LocalSocketClient.shared.exchangeLine(params)
        } else {
            guard let paired = computer.paired, !paired.isExpired,
                  let endpoint = endpointFor(computer),
                  let secret = store.loadOrCreateDeviceSecret() else {
                workspaceError = "Computer is not paired over Tailscale."
                return nil
            }
            result = await client.sendRPC(
                paired: paired,
                endpoint: endpoint,
                secret: secret,
                method: method,
                params: params,
                isWorkspace: true
            )
        }

        switch result {
        case .failure(let error):
            workspaceError = error.localizedDescription
            return nil
        case .success(let data):
            guard let parsed = WorkspaceSnapshot.parse(data) else {
                workspaceError = "Malformed workspace response."
                return nil
            }
            return parsed
        }
    }
}
