import UIKit

/// What a new session will be created with.
struct NewSessionDraft: Equatable, Codable {
    var projectId: String?
    /// Projectless sessions run on an explicit host.
    var hostId: String?
    var branch: String?
    var worktree = false
    var harness = "claude-code"
    var model: String?
    var effort: String?
}

/// Options the pickers offer (from the core's workspace + host catalogs).
struct ProjectOption: Equatable {
    let id: String
    let name: String
    let path: String
    let device: String
    /// Display name of `device` (never show the id).
    let deviceName: String
    let online: Bool
    let git: Bool
    /// Shared by every checkout of one repository, on any device; with
    /// `groupName` and `colorIndex` they read as one project.
    let groupKey: String
    let groupName: String
    let colorIndex: Int
}

struct HostOption: Equatable {
    let id: String
    let name: String
    let online: Bool

    var displayName: String {
        name == AppModel.cloudHostName ? "OpenAgents Cloud" : name
    }

    /// OpenAgents Cloud for "Run on" next to a project's checkouts: it has
    /// none, so without this the menu's only cloud choice was the account
    /// chat (#12020). Picking it runs the session there with no project.
    static func cloudBesides(_ checkouts: [ProjectOption], in hosts: [HostOption], continuationDevice: String?) -> HostOption? {
        hosts.first { h in
            h.name == AppModel.cloudHostName
                && !checkouts.contains { $0.device == h.id }
                && (continuationDevice == nil || h.id == continuationDevice)
        }
    }
}

struct ModelChoice: Equatable {
    let harness: String
    let harnessLabel: String
    let id: String
    let label: String
    let efforts: [String]
}

/// "Ask anything" → a composer-first canvas. Context is set with native menus
/// (project, branch/worktree, model, effort) that load lazily; the composer is
/// focused immediately so the common case is: tap, type, send.
final class NewSessionViewController: UIViewController, UIGestureRecognizerDelegate {
    private let app: AppModel
    private let onCreated: (String, DraftHandoff?) -> Void
    private let composer = ComposerBar()
    private let hero = UILabel()
    private let wallpaper = WallpaperView()
    private let mark = UIImageView()
    private var draft: NewSessionDraft
    private var models: [ModelChoice] = []
    private var agentsObservation: AnyObject?
    private var waitingForAgent = false
    private let continuation: CoreContinuationDraft?
    private var creatingContinuation = false
    /// No computer online: the chat runs on OpenAgents (#11979).
    private var onOpenAgents = false
    private var offersOpenAgents: Bool { continuation == nil && app.cloudChat != nil && !app.isDemo }

    /// Embedded in the iPad split's main column (no sheet chrome).
    private let embedded: Bool
    private var backgroundObserver: NSObjectProtocol?

    deinit {
        if let backgroundObserver { NotificationCenter.default.removeObserver(backgroundObserver) }
    }
    /// Take the keyboard on appearing. The sheet always does; the iPad column
    /// only when opened on purpose (not the launch page).
    var focusOnAppear: Bool

    init(app: AppModel, prompt: String?, embedded: Bool = false, continuation: CoreContinuationDraft? = nil, onCreated: @escaping (String, DraftHandoff?) -> Void) {
        self.app = app
        self.continuation = continuation
        self.embedded = embedded
        self.focusOnAppear = !embedded
        self.onCreated = onCreated
        self.draft = app.lastDraft
        if let continuation {
            self.draft.projectId = nil
            self.draft.hostId = continuation.deviceId()
        }
        super.init(nibName: nil, bundle: nil)
        // Pick up where the page was left (closed without sending).
        composer.text = prompt ?? app.newSessionText
        composer.images = app.newSessionImages
    }

    /// A session was created from this page: nothing to keep.
    private var created = false

    /// Closed (swipe down, ✕, or replaced in the iPad column) without
    /// sending: keep what was typed and picked for next time.
    private func rememberDraft() {
        guard !created else { return }
        app.lastDraft = draft
        app.newSessionText = composer.text
        app.newSessionImages = composer.images
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        rememberDraft()
    }

    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        Task { @MainActor [weak self] in await self?.app.refreshCloudHost() }
        // The app can be killed in the background: keep the draft first.
        backgroundObserver = NotificationCenter.default.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { [weak self] _ in
            self?.rememberDraft()
        }
        // Tapping the canvas puts the keyboard away (the card stays open here).
        let dismissTap = UITapGestureRecognizer(target: self, action: #selector(dismissKeyboard))
        dismissTap.cancelsTouchesInView = false
        dismissTap.delegate = self
        view.addGestureRecognizer(dismissTap)
        view.backgroundColor = Palette.background
        title = "New Session"
        if !embedded {
            navigationItem.leftBarButtonItem = UIBarButtonItem(systemItem: .close, primaryAction: UIAction { [weak self] _ in
                self?.dismiss(animated: true)
            })
        }

        mark.image = BrandMarks.image(for: draft.harness, side: 34)
        mark.tintColor = Palette.text
        mark.contentMode = .center
        hero.text = "What are we building?"
        hero.numberOfLines = 2
        hero.font = Fonts.ui(.sansSemibold, 22)
        hero.textColor = Palette.text
        hero.textAlignment = .center
        let heroStack = UIStackView(arrangedSubviews: [mark, hero])
        heroStack.axis = .vertical
        heroStack.spacing = 14
        heroStack.alignment = .center
        heroStack.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(wallpaper)
        view.addSubview(heroStack)

        composer.translatesAutoresizingMaskIntoConstraints = false
        composer.placeholder = "Describe the task"
        composer.chipsAlwaysVisible = true
        composer.attachMenu = { [weak self] in
            guard let self else { return UIMenu() }
            return AttachmentPicker.menu(host: self, limit: 8 - self.composer.images.count) { [weak self] in self?.composer.addImages($0) }
        }
        composer.onChipTap = { _, _ in }
        composer.mentionSearch = { [weak self] q in
            guard let self, let p = self.project else { return [] }
            return await self.app.searchFiles(deviceId: p.device, spaceId: p.id, query: q)
        }
        composer.onSend = { [weak self] text, images, _ in
            self?.create(text: text, images: images) ?? false ? .sent : .kept
        }
        view.addSubview(composer)
        heroStack.leadingAnchor.constraint(greaterThanOrEqualTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 24).isActive = true
        // The headline lives in the free space above the composer — from the
        // top of the page to the composer's top edge — centered there, so
        // it's always above the composer (keyboard up or down, one line or
        // two). On iPad the composer keeps the transcript's reading width.
        let free = UILayoutGuide()
        view.addLayoutGuide(free)
        heroStack.setContentCompressionResistancePriority(.required, for: .vertical)
        let fill = composer.widthAnchor.constraint(equalTo: view.safeAreaLayoutGuide.widthAnchor, constant: -24)
        fill.priority = .defaultHigh
        NSLayoutConstraint.activate([
            free.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            free.bottomAnchor.constraint(equalTo: composer.topAnchor, constant: -20),
            free.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            free.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            heroStack.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            heroStack.centerYAnchor.constraint(equalTo: free.centerYAnchor),
            heroStack.topAnchor.constraint(greaterThanOrEqualTo: free.topAnchor),
            heroStack.bottomAnchor.constraint(lessThanOrEqualTo: free.bottomAnchor),
            composer.centerXAnchor.constraint(equalTo: view.safeAreaLayoutGuide.centerXAnchor),
            composer.leadingAnchor.constraint(greaterThanOrEqualTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 12),
            composer.widthAnchor.constraint(lessThanOrEqualToConstant: 768),
            fill,
            composer.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor, constant: -8),
        ])
        if draft.projectId == nil, draft.hostId == nil { draft.projectId = app.projectOptions.first?.id }
        if draft.projectId == nil, draft.hostId == nil {
            // No projects yet: run on the first reachable host.
            draft.hostId = app.hostOptions.first(where: \.online)?.id ?? app.hostOptions.first?.id
        }
        onOpenAgents = offersOpenAgents && !app.projectOptions.contains(where: \.online) && !app.hostOptions.contains(where: \.online)
        agentsObservation = app.observe { [weak self] in self?.agentsChanged() }
        app.customAgents?.refresh()
        loadModels()
    }

    /// The last catalog each host reported, so the chip opens on a real model
    /// name while a fresh one comes back over the relay.
    private static var modelCache: [String: [ModelChoice]] = [:]
    private var modelsDevice: String?
    private var modelLoadRevision: UInt64 = 0

    /// Models for the draft's host: cached (or the built-in catalog) at once,
    /// then the host's own list.
    private func loadModels() {
        let device = deviceId
        guard device != modelsDevice else { return refreshChips() }
        modelsDevice = device
        modelLoadRevision += 1
        let revision = modelLoadRevision
        models = Self.modelCache[device] ?? Self.catalogModels()
        refreshChips()
        Task { [weak self] in
            guard let self else { return }
            let requested = self.draft
            let fresh = await self.app.models(for: device)
            guard !fresh.isEmpty else { return }
            guard self.modelsDevice == device, self.modelLoadRevision == revision else { return }
            Self.modelCache[device] = fresh
            self.models = fresh
            self.refreshChips()
            guard self.continuation == nil, self.app.customAgents?.selectionActive() != true,
                  let client = self.app.client else { return }
            let config = ChatConfig(harness: requested.harness, model: requested.model, reasoning: requested.effort, modelOptions: [:], sandbox: .workspaceWrite)
            guard let selected = try? await client.availableChatConfig(deviceId: device, config: config),
                  self.modelsDevice == device, self.modelLoadRevision == revision,
                  self.draft == requested, !self.created,
                  self.app.customAgents?.selectionActive() != true else { return }
            self.draft.harness = selected.harness
            self.draft.model = selected.model
            self.draft.effort = selected.reasoning
            self.refreshChips()
        }
    }

    private static func catalogModels() -> [ModelChoice] {
        fallbackHarnesses().filter(\.offered).flatMap { h in
            fallbackModels(harness: h.id).map { ModelChoice(harness: h.id, harnessLabel: h.label, id: $0.id, label: $0.label, efforts: $0.reasoningLevels) }
        }
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        if focusOnAppear { composer.becomeFirstResponder() }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        // Desktop hero: from the top, 72% of the viewport (≤ 760pt), the
        // artwork softened behind the composer.
        wallpaper.frame = CGRect(x: 0, y: 0, width: view.bounds.width, height: min(view.bounds.height * 0.72, 760))
        wallpaper.cutout = composer.convert(composer.bounds, to: wallpaper)
    }

    @objc private func dismissKeyboard() {
        view.endEditing(true)
    }

    func gestureRecognizer(_ g: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        touch.view.map { !$0.isDescendant(of: composer) } ?? true
    }

    private var project: ProjectOption? { app.projectOptions.first { $0.id == draft.projectId } }
    private var deviceId: String { project?.device ?? draft.hostId ?? "" }

    /// Checkouts of the picked project, one per row of the host menu.
    private var checkouts: [ProjectOption] {
        guard let p = project else { return [] }
        return app.projectOptions
            .filter { $0.groupKey == p.groupKey && (continuation == nil || $0.device == continuation?.deviceId()) }
            .sorted { ($0.deviceName.lowercased(), $0.path) < ($1.deviceName.lowercased(), $1.path) }
    }

    private func refreshChips() {
        if onOpenAgents {
            composer.chips = [ComposerChip(id: "host", title: "OpenAgents", symbol: "cloud")]
            installChipMenus()
            mark.image = BrandMarks.image(for: "openagents", side: 34)
            return
        }
        var chips: [ComposerChip] = []
        if let p = project {
            // The project, then the host it runs on.
            chips.append(ComposerChip(id: "project", title: p.groupName, symbol: nil, icon: ProjectTile.image(name: p.groupName, colorIndex: p.colorIndex)))
            chips.append(ComposerChip(id: "host", title: p.deviceName, symbol: "desktopcomputer"))
            if p.git && (continuation != nil || app.customAgents?.snapshot().selected == nil) {
                chips.append(ComposerChip(id: "branch", title: draft.worktree ? "New worktree" : (draft.branch ?? "Current branch"), symbol: nil, icon: BranchIcon.sized()))
            }
        } else {
            let host = app.hostOptions.first { $0.id == draft.hostId }
            chips.append(ComposerChip(id: "project", title: "No project", symbol: "tray"))
            chips.append(ComposerChip(id: "host", title: host?.displayName ?? "Choose host", symbol: "desktopcomputer"))
        }
        let model = models.first { $0.harness == draft.harness && $0.id == draft.model } ?? models.first { $0.harness == draft.harness }
        // Never the harness name in place of a model: a model this host
        // hasn't listed still gets its catalog label.
        let modelTitle = model?.label ?? draft.model.map { modelLabel(harness: draft.harness, model: $0) } ?? fallbackModels(harness: draft.harness).first?.label ?? HarnessNames.label(draft.harness)
        let custom = continuation == nil ? app.customAgents?.snapshot().selected : nil
        chips.insert(ComposerChip(id: "model", title: custom?.name ?? (continuation == nil && app.customAgents?.selectionActive() == true ? "Choose agent" : modelTitle), symbol: nil, icon: BrandMarks.image(for: custom == nil ? draft.harness : "openagents", side: 13)), at: 0)
        if custom == nil, let efforts = model?.efforts, !efforts.isEmpty {
            chips.append(ComposerChip(id: "effort", title: (draft.effort ?? efforts[efforts.count / 2]).capitalized, symbol: "gauge.with.dots.needle.67percent"))
        }
        composer.chips = chips
        installChipMenus()
        mark.image = BrandMarks.image(for: custom != nil ? "openagents" : draft.harness, side: 34)
    }

    private func installChipMenus() {
        if composer.chipMenus.isEmpty { composer.chipMenus = [
            "project": { [weak self] in self?.projectMenu() },
            "host": { [weak self] in self?.hostMenu() },
            "branch": { [weak self] in self?.branchMenu() },
            "model": { [weak self] in self?.modelMenu() },
            "effort": { [weak self] in self?.effortMenu() },
        ] }
    }

    /// Run on OpenAgents: we answer here, with no computer.
    private func openAgentsChoice() -> [UIMenuElement] {
        guard offersOpenAgents else { return [] }
        return [UIAction(title: "OpenAgents", subtitle: "We answer here, no computer needed", image: BrandMarks.image(for: "openagents", side: 18), state: onOpenAgents ? .on : .off) { [weak self] _ in
            self?.onOpenAgents = true
            self?.refreshChips()
        }]
    }

    /// One entry per project — checkouts of one repository, on any device,
    /// are one entry listing their hosts; the host chip then picks which.
    private func projectMenu() -> UIMenu {
        var groups: [[ProjectOption]] = []
        for p in app.projectOptions where continuation == nil || p.device == continuation?.deviceId() {
            if let i = groups.firstIndex(where: { $0[0].groupKey == p.groupKey }) {
                groups[i].append(p)
            } else {
                groups.append([p])
            }
        }
        groups.sort { $0[0].groupName.localizedCaseInsensitiveCompare($1[0].groupName) == .orderedAscending }
        let picked = project?.groupKey
        var sections: [UIMenuElement] = [UIMenu(options: .displayInline, children: groups.map { members in
            let first = members[0]
            var hosts: [String] = []
            for m in members where !hosts.contains(m.deviceName) { hosts.append(m.deviceName) }
            return UIAction(title: first.groupName, subtitle: hosts.sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }.joined(separator: ", "), image: ProjectTile.image(name: first.groupName, colorIndex: first.colorIndex, side: 18), state: first.groupKey == picked ? .on : .off) { [weak self] _ in
                self?.pickProject(members)
            }
        })]
        sections.append(UIMenu(options: .displayInline, children: [
            UIAction(title: "No Project…", image: UIImage(systemName: "tray"), state: draft.projectId == nil ? .on : .off) { [weak self] _ in
                guard let self else { return }
                self.draft.projectId = nil
                self.draft.hostId = self.draft.hostId ?? self.app.hostOptions.first(where: \.online)?.id ?? self.app.hostOptions.first?.id
                self.loadModels()
            },
            UIAction(title: "New Project…", image: UIImage(systemName: "folder.badge.plus")) { [weak self] _ in
                guard let self else { return }
                let vc = NewProjectViewController(app: self.app)
                vc.onCreated = { [weak self] id in
                    self?.draft.projectId = id
                    self?.draft.hostId = nil
                    self?.draft.branch = nil
                    self?.loadModels()
                }
                self.present(UINavigationController(rootViewController: vc), animated: true)
            },
        ]))
        return UIMenu(title: "Project", children: sections)
    }

    /// Keep the host when it has a checkout of the project, else the first
    /// reachable one.
    private func pickProject(_ members: [ProjectOption]) {
        let pick = members.first { $0.id == draft.projectId }
            ?? members.first { $0.device == deviceId }
            ?? members.first(where: \.online)
            ?? members[0]
        pickCheckout(pick)
    }

    private func pickCheckout(_ p: ProjectOption) {
        if p.id != draft.projectId { draft.branch = nil }
        draft.projectId = p.id
        draft.hostId = nil
        loadModels()
    }

    /// With a project: its checkouts, by host (the path tells apart several
    /// on one host). Without: every host — project-less sessions run in its
    /// home.
    private func hostMenu() -> UIMenu {
        let checkouts = self.checkouts
        var hosts: [UIMenuElement] = openAgentsChoice()
        if !checkouts.isEmpty {
            hosts += checkouts.map { p in
                let shared = checkouts.filter { $0.device == p.device }.count > 1
                return UIAction(title: p.deviceName, subtitle: shared ? p.path : (p.online ? "Online" : "Offline"), image: UIImage(systemName: "desktopcomputer"), state: !onOpenAgents && p.id == draft.projectId ? .on : .off) { [weak self] _ in
                    self?.onOpenAgents = false
                    self?.pickCheckout(p)
                }
            }
            if let cloud = HostOption.cloudBesides(checkouts, in: app.hostOptions, continuationDevice: continuation?.deviceId()) {
                hosts.append(hostAction(cloud, leavingProject: true))
            }
        } else {
            hosts += app.hostOptions.filter { continuation == nil || $0.id == continuation?.deviceId() }.map { hostAction($0) }
        }
        if continuation == nil, app.offersCloudHost, app.cloudHostDevice?.online != true {
            let starting = app.cloudHost?.running == true
            hosts.append(UIAction(title: starting ? "OpenAgents Cloud is starting…" : "Start OpenAgents Cloud", subtitle: "Runs on OpenAgents Cloud, no desktop needed", image: UIImage(systemName: "cloud"), attributes: starting || app.cloudHostBusy ? .disabled : []) { [weak self] _ in
                Task { @MainActor [weak self] in
                    guard let self else { return }
                    if let message = await self.app.setCloudHost(running: true) {
                        let alert = UIAlertController(title: "Couldn't start OpenAgents Cloud", message: message, preferredStyle: .alert)
                        alert.addAction(UIAlertAction(title: "OK", style: .default))
                        self.present(alert, animated: true)
                    } else {
                        Toast.show("Starting OpenAgents Cloud. Choose it under Run on when it's ready.", in: self.view.window)
                    }
                }
            })
        }
        return UIMenu(title: "Run on", children: hosts)
    }

    /// A project-less session on `h`, in its own folder there; offered next
    /// to a project's checkouts, it says it leaves the project.
    private func hostAction(_ h: HostOption, leavingProject: Bool = false) -> UIAction {
        let status = h.online ? "Online" : "Offline"
        return UIAction(title: h.displayName, subtitle: leavingProject ? "No project · \(status)" : status, image: UIImage(systemName: h.name == AppModel.cloudHostName ? "cloud" : "desktopcomputer"), state: !onOpenAgents && draft.projectId == nil && h.id == draft.hostId ? .on : .off) { [weak self] _ in
            guard let self else { return }
            self.onOpenAgents = false
            if self.draft.projectId != nil {
                self.draft.projectId = nil
                self.draft.branch = nil
                self.draft.worktree = false
            }
            self.draft.hostId = h.id
            self.loadModels()
        }
    }

    private func branchMenu() -> UIMenu {
        UIMenu(title: "Checkout", children: [
            UIAction(title: "New worktree", state: draft.worktree ? .on : .off) { [weak self] _ in
                self?.draft.worktree.toggle()
                self?.refreshChips()
            },
            UIMenu(title: "Branch", options: .displayInline, children: [UIDeferredMenuElement { [weak self] done in
                guard let self, let p = self.project else { return done([]) }
                Task { @MainActor in
                    let refs = await self.app.refs(projectId: p.id)
                    done(refs.map { ref in
                        UIAction(title: ref, state: ref == (self.draft.branch ?? refs.first) ? .on : .off) { [weak self] _ in
                            self?.draft.branch = ref
                            self?.refreshChips()
                        }
                    })
                }
            }]),
        ])
    }

    private func modelMenu() -> UIMenu {
        let byHarness = Dictionary(grouping: models, by: \.harness)
        var sections: [UIMenuElement] = byHarness.keys.sorted().map { h in
            UIMenu(title: byHarness[h]!.first!.harnessLabel, image: BrandMarks.image(for: h, side: 16), options: .displayInline, children: byHarness[h]!.map { m in
                UIAction(title: m.label, state: m.harness == draft.harness && m.id == draft.model ? .on : .off) { [weak self] _ in
                    self?.app.customAgents?.clearSelection()
                    self?.draft.harness = m.harness
                    self?.draft.model = m.id
                    self?.draft.effort = nil
                    self?.refreshChips()
                }
            })
        }
        if continuation == nil, let agents = app.customAgents {
            let snapshot = agents.snapshot()
            let personal = UIMenu(title: "Your agents", options: .displayInline, children: snapshot.rows.map { agent in
                UIAction(title: agent.name, subtitle: HarnessNames.label(agent.runtime), image: BrandMarks.image(for: "openagents", side: 16), state: snapshot.selected?.id == agent.id ? .on : .off) { [weak self] _ in
                    do { try agents.select(id: agent.id); self?.refreshChips() }
                    catch { self?.showAgentError(error.localizedDescription) }
                }
            })
            var actions: [UIMenuElement] = [UIAction(title: "Create agent…", image: UIImage(systemName: "plus")) { [weak self] _ in self?.openAgentForm(id: nil) }]
            if let lab = app.agentLab {
                actions.append(UIAction(title: "Agent Lab…", image: UIImage(systemName: "flask")) { [weak self] _ in
                    guard let self else { return }
                    self.present(UINavigationController(rootViewController: AgentFormController(app: self.app, lab: lab)), animated: true)
                })
            }
            if let selected = snapshot.selected {
                actions.append(UIAction(title: "Edit agent…", image: UIImage(systemName: "pencil")) { [weak self] _ in self?.openAgentForm(id: selected.id) })
                actions.append(UIAction(title: "Agent card…", image: UIImage(systemName: "person.text.rectangle")) { [weak self] _ in
                    guard let self else { return }
                    self.present(UINavigationController(rootViewController: AgentCardController(agents: agents, agentId: selected.id)), animated: true)
                })
            }
            sections.insert(personal, at: 0)
            sections.insert(UIMenu(options: .displayInline, children: actions), at: 0)
        }
        return UIMenu(title: "Agents and models", children: sections)
    }

    private func openAgentForm(id: String?) {
        guard let agents = app.customAgents else { return }
        do {
            if let id { try agents.editForm(id: id) } else { try agents.createForm() }
            if let client = app.client { try agents.loadModels(core: client, deviceId: deviceId) }
            present(UINavigationController(rootViewController: AgentFormController(app: app, agents: agents)), animated: true)
        } catch { showAgentError(error.localizedDescription) }
    }

    private func showAgentError(_ message: String) {
        guard presentedViewController == nil else { return }
        let alert = UIAlertController(title: "Couldn't assign the task", message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default))
        present(alert, animated: true)
    }

    private func agentsChanged() {
        guard continuation == nil else { return }
        refreshChips()
        if let snapshot = app.customAgents?.snapshot() { composer.isUserInteractionEnabled = !snapshot.assigning }
        guard waitingForAgent, let snapshot = app.customAgents?.snapshot() else { return }
        if let chat = snapshot.assignmentChat, !created {
            created = true
            waitingForAgent = false
            app.newSessionText = ""
            app.newSessionImages = []
            onCreated(chat, DraftHandoff.capture(from: self, composer: composer, text: composer.text))
        } else if !snapshot.assigning {
            waitingForAgent = false
            hero.text = "What are we building?"
            if let notice = snapshot.notice { showAgentError(notice) }
        }
    }

    private func effortMenu() -> UIMenu {
        let model = models.first { $0.harness == draft.harness && $0.id == draft.model } ?? models.first { $0.harness == draft.harness }
        return UIMenu(title: "Reasoning effort", children: (model?.efforts ?? []).map { e in
            UIAction(title: e.capitalized, state: e == draft.effort ? .on : .off) { [weak self] _ in
                self?.draft.effort = e
                self?.refreshChips()
            }
        })
    }

    /// False when the session couldn't be created (the prompt stays put).
    private func create(text: String, images: [StagedImage]) -> Bool {
        app.lastDraft = draft
        if onOpenAgents {
            guard images.isEmpty else {
                showAgentError("We can't read images here yet. Send the text, or pick a computer under Run on.")
                return false
            }
            created = true
            app.newSessionText = ""
            app.newSessionImages = []
            return CloudChatViewController.start(text, from: self, app: app)
        }
        if let continuation {
            guard !creatingContinuation else { return false }
            creatingContinuation = true
            composer.isUserInteractionEnabled = false
            Task { @MainActor [weak self] in
                guard let self else { return }
                defer { self.creatingContinuation = false; self.composer.isUserInteractionEnabled = true }
                do {
                    guard let id = try await self.app.createContinuation(continuation, draft: self.draft, text: text, images: images) else { return }
                    self.created = true
                    self.app.newSessionText = ""
                    self.app.newSessionImages = []
                    self.onCreated(id, DraftHandoff.capture(from: self, composer: self.composer, text: text))
                } catch { self.showAgentError(error.localizedDescription) }
            }
            return false
        }
        if let agents = app.customAgents, agents.selectionActive() {
            guard agents.snapshot().selected != nil else { showAgentError("Choose an available agent."); return false }
            guard !waitingForAgent, let core = app.client else { return false }
            guard images.isEmpty else { showAgentError("Send the text first, then add images in the conversation."); return false }
            do {
                try agents.assign(core: core, host: deviceId, project: project?.id, task: text)
                waitingForAgent = true
                hero.text = "Waiting for your computer…"
            } catch { showAgentError(error.localizedDescription) }
            return false
        }
        guard let chatId = app.createSession(draft: draft, text: text, images: images) else {
            let alert = UIAlertController(title: "Couldn't start the session", message: "Choose a project or a host that can run it.", preferredStyle: .alert)
            alert.addAction(UIAlertAction(title: "OK", style: .default))
            present(alert, animated: true)
            return false
        }
        created = true
        PushNotifications.shared.askAfterFirstSession()
        app.newSessionText = ""
        app.newSessionImages = []
        // Lift the draft out (page + composer + typed text) so the chat can
        // take over in one motion.
        onCreated(chatId, DraftHandoff.capture(from: self, composer: composer, text: text))
        return true
    }
}

enum HarnessNames {
    /// The core's harness catalog label (same table as desktop).
    static func label(_ id: String) -> String { harnessLabel(harness: id) }
}
