// Firm reason: 6 (SwiftUI @MainActor application store orchestrating TailscaleClient, MobileDeviceStore, and Coax-enforced Antigravity RPCs).
import Foundation
import SwiftUI

@MainActor
final class AppStore: ObservableObject {
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
    @Published var searchText: String = ""
    @Published var isLoadingSessions: Bool = false
    @Published var isLoadingTranscript: Bool = false
    @Published var isSending: Bool = false
    @Published var bannerError: String?
    @Published var isComputersPresented: Bool = false
    @Published var isSidebarOpen: Bool = false

    private var pollTask: Task<Void, Never>?
    private var reachabilityByID: [String: ReachabilityState] = [:]

    init(store: MobileDeviceStore = MobileDeviceStore()) {
        self.store = store
        self.client = TailscaleClient(book: store.tailnetBook)
        if ProcessInfo.processInfo.arguments.contains("-OANoLocalNetworkBrowse") {
            self.client.browsesLocalNetwork = false
        }
        self.client.onDiscoveredChange = { [weak self] in
            self?.reloadComputers()
        }
        self.client.onReachabilityChange = { [weak self] host, state in
            self?.reachabilityByID[host] = state
            self?.reloadComputers()
        }
        reloadComputers()
        if let activeID = activeComputerID {
            loadPreferences(for: activeID)
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
            $0.displayTitle.lowercased().contains(q) || $0.model.lowercased().contains(q)
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

    // MARK: - Lifecycle

    func onForeground() {
        client.resume()
        reloadComputers()
        startPolling()
        Task { await refreshActiveComputer() }
    }

    func onBackground() {
        pollTask?.cancel()
        pollTask = nil
        client.suspend()
    }

    // MARK: - Computer management

    func reloadComputers() {
        let paired = store.pairedComputers
        let tailnet = store.tailnetBook.computers

        var byID: [String: ComputerEntry] = [:]
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
                reachability: reachabilityByID[p.host] ?? .unknown
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
                    reachability: reachabilityByID[tn.host] ?? .unknown
                )
            }
        }

        let sorted = byID.values.sorted { a, b in
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
        bannerError = nil
        loadPreferences(for: computer.id)
        Task { await refreshActiveComputer() }
    }

    func forgetComputer(_ computer: ComputerEntry) {
        if let key = computer.hostKey {
            store.removePaired(host: key)
        }
        store.tailnetBook.forget(computer.id)
        reachabilityByID.removeValue(forKey: computer.id)
        if activeComputerID == computer.id {
            activeComputerID = nil
            activeSessionID = nil
            sessions = []
            items = []
        }
        reloadComputers()
    }

    /// Validates and saves a manually entered Tailscale IP (`100.x.y.z`) or MagicDNS hostname,
    /// then probes reachability over port `port` (default 47109).
    func addManualComputer(address: String, name: String, port: UInt16 = AppTailnet.port) async -> String? {
        let trimmed = address.trimmingCharacters(in: .whitespacesAndNewlines)
        let isIP = AppTailnet.isTailnet(trimmed)
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

        let probeAddress = isIP ? entry.ip : entry.dns
        let state = await client.probe(address: probeAddress, port: port)
        reachabilityByID[endpointID] = state
        reloadComputers()
        return nil
    }

    func probeComputer(_ computer: ComputerEntry) async {
        let target = computer.ip.isEmpty ? computer.dns : computer.ip
        guard !target.isEmpty else { return }
        reachabilityByID[computer.id] = .checking
        reloadComputers()
        let state = await client.probe(address: target, port: computer.port)
        reachabilityByID[computer.id] = state
        reloadComputers()
    }

    // MARK: - Antigravity preferences

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

    // MARK: - Chat actions

    func startNewChat() {
        activeSessionID = nil
        items = []
        bannerError = nil
        isSidebarOpen = false
    }

    func selectSession(_ session: ChatSessionSummary) {
        activeSessionID = session.id
        bannerError = nil
        isSidebarOpen = false
        Task { await refreshActiveTranscript() }
    }

    func refreshActiveComputer() async {
        guard let computer = activeComputer else { return }
        guard let paired = computer.paired, !paired.isExpired,
              let endpoint = endpointFor(computer) else {
            if !computer.ip.isEmpty || !computer.dns.isEmpty {
                await probeComputer(computer)
            }
            return
        }
        isLoadingSessions = true
        defer { isLoadingSessions = false }

        if let descView = await performRPC(paired: paired, endpoint: endpoint, method: "DescribeEnvironment") {
            applyMetadata(from: descView)
        }
        if let listView = await performRPC(paired: paired, endpoint: endpoint, method: "ListSessions") {
            applyMetadata(from: listView)
            sessions = listView.sessions
            if let sid = activeSessionID, !sessions.contains(where: { $0.id == sid }) {
                activeSessionID = nil
                items = []
            }
        }
        if activeSessionID != nil {
            await refreshActiveTranscript()
        }
    }

    func refreshActiveTranscript() async {
        guard let sid = activeSessionID,
              let computer = activeComputer,
              let paired = computer.paired, !paired.isExpired,
              let endpoint = endpointFor(computer) else { return }

        isLoadingTranscript = items.isEmpty
        defer { isLoadingTranscript = false }

        if let view = await performRPC(paired: paired, endpoint: endpoint, method: "ReadSession", session: sid) {
            if let updated = view.sessions.first(where: { $0.id == sid }) {
                if let idx = sessions.firstIndex(where: { $0.id == sid }) {
                    sessions[idx] = updated
                } else {
                    sessions.insert(updated, at: 0)
                }
            }
            items = view.items
        }
    }

    func sendMessage() async {
        let trimmed = draftText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        guard let computer = activeComputer,
              let paired = computer.paired, !paired.isExpired,
              let endpoint = endpointFor(computer) else {
            isComputersPresented = true
            return
        }

        isSending = true
        bannerError = nil
        defer { isSending = false }

        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        let expiresMs = nowMs + 300_000
        let commandID = MobileCore.randomHex(bytes: 16)
        let model = MobileCore.normalizeAntigravityModel(selectedModelID)
        let effort = MobileCore.normalizeAntigravityEffort(model: model, want: selectedEffort.rawValue)

        let view: ParsedEnvironmentView?
        if let current = activeSession {
            view = await performRPC(
                paired: paired,
                endpoint: endpoint,
                method: "SendMessage",
                session: current.id,
                command: commandID,
                generation: max(1, current.generation),
                revision: current.revision,
                text: trimmed,
                issuedMs: nowMs,
                expiresMs: expiresMs,
                model: model,
                effort: effort
            )
        } else {
            let folder = selectedFolder.isEmpty || !MobileCore.isValidFolder(selectedFolder) ? "" : selectedFolder
            view = await performRPC(
                paired: paired,
                endpoint: endpoint,
                method: "OpenSession",
                command: commandID,
                generation: 1,
                revision: 0,
                text: trimmed,
                issuedMs: nowMs,
                expiresMs: expiresMs,
                model: model,
                effort: effort,
                folder: folder
            )
        }

        guard let view else { return }
        draftText = ""
        if let opened = view.sessions.first {
            activeSessionID = opened.id
            if let idx = sessions.firstIndex(where: { $0.id == opened.id }) {
                sessions[idx] = opened
            } else {
                sessions.insert(opened, at: 0)
            }
        }
        if !view.items.isEmpty {
            items = view.items
        }
        await refreshActiveTranscript()
    }

    func stopActiveTurn() async {
        guard let current = activeSession,
              let computer = activeComputer,
              let paired = computer.paired, !paired.isExpired,
              let endpoint = endpointFor(computer) else { return }

        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        _ = await performRPC(
            paired: paired,
            endpoint: endpoint,
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

    // MARK: - Foreground polling (Phase 1 replaces with server push frames)

    private func startPolling() {
        pollTask?.cancel()
        pollTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(2.5))
                guard !Task.isCancelled, let self else { break }
                guard let computer = self.activeComputer, computer.canSendRPC else { continue }
                if self.activeSession?.busy == true {
                    await self.refreshActiveTranscript()
                }
            }
        }
    }

    // MARK: - Internal RPC helpers

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

    private func performRPC(
        paired: PairedComputer,
        endpoint: AppTailnetComputer,
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
        item: String = ""
    ) async -> ParsedEnvironmentView? {
        guard let secret = store.loadOrCreateDeviceSecret(),
              let work = MobileCoreWork(),
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
                item: item
              ) else {
            bannerError = "Could not build Antigravity request."
            return nil
        }

        let result = await client.sendRPC(
            paired: paired,
            endpoint: endpoint,
            secret: secret,
            method: method,
            params: params
        )
        switch result {
        case .failure(let error):
            bannerError = error.localizedDescription
            return nil
        case .success(let data):
            guard let parsed = ParsedEnvironmentView.parse(data) else {
                bannerError = "Malformed response from computer."
                return nil
            }
            if !parsed.error.isEmpty {
                bannerError = parsed.error
            } else {
                bannerError = nil
            }
            return parsed
        }
    }
}
