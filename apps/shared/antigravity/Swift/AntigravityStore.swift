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
    @Published var selectedEffort: ThinkingEffort = .high
    @Published var hostMetaByID: [String: HostRuntimeMeta] = [:]
    @Published var isUpdatingRemoteAgyHostID: String?
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
    @Published var liveACPCommands: [SlashCommandItem] = []
    @Published var activeTodos: [SessionTodoItem] = []
    @Published var activeUsage: SessionUsageStats?

    // Cursor pagination state (#14011)
    @Published var sessionsNextCursor: String = ""
    @Published var agySessionsNextCursor: String = ""
    @Published var transcriptNextCursor: String = ""
    @Published var subagentNextCursor: String = ""
    @Published var isLoadingMoreSessions: Bool = false
    @Published var isLoadingOlderMessages: Bool = false
    @Published var isLoadingOlderSubagentItems: Bool = false

    // Workspace File Viewer, Branch Diff & Inline Review Comments (#14012)
    @Published var workspaceFiles: [String] = []
    @Published var workspaceFilesNextCursor: Int = 0
    @Published var isLoadingWorkspaceFiles: Bool = false
    @Published var activeFileContent: WorkspaceFileContent?
    @Published var isLoadingFileContent: Bool = false
    @Published var branchDiffText: String = ""
    @Published var inlineReviewComments: [InlineReviewComment] = []

    // Rich Session Operations: metadata (pin/rename/archive) & /btw side questions (#14013)
    @Published var sessionMetaByID: [String: SessionMetadataRecord] = [:]
    @Published var showArchivedSessions: Bool = false
    @Published var isSideQuestionPresented: Bool = false
    @Published var sideQuestions: [SideQuestionEntry] = []

    // Wave 4: Artifacts, Swarms, Automations, Extensibility, Voice/Browser/Quotas, Durable Outbox & Live Status (#14014-#14019)
    @Published var isHubPresented: Bool = false
    @Published var composerFocusNonce: UInt64 = 0
    @Published var artifacts: [ArtifactSummary] = []
    @Published var activeArtifact: ArtifactDocument?
    @Published var isLoadingArtifacts: Bool = false
    @Published var swarmSnapshot: SwarmSnapshot?
    @Published var isLoadingSwarm: Bool = false
    @Published var automations: [AutomationRecord] = []
    @Published var isLoadingAutomations: Bool = false
    @Published var agyConfig: AntigravityConfigSnapshot?
    @Published var isLoadingAgyConfig: Bool = false
    @Published var selectedCustomAgent: String = ""
    @Published var rateLimitBanner: RateLimitBannerState?
    @Published var browserHandoffWarning: String?
    @Published var pendingOutbox: [PendingOutboxMessage] = []
    @Published var activeTurnLiveStatus: ActiveTurnLiveStatus?

    /// Test-only switch enabled strictly via `-OAE2ELoopbackAllowed` launch argument for local Simulator E2E harnesses.
    var allowLoopbackForTests: Bool = false

    /// Zero-secret notification callback (`kind`: "completed" | "approval", `title`: generic non-secret status).
    var onTurnNotificationEvent: ((_ kind: String, _ title: String) -> Void)?

    private var pollTask: Task<Void, Never>?
    private var pushDebounceTask: Task<Void, Never>?
    private var activeComputerRefreshTask: Task<Void, Never>?
    private var activeTranscriptRefreshTask: Task<Void, Never>?
    private var hostSelectionGen: UInt64 = 0
    private var sessionSelectionGen: UInt64 = 0
    private var lastGrantRenewAttemptByHost: [String: Int64] = [:]
    private var lastPushReceivedAt: Date = .distantPast
    private var fastPollUntil: Date = .distantPast
    private var isBackgrounded = false
    private var lastForegroundRefreshAt: Date = .distantPast
    private var reachabilityByID: [String: ReachabilityState] = [:]
    private var legacyNoModeHosts: Set<String> = []
    private var wasActiveSessionBusy = false
    private var lastNotifiedInteractionID: String = ""
    private var pendingOptimisticUserItem: TranscriptItem?
    private var pendingOptimisticBaselineUserCount: Int = 0
    private var sentImagesByName: [String: Data] = [:]
    private var interactionMeta: (sessionID: String, interactionID: String, prompt: String, warning: String, isQuestion: Bool, options: [InteractionOption])?

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
            if state != .reachable, self?.activeComputerID == host {
                self?.livePushConnected = false
            }
            self?.reloadComputers()
            if state == .reachable {
                Task { @MainActor [weak self] in
                    await self?.flushPendingOutbox()
                }
            }
        }
        self.client.onLivePushStateChange = { [weak self] host, connected in
            guard let self, self.activeComputerID == host else { return }
            self.livePushConnected = connected
        }
        self.client.onPushEvent = { [weak self] host, event in
            self?.handlePushEvent(host: host, event: event)
        }
        pendingOutbox = store.loadPendingOutbox()
        reloadComputers()
        if let warn = store.lastPersistenceWarning {
            statusToast = warn
            store.lastPersistenceWarning = nil
        }
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
        if let meta = interactionMeta,
           meta.sessionID == session.id,
           meta.interactionID == session.interactionID {
            let prompt = meta.prompt.isEmpty
                ? (items.last(where: { $0.role == .notice || $0.role == .tool })?.text ?? "Antigravity is waiting for your approval to continue.")
                : meta.prompt
            return PendingInteraction(
                id: session.interactionID,
                sessionID: session.id,
                prompt: prompt,
                warning: meta.warning,
                isQuestion: meta.isQuestion,
                options: meta.options
            )
        }
        let hint = items.last(where: { $0.role == .notice || $0.role == .tool })?.text ?? "Antigravity is waiting for your approval to continue."
        return PendingInteraction(id: session.interactionID, sessionID: session.id, prompt: hint)
    }

    var hasMoreSessions: Bool {
        !sessionsNextCursor.isEmpty || !agySessionsNextCursor.isEmpty
    }

    var hasOlderMessages: Bool {
        !transcriptNextCursor.isEmpty
    }

    var hasOlderSubagentItems: Bool {
        !subagentNextCursor.isEmpty
    }

    func displayTitle(for session: ChatSessionSummary) -> String {
        if let meta = sessionMetaByID[session.id], !meta.title.isEmpty {
            return meta.title
        }
        return session.displayTitle
    }

    func isPinned(_ session: ChatSessionSummary) -> Bool {
        sessionMetaByID[session.id]?.pinned == true
    }

    func isArchived(_ session: ChatSessionSummary) -> Bool {
        sessionMetaByID[session.id]?.archived == true
    }

    var workingSessions: [ChatSessionSummary] {
        filteredSessions.filter(\.busy)
    }

    var recentSessions: [ChatSessionSummary] {
        filteredSessions.filter { !$0.busy }
    }

    private var filteredSessions: [ChatSessionSummary] {
        let q = searchText.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let visible = sessions.filter { session in
            let archived = isArchived(session)
            if !showArchivedSessions && archived { return false }
            if q.isEmpty { return true }
            let title = displayTitle(for: session).lowercased()
            let folder = (sessionMetaByID[session.id]?.folder ?? "").lowercased()
            return title.contains(q)
                || session.model.lowercased().contains(q)
                || session.id.lowercased().contains(q)
                || folder.contains(q)
        }
        return visible.sorted { a, b in
            let pa = isPinned(a)
            let pb = isPinned(b)
            if pa != pb { return pa }
            return a.updatedAtMs > b.updatedAtMs
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

    var selectedModelSupportsThinking: Bool {
        selectedModelOption.supportsThinking
    }

    var groupedAvailableModels: [(family: String, models: [AntigravityModelOption])] {
        var order: [String] = []
        var buckets: [String: [AntigravityModelOption]] = [:]
        for model in availableModels {
            let fam = model.family
            if buckets[fam] == nil { order.append(fam) }
            buckets[fam, default: []].append(model)
        }
        return order.map { ($0, buckets[$0] ?? []) }
    }

    var activeHostMeta: HostRuntimeMeta? {
        guard let id = activeComputerID else { return nil }
        return hostMetaByID[id]
    }

    var matchingSlashCommands: [SlashCommandItem] {
        let trimmed = draftText.trimmingCharacters(in: .whitespaces)
        guard trimmed.hasPrefix("/") else { return [] }
        let query = trimmed.lowercased()
        var seenCmds = Set<String>()
        let catalog = (liveACPCommands + SlashCommandItem.defaults + dynamicSkills).filter {
            seenCmds.insert($0.command.lowercased()).inserted
        }
        if query == "/" { return Array(catalog.prefix(10)) }
        return Array(catalog.filter { $0.command.lowercased().hasPrefix(query) }.prefix(10))
    }

    /// Fuzzy `@` mention autocomplete supporting workspace files, folders, `@rules:`, and `@conversation:` (#14012).
    var matchingMentions: [MentionCandidate] {
        guard let lastWord = draftText.split(separator: " ").last, lastWord.hasPrefix("@") else {
            return []
        }
        let rawQuery = String(lastWord.dropFirst()).lowercased()
        var candidates: [MentionCandidate] = []

        if rawQuery.hasPrefix("rules:") || "rules:".hasPrefix(rawQuery) {
            let sub = rawQuery.hasPrefix("rules:") ? String(rawQuery.dropFirst(6)) : ""
            let ruleFiles = workspaceFiles.filter { path in
                let lower = path.lowercased()
                return lower.hasSuffix("agents.md") || lower.hasSuffix("gemini.md")
                    || lower.contains("/rules/") || lower.hasPrefix(".gemini/rules/")
            }
            for rf in ruleFiles {
                if sub.isEmpty || Self.fuzzyMatches(rf.lowercased(), query: sub) {
                    candidates.append(MentionCandidate(
                        id: "rule-\(rf)",
                        kind: .rule,
                        label: "@rules:\(rf)",
                        insertion: "@rules:\(rf)",
                        subtitle: "Workspace rule"
                    ))
                }
            }
            if candidates.isEmpty {
                candidates.append(MentionCandidate(
                    id: "rule-hint",
                    kind: .rule,
                    label: "@rules:AGENTS.md",
                    insertion: "@rules:AGENTS.md",
                    subtitle: "Project instructions & rules"
                ))
            }
        }

        if rawQuery.hasPrefix("conversation:") || "conversation:".hasPrefix(rawQuery) {
            let sub = rawQuery.hasPrefix("conversation:") ? String(rawQuery.dropFirst(13)) : ""
            for s in sessions.prefix(12) {
                let t = displayTitle(for: s)
                if sub.isEmpty || Self.fuzzyMatches(t.lowercased(), query: sub) || s.id.lowercased().contains(sub) {
                    candidates.append(MentionCandidate(
                        id: "conv-\(s.id)",
                        kind: .conversation,
                        label: "@conversation:\(t)",
                        insertion: "@conversation:\(s.id)",
                        subtitle: "Reference earlier chat"
                    ))
                }
            }
        }

        if !rawQuery.hasPrefix("rules:") && !rawQuery.hasPrefix("conversation:") {
            var seen = Set<String>()
            // Prioritize changed files, then full workspace tree files, then top-level folders.
            if let changed = workspaceSnapshot?.changedFiles {
                for f in changed where seen.insert(f.path).inserted {
                    if rawQuery.isEmpty || Self.fuzzyMatches(f.path.lowercased(), query: rawQuery) {
                        candidates.append(MentionCandidate(
                            id: "file-\(f.path)",
                            kind: .file,
                            label: "@\(f.path)",
                            insertion: "@\(f.path)",
                            subtitle: "Modified (\(f.code))"
                        ))
                    }
                }
            }
            for path in workspaceFiles where seen.insert(path).inserted {
                if rawQuery.isEmpty || Self.fuzzyMatches(path.lowercased(), query: rawQuery) {
                    candidates.append(MentionCandidate(
                        id: "tree-\(path)",
                        kind: .file,
                        label: "@\(path)",
                        insertion: "@\(path)",
                        subtitle: "Workspace file"
                    ))
                }
                if candidates.count >= 16 { break }
            }
            for folder in folders where seen.insert(folder).inserted {
                if rawQuery.isEmpty || Self.fuzzyMatches(folder.lowercased(), query: rawQuery) {
                    candidates.append(MentionCandidate(
                        id: "dir-\(folder)",
                        kind: .folder,
                        label: "@\(folder)",
                        insertion: "@\(folder)",
                        subtitle: "Folder"
                    ))
                }
            }
        }
        return Array(candidates.prefix(10))
    }

    /// Subsequence + substring fuzzy matcher for `@` file and mention autocomplete.
    static func fuzzyMatches(_ haystack: String, query: String) -> Bool {
        if query.isEmpty { return true }
        if haystack.contains(query) { return true }
        var qi = query.startIndex
        for ch in haystack {
            if ch == query[qi] {
                qi = query.index(after: qi)
                if qi == query.endIndex { return true }
            }
        }
        return false
    }

    var matchingFolders: [String] {
        matchingMentions.map { $0.insertion.hasPrefix("@") ? String($0.insertion.dropFirst()) : $0.insertion }
    }

    func applyMention(_ candidate: MentionCandidate) {
        let token = candidate.insertion.hasPrefix("@") ? candidate.insertion : "@\(candidate.insertion)"
        if let range = draftText.range(of: "@[^\\s]*$", options: .regularExpression) {
            draftText.replaceSubrange(range, with: "\(token) ")
        } else {
            draftText += (draftText.isEmpty || draftText.hasSuffix(" ") ? "" : " ") + "\(token) "
        }
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

    private func ensureTransportActive(force: Bool = false) {
        if force { isBackgrounded = false }
        guard !isBackgrounded, !client.isActive else { return }
        client.resume()
        subscribeActiveHost()
    }

    func onForeground() {
        let wasInactive = !client.isActive || isBackgrounded
        isBackgrounded = false
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
        let now = Date()
        if wasInactive || now.timeIntervalSince(lastForegroundRefreshAt) > 1.0 {
            lastForegroundRefreshAt = now
            Task {
                await refreshActiveComputer()
                await flushPendingOutbox()
            }
        }
    }

    func onBackground() {
        isBackgrounded = true
        pollTask?.cancel()
        pollTask = nil
        pushDebounceTask?.cancel()
        pushDebounceTask = nil
        activeComputerRefreshTask?.cancel()
        activeComputerRefreshTask = nil
        activeTranscriptRefreshTask?.cancel()
        activeTranscriptRefreshTask = nil
        livePushConnected = false
        #if os(macOS)
        LocalSocketClient.shared.stopWatchingRuns()
        #endif
        client.suspend()
        MobileCorePool.shared.drain()
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
        hostSelectionGen &+= 1
        sessionSelectionGen &+= 1
        activeComputerRefreshTask?.cancel()
        activeTranscriptRefreshTask?.cancel()
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
        livePushConnected = computer.isLocalMac && LocalSocketClient.shared.isSocketAvailable
        workspaceSnapshot = nil
        activeSubagentItem = nil
        subagentItems = []
        activeTodos = []
        activeUsage = nil
        transcriptNextCursor = ""
        subagentNextCursor = ""
        interactionMeta = nil
        bannerError = nil
        loadPreferences(for: computer.id)
        restoreOfflineCache(for: computer.id)
        subscribeActiveHost()
        activeComputerRefreshTask = Task { await refreshActiveComputer() }
    }

    func forgetComputer(_ computer: ComputerEntry) {
        guard !computer.isLocalMac else { return }
        if let key = computer.hostKey {
            store.removePaired(host: key)
            client.unsubscribe(host: key)
            store.evictCache(for: key)
        }
        store.evictCache(for: computer.id)
        store.tailnetBook.forget(computer.id)
        reachabilityByID.removeValue(forKey: computer.id)
        hostMetaByID.removeValue(forKey: computer.id)
        if activeComputerID == computer.id {
            hostSelectionGen &+= 1
            sessionSelectionGen &+= 1
            activeComputerRefreshTask?.cancel()
            activeTranscriptRefreshTask?.cancel()
            activeComputerID = nil
            activeSessionID = nil
            livePushConnected = false
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
        ensureTransportActive(force: true)
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
            let switchedHost = activeComputerID != answer.host
            if switchedHost {
                hostSelectionGen &+= 1
                sessionSelectionGen &+= 1
                activeComputerRefreshTask?.cancel()
                activeTranscriptRefreshTask?.cancel()
                activeSessionID = nil
                items = []
                sessions = []
                workspaceSnapshot = nil
                workspaceFiles = []
                sessionMetaByID = [:]
                activeTodos = []
                activeUsage = nil
                activeTurnLiveStatus = nil
                interactionMeta = nil
                pendingOptimisticUserItem = nil
                isAwaitingTurnStart = false
                loadPreferences(for: answer.host)
            }
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
        ensureTransportActive(force: true)
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
        selectedEffort = ThinkingEffort(rawValue: effortRaw) ?? .high
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
        selectedEffort = ThinkingEffort(rawValue: clamped) ?? .high
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
        activeTodos = []
        activeUsage = nil
        transcriptNextCursor = ""
        subagentNextCursor = ""
        interactionMeta = nil
        bannerError = nil
        isSidebarOpen = false
        composerFocusNonce &+= 1
        subscribeActiveHost()
    }

    func selectSession(_ session: ChatSessionSummary) {
        sessionSelectionGen &+= 1
        activeTranscriptRefreshTask?.cancel()
        activeSessionID = session.id
        pendingOptimisticUserItem = nil
        isAwaitingTurnStart = false
        fastPollUntil = .distantPast
        activeSubagentItem = nil
        subagentItems = []
        activeTodos = []
        activeUsage = nil
        transcriptNextCursor = ""
        subagentNextCursor = ""
        interactionMeta = nil
        if let wireMode = ReviewMode.fromWire(session.mode) {
            reviewMode = wireMode
        }
        bannerError = nil
        isSidebarOpen = false
        subscribeActiveHost()
        activeTranscriptRefreshTask = Task { await refreshActiveTranscript() }
    }

    /// Proactively renews direct Tailscale grants nearing expiry (< 3 days) via `AppTailnetBook.wantsAdmission` (#14007).
    private func renewGrantIfExpiringSoon(for computer: ComputerEntry) async {
        guard !computer.isLocalMac, computer.canRequestDirectGrant, let paired = computer.paired else { return }
        let now = Int64(Date().timeIntervalSince1970)
        let last = lastGrantRenewAttemptByHost[computer.id]
        // Renew when within 3 days of expiry (or AppTailnetBook.wantsAdmission window).
        let nearExpiry = (paired.expires - now) < (3 * 86_400) || AppTailnetBook.wantsAdmission(expires: paired.expires, lastAsked: last, now: now)
        guard nearExpiry, (last == nil || now - (last ?? 0) >= 60) else { return }
        lastGrantRenewAttemptByHost[computer.id] = now
        let target = computer.ip.isEmpty ? computer.dns : computer.ip
        guard !target.isEmpty, let pubkey = store.devicePublicKeyHex(), !pubkey.isEmpty else { return }
        if case .success(let answer) = await client.requestDirectGrant(
            address: target,
            port: computer.port,
            devicePubkey: pubkey,
            clientLabel: store.localClientLabel
        ), let grant = answer.grant, !grant.isEmpty {
            let renewed = PairedComputer(
                host: answer.host,
                relay: paired.relay,
                grant: grant,
                epoch: answer.epoch,
                expires: answer.expires,
                name: answer.label.isEmpty ? paired.name : answer.label
            )
            store.upsertPaired(renewed)
            reloadComputers()
        }
    }

    func refreshActiveComputer() async {
        guard let computer = activeComputer else { return }
        let capturedHostGen = hostSelectionGen
        await renewGrantIfExpiringSoon(for: computer)
        guard capturedHostGen == hostSelectionGen, activeComputerID == computer.id else { return }
        guard let currentComputer = activeComputer, currentComputer.id == computer.id, currentComputer.canSendRPC else {
            if !computer.isLocalMac && (!computer.ip.isEmpty || !computer.dns.isEmpty) {
                await probeComputer(computer)
            }
            return
        }
        isLoadingSessions = sessions.isEmpty
        defer { isLoadingSessions = false }

        if let descView = await performEnvironmentRPC(computer: currentComputer, method: "DescribeEnvironment") {
            guard capturedHostGen == hostSelectionGen, activeComputerID == computer.id else { return }
            applyMetadata(from: descView, computerID: computer.id)
        }

        var mergedByID: [String: ChatSessionSummary] = [:]
        if let listView = await performEnvironmentRPC(computer: currentComputer, method: "ListSessions") {
            guard capturedHostGen == hostSelectionGen, activeComputerID == computer.id else { return }
            applyMetadata(from: listView, computerID: computer.id)
            sessionsNextCursor = listView.nextCursor
            for s in listView.sessions { mergedByID[s.id] = s }
        }
        // Also fetch native ~/.gemini Antigravity sessions (`host-agy.coax`) and merge cleanly
        if let agyView = await performEnvironmentRPC(computer: currentComputer, method: "ListSessions", model: "antigravity", quietErrors: true) {
            guard capturedHostGen == hostSelectionGen, activeComputerID == computer.id else { return }
            agySessionsNextCursor = agyView.nextCursor
            for s in agyView.sessions where mergedByID[s.id] == nil {
                mergedByID[s.id] = s
            }
        }

        // Sync pin/rename/archive session metadata in background so legacy/minimal hosts never stall pairing (#14013)
        Task { [weak self] in
            await self?.refreshSessionMetadata()
        }

        guard capturedHostGen == hostSelectionGen, activeComputerID == computer.id else { return }
        if !mergedByID.isEmpty || bannerError == nil {
            // Preserve the currently open session across page-1 pagination boundaries (#14007).
            if let sid = activeSessionID, mergedByID[sid] == nil,
               let existing = sessions.first(where: { $0.id == sid }) {
                mergedByID[sid] = existing
            }
            sessions = mergedByID.values.sorted { $0.updatedAtMs > $1.updatedAtMs }
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

        let capturedHostGen = hostSelectionGen
        let capturedSessionGen = sessionSelectionGen
        isLoadingTranscript = items.isEmpty
        defer { isLoadingTranscript = false }

        if let view = await performEnvironmentRPC(computer: computer, method: "ReadSession", session: sid) {
            guard capturedHostGen == hostSelectionGen,
                  capturedSessionGen == sessionSelectionGen,
                  activeComputerID == computer.id,
                  activeSessionID == sid else { return }
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
                if !currentInteraction.isEmpty {
                    if !view.interactionPrompt.isEmpty || !view.interactionWarning.isEmpty || !view.interactionOptions.isEmpty || view.interactionQuestion {
                        interactionMeta = (
                            sessionID: sid,
                            interactionID: currentInteraction,
                            prompt: view.interactionPrompt,
                            warning: view.interactionWarning,
                            isQuestion: view.interactionQuestion,
                            options: view.interactionOptions
                        )
                    }
                } else if interactionMeta?.sessionID == sid {
                    interactionMeta = nil
                }
            }
            applyServerItems(view.items, nowBusy: nowBusy)
            // Keep pagination cursor pointing at the oldest loaded page when older messages haven't been prepended yet (#14011).
            if transcriptNextCursor.isEmpty || items.count <= view.items.count + 1 {
                transcriptNextCursor = view.nextCursor
            }
            if !view.todos.isEmpty || !nowBusy {
                activeTodos = view.todos
            }
            if !view.liveCommands.isEmpty {
                liveACPCommands = view.liveCommands
            }
            if let usage = view.usage {
                activeUsage = usage
            }
            detectRateLimitAndBrowserHandoff(from: view.items, error: view.error)
            updateLiveTurnStatus(nowBusy: nowBusy, needsApproval: !currentInteraction.isEmpty)
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

            // Auto-dispatch queued follow-up without clobbering user's live draftText or attachments (#14007).
            if wasActiveSessionBusy && !nowBusy, let nextPrompt = queuedDraft, !nextPrompt.isEmpty {
                queuedDraft = nil
                wasActiveSessionBusy = false
                await sendMessage(overrideText: nextPrompt, overrideAttachments: [])
                return
            }
            wasActiveSessionBusy = nowBusy
        }
    }

    func sendMessage(overrideText: String? = nil, overrideAttachments: [FileAttachment]? = nil) async {
        let isOverride = overrideText != nil
        let rawInput = overrideText ?? draftText
        let baseText = rawInput.trimmingCharacters(in: .whitespacesAndNewlines)
        // Intercept client-handled rich session slash commands (/btw, /fork, /rewind; #14013).
        if !isOverride {
            if baseText.lowercased().hasPrefix("/btw ") {
                let q = String(baseText.dropFirst(5)).trimmingCharacters(in: .whitespacesAndNewlines)
                draftText = ""
                isSideQuestionPresented = true
                if !q.isEmpty { await askSideQuestion(q) }
                return
            } else if baseText.lowercased() == "/btw" {
                draftText = ""
                isSideQuestionPresented = true
                return
            } else if baseText.lowercased() == "/fork" {
                draftText = ""
                await forkActiveSession()
                return
            } else if baseText.lowercased() == "/rewind" {
                draftText = ""
                if let lastUser = items.last(where: { $0.role == .user }) {
                    await rewindToMessage(lastUser)
                } else {
                    statusToast = "No earlier user turn to rewind to."
                }
                return
            }
        }
        // Attach batched inline Workspace review comments when present (#14012).
        let pendingComments = inlineReviewComments
        let effectiveBaseText: String
        if !pendingComments.isEmpty {
            let block = "Inline code review comments:\n" + pendingComments.map(\.renderedMarkdown).joined(separator: "\n")
            effectiveBaseText = baseText.isEmpty ? block : "\(block)\n\n\(baseText)"
            inlineReviewComments = []
        } else {
            effectiveBaseText = baseText
        }
        let attachmentsToSend = overrideAttachments ?? pendingAttachments
        guard !effectiveBaseText.isEmpty || !attachmentsToSend.isEmpty else { return }
        guard let computer = activeComputer, computer.canSendRPC else {
            isComputersPresented = true
            return
        }
        if !computer.isLocalMac {
            ensureTransportActive(force: true)
        }

        let capturedDraft = draftText
        let capturedAttachments = pendingAttachments
        let previousSessionID = activeSessionID
        for att in attachmentsToSend {
            if let img = att.imageData {
                sentImagesByName[att.name] = img
            }
        }

        // If viewing a read-only ended desktop chat, smoothly continue in a fresh interactive session
        // seeded with a compact context header instead of hitting a host "conflict" refusal.
        let continueFromReadOnly = activeSession?.isObserveOnly == true
        var seedPrefix = ""
        if continueFromReadOnly, let oldTitle = activeSession?.displayTitle {
            let lastReply = (items.last(where: { $0.role == .assistant })?.text ?? "").truncatedToUTF8Bytes(1_200)
            seedPrefix = "[Continuing from earlier session \"\(oldTitle)\"]\n"
            if !lastReply.isEmpty {
                seedPrefix += "[Last reply summary]: \(lastReply)\n\n"
            }
        }

        // Legacy 16-key openagents-environment hosts reject EaRequest.mode / attachments with "unsupported".
        // Always omit "mode" for the default (.auto) and refuse silent downgrade of interactive approval
        // modes (.ask / .acceptEdits / .turbo) on legacy hosts so safety guarantees are never silently bypassed (#13996).
        let hostSupportsV2 = !legacyNoModeHosts.contains(computer.id)
        if !hostSupportsV2 && (reviewMode == .ask || reviewMode == .acceptEdits || reviewMode == .turbo) {
            bannerError = "Your Mac is running an older OpenAgents host daemon that doesn't support \(reviewMode.label) mode yet. Update OpenAgents on your Mac, or switch Review Mode to Auto or Plan."
            return
        }

        func composePayload(useStructuredAttachments: Bool, modeWire: String) -> (text: String, attachmentsJSON: String) {
            var body = seedPrefix + effectiveBaseText
            let attJSON = useStructuredAttachments ? FileAttachment.encodeWireAttachmentsJSON(attachmentsToSend) : ""
            for att in attachmentsToSend {
                if !attJSON.isEmpty && att.hasStructuredWireData {
                    body += att.renderedV2Placeholder()
                } else {
                    body += att.renderedPromptBlock()
                }
            }
            if reviewMode == .plan && modeWire.isEmpty && !body.hasPrefix("/plan") && !body.hasPrefix("[Plan mode") {
                body = "[Plan mode — outline a plan before editing files]\n\n" + body
            }
            return (body.truncatedToUTF8Bytes(15_500), attJSON)
        }

        var modeWire = (reviewMode == .auto || !hostSupportsV2) ? "" : reviewMode.wireValue
        let initial = composePayload(useStructuredAttachments: hostSupportsV2, modeWire: modeWire)
        let payloadText = initial.text
        var attachmentsJSON = initial.attachmentsJSON

        // Clear composer & display the optimistic user bubble (including real photo thumbnails!) at 0ms.
        let commandID = MobileCore.randomHex(bytes: 16)
        let optimisticID = "pending-\(commandID)"
        let previousItems = items
        if continueFromReadOnly {
            activeSessionID = nil
            items = []
        }
        let baselineUsers = items.filter { $0.role == .user }.count
        let optimisticBubbleText = attachmentsToSend.contains(where: \.isImage) ? payloadText : (effectiveBaseText.isEmpty ? payloadText : effectiveBaseText)
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
        if !isOverride {
            draftText = ""
            pendingAttachments = []
        }

        isSending = true
        isAwaitingTurnStart = true
        bannerError = nil
        statusToast = nil
        defer { isSending = false }

        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        let expiresMs = nowMs + 300_000
        let model = MobileCore.normalizeAntigravityModel(selectedModelID)
        let effort = selectedModelSupportsThinking
            ? MobileCore.normalizeAntigravityEffort(model: model, want: selectedEffort.rawValue)
            : ""

        func dispatchSend(modeArg: String, textArg: String, attArg: String, cmdArg: String) async -> ParsedEnvironmentView? {
            let quiet = !modeArg.isEmpty || !attArg.isEmpty
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
                    attachmentsJSON: attArg,
                    quietErrors: quiet
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
                    attachmentsJSON: attArg,
                    quietErrors: quiet
                )
            }
        }

        var view = await dispatchSend(modeArg: modeWire, textArg: payloadText, attArg: attachmentsJSON, cmdArg: commandID)

        // Transparent v1 downgrade ONLY when safe (.auto or .plan with instruction prefix, or structured attachments -> inline [oa-image ...]).
        // NEVER silently downgrade .ask / .acceptEdits / .turbo to unconstrained auto (#13996)!
        if let v = view, v.error == "unsupported", (!modeWire.isEmpty || !attachmentsJSON.isEmpty) {
            legacyNoModeHosts.insert(computer.id)
            if reviewMode == .ask || reviewMode == .acceptEdits || reviewMode == .turbo {
                bannerError = "Your Mac is running an older OpenAgents host daemon that doesn't support \(reviewMode.label) mode yet. Update OpenAgents on your Mac, or switch Review Mode to Auto or Plan."
                pendingOptimisticUserItem = nil
                activeSessionID = previousSessionID
                items = previousItems
                if !isOverride {
                    draftText = capturedDraft
                    pendingAttachments = capturedAttachments
                } else {
                    queuedDraft = rawInput
                }
                isAwaitingTurnStart = false
                return
            }
            modeWire = ""
            attachmentsJSON = ""
            let fallback = composePayload(useStructuredAttachments: false, modeWire: "")
            view = await dispatchSend(modeArg: "", textArg: fallback.text, attArg: "", cmdArg: MobileCore.randomHex(bytes: 16))
        }

        guard let finalView = view, finalView.error.isEmpty else {
            let errCode = view?.error ?? ""
            if errCode.isEmpty && !computer.isLocalMac && !isOverride {
                // Network / offline transport failure: enqueue into durable disk-backed offline outbox with the exact commandID (#14019).
                enqueueOfflineOutbox(
                    commandID: commandID,
                    hostID: computer.id,
                    sessionID: continueFromReadOnly ? nil : previousSessionID,
                    text: payloadText,
                    model: model,
                    effort: effort,
                    reviewMode: modeWire,
                    folder: selectedFolder
                )
                pendingOptimisticUserItem = nil
                activeSessionID = previousSessionID
                items = previousItems
                isAwaitingTurnStart = false
                statusToast = "Saved to offline outbox — will send automatically on reconnect."
                return
            }
            if !errCode.isEmpty {
                bannerError = HostErrorFormatter.describe(errCode, method: activeSession == nil ? "OpenSession" : "SendMessage")
                detectRateLimitAndBrowserHandoff(from: items, error: errCode)
            }
            pendingOptimisticUserItem = nil
            activeSessionID = previousSessionID
            items = previousItems
            if !isOverride {
                draftText = capturedDraft
                pendingAttachments = capturedAttachments
            } else {
                queuedDraft = rawInput
            }
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

    func respondToInteraction(allow: Bool, note: String = "", optionID: String = "") async {
        guard let current = activeSession,
              !current.interactionID.isEmpty,
              let computer = activeComputer,
              computer.canSendRPC else { return }

        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        let hostSupportsV2 = !legacyNoModeHosts.contains(computer.id)
        let optWire = hostSupportsV2 ? optionID : ""
        var view = await performEnvironmentRPC(
            computer: computer,
            method: "RespondInteraction",
            session: current.id,
            command: MobileCore.randomHex(bytes: 16),
            generation: max(1, current.generation),
            revision: current.revision,
            text: note.truncatedToUTF8Bytes(4_000),
            interaction: current.interactionID,
            allow: allow,
            issuedMs: nowMs,
            expiresMs: nowMs + 300_000,
            optionID: optWire,
            quietErrors: !optWire.isEmpty
        )
        if let v = view, v.error == "unsupported", !optWire.isEmpty {
            legacyNoModeHosts.insert(computer.id)
            view = await performEnvironmentRPC(
                computer: computer,
                method: "RespondInteraction",
                session: current.id,
                command: MobileCore.randomHex(bytes: 16),
                generation: max(1, current.generation),
                revision: current.revision,
                text: note.truncatedToUTF8Bytes(4_000),
                interaction: current.interactionID,
                allow: allow,
                issuedMs: nowMs,
                expiresMs: nowMs + 300_000
            )
        }
        if let v = view, v.error.isEmpty {
            interactionMeta = nil
        }
        await refreshActiveTranscript()
    }

    // MARK: - Subagent Inspector (`ReadSubagent` & `StopSubagent`) & Cursor Pagination (#14011)

    func openSubagent(_ item: TranscriptItem) {
        activeSubagentItem = item
        subagentItems = []
        subagentNextCursor = ""
        Task { await refreshSubagent() }
    }

    func closeSubagent() {
        activeSubagentItem = nil
        subagentItems = []
        subagentNextCursor = ""
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
            if subagentItems.count <= view.items.count || subagentNextCursor.isEmpty {
                subagentNextCursor = view.nextCursor
            }
            if subagentItems.count > view.items.count {
                let freshIDs = Set(view.items.map(\.id))
                let olderPrefix = subagentItems.filter { !freshIDs.contains($0.id) }
                subagentItems = olderPrefix + view.items
            } else {
                subagentItems = view.items
            }
        }
    }

    func loadMoreSessions() async {
        guard let computer = activeComputer, computer.canSendRPC,
              !isLoadingMoreSessions, hasMoreSessions else { return }
        isLoadingMoreSessions = true
        defer { isLoadingMoreSessions = false }

        var byID: [String: ChatSessionSummary] = Dictionary(uniqueKeysWithValues: sessions.map { ($0.id, $0) })
        if let cursor = Int64(sessionsNextCursor), cursor > 0 {
            if let view = await performEnvironmentRPC(computer: computer, method: "ListSessions", before: cursor, quietErrors: true) {
                sessionsNextCursor = view.nextCursor
                for s in view.sessions { byID[s.id] = s }
            } else {
                sessionsNextCursor = ""
            }
        }
        if let cursor = Int64(agySessionsNextCursor), cursor > 0 {
            if let view = await performEnvironmentRPC(computer: computer, method: "ListSessions", before: cursor, model: "antigravity", quietErrors: true) {
                agySessionsNextCursor = view.nextCursor
                for s in view.sessions where byID[s.id] == nil { byID[s.id] = s }
            } else {
                agySessionsNextCursor = ""
            }
        }
        sessions = byID.values.sorted { $0.updatedAtMs > $1.updatedAtMs }
    }

    func loadOlderMessages() async {
        guard let sid = activeSessionID,
              let computer = activeComputer, computer.canSendRPC,
              !isLoadingOlderMessages,
              let cursor = Int64(transcriptNextCursor), cursor > 0 else { return }
        isLoadingOlderMessages = true
        defer { isLoadingOlderMessages = false }

        if let view = await performEnvironmentRPC(
            computer: computer,
            method: "ReadSession",
            session: sid,
            before: cursor,
            quietErrors: true
        ) {
            guard activeSessionID == sid else { return }
            let existingIDs = Set(items.map(\.id))
            let prepended = view.items.filter { !existingIDs.contains($0.id) }
            if !prepended.isEmpty {
                items = prepended + items
            }
            transcriptNextCursor = view.nextCursor == transcriptNextCursor ? "" : view.nextCursor
        }
    }

    func loadOlderSubagentItems() async {
        guard let sub = activeSubagentItem,
              let sid = activeSessionID,
              let computer = activeComputer, computer.canSendRPC,
              !isLoadingOlderSubagentItems,
              let cursor = Int64(subagentNextCursor), cursor > 0 else { return }
        isLoadingOlderSubagentItems = true
        defer { isLoadingOlderSubagentItems = false }

        if let view = await performEnvironmentRPC(
            computer: computer,
            method: "ReadSubagent",
            session: sid,
            before: cursor,
            item: sub.id,
            quietErrors: true
        ) {
            let existingIDs = Set(subagentItems.map(\.id))
            let prepended = view.items.filter { !existingIDs.contains($0.id) }
            if !prepended.isEmpty {
                subagentItems = prepended + subagentItems
            }
            subagentNextCursor = view.nextCursor == subagentNextCursor ? "" : view.nextCursor
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

    // MARK: - Workspace Git / Diff / Files / Branch / Worktree / PR Controls (`WsRequest`)

    /// Fetches all 4 paginated views of `ReadWorkspace` (`page=0` status/diff, `page=1` branches,
    /// `page=2` worktrees, `page=3` PRs) and merges both worktrees & branches safely (#13999, #14012).
    func refreshWorkspace() async {
        guard let computer = activeComputer, computer.canSendRPC else { return }
        isLoadingWorkspace = true
        workspaceError = nil
        defer { isLoadingWorkspace = false }

        guard var merged = await performWorkspaceRPC(computer: computer, method: "ReadWorkspace", page: 0) else { return }
        if !merged.path.isEmpty {
            refreshDynamicSkills(workspaceRoot: merged.path)
        }
        if !merged.error.isEmpty {
            workspaceSnapshot = merged
            workspaceError = HostErrorFormatter.describe(merged.error, method: "ReadWorkspace")
            return
        }

        if let p1 = await performWorkspaceRPC(computer: computer, method: "ReadWorkspace", page: 1, quietErrors: true),
           p1.error.isEmpty {
            merged = merged.merging(worktrees: p1.worktrees, branches: p1.branches)
        }
        if let p2 = await performWorkspaceRPC(computer: computer, method: "ReadWorkspace", page: 2, quietErrors: true),
           p2.error.isEmpty {
            merged = merged.merging(worktrees: p2.worktrees, branches: p2.branches)
        }
        if let p3 = await performWorkspaceRPC(computer: computer, method: "ReadWorkspace", page: 3, quietErrors: true),
           p3.error.isEmpty {
            merged = merged.merging(pullRequests: p3.pullRequests, githubError: p3.githubError)
        }

        guard activeComputerID == computer.id else { return }
        workspaceSnapshot = merged
        if workspaceFiles.isEmpty {
            Task { [weak self] in
                await self?.loadWorkspaceTree(prefix: "", reset: true)
            }
        }
    }

    func loadWorkspaceTree(prefix: String = "", reset: Bool = true) async {
        guard let computer = activeComputer, computer.canSendRPC else { return }
        isLoadingWorkspaceFiles = true
        defer { isLoadingWorkspaceFiles = false }

        let startCursor: Int64 = reset ? 0 : Int64(workspaceFilesNextCursor)
        guard let snap = await performWorkspaceRPC(
            computer: computer,
            method: "ReadWorkspaceTree",
            title: prefix,
            cursor: startCursor,
            quietErrors: true
        ), snap.error.isEmpty else { return }

        let paths = snap.status
            .split(whereSeparator: \.isNewline)
            .map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
        if reset {
            workspaceFiles = paths
        } else {
            var seen = Set(workspaceFiles)
            for p in paths where seen.insert(p).inserted {
                workspaceFiles.append(p)
            }
        }
        workspaceFilesNextCursor = snap.nextCursor
    }

    func readWorkspaceFile(path: String, startLine: Int = 1, append: Bool = false) async {
        guard let computer = activeComputer, computer.canSendRPC, !path.isEmpty else { return }
        isLoadingFileContent = true
        defer { isLoadingFileContent = false }

        guard let snap = await performWorkspaceRPC(
            computer: computer,
            method: "ReadWorkspaceFile",
            title: path,
            cursor: Int64(max(1, startLine)),
            quietErrors: false
        ) else { return }

        if !snap.error.isEmpty {
            workspaceError = HostErrorFormatter.describe(snap.error, method: "ReadWorkspaceFile")
            return
        }
        let isBinary = snap.status == "binary"
        let chunk = snap.diff
        if append, let existing = activeFileContent, existing.path == path, !isBinary {
            let combined = existing.content.isEmpty ? chunk : (existing.content + "\n" + chunk)
            activeFileContent = WorkspaceFileContent(
                path: path,
                content: combined,
                startLine: existing.startLine,
                nextLine: snap.nextCursor,
                truncated: snap.diffCut,
                isBinary: false
            )
        } else {
            activeFileContent = WorkspaceFileContent(
                path: path,
                content: chunk,
                startLine: max(1, startLine),
                nextLine: snap.nextCursor,
                truncated: snap.diffCut,
                isBinary: isBinary
            )
        }
    }

    func loadBranchDiff(base: String = "") async {
        guard let computer = activeComputer, computer.canSendRPC else { return }
        isLoadingWorkspace = true
        defer { isLoadingWorkspace = false }

        if let snap = await performWorkspaceRPC(
            computer: computer,
            method: "ReadBranchDiff",
            base: base,
            quietErrors: true
        ), snap.error.isEmpty {
            branchDiffText = snap.diff
        }
    }

    func stageFile(_ path: String) async {
        guard let computer = activeComputer, computer.canSendRPC,
              let rev = workspaceSnapshot?.revision, !rev.isEmpty else { return }
        await runWorkspaceMutation(computer: computer, method: "StageFile", title: path, expectedRevision: rev)
    }

    func unstageFile(_ path: String) async {
        guard let computer = activeComputer, computer.canSendRPC,
              let rev = workspaceSnapshot?.revision, !rev.isEmpty else { return }
        await runWorkspaceMutation(computer: computer, method: "UnstageFile", title: path, expectedRevision: rev)
    }

    func discardFile(_ path: String) async {
        guard let computer = activeComputer, computer.canSendRPC,
              let rev = workspaceSnapshot?.revision, !rev.isEmpty else { return }
        await runWorkspaceMutation(computer: computer, method: "DiscardFile", title: path, expectedRevision: rev)
    }

    func commitChanges(subject: String, body: String = "", amend: Bool = false) async {
        let trimmed = subject.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty,
              let computer = activeComputer, computer.canSendRPC,
              let rev = workspaceSnapshot?.revision, !rev.isEmpty else { return }
        await runWorkspaceMutation(
            computer: computer,
            method: "CommitChanges",
            title: trimmed,
            body: body,
            number: amend ? 1 : 0,
            expectedRevision: rev
        )
        if workspaceError == nil {
            statusToast = amend ? "Amended commit" : "Committed changes"
        }
    }

    func editPullRequest(number: Int, title: String, body: String) async {
        guard number > 0,
              let computer = activeComputer, computer.canSendRPC,
              let rev = workspaceSnapshot?.revision, !rev.isEmpty else { return }
        await runWorkspaceMutation(
            computer: computer,
            method: "EditPullRequest",
            title: title,
            body: body,
            number: Int64(number),
            expectedRevision: rev
        )
        if workspaceError == nil {
            statusToast = "Updated PR #\(number)"
        }
    }

    func addInlineComment(filePath: String, lineRange: String, comment: String) {
        let cleanComment = comment.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !filePath.isEmpty, !cleanComment.isEmpty else { return }
        inlineReviewComments.append(InlineReviewComment(filePath: filePath, lineRange: lineRange, comment: cleanComment))
        statusToast = "Added review comment (\(inlineReviewComments.count) queued)"
    }

    func removeInlineComment(_ id: UUID) {
        inlineReviewComments.removeAll { $0.id == id }
    }

    func quoteFileRange(path: String, lineRange: String = "") {
        let ref = lineRange.isEmpty ? "@\(path)" : "@\(path) #\(lineRange)"
        if draftText.isEmpty {
            draftText = "\(ref) "
        } else if draftText.hasSuffix(" ") || draftText.hasSuffix("\n") {
            draftText += "\(ref) "
        } else {
            draftText += " \(ref) "
        }
        isWorkspacePresented = false
        statusToast = "Quoted \(ref) in composer"
    }

    func openFileFromLink(_ rawURL: String) {
        let cleaned = rawURL.replacingOccurrences(of: "file://", with: "")
        let parts = cleaned.split(separator: "#", maxSplits: 1)
        var path = String(parts.first ?? "")
        if let root = workspaceSnapshot?.path, !root.isEmpty, path.hasPrefix(root + "/") {
            path = String(path.dropFirst(root.count + 1))
        }
        if path.hasPrefix("/") {
            // Try matching suffix against known workspace files
            if let match = workspaceFiles.first(where: { path.hasSuffix("/" + $0) || path == $0 }) {
                path = match
            } else {
                path = URL(fileURLWithPath: path).lastPathComponent
            }
        }
        var startLine = 1
        if parts.count > 1 {
            let frag = String(parts[1]).replacingOccurrences(of: "L", with: "")
            if let firstNum = frag.split(separator: "-").first, let parsed = Int(firstNum), parsed > 0 {
                startLine = parsed
            }
        }
        isWorkspacePresented = true
        Task { await readWorkspaceFile(path: path, startLine: startLine, append: false) }
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

    /// Removes a secondary worktree by its 64-char hex `tree.id` in `WsRequest.workspace` (#13999).
    func removeWorktree(id: String) async {
        guard let computer = activeComputer, computer.canSendRPC,
              let rev = workspaceSnapshot?.revision, !rev.isEmpty else { return }
        await runWorkspaceMutation(computer: computer, method: "RemoveWorktree", workspace: id, expectedRevision: rev)
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
        workspace: String = "default",
        branch: String = "",
        base: String = "",
        title: String = "",
        body: String = "",
        number: Int64 = 0,
        expectedRevision: String
    ) async {
        workspaceActionBusy = true
        workspaceError = nil
        defer { workspaceActionBusy = false }

        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        if var snap = await performWorkspaceRPC(
            computer: computer,
            method: method,
            workspace: workspace,
            branch: branch,
            base: base,
            title: title,
            body: body,
            number: number,
            expectedRevision: expectedRevision,
            command: MobileCore.randomHex(bytes: 16),
            issuedMs: nowMs,
            expiresMs: nowMs + 120_000
        ) {
            if !snap.error.isEmpty {
                workspaceSnapshot = snap
                workspaceError = HostErrorFormatter.describe(snap.error, method: method)
                return
            }
            if !snap.githubError.isEmpty {
                workspaceError = snap.githubError
            }
            // Refresh paginated collections after mutation so Branches/Worktrees/PRs reflect the change.
            if let p1 = await performWorkspaceRPC(computer: computer, method: "ReadWorkspace", page: 1, quietErrors: true),
               p1.error.isEmpty {
                snap = snap.merging(worktrees: p1.worktrees, branches: p1.branches)
            }
            if let p2 = await performWorkspaceRPC(computer: computer, method: "ReadWorkspace", page: 2, quietErrors: true),
               p2.error.isEmpty {
                snap = snap.merging(worktrees: p2.worktrees, branches: p2.branches)
            }
            if let p3 = await performWorkspaceRPC(computer: computer, method: "ReadWorkspace", page: 3, quietErrors: true),
               p3.error.isEmpty {
                snap = snap.merging(pullRequests: p3.pullRequests, githubError: p3.githubError)
            }
            workspaceSnapshot = snap
        }
    }

    // MARK: - Rich Session Operations: Pin / Rename / Archive / Compact / Fork / Rewind / Side Questions (#14013)

    func refreshSessionMetadata() async {
        guard let computer = activeComputer, computer.canSendRPC else { return }
        if let view = await performDaemonRPC(computer: computer, method: "ListSessionMeta", quietErrors: true), view.ok {
            var next: [String: SessionMetadataRecord] = [:]
            for row in view.sessions { next[row.id] = row }
            sessionMetaByID = next
        }
    }

    func togglePinSession(_ session: ChatSessionSummary) async {
        guard let computer = activeComputer, computer.canSendRPC else { return }
        let nextPinned = !isPinned(session)
        let current = sessionMetaByID[session.id]
        if let view = await performDaemonRPC(
            computer: computer,
            method: "UpdateSessionMeta",
            session: session.id,
            title: current?.title ?? "",
            folder: current?.folder ?? "",
            pinned: nextPinned,
            archived: current?.archived,
            quietErrors: false
        ), view.ok {
            var updated: [String: SessionMetadataRecord] = sessionMetaByID
            for row in view.sessions { updated[row.id] = row }
            if updated[session.id] == nil {
                updated[session.id] = SessionMetadataRecord(
                    id: session.id,
                    title: current?.title ?? "",
                    folder: current?.folder ?? "",
                    pinned: nextPinned,
                    archived: current?.archived ?? false,
                    updatedAt: Int64(Date().timeIntervalSince1970)
                )
            }
            sessionMetaByID = updated
            statusToast = nextPinned ? "Pinned chat" : "Unpinned chat"
        }
    }

    func renameSession(_ session: ChatSessionSummary, title: String) async {
        guard let computer = activeComputer, computer.canSendRPC else { return }
        let cleanTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !cleanTitle.isEmpty else { return }
        let current = sessionMetaByID[session.id]
        if let view = await performDaemonRPC(
            computer: computer,
            method: "UpdateSessionMeta",
            session: session.id,
            title: cleanTitle,
            folder: current?.folder ?? "",
            pinned: current?.pinned,
            archived: current?.archived,
            quietErrors: false
        ), view.ok {
            var updated = sessionMetaByID
            for row in view.sessions { updated[row.id] = row }
            if updated[session.id] == nil {
                updated[session.id] = SessionMetadataRecord(
                    id: session.id,
                    title: cleanTitle,
                    folder: current?.folder ?? "",
                    pinned: current?.pinned ?? false,
                    archived: current?.archived ?? false,
                    updatedAt: Int64(Date().timeIntervalSince1970)
                )
            }
            sessionMetaByID = updated
            statusToast = "Renamed chat"
        }
    }

    func toggleArchiveSession(_ session: ChatSessionSummary) async {
        guard let computer = activeComputer, computer.canSendRPC else { return }
        let nextArchived = !isArchived(session)
        let current = sessionMetaByID[session.id]
        if let view = await performDaemonRPC(
            computer: computer,
            method: "UpdateSessionMeta",
            session: session.id,
            title: current?.title ?? "",
            folder: current?.folder ?? "",
            pinned: current?.pinned,
            archived: nextArchived,
            quietErrors: false
        ), view.ok {
            var updated = sessionMetaByID
            for row in view.sessions { updated[row.id] = row }
            if updated[session.id] == nil {
                updated[session.id] = SessionMetadataRecord(
                    id: session.id,
                    title: current?.title ?? "",
                    folder: current?.folder ?? "",
                    pinned: current?.pinned ?? false,
                    archived: nextArchived,
                    updatedAt: Int64(Date().timeIntervalSince1970)
                )
            }
            sessionMetaByID = updated
            if nextArchived && activeSessionID == session.id && !showArchivedSessions {
                startNewChat()
            }
            statusToast = nextArchived ? "Archived chat" : "Restored chat"
        }
    }

    func triggerCompact() async {
        await sendMessage(overrideText: "/compact", overrideAttachments: [])
    }

    private func buildSeedContext(upToExclusive targetID: String? = nil) -> String {
        var slice: [TranscriptItem] = []
        for item in items {
            if let targetID, item.id == targetID { break }
            if item.role == .user || item.role == .assistant {
                slice.append(item)
            }
        }
        let recent = slice.suffix(8)
        let lines: [String] = recent.map { row in
            let speaker = row.role == .user ? "User" : "Assistant"
            let clean = row.role == .user ? ParsedUserMessage.parse(row.text).text : row.text
            return "\(speaker): \(clean.truncatedToUTF8Bytes(500))"
        }
        return lines.joined(separator: "\n\n")
    }

    func forkActiveSession() async {
        guard let current = activeSession else {
            statusToast = "Open a chat first to fork it."
            return
        }
        let seed = buildSeedContext()
        let title = displayTitle(for: current)
        startNewChat()
        let forkPrompt = seed.isEmpty
            ? "[Forked from \"\(title)\"] Ready to branch off — briefly confirm context and await my next instruction."
            : "[Forked from \"\(title)\"]\nRecent context:\n\(seed)\n\nAcknowledge this forked branch briefly and wait for my next instruction."
        await sendMessage(overrideText: forkPrompt, overrideAttachments: [])
        statusToast = "Forked session"
    }

    func rewindToMessage(_ targetItem: TranscriptItem) async {
        let original = ParsedUserMessage.parse(targetItem.text).text
        let seed = buildSeedContext(upToExclusive: targetItem.id)
        startNewChat()
        draftText = original
        if !seed.isEmpty {
            statusToast = "Rewound draft — edit and send to continue from that turn"
        } else {
            statusToast = "Restored message to composer"
        }
    }

    func editAndResendMessage(_ targetItem: TranscriptItem, editedText: String) async {
        let clean = editedText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clean.isEmpty else { return }
        let seed = buildSeedContext(upToExclusive: targetItem.id)
        let oldTitle = activeSession.map { displayTitle(for: $0) } ?? "previous chat"
        startNewChat()
        let prompt: String
        if seed.isEmpty {
            prompt = clean
        } else {
            prompt = "[Edited earlier turn from \"\(oldTitle)\"]\nContext before edited turn:\n\(seed)\n\nNew user message:\n\(clean)"
        }
        await sendMessage(overrideText: prompt, overrideAttachments: [])
    }

    func askSideQuestion(_ question: String) async {
        let clean = question.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clean.isEmpty, let computer = activeComputer, computer.canSendRPC else { return }
        let entry = SideQuestionEntry(question: clean, answer: "", isLoading: true)
        sideQuestions.append(entry)

        let context = buildSeedContext()
        let prompt = """
        [Read-only /btw side question — DO NOT edit files or run mutating commands. Answer concisely.]
        \(context.isEmpty ? "" : "Current conversation context:\n\(context)\n\n")Question: \(clean)
        """
        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        let sideSessionID = MobileCore.randomHex(bytes: 16)
        let model = MobileCore.normalizeAntigravityModel(selectedModelID)
        let folder = selectedFolder.isEmpty || !MobileCore.isValidFolder(selectedFolder) ? "" : selectedFolder

        guard let openView = await performEnvironmentRPC(
            computer: computer,
            method: "OpenSession",
            session: sideSessionID,
            command: MobileCore.randomHex(bytes: 16),
            generation: 1,
            revision: 0,
            text: prompt.truncatedToUTF8Bytes(12_000),
            issuedMs: nowMs,
            expiresMs: nowMs + 120_000,
            model: model,
            folder: folder,
            quietErrors: true
        ), openView.error.isEmpty else {
            if let idx = sideQuestions.firstIndex(where: { $0.id == entry.id }) {
                sideQuestions[idx].answer = "Could not start side question on host."
                sideQuestions[idx].isLoading = false
            }
            return
        }

        // Poll briefly for side-question reply without disturbing the main active session
        for _ in 0..<40 {
            try? await Task.sleep(for: .milliseconds(500))
            guard let readView = await performEnvironmentRPC(
                computer: computer,
                method: "ReadSession",
                session: sideSessionID,
                quietErrors: true
            ) else { continue }
            let reply = readView.items.last(where: { $0.role == .assistant })?.text ?? ""
            let stillBusy = readView.sessions.first(where: { $0.id == sideSessionID })?.busy ?? false
            if !reply.isEmpty {
                if let idx = sideQuestions.firstIndex(where: { $0.id == entry.id }) {
                    sideQuestions[idx].answer = reply
                    sideQuestions[idx].isLoading = stillBusy
                }
            }
            if !stillBusy && !reply.isEmpty { break }
        }
        if let idx = sideQuestions.firstIndex(where: { $0.id == entry.id }) {
            sideQuestions[idx].isLoading = false
            if sideQuestions[idx].answer.isEmpty {
                sideQuestions[idx].answer = "No response received yet."
            }
        }
    }

    func promoteSideQuestionToConversation(_ entry: SideQuestionEntry) {
        let block = "Regarding my side question (\(entry.question)):\n\(entry.answer)\n\n"
        draftText = draftText.isEmpty ? block : "\(draftText)\n\(block)"
        isSideQuestionPresented = false
        statusToast = "Promoted side question to composer"
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

    private func applyMetadata(from view: ParsedEnvironmentView, computerID: String? = nil) {
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
            let supported = selectedModelOption.supportedEfforts
            if !supported.isEmpty && !supported.contains(selectedEffort) {
                selectedEffort = supported.contains(.high) ? .high : supported[0]
            }
        }
        if let meta = view.hostMeta, let cid = computerID ?? activeComputerID {
            hostMetaByID[cid] = meta
        }
    }

    /// Triggers remote `HostUpdateAgy` RPC on a paired host (#14004).
    func updateRemoteAntigravity(on computer: ComputerEntry) async -> String? {
        guard computer.canSendRPC else { return "Host is not paired or reachable." }
        isUpdatingRemoteAgyHostID = computer.id
        defer { isUpdatingRemoteAgyHostID = nil }
        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        guard let view = await performEnvironmentRPC(
            computer: computer,
            method: "HostUpdateAgy",
            command: MobileCore.randomHex(bytes: 16),
            issuedMs: nowMs,
            expiresMs: nowMs + 120_000
        ) else {
            return bannerError ?? "Could not update Antigravity on host."
        }
        applyMetadata(from: view, computerID: computer.id)
        if !view.error.isEmpty {
            return HostErrorFormatter.describe(view.error, method: "HostUpdateAgy")
        }
        return nil
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
        optionID: String = "",
        attachmentsJSON: String = "",
        quietErrors: Bool = false
    ) async -> ParsedEnvironmentView? {
        guard let work = MobileCorePool.shared.checkout() else {
            if !quietErrors { bannerError = "Internal crypto/codec arena unavailable." }
            return nil
        }
        defer { MobileCorePool.shared.checkin(work) }
        guard let params = MobileCore.encodeEnvironmentRequest(
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
                mode: mode,
                optionID: optionID,
                attachmentsJSON: attachmentsJSON
              ) else {
            if !quietErrors {
                bannerError = work.lastError.errorDescription ?? "Could not build Antigravity request."
            }
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
            ensureTransportActive()
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
            // Enforce identical Coax schema validation on macOS LocalSocketClient responses (#14006).
            if computer.isLocalMac, !MobileCore.validateView(work: work, host: parsed.hostPubkey, method: method, raw: data) {
                if !quietErrors { bannerError = work.lastError.errorDescription ?? "Host response failed schema validation." }
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
        workspace: String = "default",
        branch: String = "",
        base: String = "",
        title: String = "",
        body: String = "",
        number: Int64 = 0,
        page: Int64 = 0,
        cursor: Int64 = 0,
        expectedRevision: String = "",
        command: String = "",
        issuedMs: Int64 = 0,
        expiresMs: Int64 = 0,
        quietErrors: Bool = false
    ) async -> WorkspaceSnapshot? {
        guard let work = MobileCorePool.shared.checkout() else {
            if !quietErrors { workspaceError = "Internal crypto/codec arena unavailable." }
            return nil
        }
        defer { MobileCorePool.shared.checkin(work) }
        guard let params = MobileCore.encodeWorkspaceRequest(
                work: work,
                method: method,
                workspace: workspace,
                branch: branch,
                base: base,
                title: title,
                body: body,
                number: number,
                page: page,
                cursor: cursor,
                expectedRevision: expectedRevision,
                command: command,
                issuedMs: issuedMs,
                expiresMs: expiresMs
              ) else {
            if !quietErrors { workspaceError = work.lastError.errorDescription ?? "Could not encode workspace request." }
            return nil
        }

        let result: Result<Data, TailscaleRPCError>
        if computer.isLocalMac {
            result = await LocalSocketClient.shared.exchangeLine(params)
        } else {
            guard let paired = computer.paired, !paired.isExpired,
                  let endpoint = endpointFor(computer),
                  let secret = store.loadOrCreateDeviceSecret() else {
                if !quietErrors { workspaceError = "Computer is not paired over Tailscale." }
                return nil
            }
            ensureTransportActive()
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
            if !quietErrors { workspaceError = error.localizedDescription }
            return nil
        case .success(let data):
            guard let parsed = WorkspaceSnapshot.parse(data) else {
                if !quietErrors { workspaceError = "Malformed workspace response." }
                return nil
            }
            // Enforce identical Coax schema validation on macOS LocalSocketClient responses (#14006).
            if computer.isLocalMac, !MobileCore.validateWorkspaceView(work: work, host: parsed.host, method: method, raw: data) {
                if !quietErrors { workspaceError = work.lastError.errorDescription ?? "Workspace response failed schema validation." }
                return nil
            }
            return parsed
        }
    }

    func performDaemonRPC(
        computer: ComputerEntry,
        method: String,
        session: String = "",
        title: String = "",
        folder: String = "",
        text: String = "",
        pinned: Bool? = nil,
        archived: Bool? = nil,
        quietErrors: Bool = false
    ) async -> ParsedDaemonView? {
        guard let work = MobileCorePool.shared.checkout() else {
            if !quietErrors { bannerError = "Internal crypto/codec arena unavailable." }
            return nil
        }
        defer { MobileCorePool.shared.checkin(work) }
        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        let writeMethods: Set<String> = ["UpdateSessionMeta", "UpdateForeman", "ToggleAutomation", "RunAutomation", "UpdateAntigravityConfig"]
        let isWrite = writeMethods.contains(method)
        guard let params = MobileCore.encodeDaemonRequest(
            work: work,
            method: method,
            session: session,
            command: isWrite ? MobileCore.randomHex(bytes: 16) : "",
            title: title,
            folder: folder,
            text: text,
            pinned: pinned,
            archived: archived,
            issuedMs: isWrite ? nowMs : 0,
            expiresMs: isWrite ? (nowMs + 120_000) : 0
        ) else {
            if !quietErrors { bannerError = work.lastError.errorDescription ?? "Could not encode daemon request." }
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
            ensureTransportActive()
            result = await client.sendRPC(
                paired: paired,
                endpoint: endpoint,
                secret: secret,
                method: method,
                params: params,
                isWorkspace: false,
                isDaemon: true
            )
        }

        switch result {
        case .failure(let error):
            if !quietErrors { bannerError = error.localizedDescription }
            return nil
        case .success(let data):
            guard let parsed = ParsedDaemonView.parse(data) else {
                if !quietErrors { bannerError = "Malformed daemon response." }
                return nil
            }
            if computer.isLocalMac, !MobileCore.validateDaemonView(work: work, host: parsed.host, method: method, raw: data) {
                if !quietErrors { bannerError = work.lastError.errorDescription ?? "Daemon response failed schema validation." }
                return nil
            }
            if !parsed.error.isEmpty, !quietErrors {
                bannerError = HostErrorFormatter.describe(parsed.error, method: method)
            }
            return parsed
        }
    }

    // MARK: - Wave 4 Control-Plane Methods: Artifacts, Swarms, Automations, Extensibility, Quotas & Offline Outbox (#14014-#14019)

    func refreshArtifacts() async {
        guard let computer = activeComputer, computer.canSendRPC,
              let sid = activeSessionID, !sid.isEmpty else {
            artifacts = []
            return
        }
        isLoadingArtifacts = true
        defer { isLoadingArtifacts = false }
        if let view = await performDaemonRPC(computer: computer, method: "ListArtifacts", session: sid, quietErrors: true), view.ok {
            artifacts = view.sessions.map(ArtifactSummary.fromDaemonMeta)
        }
    }

    func readArtifact(_ name: String) async {
        guard let computer = activeComputer, computer.canSendRPC,
              let sid = activeSessionID, !sid.isEmpty else { return }
        isLoadingArtifacts = true
        defer { isLoadingArtifacts = false }
        if let view = await performDaemonRPC(computer: computer, method: "ReadArtifact", session: sid, title: name, quietErrors: false), view.ok {
            let header = view.sessions.first
            let extraChunks = view.sessions.dropFirst().map(\.folder).joined()
            let fullContent = view.foremanPrompt + extraChunks
            activeArtifact = ArtifactDocument(
                name: header?.id ?? name,
                mimeType: header?.title ?? "text/markdown",
                localPath: header?.folder ?? "",
                isImage: header?.archived ?? false,
                content: fullContent,
                byteCount: header?.updatedAt ?? Int64(fullContent.utf8.count)
            )
        }
    }

    func proceedArtifactPlan(_ name: String) async {
        isHubPresented = false
        await sendMessage(overrideText: "Proceed with the implementation plan in `\(name)`.", overrideAttachments: [])
    }

    func updateArtifactReviewPolicy(_ policy: ArtifactReviewPolicy) async {
        await updateAntigravityConfig(patch: ["artifactReviewPolicy": policy.rawValue])
    }

    func refreshSwarm() async {
        guard let computer = activeComputer, computer.canSendRPC else { return }
        isLoadingSwarm = true
        defer { isLoadingSwarm = false }
        let sid = activeSessionID ?? ""
        if let view = await performDaemonRPC(computer: computer, method: "ReadSwarm", session: sid, quietErrors: true), view.ok {
            var activeCount = 0
            if let data = view.foremanPrompt.data(using: .utf8),
               let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                activeCount = obj["active_subagents"] as? Int ?? 0
            }
            var members: [SwarmMemberEntry] = []
            var posts: [SwarmBoardPost] = []
            for row in view.sessions {
                if row.id.hasPrefix("subagent:") || row.id.hasPrefix("member:") {
                    let parts = row.title.split(separator: "|", maxSplits: 1).map(String.init)
                    let model = parts.first ?? "antigravity"
                    let state = parts.count > 1 ? parts[1] : (row.pinned ? "active" : "stopped")
                    let rawName = String(row.id.drop(while: { $0 != ":" }).dropFirst())
                    members.append(SwarmMemberEntry(
                        id: row.id,
                        name: rawName.isEmpty ? row.id : rawName,
                        model: model,
                        state: state,
                        cwd: row.folder,
                        isActive: row.pinned,
                        startedAt: row.updatedAt
                    ))
                } else if row.id.hasPrefix("post:") {
                    posts.append(SwarmBoardPost(id: row.id, author: row.title, message: row.folder, index: row.updatedAt))
                }
            }
            swarmSnapshot = SwarmSnapshot(sessionID: sid, activeSubagents: activeCount, members: members, posts: posts)
        }
    }

    func sendSwarmMention(to member: String, message: String) async {
        let clean = message.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clean.isEmpty else { return }
        let target = member.hasPrefix("@") ? member : "@\(member)"
        isHubPresented = false
        await sendMessage(overrideText: "\(target) \(clean)", overrideAttachments: [])
    }

    func switchSwarmMemberModel(member: String, model: String) async {
        isHubPresented = false
        await sendMessage(overrideText: "Switch swarm worker `\(member)` to model `\(model)` and resume its work.", overrideAttachments: [])
    }

    func refreshAutomations() async {
        guard let computer = activeComputer, computer.canSendRPC else { return }
        isLoadingAutomations = true
        defer { isLoadingAutomations = false }
        if let view = await performDaemonRPC(computer: computer, method: "ListAutomations", quietErrors: true), view.ok {
            automations = view.sessions.map(AutomationRecord.fromDaemonMeta)
        }
    }

    func toggleAutomation(_ item: AutomationRecord) async {
        guard let computer = activeComputer, computer.canSendRPC else { return }
        if let view = await performDaemonRPC(
            computer: computer,
            method: "ToggleAutomation",
            session: item.id,
            pinned: !item.enabled,
            quietErrors: false
        ), view.ok {
            automations = view.sessions.map(AutomationRecord.fromDaemonMeta)
            statusToast = item.enabled ? "Paused \(item.name)" : "Resumed \(item.name)"
        }
    }

    func runAutomationNow(_ item: AutomationRecord) async {
        guard let computer = activeComputer, computer.canSendRPC else { return }
        if let view = await performDaemonRPC(
            computer: computer,
            method: "RunAutomation",
            session: item.id,
            quietErrors: false
        ), view.ok {
            automations = view.sessions.map(AutomationRecord.fromDaemonMeta)
            statusToast = view.foremanWorkspace.isEmpty ? "Triggered \(item.name)" : "Started run \(view.foremanWorkspace.prefix(8))"
            await refreshActiveComputer()
        }
    }

    func refreshAntigravityConfig() async {
        guard let computer = activeComputer, computer.canSendRPC else { return }
        isLoadingAgyConfig = true
        defer { isLoadingAgyConfig = false }
        if let view = await performDaemonRPC(computer: computer, method: "ReadAntigravityConfig", quietErrors: true), view.ok {
            agyConfig = AntigravityConfigSnapshot.fromDaemonView(view)
        }
    }

    func updateAntigravityConfig(patch: [String: Any]) async {
        guard let computer = activeComputer, computer.canSendRPC,
              let data = try? JSONSerialization.data(withJSONObject: patch),
              let json = String(data: data, encoding: .utf8) else { return }
        if let view = await performDaemonRPC(
            computer: computer,
            method: "UpdateAntigravityConfig",
            text: json,
            quietErrors: false
        ), view.ok {
            agyConfig = AntigravityConfigSnapshot.fromDaemonView(view)
            statusToast = "Saved Antigravity settings"
        }
    }

    func detectRateLimitAndBrowserHandoff(from transcript: [TranscriptItem], error: String) {
        let lowerErr = error.lowercased()
        if lowerErr == "rate_limited" || lowerErr.contains("429") {
            rateLimitBanner = RateLimitBannerState(
                isHardExhaustion: false,
                message: "Temporary rate limit (429) — retrying automatically via official agy runtime.",
                retryUntil: Date().addingTimeInterval(60)
            )
        } else if let limitItem = transcript.last(where: { $0.typedNoticeKind == .limit }) {
            let txt = limitItem.text.lowercased()
            let hard = txt.contains("weekly") || txt.contains("exhausted") || txt.contains("spend cap") || txt.contains("5-hour") || txt.contains("quota")
            rateLimitBanner = RateLimitBannerState(
                isHardExhaustion: hard,
                message: limitItem.text,
                retryUntil: hard ? nil : Date().addingTimeInterval(120)
            )
        } else if error.isEmpty {
            rateLimitBanner = nil
        }

        // Detect browser subagent SSO / MFA / CAPTCHA handoff pauses (#14018)
        if let handoff = transcript.suffix(6).last(where: { row in
            let t = row.text.lowercased()
            return t.contains("captcha") || t.contains("verify you are human") || t.contains("mfa") || t.contains("two-factor") || (t.contains("sign in") && t.contains("browser"))
        }) {
            if browserHandoffWarning != handoff.text {
                browserHandoffWarning = handoff.text
                onTurnNotificationEvent?("handoff", "Browser agent paused for human sign-in or CAPTCHA")
            }
        } else if activeSession?.busy == false {
            browserHandoffWarning = nil
        }
    }

    private func updateLiveTurnStatus(nowBusy: Bool, needsApproval: Bool) {
        if nowBusy || needsApproval || isAwaitingTurnStart {
            let host = activeComputer?.name ?? "Mac"
            let title = activeSession.map { displayTitle(for: $0) } ?? "Antigravity"
            let phase = needsApproval ? "Waiting for approval" : (streamingStatusLabel ?? "Working")
            let started = activeTurnLiveStatus?.startedAt ?? Date()
            activeTurnLiveStatus = ActiveTurnLiveStatus(
                hostName: host,
                sessionTitle: title,
                phaseLabel: phase,
                startedAt: started,
                needsApproval: needsApproval
            )
        } else {
            activeTurnLiveStatus = nil
        }
    }

    // MARK: - Durable Offline Prompt Outbox (#14019)

    func enqueueOfflineOutbox(
        commandID: String,
        hostID: String,
        sessionID: String?,
        text: String,
        model: String,
        effort: String,
        reviewMode: String,
        folder: String
    ) {
        let entry = PendingOutboxMessage(
            id: commandID,
            hostID: hostID,
            sessionID: sessionID,
            text: text,
            model: model,
            effort: effort,
            reviewMode: reviewMode,
            folder: folder,
            createdAt: Int64(Date().timeIntervalSince1970)
        )
        if !pendingOutbox.contains(where: { $0.id == commandID }) {
            pendingOutbox.append(entry)
            store.savePendingOutbox(pendingOutbox)
        }
    }

    func removeOutboxItem(_ id: String) {
        pendingOutbox.removeAll { $0.id == id }
        store.savePendingOutbox(pendingOutbox)
    }

    func flushPendingOutbox(interactive: Bool = false) async {
        guard !pendingOutbox.isEmpty else { return }
        ensureTransportActive(force: interactive)
        let snapshot = pendingOutbox
        var flushed = 0
        for item in snapshot {
            guard let computer = computers.first(where: { $0.id == item.hostID }), computer.canSendRPC else { continue }
            let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
            let method = item.sessionID == nil ? "OpenSession" : "SendMessage"
            let sid = item.sessionID ?? MobileCore.randomHex(bytes: 16)
            let currentRev = sessions.first(where: { $0.id == sid })?.revision ?? 0
            let currentGen = max(1, sessions.first(where: { $0.id == sid })?.generation ?? 1)
            if let view = await performEnvironmentRPC(
                computer: computer,
                method: method,
                session: sid,
                command: item.id,
                generation: currentGen,
                revision: currentRev,
                text: item.text,
                issuedMs: nowMs,
                expiresMs: nowMs + 300_000,
                model: item.model,
                effort: item.effort,
                folder: item.folder,
                mode: item.reviewMode,
                quietErrors: !interactive
            ), view.error.isEmpty {
                pendingOutbox.removeAll { $0.id == item.id }
                store.savePendingOutbox(pendingOutbox)
                flushed += 1
            }
        }
        if flushed > 0 {
            bannerError = nil
            statusToast = "Sent \(flushed) offline message\(flushed == 1 ? "" : "s")"
            await refreshActiveComputer()
        }
    }
}
