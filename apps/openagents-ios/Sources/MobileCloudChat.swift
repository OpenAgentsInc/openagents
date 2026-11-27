// Firm reason: 6 (iOS OpenAgents Cloud Chat engine: /api/chat/v2/* CRUD & dual SSE sync, streaming /api/v1/inference with Tier 1 /api/tools/cloud/v1 loop & thought_signature preservation, durable /api/chat/submit/v2alpha1, /api/chat/placement/v1, /api/chat/environments/v1, /api/projects/v1, and /api/v1/inference/usage).
import CryptoKit
import Foundation

struct MobileCloudConversation: Identifiable, Equatable {
    let id: String // cnv_<32 hex>
    var title: String
    var model: String
    var workspace: String
    var revision: Int64
    var updatedAtMs: Int64
    var pinned: Bool
    var archived: Bool
    var activeTurnID: String
    var compute: String
    var sourceDevice: String
    var turns: [MobileCloudTurn]
    var items: [CloudTranscriptItem]
}

struct MobileCloudTurn: Equatable {
    let id: String
    let userText: String
    var output: String
    var complete: Bool
}

struct StreamedToolCallAccumulator: Equatable {
    var index: Int
    var id: String = ""
    var name: String = ""
    var arguments: String = ""
    var thoughtSignature: String = ""
}

@MainActor
final class MobileCloudChat: ObservableObject {
    static let shared = MobileCloudChat(auth: MobileAccountController.shared.auth)

    // Event kind & state constants matching coax/web/api/account-chat-cloud-wire.coax.
    private static let evUserMessage: Int64 = 2
    private static let evToolCall: Int64 = 5
    private static let evToolResult: Int64 = 7
    private static let evCompleted: Int64 = 10
    private static let evFailed: Int64 = 11
    private static let evCancelled: Int64 = 12

    private static let stateRunning: Int64 = 2
    private static let stateCompleted: Int64 = 3
    private static let stateFailed: Int64 = 4
    private static let stateCancelled: Int64 = 5

    private static let modelKey = "oa.cloud.selectedModelID"
    private static let effortKey = "oa.cloud.selectedEffort"
    private static let targetKey = "oa.cloud.selectedTarget"
    private static let projectKey = "oa.cloud.selectedProjectID"
    private static let fullAutoKey = "oa.cloud.fullAutoEnabled"
    private static let autoAnswerKey = "oa.cloud.autoAnswerQuestions"

    let auth: AppAuth
    private let session: URLSession
    private var accountSSETask: Task<Void, Never>?
    private var convSSETask: Task<Void, Never>?
    private var activeSendTask: Task<Void, Never>?
    private var accountSSECursor: Int64 = 0
    private var convSSECursor: Int64 = 0
    private var convSSEConversationID: String?
    private var opCounter: Int64 = 0

    @Published var conversations: [MobileCloudConversation] = []
    @Published var activeConversationID: String?
    @Published var activeItems: [CloudTranscriptItem] = []
    @Published var environments: [CloudEnvironment] = []
    @Published var projects: [CloudProject] = []
    @Published var usageSnapshot = CloudUsageSnapshot()
    @Published var isLoadingUsage: Bool = false

    @Published var selectedModelID: String {
        didSet { UserDefaults.standard.set(selectedModelID, forKey: Self.modelKey) }
    }
    @Published var selectedEffort: CloudThinkingEffort {
        didSet { UserDefaults.standard.set(selectedEffort.rawValue, forKey: Self.effortKey) }
    }
    @Published var selectedTarget: CloudExecutionTarget {
        didSet { UserDefaults.standard.set(selectedTarget.persistedString, forKey: Self.targetKey) }
    }
    @Published var selectedProjectID: String? {
        didSet { UserDefaults.standard.set(selectedProjectID, forKey: Self.projectKey) }
    }
    @Published var fullAutoEnabled: Bool {
        didSet { UserDefaults.standard.set(fullAutoEnabled, forKey: Self.fullAutoKey) }
    }
    @Published var autoAnswerQuestions: Bool {
        didSet { UserDefaults.standard.set(autoAnswerQuestions, forKey: Self.autoAnswerKey) }
    }

    @Published var draftText: String = ""
    @Published var pendingAttachments: [CloudAttachment] = []
    @Published var isStreamConnected: Bool = false
    @Published var isSending: Bool = false
    @Published var activeTurnID: String?
    @Published var activeTurnIsDurable: Bool = false
    @Published var activeTurnStartedAt: Date?
    @Published var activePhaseLabel: String = "Working"
    @Published var lastPlacementNote: String?
    @Published var lastError: String?
    @Published var statusToast: String?
    @Published var composerFocusNonce: UInt64 = 0

    var voiceSpeechRate: Float = 0.52
    var voiceVadSilenceMs: Int = 1200

    var onConversationsUpdated: (() -> Void)?
    var onAssistantStreamUpdate: ((String, String, Bool) -> Void)?
    var onTurnNotificationEvent: ((String, String) -> Void)?

    init(auth: AppAuth? = nil, session: URLSession = .shared) {
        self.auth = auth ?? MobileAccountController.shared.auth
        self.session = session
        let savedModel = UserDefaults.standard.string(forKey: Self.modelKey) ?? CloudModelOption.catalog[0].id
        self.selectedModelID = CloudModelOption.catalog.contains(where: { $0.id == savedModel }) ? savedModel : CloudModelOption.catalog[0].id
        let savedEff = UserDefaults.standard.string(forKey: Self.effortKey).flatMap(CloudThinkingEffort.init(rawValue:)) ?? .medium
        self.selectedEffort = savedEff
        self.selectedTarget = CloudExecutionTarget.fromPersisted(UserDefaults.standard.string(forKey: Self.targetKey))
        self.selectedProjectID = UserDefaults.standard.string(forKey: Self.projectKey)
        if UserDefaults.standard.object(forKey: Self.fullAutoKey) == nil {
            self.fullAutoEnabled = true
        } else {
            self.fullAutoEnabled = UserDefaults.standard.bool(forKey: Self.fullAutoKey)
        }
        if UserDefaults.standard.object(forKey: Self.autoAnswerKey) == nil {
            self.autoAnswerQuestions = true
        } else {
            self.autoAnswerQuestions = UserDefaults.standard.bool(forKey: Self.autoAnswerKey)
        }
    }

    var isSignedIn: Bool { auth.loggedIn }

    var selectedModelOption: CloudModelOption {
        CloudModelOption.option(for: selectedModelID)
    }

    var selectedProject: CloudProject? {
        guard let id = selectedProjectID else { return nil }
        return projects.first { $0.id == id }
    }

    var activeConversation: MobileCloudConversation? {
        guard let id = activeConversationID else { return nil }
        return conversations.first { $0.id == id }
    }

    var matchingSlashCommands: [CloudSlashCommand] {
        let trimmed = draftText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix("/"), !trimmed.contains(" ") else { return [] }
        let q = trimmed.lowercased()
        return CloudSlashCommand.all.filter { $0.command.lowercased().hasPrefix(q) }
    }

    var usageStripSummary: String {
        let spend = CloudUsageSnapshot.formatUsd(micros: usageSnapshot.todayUsdMicros)
        let target = selectedTarget.shortChipLabel
        if usageSnapshot.creditMsat > 0 {
            let sats = usageSnapshot.creditMsat / 1000
            return "\(target) · Today \(spend) · \(sats) sats"
        }
        return "\(target) · Today \(spend)"
    }

    static func newConversationID() -> String {
        var bytes = [UInt8](repeating: 0, count: 16)
        _ = SecRandomCopyBytes(kSecRandomDefault, 16, &bytes)
        return "cnv_" + bytes.map { String(format: "%02x", $0) }.joined()
    }

    static func newTurnID() -> String {
        var bytes = [UInt8](repeating: 0, count: 16)
        _ = SecRandomCopyBytes(kSecRandomDefault, 16, &bytes)
        return "trn_" + bytes.map { String(format: "%02x", $0) }.joined()
    }

    static func turnID(conversationID: String, ordinal: Int) -> String {
        let hash = SHA256.hash(data: Data(("openagents.ios.turn:\(conversationID):\(ordinal)").utf8))
        return "trn_" + hash.prefix(16).map { String(format: "%02x", $0) }.joined()
    }

    private func nextOpID(prefix: String) -> String {
        opCounter += 1
        let now = Int64(Date().timeIntervalSince1970 * 1000)
        return "ios:\(prefix):\(now):\(opCounter)"
    }

    // MARK: - Lifecycle

    func start() {
        guard auth.loggedIn else {
            stop()
            return
        }
        Task {
            await refreshConversations()
            await refreshEnvironmentsAndProjects()
            await refreshUsage()
        }
        if accountSSETask == nil {
            accountSSETask = Task { [weak self] in
                await self?.runAccountSSELoop()
            }
        }
        if let cid = activeConversationID {
            ensureConversationSSE(for: cid)
        }
    }

    func stop() {
        accountSSETask?.cancel()
        accountSSETask = nil
        convSSETask?.cancel()
        convSSETask = nil
        convSSEConversationID = nil
        activeSendTask?.cancel()
        activeSendTask = nil
        isStreamConnected = false
        isSending = false
        activeTurnID = nil
        activeTurnIsDurable = false
        activeTurnStartedAt = nil
    }

    func clearOnSignOut() {
        stop()
        conversations = []
        activeConversationID = nil
        activeItems = []
        environments = []
        projects = []
        usageSnapshot = CloudUsageSnapshot()
        lastError = nil
        lastPlacementNote = nil
        onConversationsUpdated?()
    }

    // MARK: - User Actions & Selection

    func startNewChat() {
        convSSETask?.cancel()
        convSSETask = nil
        convSSEConversationID = nil
        activeConversationID = nil
        activeItems = []
        lastError = nil
        composerFocusNonce &+= 1
        onConversationsUpdated?()
    }

    func selectConversation(_ id: String) {
        let cleanID = id.hasPrefix("cloud:") ? String(id.dropFirst("cloud:".count)) : id
        activeConversationID = cleanID
        if let existing = conversations.first(where: { $0.id == cleanID }) {
            activeItems = existing.items.isEmpty ? Self.itemsFromTurns(existing.turns, updatedAtMs: existing.updatedAtMs) : existing.items
        } else {
            activeItems = []
        }
        ensureConversationSSE(for: cleanID)
        Task { await pullConversation(cleanID) }
        onConversationsUpdated?()
    }

    func chooseModel(_ modelID: String) {
        guard CloudModelOption.catalog.contains(where: { $0.id == modelID }) else { return }
        selectedModelID = modelID
        onConversationsUpdated?()
    }

    func chooseEffort(_ effort: CloudThinkingEffort) {
        selectedEffort = effort
        onConversationsUpdated?()
    }

    func selectTarget(_ target: CloudExecutionTarget) {
        selectedTarget = target
        showToast("Target: \(target.displayLabel)")
        onConversationsUpdated?()
    }

    func selectTargetByMachineID(_ id: String) {
        if id == CloudExecutionTarget.autoMachineID {
            selectTarget(.auto)
        } else if id == CloudExecutionTarget.cloudMachineID {
            selectTarget(.cloud)
        } else if id == CloudExecutionTarget.sandboxMachineID {
            selectTarget(.sandbox(size: "small"))
        } else if id.hasPrefix("host:") {
            let host = String(id.dropFirst("host:".count))
            let name = environments.first(where: { $0.hostPubkey == host })?.displayName ?? String(host.prefix(8))
            selectTarget(.computer(hostPubkey: host, name: name))
        }
    }

    func selectProject(_ id: String?) {
        selectedProjectID = id
        if let proj = selectedProject {
            showToast("Project: \(proj.name)")
        } else {
            showToast("General Cloud Chat")
        }
        onConversationsUpdated?()
    }

    func toggleFullAuto() {
        fullAutoEnabled.toggle()
        onConversationsUpdated?()
    }

    func setFullAuto(_ enabled: Bool) {
        fullAutoEnabled = enabled
        onConversationsUpdated?()
    }

    func setAutoAnswerQuestions(_ enabled: Bool) {
        autoAnswerQuestions = enabled
        onConversationsUpdated?()
    }

    func addAttachment(_ att: CloudAttachment) {
        pendingAttachments.append(att)
        onConversationsUpdated?()
    }

    func applySlashCommand(_ cmd: CloudSlashCommand) {
        switch cmd.command {
        case "/new":
            draftText = ""
            startNewChat()
        case "/auto":
            draftText = ""
            selectTarget(.auto)
        case "/cloud":
            draftText = ""
            selectTarget(.cloud)
        case "/sandbox":
            draftText = ""
            selectTarget(.sandbox(size: "small"))
        default:
            draftText = cmd.replacement + " "
        }
        onConversationsUpdated?()
    }

    func handleIncomingURL(_ url: URL) {
        guard url.scheme?.lowercased() == "openagents" else { return }
        if url.host?.lowercased() == "ask",
           let comps = URLComponents(url: url, resolvingAgainstBaseURL: false),
           let prompt = comps.queryItems?.first(where: { $0.name == "prompt" })?.value,
           !prompt.isEmpty {
            draftText = prompt
            composerFocusNonce &+= 1
            onConversationsUpdated?()
        }
    }

    private func showToast(_ msg: String) {
        statusToast = msg
        Task { [weak self] in
            try? await Task.sleep(nanoseconds: 2_200_000_000)
            if self?.statusToast == msg {
                self?.statusToast = nil
                self?.onConversationsUpdated?()
            }
        }
    }

    // MARK: - Conversations CRUD (/api/chat/v2/*)

    func refreshConversations() async {
        guard auth.loggedIn,
              let url = URL(string: auth.origin + "/api/chat/v2/conversations") else { return }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20)
        req.httpMethod = "GET"
        do {
            let authed = try await auth.authorize(req)
            let (data, resp) = try await session.data(for: authed)
            guard (resp as? HTTPURLResponse)?.statusCode == 200,
                  let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let list = root["conversations"] as? [[String: Any]] else { return }
            let existingByID = Dictionary(uniqueKeysWithValues: conversations.map { ($0.id, $0) })
            var next: [MobileCloudConversation] = []
            for row in list {
                guard let id = row["id"] as? String, !id.isEmpty else { continue }
                let prev = existingByID[id]
                let title = (row["title"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? prev?.title ?? "Cloud chat"
                let model = (row["model"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? prev?.model ?? selectedModelID
                let workspace = (row["workspace"] as? String) ?? prev?.workspace ?? ""
                let rev = (row["revision"] as? NSNumber)?.int64Value ?? prev?.revision ?? 1
                let updatedMs = (row["updated_at_ms"] as? NSNumber)?.int64Value ?? prev?.updatedAtMs ?? Int64(Date().timeIntervalSince1970 * 1000)
                let pinned = (row["pinned"] as? Bool) ?? prev?.pinned ?? false
                let archived = (row["archived"] as? Bool) ?? prev?.archived ?? false
                let activeTurn = (row["active_turn_id"] as? String) ?? ""
                let compute = (row["compute"] as? String) ?? prev?.compute ?? "cloud"
                let device = (row["source_device"] as? String) ?? prev?.sourceDevice ?? "cloud"
                next.append(MobileCloudConversation(
                    id: id,
                    title: title,
                    model: model,
                    workspace: workspace,
                    revision: rev,
                    updatedAtMs: updatedMs,
                    pinned: pinned,
                    archived: archived,
                    activeTurnID: activeTurn,
                    compute: compute,
                    sourceDevice: device,
                    turns: prev?.turns ?? [],
                    items: prev?.items ?? []
                ))
            }
            conversations = next.sorted {
                if $0.pinned != $1.pinned { return $0.pinned }
                return $0.updatedAtMs > $1.updatedAtMs
            }
            onConversationsUpdated?()
        } catch {
            // Keep cached conversations on transient network failures.
        }
    }

    @discardableResult
    func pullConversation(_ convID: String) async -> MobileCloudConversation? {
        guard auth.loggedIn,
              let url = URL(string: auth.origin + "/api/chat/v2/conversations/\(convID)") else { return nil }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20)
        req.httpMethod = "GET"
        do {
            let authed = try await auth.authorize(req)
            let (data, resp) = try await session.data(for: authed)
            guard (resp as? HTTPURLResponse)?.statusCode == 200,
                  let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let conv = root["conversation"] as? [String: Any] else { return nil }
            let title = (conv["title"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "Cloud chat"
            let model = (conv["model"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? selectedModelID
            let workspace = (conv["workspace"] as? String) ?? ""
            let rev = (conv["revision"] as? NSNumber)?.int64Value ?? 1
            let pinned = (conv["pinned"] as? Bool) ?? false
            let archived = (conv["archived"] as? Bool) ?? false
            let activeTurn = (conv["active_turn_id"] as? String) ?? ""
            let compute = (conv["compute"] as? String) ?? "cloud"
            let device = (conv["source_device"] as? String) ?? "cloud"
            let updatedMs = (conv["updated_at_ms"] as? NSNumber)?.int64Value ?? Int64(Date().timeIntervalSince1970 * 1000)
            let rawTurns = (root["turns"] as? [[String: Any]]) ?? []
            let rawEvents = (root["events"] as? [[String: Any]]) ?? []

            let turns: [MobileCloudTurn] = rawTurns.enumerated().map { idx, t in
                let st = (t["state"] as? String) ?? "completed"
                return MobileCloudTurn(
                    id: (t["id"] as? String) ?? Self.turnID(conversationID: convID, ordinal: idx + 1),
                    userText: (t["user_text"] as? String) ?? "",
                    output: (t["output"] as? String) ?? "",
                    complete: st != "queued" && st != "running"
                )
            }
            let items = Self.buildTranscriptItems(turns: turns, events: rawEvents, updatedAtMs: updatedMs)
            let updated = MobileCloudConversation(
                id: convID,
                title: title,
                model: model,
                workspace: workspace,
                revision: rev,
                updatedAtMs: updatedMs,
                pinned: pinned,
                archived: archived,
                activeTurnID: activeTurn,
                compute: compute,
                sourceDevice: device,
                turns: turns,
                items: items
            )
            upsertConversation(updated)
            if activeConversationID == convID && !(isSending && !activeTurnIsDurable) {
                activeItems = items
                if !activeTurn.isEmpty {
                    activeTurnID = activeTurn
                    activeTurnIsDurable = true
                    isSending = true
                } else if activeTurnIsDurable {
                    isSending = false
                    activeTurnID = nil
                    activeTurnIsDurable = false
                    activeTurnStartedAt = nil
                }
            }
            onConversationsUpdated?()
            return updated
        } catch {
            return nil
        }
    }

    func archiveConversation(_ convID: String) async {
        await updateConversationMeta(convID, title: nil, pinned: nil, archived: true)
        conversations.removeAll { $0.id == convID }
        if activeConversationID == convID {
            startNewChat()
        }
        onConversationsUpdated?()
    }

    func renameConversation(_ convID: String, title: String) async {
        let trimmed = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        if let idx = conversations.firstIndex(where: { $0.id == convID }) {
            conversations[idx].title = trimmed
        }
        await updateConversationMeta(convID, title: trimmed, pinned: nil, archived: nil)
        onConversationsUpdated?()
    }

    func togglePinConversation(_ convID: String) async {
        guard let idx = conversations.firstIndex(where: { $0.id == convID }) else { return }
        let nextPin = !conversations[idx].pinned
        conversations[idx].pinned = nextPin
        await updateConversationMeta(convID, title: nil, pinned: nextPin, archived: nil)
        onConversationsUpdated?()
    }

    private func updateConversationMeta(_ convID: String, title: String?, pinned: Bool?, archived: Bool?) async {
        guard auth.loggedIn,
              let url = URL(string: auth.origin + "/api/chat/v2/conversations/\(convID)/meta"),
              let body = CloudCore.encodeMeta(operation: nextOpID(prefix: "meta"), title: title, pinned: pinned, archived: archived) else {
            return
        }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = body
        if let authed = try? await auth.authorize(req) {
            _ = try? await session.data(for: authed)
        }
    }

    // MARK: - Environments, Projects, Usage & Placement

    func refreshEnvironmentsAndProjects() async {
        async let envsTask: Void = refreshEnvironments()
        async let projTask: Void = refreshProjects()
        _ = await (envsTask, projTask)
    }

    func refreshEnvironments() async {
        guard auth.loggedIn,
              let url = URL(string: auth.origin + "/api/chat/environments/v1?detail=1") else { return }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 15)
        req.httpMethod = "GET"
        do {
            let authed = try await auth.authorize(req)
            let (data, resp) = try await session.data(for: authed)
            guard (resp as? HTTPURLResponse)?.statusCode == 200,
                  let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let list = root["environments"] as? [[String: Any]] else { return }
            environments = Self.parseEnvironments(list)
            onConversationsUpdated?()
        } catch {
            // Keep prior enrolled computers snapshot on transient error.
        }
    }

    static func parseEnvironments(_ list: [[String: Any]]) -> [CloudEnvironment] {
        list.compactMap { row in
            guard let host = row["host_pubkey"] as? String, host.count == 64 else { return nil }
            let name = (row["display_name"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? String(host.prefix(8))
            let os = (row["os"] as? String) ?? ""
            let online = (row["online"] as? Bool) ?? false
            let seen = (row["last_seen_ms"] as? NSNumber)?.int64Value ?? 0
            let rawWs = (row["workspaces"] as? [[String: Any]]) ?? []
            let workspaces: [CloudWorkspace] = rawWs.compactMap { w in
                guard let wid = w["workspace_id"] as? String else { return nil }
                return CloudWorkspace(
                    workspaceID: wid,
                    name: (w["name"] as? String) ?? wid,
                    localPath: (w["local_path"] as? String) ?? "",
                    repoRemote: (w["repo_remote"] as? String) ?? "",
                    repoCanonical: (w["repo_canonical"] as? String) ?? "",
                    branch: (w["branch"] as? String) ?? ""
                )
            }
            return CloudEnvironment(
                hostPubkey: host,
                displayName: name,
                os: os,
                online: online,
                lastSeenMs: seen,
                workspaces: workspaces
            )
        }
    }

    func refreshProjects() async {
        guard auth.loggedIn,
              let url = URL(string: auth.origin + "/api/projects/v1") else { return }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 15)
        req.httpMethod = "GET"
        do {
            let authed = try await auth.authorize(req)
            let (data, resp) = try await session.data(for: authed)
            guard (resp as? HTTPURLResponse)?.statusCode == 200,
                  let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let rows = root["data"] as? [[String: Any]] else { return }
            projects = Self.parseProjects(rows)
            if let sel = selectedProjectID, !projects.contains(where: { $0.id == sel }) {
                selectedProjectID = nil
            }
            onConversationsUpdated?()
        } catch {
            // Keep cached projects on transient error.
        }
    }

    static func parseProjects(_ rows: [[String: Any]]) -> [CloudProject] {
        rows.compactMap { row in
            guard let id = row["id"] as? String, !id.isEmpty,
                  let name = row["name"] as? String, !name.isEmpty else { return nil }
            return CloudProject(
                id: id,
                revision: (row["revision"] as? NSNumber)?.int64Value ?? 1,
                name: name,
                description: (row["description"] as? String) ?? "",
                repoCanonical: (row["repo_canonical"] as? String) ?? "",
                updatedAtMs: (row["updated_at_ms"] as? NSNumber)?.int64Value ?? 0
            )
        }
    }

    func refreshUsage() async {
        guard auth.loggedIn,
              let url = URL(string: auth.origin + "/api/v1/inference/usage") else { return }
        isLoadingUsage = true
        defer { isLoadingUsage = false }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 15)
        req.httpMethod = "GET"
        do {
            let authed = try await auth.authorize(req)
            let (data, resp) = try await session.data(for: authed)
            guard (resp as? HTTPURLResponse)?.statusCode == 200,
                  let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
            usageSnapshot = Self.parseUsageSnapshot(root)
            onConversationsUpdated?()
        } catch {
            // Keep prior usage snapshot.
        }
    }

    static func parseUsageSnapshot(_ root: [String: Any]) -> CloudUsageSnapshot {
        var snap = CloudUsageSnapshot()
        if let today = root["today"] as? [String: Any] {
            snap.todayUsdMicros = (today["usd_micros"] as? NSNumber)?.int64Value ?? 0
            snap.todayTokens = (today["tokens"] as? NSNumber)?.int64Value ?? 0
            snap.todayRequests = (today["requests"] as? NSNumber)?.int64Value ?? 0
        }
        if let month = root["month"] as? [String: Any] {
            snap.monthUsdMicros = (month["usd_micros"] as? NSNumber)?.int64Value ?? 0
        }
        snap.creditMsat = (root["credit_msat"] as? NSNumber)?.int64Value ?? 0
        if let sb = root["sandbox"] as? [String: Any] {
            snap.sandboxTurns = (sb["turns"] as? NSNumber)?.int64Value ?? 0
            snap.sandboxSeconds = (sb["seconds"] as? NSNumber)?.int64Value ?? 0
            snap.sandboxMsat = (sb["msat"] as? NSNumber)?.int64Value ?? 0
        }
        snap.localTurns = (root["local_turns"] as? NSNumber)?.int64Value ?? 0
        return snap
    }

    func resolvePlacement(for prompt: String) async -> CloudPlacementDecision? {
        guard auth.loggedIn,
              let url = URL(string: auth.origin + "/api/chat/placement/v1") else { return nil }
        let payload: [String: Any] = [
            "prompt": String(prompt.prefix(480)),
            "project_id": selectedProjectID ?? "",
            "size": "small",
        ]
        guard let body = try? JSONSerialization.data(withJSONObject: payload) else { return nil }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 10)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = body
        do {
            let authed = try await auth.authorize(req)
            let (data, resp) = try await session.data(for: authed)
            guard (resp as? HTTPURLResponse)?.statusCode == 200,
                  let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
            return CloudPlacementDecision(
                tier: (obj["tier"] as? NSNumber)?.intValue ?? 1,
                compute: (obj["compute"] as? String) ?? "cloud",
                hostPubkey: (obj["host_pubkey"] as? String) ?? "",
                hostName: (obj["host_name"] as? String) ?? "Cloud",
                reason: (obj["reason"] as? String) ?? "",
                sandboxRateUsdMicrosHr: (obj["sandbox_rate_usd_micros_hr"] as? NSNumber)?.int64Value ?? 0,
                creditMsat: (obj["credit_msat"] as? NSNumber)?.int64Value ?? 0
            )
        } catch {
            return nil
        }
    }

    // MARK: - Sending Messages (Tier 1 Streaming + Tool Loop, Tier 2/3 Durable Turns)

    func sendMessage(overrideText: String? = nil) {
        let rawText = overrideText ?? draftText
        let attachments = overrideText == nil ? pendingAttachments : []
        var prompt = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        if !attachments.isEmpty {
            let notes = attachments.map { $0.promptNote() }.joined()
            prompt = prompt.isEmpty ? "Please analyze the attached file(s)." + notes : prompt + notes
        }
        guard !prompt.isEmpty else { return }

        // Handle local slash commands cleanly before dispatching to network.
        if let cmd = CloudSlashCommand.all.first(where: { $0.command == prompt }) {
            applySlashCommand(cmd)
            return
        }

        guard auth.loggedIn else {
            lastError = "Sign in to your OpenAgents account to start chatting."
            onConversationsUpdated?()
            return
        }
        guard !isSending else { return }

        if overrideText == nil {
            draftText = ""
            pendingAttachments = []
        }
        lastError = nil

        activeSendTask?.cancel()
        activeSendTask = Task { [weak self] in
            await self?.executeTurn(prompt: prompt)
        }
    }

    func stopActiveTurn() async {
        if activeTurnIsDurable, let tid = activeTurnID {
            await cancelDurableTurn(tid)
        } else {
            activeSendTask?.cancel()
            activeSendTask = nil
            if isSending, let cid = activeConversationID, let tid = activeTurnID {
                let partial = activeItems.last(where: { $0.role == .assistant })?.text ?? ""
                finalizeAssistantItem(complete: true, failed: false)
                await syncTurnToCloudV2(
                    conversationID: cid,
                    turnID: tid,
                    title: activeConversation?.title ?? "",
                    model: selectedModelID,
                    userText: "",
                    output: partial,
                    state: Self.stateCancelled,
                    stopReason: "user_stop",
                    errorText: "",
                    eventKind: Self.evCancelled,
                    sequence: 99,
                    existingConversation: true
                )
            }
        }
        isSending = false
        activeTurnID = nil
        activeTurnIsDurable = false
        activeTurnStartedAt = nil
        onConversationsUpdated?()
    }

    private func executeTurn(prompt: String) async {
        isSending = true
        activeTurnStartedAt = Date()
        activePhaseLabel = "Routing…"
        onConversationsUpdated?()

        // Resolve execution target (explicit Computer / Sandbox / Cloud or Auto via /api/chat/placement/v1).
        var effectiveTarget = selectedTarget
        if effectiveTarget == .auto {
            if let placement = await resolvePlacement(for: prompt) {
                lastPlacementNote = placement.reason
                if placement.tier == 2 && placement.hostPubkey.count == 64 {
                    effectiveTarget = .computer(hostPubkey: placement.hostPubkey, name: placement.hostName)
                } else if placement.tier == 3 {
                    effectiveTarget = .sandbox(size: "small")
                } else {
                    effectiveTarget = .cloud
                }
            } else {
                // Local #authority fallback when placement probe is unavailable.
                if CloudCore.classifyPrompt(prompt) == CloudCore.workCodeExec,
                   let onlineEnv = environments.first(where: { $0.online }) {
                    effectiveTarget = .computer(hostPubkey: onlineEnv.hostPubkey, name: onlineEnv.displayName)
                } else {
                    effectiveTarget = .cloud
                }
            }
        }

        switch effectiveTarget {
        case .computer(let host, let name):
            activePhaseLabel = "Running on \(name)…"
            await executeDurableTurn(prompt: prompt, hostPubkey: host, sandboxSize: nil, targetLabel: name)
        case .sandbox(let size):
            activePhaseLabel = "Starting Cloud Sandbox…"
            await executeDurableTurn(prompt: prompt, hostPubkey: nil, sandboxSize: size, targetLabel: "Cloud Sandbox")
        case .cloud, .auto:
            activePhaseLabel = "Streaming from Cloud…"
            await executeStreamingCloudTurn(prompt: prompt)
        }
    }

    // MARK: - Tier 1 Streaming Cloud Turn (/api/v1/inference + /api/tools/cloud/v1 + thought_signature)

    private func buildSystemPrompt() -> String {
        var parts = [
            "You are OpenAgents, a concise, capable AI coding and research assistant with live Tier 1 cloud tools: web_search, web_fetch, github_tree, github_read_file, github_search_code, github_list_issues, and github_get_pr. Use these tools proactively whenever the user asks about live web info, URLs, or GitHub repositories. Use Markdown and fenced code blocks with language names."
        ]
        if let proj = selectedProject {
            var p = "Active project: \(proj.name)."
            if !proj.repoCanonical.isEmpty {
                p += " GitHub repo: \(proj.repoCanonical)."
            }
            parts.append(p)
        }
        if !environments.isEmpty {
            let names = environments.map { "\($0.displayName) (\($0.online ? "online" : "offline"))" }.joined(separator: ", ")
            parts.append("Enrolled user computers available in Environments: \(names).")
        }
        return parts.joined(separator: "\n")
    }

    private func executeStreamingCloudTurn(prompt: String) async {
        let model = selectedModelID
        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        let isFresh = activeConversationID == nil
        let convID = activeConversationID ?? Self.newConversationID()
        activeConversationID = convID
        let tid = Self.newTurnID()
        activeTurnID = tid
        activeTurnIsDurable = false

        var conv: MobileCloudConversation
        if let existing = conversations.first(where: { $0.id == convID }) {
            conv = existing
        } else {
            conv = MobileCloudConversation(
                id: convID,
                title: String(prompt.prefix(64)),
                model: model,
                workspace: selectedProject?.name ?? "",
                revision: 1,
                updatedAtMs: nowMs,
                pinned: false,
                archived: false,
                activeTurnID: tid,
                compute: "cloud",
                sourceDevice: "ios-mobile",
                turns: [],
                items: []
            )
        }

        // Build OpenAI-compatible conversation history before appending the new streaming assistant bubble.
        var wireMessages: [[String: Any]] = [
            ["role": "system", "content": buildSystemPrompt()]
        ]
        for turn in conv.turns {
            if !turn.userText.isEmpty {
                wireMessages.append(["role": "user", "content": turn.userText])
            }
            if !turn.output.isEmpty {
                wireMessages.append(["role": "assistant", "content": turn.output])
            }
        }
        wireMessages.append(["role": "user", "content": prompt])

        let userItem = CloudTranscriptItem(id: "\(tid)-u", role: .user, text: prompt, complete: true, startedAtMs: nowMs)
        let assistantItemID = "\(tid)-a"
        let assistantItem = CloudTranscriptItem(id: assistantItemID, role: .assistant, text: "", complete: false, startedAtMs: nowMs)
        activeItems.append(userItem)
        activeItems.append(assistantItem)

        conv.turns.append(MobileCloudTurn(id: tid, userText: prompt, output: "", complete: false))
        conv.items = activeItems
        conv.activeTurnID = tid
        conv.updatedAtMs = nowMs
        upsertConversation(conv)
        onConversationsUpdated?()

        // Persist user message immediately so Desktop & Web see the active turn right away.
        await syncTurnToCloudV2(
            conversationID: convID,
            turnID: tid,
            title: conv.title,
            model: model,
            userText: prompt,
            output: "",
            state: Self.stateRunning,
            stopReason: "",
            errorText: "",
            eventKind: Self.evUserMessage,
            sequence: 1,
            existingConversation: !isFresh
        )
        ensureConversationSSE(for: convID)

        let toolsJSON = CloudCore.webToolsObjects()
        var seq: Int64 = 2
        var finalOutput = ""
        var turnSucceeded = true
        let maxToolRounds = 6

        for round in 0...maxToolRounds {
            if Task.isCancelled {
                turnSucceeded = false
                break
            }
            let allowTools = round < maxToolRounds && !toolsJSON.isEmpty
            let step = await runStreamingInferenceStep(
                model: model,
                messages: wireMessages,
                tools: allowTools ? toolsJSON : nil,
                assistantItemID: round == 0 ? assistantItemID : "\(tid)-a-\(round)",
                turnID: tid
            )

            if Task.isCancelled {
                turnSucceeded = false
                break
            }

            if let err = step.error {
                lastError = err
                turnSucceeded = false
                if finalOutput.isEmpty {
                    finalOutput = err
                    setAssistantText(itemID: round == 0 ? assistantItemID : "\(tid)-a-\(round)", text: err, complete: true, failed: true)
                }
                break
            }

            if !step.text.isEmpty {
                finalOutput = finalOutput.isEmpty ? step.text : (finalOutput + "\n\n" + step.text)
            }

            if step.toolCalls.isEmpty || !allowTools {
                break
            }

            // Record assistant message WITH tool_calls & exact preserved thought_signature for Gemini/Vertex!
            wireMessages.append(Self.encodeAssistantToolCallMessage(content: step.text, toolCalls: step.toolCalls))

            for call in step.toolCalls {
                if Task.isCancelled { break }
                activePhaseLabel = "Running \(call.name)…"
                let toolItemID = "\(tid)-tool-\(call.id)"
                activeItems.append(CloudTranscriptItem(
                    id: toolItemID,
                    role: .tool,
                    text: call.arguments,
                    toolName: call.name,
                    complete: false,
                    failed: false,
                    startedAtMs: Int64(Date().timeIntervalSince1970 * 1000)
                ))
                onConversationsUpdated?()

                seq += 1
                await syncToolEvent(
                    conversationID: convID, turnID: tid, sequence: seq,
                    kind: Self.evToolCall, toolID: call.id, toolName: call.name,
                    toolArgs: call.arguments, text: "", failed: false, model: model
                )

                let (toolOK, toolOut) = await executeCloudTool(name: call.name, argumentsJSON: call.arguments)
                if let idx = activeItems.firstIndex(where: { $0.id == toolItemID }) {
                    activeItems[idx].text = toolOut
                    activeItems[idx].complete = true
                    activeItems[idx].failed = !toolOK
                }
                onConversationsUpdated?()

                seq += 1
                await syncToolEvent(
                    conversationID: convID, turnID: tid, sequence: seq,
                    kind: Self.evToolResult, toolID: call.id, toolName: call.name,
                    toolArgs: "", text: String(toolOut.prefix(12_000)), failed: !toolOK, model: model
                )

                wireMessages.append([
                    "role": "tool",
                    "tool_call_id": call.id,
                    "content": String(toolOut.prefix(12_000)),
                ])
            }

            // Prepare continuation streaming assistant item for next round.
            activePhaseLabel = "Streaming reply…"
            activeItems.append(CloudTranscriptItem(
                id: "\(tid)-a-\(round + 1)",
                role: .assistant,
                text: "",
                complete: false,
                startedAtMs: Int64(Date().timeIntervalSince1970 * 1000)
            ))
            onConversationsUpdated?()
        }

        finalizeAssistantItem(complete: true, failed: !turnSucceeded)
        if let idx = conversations.firstIndex(where: { $0.id == convID }) {
            if !conversations[idx].turns.isEmpty {
                let lastIdx = conversations[idx].turns.count - 1
                conversations[idx].turns[lastIdx].output = finalOutput
                conversations[idx].turns[lastIdx].complete = true
            }
            conversations[idx].items = activeItems
            conversations[idx].activeTurnID = ""
            conversations[idx].updatedAtMs = Int64(Date().timeIntervalSince1970 * 1000)
        }

        if !Task.isCancelled {
            seq += 1
            await syncTurnToCloudV2(
                conversationID: convID,
                turnID: tid,
                title: "",
                model: model,
                userText: "",
                output: finalOutput,
                state: turnSucceeded ? Self.stateCompleted : Self.stateFailed,
                stopReason: turnSucceeded ? "end_turn" : "error",
                errorText: turnSucceeded ? "" : (lastError ?? "Cloud request failed."),
                eventKind: turnSucceeded ? Self.evCompleted : Self.evFailed,
                sequence: seq,
                existingConversation: true
            )
            onTurnNotificationEvent?("done", activeConversation?.title ?? "Cloud chat")
            await refreshUsage()
        }

        isSending = false
        activeTurnID = nil
        activeTurnStartedAt = nil
        onConversationsUpdated?()
    }

    private struct InferenceStepResult {
        var text: String = ""
        var thought: String = ""
        var toolCalls: [StreamedToolCallAccumulator] = []
        var error: String?
    }

    private func runStreamingInferenceStep(
        model: String,
        messages: [[String: Any]],
        tools: [[String: Any]]?,
        assistantItemID: String,
        turnID: String
    ) async -> InferenceStepResult {
        var result = InferenceStepResult()
        guard let url = URL(string: auth.origin + "/api/v1/inference") else {
            result.error = "Invalid OpenAgents Cloud API URL."
            return result
        }
        var bodyObj: [String: Any] = [
            "model": model,
            "messages": messages,
            "stream": true,
            "stream_options": ["include_usage": true],
        ]
        if let tools, !tools.isEmpty {
            bodyObj["tools"] = tools
        }
        guard let bodyData = try? JSONSerialization.data(withJSONObject: bodyObj) else {
            result.error = "Could not encode Cloud inference request."
            return result
        }

        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 120)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        req.httpBody = bodyData

        do {
            let authed = try await auth.authorize(req)
            let (bytes, resp) = try await session.bytes(for: authed)
            let status = (resp as? HTTPURLResponse)?.statusCode ?? 0
            if status != 200 {
                var errData = Data()
                for try await b in bytes { errData.append(b); if errData.count >= 8192 { break } }
                let parsedErr = (try? JSONSerialization.jsonObject(with: errData) as? [String: Any])?["error"] as? String
                if status == 401 {
                    result.error = "Sign in to OpenAgents to continue."
                } else if status == 402 {
                    result.error = "Insufficient Cloud credits (0 sats). Top up in Account & Cloud."
                } else {
                    result.error = parsedErr ?? "OpenAgents Cloud error (HTTP \(status))."
                }
                return result
            }

            var callsByIndex: [Int: StreamedToolCallAccumulator] = [:]
            let thoughtItemID = "\(turnID)-thought-\(assistantItemID)"

            for try await line in bytes.lines {
                if Task.isCancelled { break }
                guard line.hasPrefix("data:") else { continue }
                let payload = line.hasPrefix("data: ") ? String(line.dropFirst(6)) : String(line.dropFirst(5))
                if payload == "[DONE]" { break }
                guard let raw = payload.data(using: .utf8),
                      let obj = try? JSONSerialization.jsonObject(with: raw) as? [String: Any] else { continue }

                Self.applyInferenceChunk(
                    obj,
                    text: &result.text,
                    thought: &result.thought,
                    toolCallsByIndex: &callsByIndex
                )

                if !result.thought.isEmpty {
                    if let idx = activeItems.firstIndex(where: { $0.id == thoughtItemID }) {
                        activeItems[idx].text = result.thought
                    } else if let aIdx = activeItems.firstIndex(where: { $0.id == assistantItemID }) {
                        activeItems.insert(CloudTranscriptItem(
                            id: thoughtItemID,
                            role: .thought,
                            text: result.thought,
                            complete: false,
                            startedAtMs: Int64(Date().timeIntervalSince1970 * 1000)
                        ), at: aIdx)
                    }
                }
                if !result.text.isEmpty {
                    setAssistantText(itemID: assistantItemID, text: result.text, complete: false, failed: false)
                    onAssistantStreamUpdate?(assistantItemID, result.text, false)
                }
                onConversationsUpdated?()
            }

            if let tIdx = activeItems.firstIndex(where: { $0.id == thoughtItemID }) {
                activeItems[tIdx].complete = true
            }
            if !result.text.isEmpty {
                setAssistantText(itemID: assistantItemID, text: result.text, complete: true, failed: false)
                onAssistantStreamUpdate?(assistantItemID, result.text, true)
            }
            result.toolCalls = callsByIndex.keys.sorted().compactMap { callsByIndex[$0] }.filter { !$0.name.isEmpty }
        } catch {
            if !Task.isCancelled {
                result.error = AppAccount.words(error)
            }
        }
        return result
    }

    /// Parses one OpenAI/Vertex Chat Completions SSE JSON chunk, preserving `extra_content.google.thought_signature`.
    static func applyInferenceChunk(
        _ obj: [String: Any],
        text: inout String,
        thought: inout String,
        toolCallsByIndex: inout [Int: StreamedToolCallAccumulator]
    ) {
        guard let choices = obj["choices"] as? [[String: Any]],
              let first = choices.first else { return }
        let delta = (first["delta"] as? [String: Any]) ?? (first["message"] as? [String: Any]) ?? [:]

        if let c = delta["content"] as? String, !c.isEmpty {
            text.append(c)
        }
        if let r = (delta["reasoning_content"] as? String) ?? (delta["reasoning"] as? String), !r.isEmpty {
            thought.append(r)
        }
        if let tcs = delta["tool_calls"] as? [[String: Any]] {
            for tc in tcs {
                let idx = (tc["index"] as? NSNumber)?.intValue ?? 0
                var acc = toolCallsByIndex[idx] ?? StreamedToolCallAccumulator(index: idx)
                if let id = tc["id"] as? String, !id.isEmpty {
                    acc.id = id
                }
                if let fn = tc["function"] as? [String: Any] {
                    if let name = fn["name"] as? String, !name.isEmpty {
                        acc.name.append(name)
                    }
                    if let args = fn["arguments"] as? String, !args.isEmpty {
                        acc.arguments.append(args)
                    }
                }
                if let extra = tc["extra_content"] as? [String: Any],
                   let google = extra["google"] as? [String: Any],
                   let sig = google["thought_signature"] as? String,
                   !sig.isEmpty {
                    acc.thoughtSignature.append(sig)
                } else if let sig = tc["thought_signature"] as? String, !sig.isEmpty {
                    acc.thoughtSignature.append(sig)
                }
                if acc.id.isEmpty {
                    acc.id = "call_\(idx)"
                }
                toolCallsByIndex[idx] = acc
            }
        }
    }

    /// Encodes the assistant `tool_calls` turn message back into Chat Completions JSON, echoing `thought_signature` verbatim.
    static func encodeAssistantToolCallMessage(content: String, toolCalls: [StreamedToolCallAccumulator]) -> [String: Any] {
        let encodedCalls: [[String: Any]] = toolCalls.map { tc in
            var item: [String: Any] = [
                "id": tc.id,
                "type": "function",
                "function": [
                    "name": tc.name,
                    "arguments": tc.arguments.isEmpty ? "{}" : tc.arguments,
                ],
            ]
            if !tc.thoughtSignature.isEmpty {
                item["extra_content"] = [
                    "google": [
                        "thought_signature": tc.thoughtSignature,
                    ],
                ]
            }
            return item
        }
        return [
            "role": "assistant",
            "content": content,
            "tool_calls": encodedCalls,
        ]
    }

    private func executeCloudTool(name: String, argumentsJSON: String) async -> (Bool, String) {
        guard CloudCore.isCloudToolAllowed(name) else {
            return (false, "Tool '\(name)' is not in the Tier 1 OpenAgents Cloud allowlist.")
        }
        guard let url = URL(string: auth.origin + "/api/tools/cloud/v1") else {
            return (false, "Invalid Cloud Tools endpoint.")
        }
        let argsObj: Any
        if let raw = argumentsJSON.data(using: .utf8),
           let parsed = try? JSONSerialization.jsonObject(with: raw) as? [String: Any] {
            argsObj = parsed
        } else {
            argsObj = [String: Any]()
        }
        let bodyObj: [String: Any] = ["tool": name, "arguments": argsObj]
        guard let bodyData = try? JSONSerialization.data(withJSONObject: bodyObj) else {
            return (false, "Could not encode Cloud tool request.")
        }

        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 30)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = bodyData

        do {
            let authed = try await auth.authorize(req)
            let (data, resp) = try await session.data(for: authed)
            let status = (resp as? HTTPURLResponse)?.statusCode ?? 0
            if let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                let ok = status == 200 && (obj["ok"] as? Bool) == true
                if ok {
                    return (true, (obj["output"] as? String) ?? "(empty output)")
                }
                return (false, (obj["error"] as? String) ?? "Cloud tool failed (HTTP \(status)).")
            }
            return (false, "Cloud tool failed (HTTP \(status)).")
        } catch {
            return (false, AppAccount.words(error))
        }
    }

    // MARK: - Tier 2 / Tier 3 Durable Turns (/api/chat/submit/v2alpha1 & /api/chat/stop/v2alpha1)

    private func executeDurableTurn(prompt: String, hostPubkey: String?, sandboxSize: String?, targetLabel: String) async {
        let existingConv = activeConversation
        let cid = existingConv?.id ?? ""
        let rev = existingConv?.revision ?? 0
        let proj = selectedProject
        guard let body = CloudCore.encodeSubmit(
            operation: nextOpID(prefix: "sub"),
            conversation: cid,
            revision: rev,
            project: proj?.id ?? "",
            projectRevision: proj?.revision ?? -1,
            prompt: prompt
        ) else {
            lastError = "Could not encode durable Cloud turn request."
            isSending = false
            activeTurnStartedAt = nil
            onConversationsUpdated?()
            return
        }

        let query: String
        if let hostPubkey, hostPubkey.count == 64 {
            query = "?host=\(hostPubkey)"
        } else {
            query = "?target=sandbox&size=\(sandboxSize ?? "small")"
        }
        guard let url = URL(string: auth.origin + "/api/chat/submit/v2alpha1" + query) else {
            lastError = "Invalid submit endpoint."
            isSending = false
            return
        }

        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        let pendingUserID = "pending-u-\(nowMs)"
        let pendingAssistID = "pending-a-\(nowMs)"
        activeItems.append(CloudTranscriptItem(id: pendingUserID, role: .user, text: prompt, complete: true, startedAtMs: nowMs))
        activeItems.append(CloudTranscriptItem(id: pendingAssistID, role: .assistant, text: "", complete: false, startedAtMs: nowMs))
        onConversationsUpdated?()

        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 30)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = body

        do {
            let authed = try await auth.authorize(req)
            let (data, resp) = try await session.data(for: authed)
            let status = (resp as? HTTPURLResponse)?.statusCode ?? 0
            guard (200..<300).contains(status),
                  let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let retConvID = obj["conversation_id"] as? String,
                  let retTurnID = obj["turn_id"] as? String else {
                let errMsg = (try? JSONSerialization.jsonObject(with: data) as? [String: Any])?["error"] as? String
                lastError = errMsg ?? "Durable turn refused (HTTP \(status))."
                setAssistantText(itemID: pendingAssistID, text: lastError ?? "Request failed.", complete: true, failed: true)
                isSending = false
                activeTurnStartedAt = nil
                onConversationsUpdated?()
                return
            }

            let newRev = (obj["conversation_revision"] as? NSNumber)?.int64Value ?? (rev + 1)
            activeConversationID = retConvID
            activeTurnID = retTurnID
            activeTurnIsDurable = true
            let conv = MobileCloudConversation(
                id: retConvID,
                title: existingConv?.title ?? String(prompt.prefix(64)),
                model: selectedModelID,
                workspace: targetLabel,
                revision: newRev,
                updatedAtMs: nowMs,
                pinned: existingConv?.pinned ?? false,
                archived: false,
                activeTurnID: retTurnID,
                compute: hostPubkey != nil ? "local" : "sandbox",
                sourceDevice: targetLabel,
                turns: (existingConv?.turns ?? []) + [MobileCloudTurn(id: retTurnID, userText: prompt, output: "", complete: false)],
                items: activeItems
            )
            upsertConversation(conv)
            ensureConversationSSE(for: retConvID)
            onConversationsUpdated?()
        } catch {
            lastError = AppAccount.words(error)
            setAssistantText(itemID: pendingAssistID, text: lastError ?? "Network error.", complete: true, failed: true)
            isSending = false
            activeTurnStartedAt = nil
            onConversationsUpdated?()
        }
    }

    private func cancelDurableTurn(_ turnID: String) async {
        guard auth.loggedIn,
              let url = URL(string: auth.origin + "/api/chat/stop/v2alpha1"),
              let body = CloudCore.encodeStop(operation: nextOpID(prefix: "stop"), turn: turnID) else { return }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 15)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = body
        if let authed = try? await auth.authorize(req) {
            _ = try? await session.data(for: authed)
        }
    }

    // MARK: - Cloud Sync Helpers

    private func syncTurnToCloudV2(
        conversationID: String,
        turnID: String,
        title: String,
        model: String,
        userText: String,
        output: String,
        state: Int64,
        stopReason: String,
        errorText: String,
        eventKind: Int64,
        sequence: Int64,
        existingConversation: Bool
    ) async {
        guard auth.loggedIn else { return }
        let path = existingConversation ? "/api/chat/v2/conversations/\(conversationID)/events" : "/api/chat/v2/sync"
        guard let url = URL(string: auth.origin + path) else { return }
        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        var evBuf = [UInt8](repeating: 0, count: 65536)
        let evText = eventKind == Self.evUserMessage ? userText : output
        let evLen = oa_cc_event_write(
            conversationID, turnID, sequence,
            eventKind, state, evText,
            "cloud", model, 0,
            "ios-mobile", selectedProject?.name ?? "",
            stopReason, nowMs,
            &evBuf, Int64(evBuf.count)
        )
        let evJSON = evLen > 0 ? String(decoding: evBuf[0..<Int(evLen)], as: UTF8.self) : ""
        let opID = nextOpID(prefix: "sync")
        var syncBuf = [UInt8](repeating: 0, count: 131072)
        let syncLen = oa_cc_sync_write(
            opID, conversationID, turnID,
            title, false, false, false, false,
            "cloud", model, 0,
            "ios-mobile", selectedProject?.name ?? "",
            userText, state, output,
            stopReason, errorText,
            evJSON,
            &syncBuf, Int64(syncBuf.count)
        )
        guard syncLen > 0 else { return }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = Data(syncBuf[0..<Int(syncLen)])
        if let authed = try? await auth.authorize(req) {
            _ = try? await session.data(for: authed)
        }
    }

    private func syncToolEvent(
        conversationID: String,
        turnID: String,
        sequence: Int64,
        kind: Int64,
        toolID: String,
        toolName: String,
        toolArgs: String,
        text: String,
        failed: Bool,
        model: String
    ) async {
        guard auth.loggedIn,
              let url = URL(string: auth.origin + "/api/chat/v2/conversations/\(conversationID)/events") else { return }
        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        var evBuf = [UInt8](repeating: 0, count: 65536)
        let evLen = oa_cc_tool_event_write(
            conversationID, turnID, sequence,
            kind, Self.stateRunning,
            toolID, toolName, toolArgs,
            text, failed,
            "cloud", model, 0,
            "ios-mobile", selectedProject?.name ?? "",
            nowMs,
            &evBuf, Int64(evBuf.count)
        )
        guard evLen > 0 else { return }
        let evJSON = String(decoding: evBuf[0..<Int(evLen)], as: UTF8.self)
        var syncBuf = [UInt8](repeating: 0, count: 131072)
        let syncLen = oa_cc_sync_write(
            nextOpID(prefix: "tev"), conversationID, turnID,
            "", false, false, false, false,
            "cloud", model, 0,
            "ios-mobile", selectedProject?.name ?? "",
            "", Self.stateRunning, "",
            "", "",
            evJSON,
            &syncBuf, Int64(syncBuf.count)
        )
        guard syncLen > 0 else { return }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = Data(syncBuf[0..<Int(syncLen)])
        if let authed = try? await auth.authorize(req) {
            _ = try? await session.data(for: authed)
        }
    }

    // MARK: - Dual Live SSE Sync (/api/chat/v2/stream & /api/chat/v2/conversations/:id/stream)

    private func runAccountSSELoop() async {
        var backoffMs: UInt64 = 1000
        while !Task.isCancelled {
            guard auth.loggedIn,
                  let url = URL(string: auth.origin + "/api/chat/v2/stream?after=\(accountSSECursor)") else {
                isStreamConnected = false
                try? await Task.sleep(nanoseconds: 2_000_000_000)
                continue
            }
            var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 1860)
            req.httpMethod = "GET"
            req.setValue("text/event-stream", forHTTPHeaderField: "Accept")
            if accountSSECursor > 0 { req.setValue("\(accountSSECursor)", forHTTPHeaderField: "Last-Event-ID") }

            do {
                let authed = try await auth.authorize(req)
                let (bytes, resp) = try await session.bytes(for: authed)
                guard (resp as? HTTPURLResponse)?.statusCode == 200 else {
                    isStreamConnected = false
                    try? await Task.sleep(nanoseconds: backoffMs * 1_000_000)
                    backoffMs = min(backoffMs * 2, 15_000)
                    continue
                }
                isStreamConnected = true
                backoffMs = 1000
                var evID: Int64 = -1
                var dataLines: [String] = []
                var lineBuf: [UInt8] = []
                lineBuf.reserveCapacity(1024)
                // Read raw bytes instead of bytes.lines: Foundation's AsyncBytes.lines skips
                // empty lines, dropping SSE "\n\n" frame boundaries.
                for try await byte in bytes {
                    if Task.isCancelled { break }
                    if byte == 0x0A {
                        if lineBuf.last == 0x0D { lineBuf.removeLast() }
                        let line = String(decoding: lineBuf, as: UTF8.self)
                        lineBuf.removeAll(keepingCapacity: true)
                        if line.isEmpty {
                            if !dataLines.isEmpty {
                                let payload = dataLines.joined(separator: "\n")
                                await handleAccountSSEPayload(id: evID, payload: payload)
                            }
                            evID = -1
                            dataLines = []
                        } else if line.hasPrefix("id:") {
                            if let parsed = Int64(line.dropFirst(3).trimmingCharacters(in: .whitespaces)) {
                                evID = parsed
                            }
                        } else if line.hasPrefix("data:") {
                            let d = line.hasPrefix("data: ") ? String(line.dropFirst(6)) : String(line.dropFirst(5))
                            dataLines.append(d)
                        }
                    } else if lineBuf.count < 262_144 {
                        lineBuf.append(byte)
                    }
                }
            } catch {
                if Task.isCancelled { break }
                isStreamConnected = false
                try? await Task.sleep(nanoseconds: backoffMs * 1_000_000)
                backoffMs = min(backoffMs * 2, 15_000)
            }
        }
        isStreamConnected = false
    }

    private func handleAccountSSEPayload(id: Int64, payload: String) async {
        if id > accountSSECursor { accountSSECursor = id }
        guard let raw = payload.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: raw) as? [String: Any],
              let convID = obj["conversation_id"] as? String, !convID.isEmpty else { return }
        let evType = (obj["event_type"] as? String) ?? ""
        if evType == "turn_completed" || evType == "turn_failed" || evType == "turn_cancelled" {
            if activeConversationID == convID && activeTurnIsDurable {
                isSending = false
                activeTurnID = nil
                activeTurnIsDurable = false
                activeTurnStartedAt = nil
                onTurnNotificationEvent?("done", (obj["title"] as? String) ?? "Cloud chat")
            }
        }
        await refreshConversations()
        if activeConversationID == convID && !(isSending && !activeTurnIsDurable) {
            await pullConversation(convID)
        }
    }

    private func ensureConversationSSE(for convID: String) {
        guard auth.loggedIn else { return }
        if convSSEConversationID == convID && convSSETask != nil { return }
        convSSETask?.cancel()
        convSSEConversationID = convID
        convSSECursor = 0
        convSSETask = Task { [weak self] in
            await self?.runConversationSSELoop(convID: convID)
        }
    }

    private func runConversationSSELoop(convID: String) async {
        var backoffMs: UInt64 = 1000
        while !Task.isCancelled && convSSEConversationID == convID {
            guard auth.loggedIn,
                  let url = URL(string: auth.origin + "/api/chat/v2/conversations/\(convID)/stream?after=\(convSSECursor)") else {
                try? await Task.sleep(nanoseconds: 2_000_000_000)
                continue
            }
            var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 1860)
            req.httpMethod = "GET"
            req.setValue("text/event-stream", forHTTPHeaderField: "Accept")
            if convSSECursor > 0 { req.setValue("\(convSSECursor)", forHTTPHeaderField: "Last-Event-ID") }

            do {
                let authed = try await auth.authorize(req)
                let (bytes, resp) = try await session.bytes(for: authed)
                guard (resp as? HTTPURLResponse)?.statusCode == 200 else {
                    try? await Task.sleep(nanoseconds: backoffMs * 1_000_000)
                    backoffMs = min(backoffMs * 2, 15_000)
                    continue
                }
                backoffMs = 1000
                var evID: Int64 = -1
                var dataLines: [String] = []
                var lineBuf: [UInt8] = []
                lineBuf.reserveCapacity(1024)
                for try await byte in bytes {
                    if Task.isCancelled { break }
                    if byte == 0x0A {
                        if lineBuf.last == 0x0D { lineBuf.removeLast() }
                        let line = String(decoding: lineBuf, as: UTF8.self)
                        lineBuf.removeAll(keepingCapacity: true)
                        if line.isEmpty {
                            if !dataLines.isEmpty {
                                let payload = dataLines.joined(separator: "\n")
                                await handleConversationSSEEvent(convID: convID, id: evID, payload: payload)
                            }
                            evID = -1
                            dataLines = []
                        } else if line.hasPrefix("id:") {
                            if let parsed = Int64(line.dropFirst(3).trimmingCharacters(in: .whitespaces)) {
                                evID = parsed
                            }
                        } else if line.hasPrefix("data:") {
                            let d = line.hasPrefix("data: ") ? String(line.dropFirst(6)) : String(line.dropFirst(5))
                            dataLines.append(d)
                        }
                    } else if lineBuf.count < 262_144 {
                        lineBuf.append(byte)
                    }
                }
            } catch {
                if Task.isCancelled { break }
                try? await Task.sleep(nanoseconds: backoffMs * 1_000_000)
                backoffMs = min(backoffMs * 2, 15_000)
            }
        }
    }

    private func handleConversationSSEEvent(convID: String, id: Int64, payload: String) async {
        if id > convSSECursor { convSSECursor = id }
        guard activeConversationID == convID,
              let raw = payload.data(using: .utf8),
              let ev = try? JSONSerialization.jsonObject(with: raw) as? [String: Any] else { return }
        // Avoid duplicating our own live streaming SSE events on Tier 1 local streaming turns.
        if isSending && !activeTurnIsDurable { return }

        let kind = (ev["type"] as? String) ?? ""
        let turnID = (ev["turn_id"] as? String) ?? "ev"
        let delta = (ev["delta"] as? String) ?? (ev["text"] as? String) ?? ""
        let assistID = "\(turnID)-a"

        switch kind {
        case "text_delta":
            if let idx = activeItems.firstIndex(where: { $0.id == assistID }) {
                activeItems[idx].text.append(delta)
                activeItems[idx].complete = false
                onAssistantStreamUpdate?(assistID, activeItems[idx].text, false)
            } else {
                // Replace pending placeholder if present.
                if let pIdx = activeItems.lastIndex(where: { $0.role == .assistant && $0.id.hasPrefix("pending-a-") }) {
                    activeItems[pIdx] = CloudTranscriptItem(id: assistID, role: .assistant, text: delta, complete: false, startedAtMs: Int64(Date().timeIntervalSince1970 * 1000))
                } else {
                    activeItems.append(CloudTranscriptItem(id: assistID, role: .assistant, text: delta, complete: false, startedAtMs: Int64(Date().timeIntervalSince1970 * 1000)))
                }
                onAssistantStreamUpdate?(assistID, delta, false)
            }
            onConversationsUpdated?()
        case "thinking_delta":
            let thID = "\(turnID)-th"
            if let idx = activeItems.firstIndex(where: { $0.id == thID }) {
                activeItems[idx].text.append(delta)
            } else {
                activeItems.append(CloudTranscriptItem(id: thID, role: .thought, text: delta, complete: false, startedAtMs: Int64(Date().timeIntervalSince1970 * 1000)))
            }
            onConversationsUpdated?()
        case "tool_call":
            let toolID = (ev["tool_id"] as? String) ?? "\(turnID)-tool-\(id)"
            let toolName = (ev["tool_name"] as? String) ?? "Tool"
            let args = (ev["tool_args"] as? String) ?? ""
            activeItems.append(CloudTranscriptItem(id: toolID, role: .tool, text: args, toolName: toolName, complete: false, startedAtMs: Int64(Date().timeIntervalSince1970 * 1000)))
            onConversationsUpdated?()
        case "tool_result":
            let toolID = (ev["tool_id"] as? String) ?? ""
            let failed = (ev["failed"] as? Bool) ?? false
            if let idx = activeItems.firstIndex(where: { $0.id == toolID }) {
                activeItems[idx].text = delta
                activeItems[idx].complete = true
                activeItems[idx].failed = failed
                if let diff = ev["diff"] as? String { activeItems[idx].diff = diff }
            }
            onConversationsUpdated?()
        case "completed", "failed", "cancelled":
            finalizeAssistantItem(complete: true, failed: kind == "failed")
            if let lastA = activeItems.last(where: { $0.role == .assistant }) {
                onAssistantStreamUpdate?(lastA.id, lastA.text, true)
            }
            isSending = false
            activeTurnID = nil
            activeTurnIsDurable = false
            activeTurnStartedAt = nil
            await pullConversation(convID)
            onConversationsUpdated?()
        default:
            break
        }
    }

    // MARK: - Helpers

    private func setAssistantText(itemID: String, text: String, complete: Bool, failed: Bool) {
        if let idx = activeItems.firstIndex(where: { $0.id == itemID }) {
            activeItems[idx].text = text
            activeItems[idx].complete = complete
            activeItems[idx].failed = failed
        }
    }

    private func finalizeAssistantItem(complete: Bool, failed: Bool) {
        for idx in activeItems.indices {
            if !activeItems[idx].complete {
                activeItems[idx].complete = complete
                if activeItems[idx].role == .assistant && failed {
                    activeItems[idx].failed = true
                }
            }
        }
    }

    private func upsertConversation(_ conv: MobileCloudConversation) {
        if let idx = conversations.firstIndex(where: { $0.id == conv.id }) {
            conversations[idx] = conv
        } else {
            conversations.insert(conv, at: 0)
        }
        conversations.sort {
            if $0.pinned != $1.pinned { return $0.pinned }
            return $0.updatedAtMs > $1.updatedAtMs
        }
    }

    private static func itemsFromTurns(_ turns: [MobileCloudTurn], updatedAtMs: Int64) -> [CloudTranscriptItem] {
        var out: [CloudTranscriptItem] = []
        for turn in turns {
            if !turn.userText.isEmpty {
                out.append(CloudTranscriptItem(
                    id: "\(turn.id)-u",
                    role: .user,
                    text: turn.userText,
                    complete: true,
                    startedAtMs: updatedAtMs
                ))
            }
            if !turn.output.isEmpty || !turn.complete {
                out.append(CloudTranscriptItem(
                    id: "\(turn.id)-a",
                    role: .assistant,
                    text: turn.output.isEmpty ? "Thinking…" : turn.output,
                    complete: turn.complete,
                    startedAtMs: updatedAtMs
                ))
            }
        }
        return out
    }

    private static func buildTranscriptItems(
        turns: [MobileCloudTurn],
        events: [[String: Any]],
        updatedAtMs: Int64
    ) -> [CloudTranscriptItem] {
        // Group tool/thought events by turn_id so rich turns show tool activity above the assistant answer.
        var toolsByTurn: [String: [CloudTranscriptItem]] = [:]
        var thoughtsByTurn: [String: String] = [:]
        for ev in events {
            guard let tid = ev["turn_id"] as? String else { continue }
            let kind = (ev["type"] as? String) ?? ""
            if kind == "thinking_delta", let txt = ev["text"] as? String, !txt.isEmpty {
                thoughtsByTurn[tid, default: ""].append(txt)
            } else if kind == "tool_call" {
                let toolID = (ev["tool_id"] as? String) ?? UUID().uuidString
                let name = (ev["tool_name"] as? String) ?? "Tool"
                let args = (ev["tool_args"] as? String) ?? ""
                toolsByTurn[tid, default: []].append(CloudTranscriptItem(
                    id: "\(tid)-\(toolID)",
                    role: .tool,
                    text: args,
                    toolName: name,
                    complete: false,
                    startedAtMs: updatedAtMs
                ))
            } else if kind == "tool_result" {
                let toolID = (ev["tool_id"] as? String) ?? ""
                let out = (ev["text"] as? String) ?? ""
                let failed = (ev["failed"] as? Bool) ?? false
                let diff = (ev["diff"] as? String) ?? ""
                let key = "\(tid)-\(toolID)"
                if var list = toolsByTurn[tid], let idx = list.firstIndex(where: { $0.id == key }) {
                    list[idx].text = out
                    list[idx].complete = true
                    list[idx].failed = failed
                    list[idx].diff = diff
                    toolsByTurn[tid] = list
                }
            }
        }

        var out: [CloudTranscriptItem] = []
        for turn in turns {
            if !turn.userText.isEmpty {
                out.append(CloudTranscriptItem(id: "\(turn.id)-u", role: .user, text: turn.userText, complete: true, startedAtMs: updatedAtMs))
            }
            if let th = thoughtsByTurn[turn.id], !th.isEmpty {
                out.append(CloudTranscriptItem(id: "\(turn.id)-th", role: .thought, text: th, complete: turn.complete, startedAtMs: updatedAtMs))
            }
            if let tools = toolsByTurn[turn.id] {
                out.append(contentsOf: tools)
            }
            if !turn.output.isEmpty || !turn.complete {
                out.append(CloudTranscriptItem(
                    id: "\(turn.id)-a",
                    role: .assistant,
                    text: turn.output.isEmpty ? "Thinking…" : turn.output,
                    complete: turn.complete,
                    startedAtMs: updatedAtMs
                ))
            }
        }
        return out
    }
}
