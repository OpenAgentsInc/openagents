import UIKit

/// Native lists and text controls over the shared Rust history reader.
class SavedHistoryController: UITableViewController {
    enum Page { case projects, download }
    enum Row {
        case note(String, String)
        case body(String)
        case button(String, String, () -> Void)
    }
    let app: AppModel
    let page: Page
    private var token: AnyObject?
    private var rows: [Row] = []
    private var part: UInt64 = 0

    init(app: AppModel, page: Page) {
        self.app = app
        self.page = page
        super.init(style: .insetGrouped)
    }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        tableView.backgroundColor = Palette.background
        tableView.accessibilityIdentifier = "saved-history-page"
        tableView.estimatedRowHeight = 76
        token = app.observe { [weak self] in self?.reload() }
        reload()
    }

    private func perform(_ action: SavedHistoryAction) {
        do { try app.savedHistory?.action(action: action) }
        catch { Toast.show("Couldn’t load this saved content. Try again.", in: view.window) }
    }

    private func action(_ title: String, _ id: String, _ value: SavedHistoryAction) -> Row {
        .button(title, id, { [weak self] in self?.perform(value) })
    }

    private func stateRows(_ state: SavedLoadState) -> [Row] {
        switch state {
        case .loading: return [.note("Loading…", "saved-loading")]
        case let .failed(message, stale):
            return [.note((stale ? "Couldn’t refresh. Showing saved results.\n" : "") + message, "saved-error")]
        default: return []
        }
    }

    private func attachments(_ files: [SavedAttachment]) -> [Row] {
        files.map { file in
            guard file.available else { return .note(file.name + " · Not saved", "saved-file-unavailable") }
            return .button(file.name, "saved-file-" + file.key, { [weak self] in
                guard let self else { return }
                do {
                    try self.app.savedHistory?.action(action: .download(key: file.key))
                    self.navigationController?.pushViewController(SavedHistoryController(app: self.app, page: .download), animated: true)
                } catch { Toast.show("Couldn’t open this file. Try again.", in: self.view.window) }
            })
        }
    }

    private func parts(_ current: UInt64, _ count: UInt64) -> [Row] {
        guard count > 1 else { return [] }
        var result: [Row] = [.note("Part \(current + 1) of \(count)", "saved-part-count")]
        if current > 0 { result.append(.button("Previous part", "saved-part-previous", { [weak self] in self?.part = current - 1; self?.reload() })) }
        if current + 1 < count { result.append(.button("Next part", "saved-part-next", { [weak self] in self?.part = current + 1; self?.reload() })) }
        return result
    }

    private func reload() {
        rows = []
        guard let state = app.savedSnapshot, state.signedIn else {
            title = "OpenAgents chats"
            rows = [.note("Sign in to see your chats.", "saved-signed-out")]
            tableView.reloadData()
            return
        }
        switch page {
        case .projects:
            title = "Shared projects"
            rows.append(.button("Project invitations", "saved-project-invitations", { [weak self] in
                self?.app.openProjectInvitations()
            }))
            rows.append(action("Refresh projects", "saved-project-refresh", .refreshProjects))
            rows.append(.button("My chats", "saved-project-owned", { [weak self] in
                self?.perform(.selectProject(id: nil)); self?.navigationController?.popViewController(animated: true)
            }))
            for project in state.projects {
                rows.append(.button(project.name, "saved-project-" + project.id, { [weak self] in
                    self?.perform(.selectProject(id: project.id)); self?.navigationController?.popViewController(animated: true)
                }))
            }
            rows += stateRows(state.projectsState)
            if case .failed = state.projectsState { rows.append(action("Retry projects", "saved-project-retry", .refreshProjects)) }
            else if state.moreProjects { rows.append(action("More projects", "saved-project-more", .moreProjects)) }
            if state.projects.isEmpty, case .ready = state.projectsState { rows.append(.note("No shared projects yet.", "saved-project-empty")) }
        case .download:
            title = state.transcript.downloadName ?? "Saved file"
            rows += stateRows(state.transcript.downloadState)
            if case .ready = state.transcript.downloadState {
                if let file = try? app.savedHistory?.downloadedFilePart(part: part) {
                    part = file.part
                    if let text = file.text { rows.append(.body(text)); rows += parts(file.part, file.parts) }
                }
                rows.append(.button("Share file", "saved-file-share", { [weak self] in
                    guard let self, let file = try? self.app.savedHistory?.downloadedFile() else { return }
                    let share = UIActivityViewController(activityItems: [file.bytes], applicationActivities: nil)
                    share.popoverPresentationController?.sourceView = self.view
                    self.present(share, animated: true)
                }))
            }
        }
        tableView.reloadData()

    }

    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { rows.count }

    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let row = rows[indexPath.row]
        if case let .body(text) = row {
            let cell = UITableViewCell(style: .default, reuseIdentifier: nil)
            let body = UITextView()
            body.text = text
            body.isEditable = false
            body.isSelectable = true
            body.dataDetectorTypes = []
            body.font = UIFont.preferredFont(forTextStyle: .body)
            body.adjustsFontForContentSizeCategory = true
            body.textColor = Palette.text
            body.backgroundColor = .clear
            body.accessibilityIdentifier = "saved-message-body"
            body.translatesAutoresizingMaskIntoConstraints = false
            cell.contentView.addSubview(body)
            NSLayoutConstraint.activate([
                body.topAnchor.constraint(equalTo: cell.contentView.topAnchor, constant: 12),
                body.leadingAnchor.constraint(equalTo: cell.contentView.leadingAnchor, constant: 12),
                body.trailingAnchor.constraint(equalTo: cell.contentView.trailingAnchor, constant: -12),
                body.bottomAnchor.constraint(equalTo: cell.contentView.bottomAnchor, constant: -12),
                body.heightAnchor.constraint(equalToConstant: max(320, view.bounds.height * 0.65)),
            ])
            cell.selectionStyle = .none
            return cell
        }
        let cell = UITableViewCell(style: .default, reuseIdentifier: nil)
        var content = UIListContentConfiguration.cell()
        switch row {
        case let .note(text, id):
            content.text = text; content.textProperties.color = Palette.secondary
            content.textProperties.numberOfLines = 0
            cell.accessibilityIdentifier = id; cell.selectionStyle = .none
        case let .button(text, id, _):
            content.text = text; content.textProperties.color = Palette.text
            content.textProperties.numberOfLines = 3
            cell.accessibilityIdentifier = id; cell.accessoryType = .disclosureIndicator
        case .body: break
        }
        cell.contentConfiguration = content
        return cell
    }

    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: true)
        if case let .button(_, _, run) = rows[indexPath.row] { run() }
    }
}

final class SavedProjectsController: SavedHistoryController {
    init(app: AppModel) { super.init(app: app, page: .projects) }
    required init?(coder: NSCoder) { fatalError() }
}

/// Native presentation over the shared Rust transcript and account history state.
final class SavedTranscriptController: UIViewController {
    private let app: AppModel
    private lazy var relay = FrameRelay { [weak self] in self?.applyFrame() }
    private lazy var engine = TranscriptView(text: TextEngine.shared, listener: relay)
    private lazy var list = TranscriptListView(engine: engine)
    private let notice = UILabel()
    private let composer = ComposerBar()
    private let retrySend = UIButton(type: .system)
    private var accepted: UInt64 = 0
    private var shownChat: String?
    private var submitted = ""
    private let status = UIButton(type: .system)
    private let images = NSCache<NSString, UIImage>()
    private let labs = LabCells()
    private var token: AnyObject?

    init(app: AppModel) {
        self.app = app
        super.init(nibName: nil, bundle: nil)
        hidesBottomBarWhenPushed = true
    }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = Palette.background
        list.accessibilityIdentifier = "transcript"
        list.onDistanceFromBottom = { [weak self] _ in
            guard let self, self.list.isDragging || self.list.isDecelerating,
                  self.list.contentOffset.y < 160,
                  let transcript = self.app.savedSnapshot?.transcript,
                  transcript.hasMore, case .ready = transcript.state else { return }
            try? self.app.savedHistory?.action(action: .moreMessages)
        }
        labs.engine = engine
        list.labCells = labs
        images.countLimit = 16
        images.totalCostLimit = 64 * 1024 * 1024
        list.imageLoader = { [weak self] path, imageView in
            imageView.layer.setValue(path, forKey: "savedPath")
            if let image = self?.images.object(forKey: path as NSString) { imageView.image = image; return }
            Task { [weak self, weak imageView] in
                guard let bytes = try? await self?.app.savedHistory?.attachmentBytes(path: path),
                      let image = UIImage(data: bytes),
                      imageView?.layer.value(forKey: "savedPath") as? String == path else { return }
                self?.images.setObject(image, forKey: path as NSString, cost: Int(image.size.width * image.size.height * image.scale * image.scale * 4))
                imageView?.image = image
            }
        }
        list.imageFetcher = { [weak self] path in
            guard let bytes = try? await self?.app.savedHistory?.attachmentBytes(path: path) else { return nil }
            return UIImage(data: bytes)
        }
        list.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(list)
        setContentScrollView(list, for: .top)
        list.topEdgeEffect.style = .soft
        list.bottomEdgeEffect.isHidden = true
        composer.translatesAutoresizingMaskIntoConstraints = false
        composer.useCloudControls()
        composer.onSend = { [weak self] text, _, _ in
            guard let self, let transcript = self.app.savedSnapshot?.transcript,
                  transcript.canReply, !transcript.answering, !transcript.retrySend else { return .kept }
            self.submitted = text
            try? self.app.savedHistory?.action(action: .send(text: text))
            return .kept
        }
        composer.onStop = { [weak self] in try? self?.app.savedHistory?.action(action: .stopAnswer) }
        composer.onHeightChange = { [weak self] in self?.view.setNeedsLayout() }
        view.addSubview(composer)
        retrySend.translatesAutoresizingMaskIntoConstraints = false
        retrySend.setTitle("Retry", for: .normal)
        retrySend.accessibilityIdentifier = "cloud-retry"
        retrySend.addAction(UIAction { [weak self] _ in try? self?.app.savedHistory?.action(action: .retrySend) }, for: .touchUpInside)
        view.addSubview(retrySend)
        notice.text = "Use Runs on to open another part of this chat."
        notice.font = .preferredFont(forTextStyle: .footnote)
        notice.textColor = Palette.secondary
        notice.numberOfLines = 0
        notice.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(notice)
        status.translatesAutoresizingMaskIntoConstraints = false
        status.addAction(UIAction { [weak self] _ in self?.retry() }, for: .touchUpInside)
        view.addSubview(status)
        NSLayoutConstraint.activate([
            list.topAnchor.constraint(equalTo: view.topAnchor),
            list.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            list.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            list.bottomAnchor.constraint(equalTo: status.topAnchor),
            status.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 24),
            status.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -24),
            status.bottomAnchor.constraint(equalTo: notice.topAnchor, constant: -8),
            notice.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 24),
            notice.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -24),
            notice.bottomAnchor.constraint(equalTo: retrySend.topAnchor, constant: -4),
            retrySend.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 24),
            retrySend.bottomAnchor.constraint(equalTo: composer.topAnchor, constant: -4),
            composer.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 12),
            composer.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -12),
            composer.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor, constant: -8),
        ])
        token = app.observe { [weak self] in self?.reload() }
        reload()
    }
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        drawerShell?.setAccessoryVisible(false, animated: animated)
    }
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        drawerShell?.syncAccessory()
        app.savedHistory?.follow(visible: true)
    }
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        app.savedHistory?.follow(visible: false)
        if isMovingFromParent, let tabs = drawerShell {
            tabs.setAccessoryVisible(true, animated: animated)
            transitionCoordinator?.animate(alongsideTransition: nil) { _ in tabs.syncAccessory() }
        }
    }
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        if isMovingFromParent || navigationController == nil { labs.clear(); engine.close() }
    }
    private func applyFrame() { list.apply(engine.frame()) }
    private func reload() {
        guard let snapshot = app.savedSnapshot, snapshot.signedIn else {
            labs.bind(nil)
            engine.clear()
            images.removeAllObjects()
            title = "Chat"
            notice.text = "Sign in to open this chat."
            composer.isHidden = true
            retrySend.isHidden = true
            status.isHidden = true
            return
        }
        labs.bind(app.savedHistory)
        let transcript = snapshot.transcript
        if shownChat != transcript.chatId {
            shownChat = transcript.chatId
            accepted = 0
            submitted = ""
            composer.text = ""
            images.removeAllObjects()
        }
        title = transcript.title ?? "Chat"
        composer.isHidden = !transcript.canReply
        composer.running = transcript.answering
        composer.isUserInteractionEnabled = !transcript.stopping
        retrySend.isHidden = !transcript.retrySend
        notice.text = transcript.sendError ?? (transcript.canReply ? nil : "Use Runs on to open another part of this chat.")
        if transcript.accepted > accepted, composer.text == submitted { composer.text = "" }
        accepted = transcript.accepted
        list.accessibilityValue = "\(transcript.messages.count) messages loaded"
        let files = app.savedHistory?.attachments() ?? []
        navigationItem.rightBarButtonItem = files.isEmpty ? nil : UIBarButtonItem(
            image: UIImage(systemName: "paperclip"),
            menu: UIMenu(children: files.map { file in
                UIAction(title: file.name, attributes: file.available ? [] : [.disabled]) { [weak self] _ in
                    guard let self else { return }
                    try? self.app.savedHistory?.action(action: .download(key: file.key))
                    self.navigationController?.pushViewController(SavedHistoryController(app: self.app, page: .download), animated: true)
                }
            })
        )
        navigationItem.rightBarButtonItem?.accessibilityIdentifier = "saved-chat-files"
        let filesItem = navigationItem.rightBarButtonItem
        let homeItem = UIBarButtonItem(title: "Runs on", primaryAction: UIAction { [weak self] _ in
            guard let self, let client = self.app.client else { return }
            guard self.composer.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                Toast.show("Send or clear your draft before moving this chat.", in: self.view.window)
                return
            }
            do {
                if let homes = try self.app.savedHistory?.chatHomes(client: client) {
                    self.present(UINavigationController(rootViewController: ChatHomesController(app: self.app, homes: homes, host: self)), animated: true)
                }
            } catch { Toast.show(error.localizedDescription, in: self.view.window) }
        })
        navigationItem.rightBarButtonItems = [homeItem] + (filesItem.map { [$0] } ?? [])

        try? app.savedHistory?.renderTranscript(view: engine)
        let label: String?
        if transcript.loginRequired { label = "Sign in again" }
        else if transcript.accessLost || transcript.reloadRequired { label = "Reload chat" }
        else if transcript.smallerPage { label = "Load one message" }
        else if transcript.continueRequired { label = "Continue reading" }
        else if case let .failed(message, _) = transcript.state { label = message + " · Retry" }
        else if transcript.hasMore { label = "Load earlier messages" }
        else { label = nil }
        status.accessibilityIdentifier = "saved-messages-more"
        status.setTitle(label, for: .normal)
        status.isHidden = label == nil
    }
    private func retry() {
        guard let transcript = app.savedSnapshot?.transcript else { return }
        if transcript.loginRequired { app.restoreAccount(); return }
        let action: SavedHistoryAction
        if transcript.reloadRequired || transcript.accessLost { action = .refreshMessages }
        else if transcript.smallerPage { action = .smallerPage }
        else if transcript.continueRequired { action = .continueReading }
        else { action = .moreMessages }
        try? app.savedHistory?.action(action: action)
    }
}
