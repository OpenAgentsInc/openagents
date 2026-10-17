import Network
import Security
import UIKit

/// App-wide state owner: holds the Rust `CoreClient`, maps its snapshots
/// into view models, and fans change notifications out to screens. All
/// members are main-thread; core events hop here through `ListenerBridge`.
final class AppModel {
    struct FrontPage: Equatable {
        var folders: [FolderRowVM] = []
        var sessions: [SessionRowVM] = []
        var sectionSessions: [String: [SessionRowVM]] = [:]
        /// `sessions` grouped by repository project, groups in recency order.
        var projectGroups: [ProjectGroupVM] = []
    }

    struct ProjectGroupVM: Equatable {
        /// Section id: `project:<group key>` (`home:<device>` without a project).
        let id: String
        var name: String
        var sessions: [SessionRowVM]
    }

    /// The Sessions page groups its unsectioned sessions by project
    /// (the desktop sidebar's "By project"). Local to this phone.
    var groupByProject: Bool {
        get { AppModel.preferences.bool(forKey: "groupByProject") }
        set {
            guard newValue != groupByProject else { return }
            AppModel.preferences.set(newValue, forKey: "groupByProject")
            observers.values.forEach { $0() }
        }
    }

    struct LiveCounts: Equatable {
        var working = 0
        var awaiting = 0
    }

    @MainActor lazy var voice = RemoteVoiceController(app: self)
    private(set) var projectSharing: CoreSharing?
    private lazy var sharingBridge = ProjectSharingBridge(app: self)
    private(set) var agentLab: CoreAgentLab?
    private lazy var labBridge = LabBridge(app: self)
    private(set) var customAgents: CoreAgents?
    private lazy var agentsBridge = AgentsBridge(app: self)
    private(set) var savedHistory: CoreSavedHistory?
    private(set) var savedSnapshot: SavedHistorySnapshot?
    private lazy var savedBridge = SavedHistoryBridge(app: self)
    private(set) var client: CoreClient?
    private(set) var frontPage = FrontPage()
    private(set) var archived: [SessionRowVM] = []
    private(set) var connectivity: Connectivity?
    /// Front-page sessions that are working / waiting on the user.
    private(set) var live = LiveCounts()
    private var rows: [String: SessionRow] = [:]
    private var rawProjects: [ProjectView] = []
    private var lastHosts: [HostOption] = []
    private var workspaceRevision: UInt64 = 0
    private var observers: [UUID: () -> Void] = [:]
    private var sessionObservers: [String: [UUID: () -> Void]] = [:]
    private lazy var bridge = ListenerBridge(app: self)
    private var refreshScheduled = false
    private var clock: Timer?
    private let path = NWPathMonitor()
    private var account: CoreAccount?
    private lazy var accountBridge = AccountBridge(app: self)
    private(set) var accountSnapshot: AccountSnapshot?
    /// The chat answered by OpenAgents, signed in or not (#11978).
    private(set) lazy var cloudChat: CloudChatModel? = account.map(CloudChatModel.init) ?? (isDemo ? CloudChatModel.demo() : nil)
    /// The own-computer card's record (#11982); the demo's starts fresh each launch.
    private(set) lazy var ownComputerOffer: CoreOwnComputerOffer? = account?.ownComputerOffer()
        ?? (isDemo ? CoreOwnComputerOffer.demo(dataDir: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString).path) : nil)
    /// The first-launch tutorial's record (#11996), or nil when it doesn't
    /// apply: the demo shows it only for `-route onboarding`, and the test
    /// hooks that sign in or out skip it.
    private(set) lazy var onboarding: CoreOnboarding? = {
        let args = ProcessInfo.processInfo.arguments
        if isDemo {
            guard let i = args.firstIndex(of: "-route"), i + 1 < args.count, args[i + 1] == "onboarding" else { return nil }
            return CoreOnboarding.demo()
        }
        #if DEBUG
        let hooks = ["-no-onboarding", "-signedout", "-account-sign-in", "-dev", "-account-test-credential-url"]
        if hooks.contains(where: args.contains) { return nil }
        #endif
        return CoreOnboarding(dataDir: Self.coreDir.path)
    }()
    private var accountGeneration: UInt64 = 0
    private var openedAccountGeneration: UInt64?
    #if DEBUG
    private var fixtureSignedIn = false
    private var fixtureSignOut = false
    private var fixtureSawSession = false
    #endif

    var onSignedIn: (() -> Void)?
    var onSignOut: (() -> Void)?
    /// The new-session page's options (project, host, branch, model,
    /// effort), kept across launches.
    var lastDraft: NewSessionDraft = {
        var d = !AppModel.persistsNewSession ? NewSessionDraft() : AppModel.preferences.data(forKey: "newSessionDraft").flatMap { try? JSONDecoder().decode(NewSessionDraft.self, from: $0) } ?? NewSessionDraft()
        // `-harness mock`: live-stack tests must never start a real agent.
        let args = ProcessInfo.processInfo.arguments
        if let i = args.firstIndex(of: "-harness"), i + 1 < args.count { d.harness = args[i + 1] }
        return d
    }() {
        didSet {
            guard Self.persistsNewSession else { return }
            AppModel.preferences.set(try? JSONEncoder().encode(lastDraft), forKey: "newSessionDraft")
        }
    }

    /// The demo (and the UI tests on it) keep the new-session page in memory
    /// only: one run's leftovers must not seed the next.
    private static let persistsNewSession = !ProcessInfo.processInfo.arguments.contains("-demo") && !AppModel.isAccountFixture
    private var volatileNewSessionText = ""

    /// What was typed on the new-session page when it was closed unsent.
    var newSessionText: String {
        get { Self.persistsNewSession ? Drafts.load(Self.newSessionDraftKey) : volatileNewSessionText }
        set {
            if Self.persistsNewSession { Drafts.save(Self.newSessionDraftKey, newValue) } else { volatileNewSessionText = newValue }
        }
    }

    /// Images staged there (this launch only).
    var newSessionImages: [StagedImage] = []

    private static let newSessionDraftKey = "new-session"

    var isSignedIn: Bool { client != nil }
    var isDemo: Bool { client?.isDemo() ?? false }

    init() {
        // The wallet's seed is read first, before anything else in the app
        // touches the Keychain; Rust never makes a seed in place of one it
        // couldn't read (wallet::seed).
        WalletModel.shared.open()
        // Online/offline + interface changes cut sync backoff short.
        path.pathUpdateHandler = { [weak self] p in
            DispatchQueue.main.async { self?.client?.setNetworkOnline(online: p.status == .satisfied) }
        }
        path.start(queue: DispatchQueue(label: "com.openagents.app.path"))
        let args = ProcessInfo.processInfo.arguments
        #if DEBUG
        // Test hooks (never in release builds): wipe the Keychain, or run
        // against a local dev stack.
        if let i = args.firstIndex(of: "-dev"), i + 2 < args.count {
            start(.dev(userId: args[i + 1], orgId: args[i + 2]))
            return
        }
        #endif
        if args.contains("-demo") || args.contains("-route") {
            start(.demo(options: Self.demoOptions()))
            return
        }
        do {
            account = try CoreAccount(dataDir: Self.coreDir.path, apiUrl: Self.accountAPI, websiteUrl: Self.accountWebsite, listener: accountBridge)
            #if DEBUG
            #if CODER_ACCOUNT_FIXTURE
            if let i = args.firstIndex(of: "-account-test-credential-url"), i + 1 < args.count {
                account?.signInFromFixture(url: args[i + 1])
                return
            }
            #endif
            // The account fixture's tests sign in with a code, which the app
            // no longer offers on screen.
            if args.contains("-account-sign-in") { account?.signIn(deviceName: UIDevice.current.name); return }
            if args.contains("-signedout") { account?.signOut(); return }
            #endif
            account?.startIdentity()
        } catch {
            accountSnapshot = AccountSnapshot(generation: 0, state: .failed(message: "Couldn't start sign-in. Try again.", canRestore: false))
        }
    }

    func openDemo() {
        account?.cancel()
        if start(.demo(options: Self.demoOptions())) { onSignedIn?() }
    }

    private static var accountAPI: String {
        #if DEBUG
        let args = ProcessInfo.processInfo.arguments
        if let i = args.firstIndex(of: "-account-api"), i + 1 < args.count { return args[i + 1] }
        #endif
        return "https://api.openagents.com/v1"
    }
    private static var accountWebsite: String {
        #if DEBUG
        let args = ProcessInfo.processInfo.arguments
        if let i = args.firstIndex(of: "-account-site"), i + 1 < args.count { return args[i + 1] }
        #endif
        return "https://openagents.com"
    }
    func openProjectSharing() throws -> CoreSharing {
        if let projectSharing { return projectSharing }
        guard let account else { throw NSError(domain: "OpenAgents", code: 1, userInfo: [NSLocalizedDescriptionKey: "Log in again to continue."]) }
        let sharing = try account.projectSharing(listener: sharingBridge)
        projectSharing = sharing
        return sharing
    }
    func openProjectInvitations() {
        guard let site = URL(string: Self.accountWebsite), let url = URL(string: "projects", relativeTo: site)?.absoluteURL else { return }
        UIApplication.shared.open(url)
    }
    func restoreAccount() { account?.startIdentity() }
    /// The person's account identity for Settings' "Your ID", once there is
    /// one (#11995).
    var identity: IdentityView? { account?.identity() }
    /// Only the account fixture's tests reach a code approval now; the app
    /// itself never asks anyone to sign in (#11996).
    func openAccountWebsite() {
        guard case let .waiting(_, url)? = accountSnapshot?.state, let url = URL(string: url) else { return }
        UIApplication.shared.open(url)
    }
    fileprivate func handleAccount(_ snapshot: AccountSnapshot) {
        guard snapshot.generation >= accountGeneration else { return }
        if snapshot.generation != accountGeneration {
            projectSharing = nil
            cloudHost = nil
            cloudHostBusy = false
            cloudHostRequest &+= 1
        }
        accountGeneration = snapshot.generation
        accountSnapshot = snapshot
        if isDemo { return }
        switch snapshot.state {
        case let .signedIn(_, name):
            if client == nil, let account {
                do {
                    let config = CoreConfig(edgeUrl: Self.edgeURL, dataDir: Self.coreDir.path, deviceId: Self.deviceId, deviceName: UIDevice.current.name, platform: "ios", appVersion: Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0")
                    install(try account.connect(config: config, listener: bridge))
                    customAgents = account.customAgents(listener: agentsBridge)
                    agentLab = account.agentLab(listener: labBridge)
                    savedHistory = try account.savedHistory(listener: savedBridge)
                    handleSavedHistory()
                    AccountProfile(name: name, email: nil, orgName: nil).save()
                    #if DEBUG
                    fixtureSignedIn = true
                    #endif
                    onSignedIn?()
                } catch {
                    accountSnapshot = AccountSnapshot(generation: snapshot.generation, state: .failed(message: "Couldn't open your chats. Try again.", canRestore: true))
                }
            }
        case .signedOut:
            if client != nil { onSignOut?() }
            reportAccountFixture()
        case .restoring, .starting, .waiting, .failed:
            if client != nil { onSignOut?() }
            if case .waiting = snapshot.state, openedAccountGeneration != snapshot.generation {
                openedAccountGeneration = snapshot.generation
                #if DEBUG
                if !ProcessInfo.processInfo.arguments.contains("-no-auth-browser") { openAccountWebsite() }
                #else
                openAccountWebsite()
                #endif
            }
        }
        observers.values.forEach { $0() }
    }
    fileprivate func handleAgents() {
        observers.values.forEach { $0() }
    }

    fileprivate func handleSavedHistory() {
        savedSnapshot = savedHistory?.snapshot()
        observers.values.forEach { $0() }
    }

    private func reportAccountFixture() {
        #if DEBUG
        guard ProcessInfo.processInfo.arguments.contains("-account-fixture"), fixtureSignedIn, fixtureSignOut else { return }
        let sawSession = fixtureSawSession
        Task { @MainActor [weak self] in
            for _ in 0..<100 {
                guard let self else { return }
                if (try? self.account?.storedLoginPresent()) == false {
                    let report: [String: Any] = ["signed_in": true, "saw_session": sawSession, "keychain_empty": true]
                    let url = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent("account-fixture-result.json")
                    if let bytes = try? JSONSerialization.data(withJSONObject: report) { try? bytes.write(to: url, options: .atomic) }
                    return
                }
                try? await Task.sleep(for: .milliseconds(50))
            }
        }
        #endif
    }

    static func demoOptions() -> DemoOptions {
        let args = ProcessInfo.processInfo.arguments
        let scale: TranscriptScale = args.contains("-huge") ? .huge : args.contains("-big") ? .big : .normal
        return DemoOptions(
            fixture: args.contains("-no-projects") ? .noProjects : args.contains("-ios-only") ? .iosOnly : .standard,
            transcriptScale: scale,
            streamSpeed: args.contains("-fast") ? .fast : .realistic,
            longReply: args.contains("-longreply")
        )
    }

    // MARK: Lifecycle

    static var isAccountFixture: Bool {
        #if DEBUG
        return ProcessInfo.processInfo.arguments.contains("-account-fixture")
        #else
        return false
        #endif
    }
    static var preferences: UserDefaults {
        isAccountFixture ? UserDefaults(suiteName: "com.openagents.app.account-fixture")! : .standard
    }
    private static var coreDir: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent(isAccountFixture ? "account-fixture-core" : "core", isDirectory: true)
    }

    /// Local docs belong to one identity: another account/org starts from an
    /// empty store (it must never see — or sync — the last one's cache).
    private static func claimCoreDir(for credentials: Credentials) {
        let owner: String
        switch credentials {
        case let .workOs(userId, orgId, _), let .dev(userId, orgId): owner = "\(userId)/\(orgId)"
        case .demo: return
        }
        let marker = coreDir.appendingPathComponent(".owner")
        if (try? String(contentsOf: marker, encoding: .utf8)) != owner {
            try? FileManager.default.removeItem(at: coreDir)
        }
        try? FileManager.default.createDirectory(at: coreDir, withIntermediateDirectories: true)
        try? owner.write(to: marker, atomically: true, encoding: .utf8)
    }

    @discardableResult
    private func start(_ credentials: Credentials) -> Bool {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let dir = credentials.isDemo ? support.appendingPathComponent("demo", isDirectory: true) : Self.coreDir
        if !credentials.isDemo { Self.claimCoreDir(for: credentials) }
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let config = CoreConfig(
            edgeUrl: Self.edgeURL,
            dataDir: dir.path,
            deviceId: Self.deviceId,
            deviceName: UIDevice.current.name,
            platform: "ios",
            appVersion: Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0"
        )
        do {
            install(try CoreClient(config: config, credentials: credentials, listener: bridge))
            return true
        } catch {
            NSLog("core start failed: \(error)")
            client = nil
            return false
        }
    }

    private func install(_ next: CoreClient) {
        client = next
        refreshWorkspace()
        next.preloadSessions()
        clock?.invalidate()
        clock = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in self?.refreshWorkspace() }
    }

    /// `-edge <url>` points at a local `wrangler dev` edge; otherwise production.
    static var edgeURL: String {
        #if DEBUG
        let args = ProcessInfo.processInfo.arguments
        if let i = args.firstIndex(of: "-edge"), i + 1 < args.count { return args[i + 1] }
        #endif
        return productionEdgeUrl()
    }

    static var deviceId: String {
        if let id = AppModel.preferences.string(forKey: "deviceId") { return id }
        let id = "ios-" + UUID().uuidString.lowercased().prefix(8)
        AppModel.preferences.set(id, forKey: "deviceId")
        return id
    }

    /// The user signing out: this device forgets the account's local docs.
    @MainActor func signOut() {
        voice.stop()
        forgetOnSignOut = true
        #if DEBUG
        fixtureSignOut = true
        #endif
        if !isDemo { account?.signOut() }
        onSignOut?()
    }

    /// Delete the account on the server (#12002). On success the account
    /// signs out like Sign Out, and this device forgets its local chats.
    /// The wallet's 12 words aren't touched.
    @MainActor func deleteAccount(forfeitBalance: Bool) async -> AccountDeletion {
        guard let account, !isDemo else { return .failed(message: "Sign in first, then delete your account.") }
        forgetOnSignOut = true
        let result = await account.deleteAccount(forfeitBalance: forfeitBalance)
        if case .deleted = result {
            voice.stop()
        } else {
            forgetOnSignOut = false
        }
        return result
    }

    /// Set only by a sign-out the user asked for. An expired session keeps its
    /// store (the outbox may hold unsent messages); the `.owner` marker still
    /// keeps a different account out of it.
    private var forgetOnSignOut = false

    @MainActor func signOutLocally() {
        let wasDemo = isDemo
        voice.stop()
        endDeliveryBackgroundTask()
        lastDeliveryActivity = .distantPast
        savedHistory?.shutdown()
        savedHistory = nil
        savedSnapshot = nil
        client?.shutdown()
        client = nil
        AccountProfile.clear()
        clock?.invalidate()
        if forgetOnSignOut {
            if wasDemo {
                let demo = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("demo", isDirectory: true)
                try? FileManager.default.removeItem(at: demo)
            } else if let account { try? account.clearLocalData() }
            else { try? FileManager.default.removeItem(at: Self.coreDir) }
        }
        forgetOnSignOut = false
        // Nothing of the account stays in memory.
        frontPage = FrontPage()
        archived = []
        rows = [:]
        rawProjects = []
        lastHosts = []
        connectivity = nil
        live = LiveCounts()
        workspaceRevision = 0
        lastDraft = NewSessionDraft()
        newSessionText = ""
        newSessionImages = []
    }

    private var lastDeliveryActivity = Date.distantPast

    func noteDeliveryActivity() { lastDeliveryActivity = Date() }

    private var deliveryBackgroundTask: UIBackgroundTaskIdentifier = .invalid
    private var deliveryBackgroundDeadline: DispatchWorkItem?

    private func endDeliveryBackgroundTask() {
        deliveryBackgroundDeadline?.cancel()
        deliveryBackgroundDeadline = nil
        guard deliveryBackgroundTask != .invalid else { return }
        UIApplication.shared.endBackgroundTask(deliveryBackgroundTask)
        deliveryBackgroundTask = .invalid
    }

    func didEnterBackground() {
        savedHistory?.setForeground(active: false)
        // Give the already-durable room outbox and host wake time to reach
        // the edge when the user leaves immediately after tapping Send.
        endDeliveryBackgroundTask()
        if client != nil, !isDemo,
           Date().timeIntervalSince(lastDeliveryActivity) < 120 || rows.values.contains(where: { $0.sendState == .sending || $0.sendState == .queued }) {
            deliveryBackgroundTask = UIApplication.shared.beginBackgroundTask(withName: "Finish message delivery") { [weak self] in
                self?.endDeliveryBackgroundTask()
            }
            let deadline = DispatchWorkItem { [weak self] in self?.endDeliveryBackgroundTask() }
            deliveryBackgroundDeadline = deadline
            DispatchQueue.main.asyncAfter(deadline: .now() + 25, execute: deadline)
        }
        client?.onBackground()
    }
    func willEnterForeground() {
        // Retries a read that failed while the phone was locked.
        WalletModel.shared.open()
        savedHistory?.setForeground(active: true)
        if !isDemo { account?.revalidate() }
        endDeliveryBackgroundTask()
        client?.onForeground()
        refreshWorkspace()
    }

    /// Pull to refresh: redial the registry and rooms and re-probe health (the
    /// same resync as returning to the foreground), then report back once the
    /// workspace has updated — at least `minimum` so the spinner doesn't
    /// flicker, at most `maximum` if nothing changes.
    func refresh(minimum: TimeInterval = 0.6, maximum: TimeInterval = 3, completion: @escaping () -> Void) {
        try? savedHistory?.action(action: .refreshChats)
        let started = Date()
        var finished = false
        var token: AnyObject?
        let finish = {
            guard !finished else { return }
            finished = true
            token = nil
            let wait = max(0, minimum - Date().timeIntervalSince(started))
            DispatchQueue.main.asyncAfter(deadline: .now() + wait, execute: completion)
        }
        // A reconnecting/offline blip from the redial isn't the resync landing.
        token = observe { [weak self] in
            if let state = self?.connectivity?.state, state == .reconnecting || state == .offline { return }
            finish()
        }
        client?.onForeground()
        refreshWorkspace()
        DispatchQueue.main.asyncAfter(deadline: .now() + maximum) {
            finish()
            _ = token
        }
    }

    // MARK: Observation

    @discardableResult
    func observe(_ handler: @escaping () -> Void) -> AnyObject {
        let id = UUID()
        observers[id] = handler
        return Token { [weak self] in self?.observers[id] = nil }
    }

    fileprivate func notifyChanged() { observers.values.forEach { $0() } }

    func observeSession(_ chatId: String, _ handler: @escaping () -> Void) -> AnyObject {
        let id = UUID()
        sessionObservers[chatId, default: [:]][id] = handler
        return Token { [weak self] in self?.sessionObservers[chatId]?[id] = nil }
    }

    fileprivate final class Token {
        let cancel: () -> Void
        init(cancel: @escaping () -> Void) { self.cancel = cancel }
        deinit { cancel() }
    }

    fileprivate func handle(_ event: ClientEvent) {
        switch event {
        case .workspaceChanged:
            scheduleRefresh()
        case let .sessionChanged(chatId, _), let .composerChanged(chatId, _):
            sessionObservers[chatId]?.values.forEach { $0() }
        case let .connectivityChanged(c):
            connectivity = c
            observers.values.forEach { $0() }
        case .authRefreshed:
            break
        case .authExpired:
            if let account { account.restore() } else { onSignOut?() }
        }
    }

    /// Coalesce bursts of registry frames into one rebuild per runloop turn.
    private func scheduleRefresh() {
        guard !refreshScheduled else { return }
        refreshScheduled = true
        DispatchQueue.main.async { [weak self] in
            self?.refreshScheduled = false
            self?.refreshWorkspace()
        }
    }

    private func refreshWorkspace() {
        #if DEBUG
        defer { if fixtureSignedIn && (!frontPage.sessions.isEmpty || !frontPage.projectGroups.isEmpty) { fixtureSawSession = true } }
        #endif
        guard let client else { return }
        let ws = client.workspace()
        workspaceRevision = ws.revision
        var all: [String: SessionRow] = [:]
        func vm(_ r: SessionRow) -> SessionRowVM {
            all[r.id] = r
            return Self.vm(r)
        }
        var page = FrontPage()
        let pinned = ws.front.pinned.map(vm)
        if !pinned.isEmpty {
            page.folders.append(FolderRowVM(id: "pinned", name: "Pinned", count: pinned.count, symbol: "pin"))
        }
        page.sectionSessions["pinned"] = pinned
        for s in ws.front.sections {
            let list = s.sessions.map(vm)
            page.folders.append(FolderRowVM(id: s.id, name: s.name, count: list.count, symbol: "folder"))
            page.sectionSessions[s.id] = list
        }
        page.sessions = ws.front.recent.map(vm)
        page.projectGroups = Self.projectGroups(ws.front.recent, page.sessions, projects: ws.projects)
        rawProjects = ws.projects
        // Sessions reachable only through their project still resolve by id.
        for p in ws.projects {
            for r in p.sessions where all[r.id] == nil { all[r.id] = r }
        }
        let archivedVMs = ws.archived.map(vm)
        var counts = LiveCounts()
        var seen = Set<String>()
        for row in page.sessions + page.sectionSessions.values.flatMap({ $0 }) where seen.insert(row.id).inserted {
            if row.status == .working { counts.working += 1 }
            if row.status == .awaiting { counts.awaiting += 1 }
        }
        rows = all
        // Devices coming and going (Settings, host pickers) count as changes.
        let hosts = hostOptions
        let changed = page != frontPage || archivedVMs != archived || counts != live || hosts != lastHosts
        lastHosts = hosts
        live = counts
        frontPage = page
        archived = archivedVMs
        if changed { observers.values.forEach { $0() } }
    }

    /// Like the desktop's project grouping: checkouts of one repository
    /// share a group (named for its representative), project-less sessions
    /// group per device. Groups and rows keep the incoming recency order.
    private static func projectGroups(_ rows: [SessionRow], _ vms: [SessionRowVM], projects: [ProjectView]) -> [ProjectGroupVM] {
        let byId = Dictionary(projects.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        var groups: [ProjectGroupVM] = []
        var index: [String: Int] = [:]
        for (row, vm) in zip(rows, vms) {
            let id: String
            let name: String
            if let ref = row.project {
                let project = byId[ref.id]
                id = "project:" + (project?.groupKey ?? ref.id)
                name = project?.groupName ?? ref.name
            } else {
                id = "home:" + row.deviceId
                name = row.deviceName.map { "Home · \($0)" } ?? "Home"
            }
            if let i = index[id] {
                groups[i].sessions.append(vm)
            } else {
                index[id] = groups.count
                groups.append(ProjectGroupVM(id: id, name: name, sessions: [vm]))
            }
        }
        return groups
    }

    static func status(_ i: ChatIndicator) -> SessionRowVM.Status {
        switch i {
        case .working: .working
        case .awaitingInput: .awaiting
        case .errored: .errored
        case .completed: .completed
        case .idle: .idle
        }
    }

    static func vm(_ r: SessionRow) -> SessionRowVM {
        SessionRowVM(
            id: r.id,
            title: r.title,
            projectName: r.project?.name ?? r.deviceName ?? "No project",
            hasProject: r.project != nil,
            // Project-less sessions tone like the desktop's "home" tile.
            colorIndex: Int(r.project?.colorIndex ?? projectColorIndex(spacePath: "home")),
            harness: r.harness,
            agentName: r.agentName,
            branch: r.branch,
            pr: r.pullRequest.map { pr in
                switch pr.state {
                case .open: .open
                case .merged: .merged
                case .closed: .closed
                }
            },
            prNumber: r.pullRequest?.number,
            status: status(r.indicator),
            timeLabel: r.timeLabel,
            unseen: r.unseen,
            pinned: r.pinned,
            sendFailed: r.sendState == .failed
        )
    }

    func session(_ id: String) -> SessionRowVM? {
        rows[id].map(Self.vm) ?? client?.sessionRow(chatId: id).map(Self.vm)
    }

    func row(_ id: String) -> SessionRow? {
        rows[id] ?? client?.sessionRow(chatId: id)
    }

    func sessions(inFolder id: String) -> [SessionRowVM] {
        id == "archived" ? archived : frontPage.sectionSessions[id] ?? []
    }

    func search(_ query: String) -> [SessionRowVM] {
        let q = query.trimmingCharacters(in: .whitespaces)
        guard let client, !q.isEmpty else { return frontPage.sessions }
        return client.search(query: q, limit: 60).map { Self.vm($0.session) }
    }


    // MARK: Writes

    private func attempt(_ what: String, _ body: () throws -> Void) {
        do { try body() } catch { NSLog("\(what) failed: \(error)") }
    }

    func setPinned(_ id: String, _ pinned: Bool) {
        attempt("pin") { pinned ? try client?.pinSession(chatId: id) : try client?.unpinSession(chatId: id) }
    }

    func archive(_ id: String) {
        attempt("archive") { try client?.archiveSession(chatId: id) }
        let window = UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.keyWindow }.first
        Toast.show("Archived", action: "Undo", in: window) { [weak self] in self?.unarchive(id) }
    }
    func unarchive(_ id: String) { attempt("unarchive") { try client?.unarchiveSession(chatId: id) } }
    func move(_ id: String, toSection section: String?) { attempt("assign") { try client?.assignSection(chatId: id, sectionId: section) } }
    func movePin(_ id: String, after: String?, before: String?) {
        attempt("move pin") { try client?.movePin(chatId: id, after: after, before: before) }
    }
    func renameSection(_ id: String, _ name: String) { attempt("rename section") { try client?.renameSection(sectionId: id, name: name) } }
    func deleteSection(_ id: String) { attempt("delete section") { try client?.deleteSection(sectionId: id) } }
    func createSection(_ name: String) { attempt("section") { _ = try client?.createSection(name: name) } }
    func rename(_ id: String, _ title: String) { attempt("rename") { try client?.renameSession(chatId: id, title: title) } }
    func markSeen(_ id: String) { attempt("seen") { try client?.markSeen(chatId: id) } }

    // MARK: Sessions

    func sessionSource(_ chatId: String) -> SessionSource? {
        guard let client else { return nil }
        do {
            let handle = try client.openSession(chatId: chatId)
            return CoreSessionSource(app: self, client: client, handle: handle, chatId: chatId)
        } catch {
            NSLog("open session failed: \(error)")
            return nil
        }
    }

    // MARK: New session

    /// Who's signed in, by name (never the WorkOS user / org ids).
    var accountName: String {
        if isDemo { return "Demo" }
        guard client != nil else { return "Your account" }
        let p = AccountProfile.load()
        return p.name ?? p.email ?? "Signed in"
    }

    var accountDetail: String {
        if isDemo { return "Offline demo workspace" }
        // No account yet (#11995): the chat answers as a visitor.
        if client == nil { return "On this phone" }
        let p = AccountProfile.load()
        return [p.name != nil ? p.email : nil, p.orgName].compactMap { $0 }.joined(separator: " · ").nonEmpty ?? "OpenAgents account"
    }

    var projectOptions: [ProjectOption] {
        rawProjects.map { ProjectOption(id: $0.id, name: $0.name, path: $0.path, device: $0.deviceId, deviceName: $0.deviceName ?? deviceName($0.deviceId), online: $0.deviceOnline, git: $0.gitDetected, groupKey: $0.groupKey, groupName: $0.groupName, colorIndex: Int($0.colorIndex)) }
    }

    // MARK: Our cloud (#11852)

    /// The device name Our cloud's engine shows under.
    static let cloudHostName = "Our cloud"
    /// Our cloud's last known state; nil until read or when it isn't offered.
    private(set) var cloudHost: CloudHostView?
    private(set) var cloudHostBusy = false
    private var cloudHostRequest: UInt64 = 0
    var offersCloudHost: Bool { account != nil && client != nil && !isDemo }
    /// Our cloud's engine, once it has connected as a device.
    var cloudHostDevice: HostOption? { hostOptions.first { $0.name == Self.cloudHostName } }

    @MainActor func refreshCloudHost() async {
        guard offersCloudHost, !cloudHostBusy, let account else { return }
        let generation = accountGeneration
        let request = cloudHostRequest
        if let host = try? await account.cloudHost(), generation == accountGeneration, request == cloudHostRequest { cloudHost = host }
        observers.values.forEach { $0() }
    }

    /// Start or stop Our cloud. Returns what to tell the person when it can't.
    @MainActor func setCloudHost(running: Bool) async -> String? {
        guard offersCloudHost, let account else { return "Log in to use Our cloud." }
        guard !cloudHostBusy else { return nil }
        cloudHostRequest &+= 1
        let generation = accountGeneration
        cloudHostBusy = true
        observers.values.forEach { $0() }
        defer {
            if generation == accountGeneration {
                cloudHostBusy = false
                observers.values.forEach { $0() }
            }
        }
        do {
            let host = running ? try await account.startCloudHost() : try await account.stopCloudHost()
            guard generation == accountGeneration else { return nil }
            cloudHost = host
            return nil
        } catch let error as CoreError {
            guard generation == accountGeneration else { return nil }
            switch error {
            case let .HostError(message), let .Auth(message), let .Network(message): return message
            default: return "Our cloud isn't available right now. Try again later."
            }
        } catch {
            return "Our cloud isn't available right now. Try again later."
        }
    }

    /// One line on Our cloud's state, for its row.
    var cloudHostDetail: String {
        if cloudHostBusy { return "Working…" }
        guard let host = cloudHost, host.running else { return "Run sessions on our computer, no desktop needed" }
        if cloudHostDevice?.online != true { return "Starting… it appears under Devices when ready" }
        guard let until = host.untilUnix else { return "Running" }
        let time = DateFormatter.localizedString(from: Date(timeIntervalSince1970: TimeInterval(until)), dateStyle: .none, timeStyle: .short)
        return "Running until \(time)"
    }

    var hostOptions: [HostOption] {
        (client?.executionDevices() ?? []).map { HostOption(id: $0.id, name: $0.name, online: $0.online) }
    }

    /// A device's name; never its id.
    func deviceName(_ id: String) -> String {
        client?.devices().first { $0.id == id }?.name ?? "Unknown device"
    }

    func models(for deviceId: String) async -> [ModelChoice] {
        guard let client else { return [] }
        let harnesses = (try? await client.listHarnesses(deviceId: deviceId)) ?? fallbackHarnesses()
        var out: [ModelChoice] = []
        for h in harnesses where h.offered {
            let models = (try? await client.listModels(deviceId: deviceId, harness: h.id)) ?? fallbackModels(harness: h.id)
            out += models.map { ModelChoice(harness: h.id, harnessLabel: h.label, id: $0.id, label: $0.label, efforts: $0.reasoningLevels) }
        }
        return out
    }

    func listFolders(deviceId: String, path: String?) async -> FolderListing? {
        try? await client?.listFolders(deviceId: deviceId, path: path)
    }

    @MainActor
    func createProject(deviceId: String, path: String, gitDetected: Bool) async -> String? {
        guard let client else { return nil }
        do {
            let id = try await client.createProject(deviceId: deviceId, path: path, gitDetected: gitDetected)
            refreshWorkspace()
            return id
        } catch {
            NSLog("create project failed: \(error)")
            return nil
        }
    }

    func searchFiles(deviceId: String, spaceId: String, query: String) async -> [FileMatch] {
        guard let client else { return [] }
        return (try? await client.searchFiles(deviceId: deviceId, chatId: nil, spaceId: spaceId, query: query)) ?? []
    }

    func refs(projectId: String) async -> [String] {
        guard let client, let p = rawProjects.first(where: { $0.id == projectId }) else { return [] }
        let refs = (try? await client.listRefs(deviceId: p.deviceId, repoPath: p.path)) ?? []
        return refs.sorted { $0.current && !$1.current }.map(\.name)
    }

    func chatHomes(_ id: String) throws -> CoreChatHomes? {
        guard let client else { return nil }
        return try account?.chatHomes(client: client, chat: id)
    }

    func createContinuation(_ continuation: CoreContinuationDraft, draft: NewSessionDraft, text: String, images: [StagedImage]) async throws -> String? {
        guard let client else { return nil }
        let target: SessionTarget
        if let project = draft.projectId { target = .project(spaceId: project) }
        else if let host = draft.hostId { target = .projectless(deviceId: host) }
        else { return nil }
        let config = ChatConfig(harness: draft.harness, model: draft.model, reasoning: draft.effort, modelOptions: [:], sandbox: .workspaceWrite)
        let project = rawProjects.first { $0.id == draft.projectId }
        let worktree = draft.worktree ? project.map { WorktreeSpec(repoPath: $0.path, base: draft.branch ?? "HEAD", spaceId: $0.id) } : nil
        let id = try await continuation.start(core: client,
            newSession: NewSession(target: target, config: config, branch: draft.worktree ? nil : draft.branch, cwd: nil, title: nil),
            request: SendRequest(text: text, attachments: images.map(\.outgoing), worktree: worktree, busy: .queue))
        noteDeliveryActivity()
        refreshWorkspace()
        return id
    }

    /// Create the chat, open it, and send the first message.
    func createSession(draft: NewSessionDraft, text: String, images: [StagedImage]) -> String? {
        guard let client else { return nil }
        let target: SessionTarget
        if let p = draft.projectId {
            target = .project(spaceId: p)
        } else if let h = draft.hostId {
            target = .projectless(deviceId: h)
        } else {
            return nil
        }
        let config = ChatConfig(harness: draft.harness, model: draft.model, reasoning: draft.effort, modelOptions: [:], sandbox: .workspaceWrite)
        do {
            let chatId = try client.createSession(newSession: NewSession(target: target, config: config, branch: draft.worktree ? nil : draft.branch, cwd: nil, title: nil))
            let handle = try client.openSession(chatId: chatId)
            let project = rawProjects.first { $0.id == draft.projectId }
            let worktree = draft.worktree ? project.map { WorktreeSpec(repoPath: $0.path, base: draft.branch ?? "HEAD", spaceId: $0.id) } : nil
            _ = try handle.send(request: SendRequest(text: text, attachments: images.map(\.outgoing), worktree: worktree, busy: .queue))
            noteDeliveryActivity()
            refreshWorkspace()
            return chatId
        } catch {
            NSLog("create session failed: \(error)")
            return nil
        }
    }
}

extension StagedImage {
    var outgoing: OutgoingAttachment { OutgoingAttachment(name: name, mimeType: "image/jpeg", data: data) }
}

/// Core events arrive on Rust runtime threads; hop to main.
private final class ListenerBridge: ClientListener, @unchecked Sendable {
    weak var app: AppModel?

    init(app: AppModel) { self.app = app }

    func onEvent(event: ClientEvent) {
        DispatchQueue.main.async { [weak self] in self?.app?.handle(event) }
    }
}

private final class AccountBridge: AccountListener, @unchecked Sendable {
    weak var app: AppModel?
    init(app: AppModel) { self.app = app }
    func onAccountChanged(snapshot: AccountSnapshot) {
        DispatchQueue.main.async { [weak self] in self?.app?.handleAccount(snapshot) }
    }
}

// MARK: - Display profile (Keychain)

extension Credentials {
    var isDemo: Bool {
        if case .demo = self { return true }
        return false
    }

}

enum Keychain {
    static func save(_ data: Data, service: String, account: String) {
        delete(service: service, account: account)
        let q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
        ]
        SecItemAdd(q as CFDictionary, nil)
    }

    static func load(service: String, account: String) -> Data? {
        let q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var out: AnyObject?
        return SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess ? out as? Data : nil
    }

    static func delete(service: String, account: String) {
        let q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        SecItemDelete(q as CFDictionary)
    }
}

/// Display identity of the signed-in account (Keychain, next to the
/// credentials): the client itself only knows ids.
struct AccountProfile: Codable {
    var name: String?
    var email: String?
    var orgName: String?

    private static var service: String { AppModel.isAccountFixture ? "com.openagents.app.account-fixture" : "com.openagents.app" }
    private static let account = "profile"

    static func load() -> AccountProfile {
        Keychain.load(service: service, account: account).flatMap { try? JSONDecoder().decode(AccountProfile.self, from: $0) } ?? AccountProfile()
    }

    func save() {
        if let data = try? JSONEncoder().encode(self) { Keychain.save(data, service: Self.service, account: Self.account) }
    }

    static func clear() {
        Keychain.delete(service: service, account: account)
    }
}

extension String {
    var nonEmpty: String? { isEmpty ? nil : self }
}

private final class SavedHistoryBridge: SavedHistoryListener {
    weak var app: AppModel?
    init(app: AppModel) { self.app = app }
    func onSavedHistoryChanged() {
        DispatchQueue.main.async { [weak self] in self?.app?.handleSavedHistory() }
    }
}

private final class AgentsBridge: CustomAgentsListener {
    weak var app: AppModel?
    init(app: AppModel) { self.app = app }
    func onCustomAgentsChanged(snapshot: CustomAgentsSnapshot) {
        DispatchQueue.main.async { [weak self] in self?.app?.handleAgents() }
    }
}

private final class LabBridge: AgentLabListener {
    weak var app: AppModel?
    init(app: AppModel) { self.app = app }
    func onAgentLabChanged() { DispatchQueue.main.async { [weak self] in self?.app?.notifyChanged() } }
}

private final class ProjectSharingBridge: SharingListener {
    weak var app: AppModel?
    init(app: AppModel) { self.app = app }
    func onSharingChanged() { DispatchQueue.main.async { [weak self] in self?.app?.notifyChanged() } }
}
