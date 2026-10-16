import UIKit

/// Native lists and text controls over the shared Rust history reader.
class SavedHistoryController: UITableViewController {
    enum Page { case projects, transcript, message(String), download }
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
    private var messageCount: UInt64 = 0

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

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        if case .transcript = page { app.savedHistory?.follow(visible: true) }
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        if case .transcript = page { app.savedHistory?.follow(visible: false) }
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
        let nearBottom = tableView.contentSize.height - tableView.contentOffset.y - tableView.bounds.height < 120
        var followNewMessages = false
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
        case .transcript:
            let transcript = state.transcript
            followNewMessages = !transcript.hasMore && transcript.received > messageCount && (nearBottom || messageCount == 0)
            messageCount = transcript.received
            tableView.accessibilityValue = "\(transcript.received) messages loaded"
            title = transcript.title ?? "Saved chat"
            navigationItem.prompt = "Saved chat"
            navigationItem.rightBarButtonItem = UIBarButtonItem(title: "Refresh", primaryAction: UIAction { [weak self] _ in self?.perform(.refreshMessages) })
            rows.append(.note("\(transcript.received) messages loaded", "saved-message-count"))
            if transcript.loginRequired { rows.append(.button("Sign in again", "saved-login", { [weak self] in self?.app.restoreAccount() })) }
            else if transcript.reloadRequired || transcript.accessLost { rows.append(action("Reload chat", "saved-reload", .refreshMessages)) }
            else if transcript.continueRequired { rows.append(action("Continue reading", "saved-continue", .continueReading)) }
            else if transcript.smallerPage { rows.append(action("Load smaller page", "saved-smaller", .smallerPage)) }
            else if transcript.hasMore, case .loading = transcript.state { /* The current page is still loading. */ }
            else if transcript.hasMore { rows.append(action("Load more messages", "saved-messages-more", .moreMessages)) }
            for message in transcript.messages {
                rows.append(.button(message.role + "\n" + message.preview, "saved-message-" + message.id, { [weak self] in
                    guard let self else { return }
                    self.navigationController?.pushViewController(SavedHistoryController(app: self.app, page: .message(message.id)), animated: true)
                }))
            }
            rows += attachments(transcript.captures)
            rows += stateRows(transcript.state)

        case let .message(id):
            navigationItem.prompt = "Saved chat"
            do {
                guard let value = try app.savedHistory?.message(id: id, part: part) else { break }
                title = value.role
                part = value.part
                rows.append(.body(value.text))
                rows += parts(value.part, value.parts)
                rows += attachments(value.files)
            } catch { rows.append(.note("This message is no longer available. Return to the chat and refresh.", "saved-message-unavailable")) }
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
        if followNewMessages, !rows.isEmpty {
            tableView.layoutIfNeeded()
            tableView.scrollToRow(at: IndexPath(row: rows.count - 1, section: 0), at: .bottom, animated: false)
        }
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

final class SavedTranscriptController: SavedHistoryController {
    init(app: AppModel) { super.init(app: app, page: .transcript) }
    required init?(coder: NSCoder) { fatalError() }
}
