import UIKit

/// Shared list machinery for every session list (front page, folders,
/// search): diffable by id, reconfigure-in-place on content
/// changes, fixed row heights, swipe + context actions.
class SessionListController: UIViewController, UICollectionViewDelegate {
    enum Item: Hashable {
        case folder(String)
        case session(String)
        case header(String)
        case savedChat(String)
        case savedProject(String)
        case savedControl(SavedControl)
    }

    enum SavedControl: Hashable { case refresh, archived, projects, invitations, more, myChats, login, notice }
    var showsSavedChats: Bool { false }
    private var savedRows: [String: SavedChatRow] = [:]
    private var savedProjects: [String: SavedProjectRow] = [:]

    let app: AppModel
    var collectionView: UICollectionView!
    var dataSource: UICollectionViewDiffableDataSource<String, Item>!
    private(set) var sessions: [String: SessionRowVM] = [:]
    private(set) var folders: [String: FolderRowVM] = [:]
    private var headers: [String: String] = [:]
    private var token: AnyObject?
    /// Show drag handles in edit mode (Pinned).
    var reorderable = false
    /// The Archived list: rows offer Unarchive instead of Archive / Pin / Move.
    var archivedRows = false
    /// The session open beside this list (iPad sidebar): drawn as current.
    var currentChatId: String? {
        didSet {
            guard oldValue != currentChatId, dataSource != nil else { return }
            var s = dataSource.snapshot()
            let ids = Set([oldValue, currentChatId].compactMap { $0 })
            s.reconfigureItems(s.itemIdentifiers.filter { if case let .session(id) = $0 { return ids.contains(id) } else { return false } })
            dataSource.apply(s, animatingDifferences: false)
        }
    }
    /// Headers are disclosure rows that fold their section (front page).
    var collapsible = false
    /// Long-press drag rearranges sessions (front page): reorder Pinned, or
    /// move a session into Pinned, a section, or Recent.
    var dragRearranges: Bool { false }
    /// While a session is being dragged: an empty Pinned shows as a target.
    private(set) var showsDropTargets = false
    private(set) var collapsed = Set(UserDefaults.standard.stringArray(forKey: "collapsedSections") ?? [])
    private var headerStates: [String: SectionHeaderCell.State] = [:]

    init(app: AppModel) {
        self.app = app
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = Palette.background
        var config = UICollectionLayoutListConfiguration(appearance: .plain)
        config.backgroundColor = .clear
        config.showsSeparators = false
        config.leadingSwipeActionsConfigurationProvider = { [weak self] path in self?.leadingSwipe(path) }
        config.trailingSwipeActionsConfigurationProvider = { [weak self] path in self?.trailingSwipe(path) }
        let layout = UICollectionViewCompositionalLayout.list(using: config)
        collectionView = UICollectionView(frame: view.bounds, collectionViewLayout: layout)
        collectionView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        collectionView.backgroundColor = .clear
        collectionView.delegate = self
        collectionView.dragDelegate = self
        collectionView.dropDelegate = self
        collectionView.dragInteractionEnabled = true
        view.addSubview(collectionView)

        let sessionReg = UICollectionView.CellRegistration<SessionCell, String> { [weak self] cell, path, id in
            guard let self, let vm = self.sessions[id] else { return }
            cell.configure(vm)
            cell.isCurrent = id == self.currentChatId
            cell.accessories = self.reorderable ? [.reorder(displayed: .whenEditing)] : []
        }
        let savedReg = UICollectionView.CellRegistration<UICollectionViewListCell, String> { [weak self] cell, _, id in
            guard let row = self?.savedRows[id] else { return }
            var c = UIListContentConfiguration.subtitleCell()
            c.text = row.title
            c.secondaryText = row.group + " · " + row.subtitle
            c.textProperties.numberOfLines = 1
            c.textProperties.color = Palette.text
            c.secondaryTextProperties.color = Palette.secondary
            cell.contentConfiguration = c
            cell.accessories = [.disclosureIndicator()]
            cell.accessibilityIdentifier = "saved-chat-" + id
        }
        let savedProjectReg = UICollectionView.CellRegistration<UICollectionViewListCell, String> { [weak self] cell, _, id in
            guard let project = self?.savedProjects[id] else { return }
            var c = UIListContentConfiguration.cell()
            c.text = project.name
            c.image = UIImage(systemName: "folder")
            c.textProperties.color = Palette.text
            cell.contentConfiguration = c
            cell.accessories = [.disclosureIndicator()]
            cell.accessibilityIdentifier = "home-project-" + id
        }
        let savedControlReg = UICollectionView.CellRegistration<UICollectionViewListCell, SavedControl> { [weak self] cell, _, item in
            var c = UIListContentConfiguration.cell()
            c.text = self?.savedControlTitle(item)
            c.textProperties.color = item == .notice ? Palette.secondary : Palette.accent
            c.textProperties.numberOfLines = 0
            cell.contentConfiguration = c
            cell.accessories = []
            cell.accessibilityIdentifier = "saved-" + String(describing: item)
        }
        let folderReg = UICollectionView.CellRegistration<FolderCell, String> { [weak self] cell, _, id in
            guard let vm = self?.folders[id] else { return }
            cell.configure(vm)
        }
        let headerReg = UICollectionView.CellRegistration<UICollectionViewListCell, String> { [weak self] cell, _, id in
            var c = UIListContentConfiguration.plainHeader()
            c.text = self?.headers[id]
            c.textProperties.font = Fonts.ui(.sansSemibold, 13)
            c.textProperties.color = Palette.secondary
            c.directionalLayoutMargins = NSDirectionalEdgeInsets(top: 18, leading: 20, bottom: 6, trailing: 20)
            cell.contentConfiguration = c
            var bg = UIBackgroundConfiguration.clear()
            bg.backgroundColor = .clear
            cell.backgroundConfiguration = bg
        }
        let sectionReg = UICollectionView.CellRegistration<SectionHeaderCell, String> { [weak self] cell, _, id in
            guard let state = self?.headerStates[id] else { return }
            cell.configure(state)
        }
        dataSource = UICollectionViewDiffableDataSource(collectionView: collectionView) { [weak self] cv, path, item in
            switch item {
            case let .session(id): cv.dequeueConfiguredReusableCell(using: sessionReg, for: path, item: id)
            case let .savedChat(id): cv.dequeueConfiguredReusableCell(using: savedReg, for: path, item: id)
            case let .savedProject(id): cv.dequeueConfiguredReusableCell(using: savedProjectReg, for: path, item: id)
            case let .savedControl(item): cv.dequeueConfiguredReusableCell(using: savedControlReg, for: path, item: item)
            case let .folder(id): cv.dequeueConfiguredReusableCell(using: folderReg, for: path, item: id)
            case let .header(id) where self?.collapsible == true: cv.dequeueConfiguredReusableCell(using: sectionReg, for: path, item: id)
            case let .header(id): cv.dequeueConfiguredReusableCell(using: headerReg, for: path, item: id)
            }
        }
        token = app.observe { [weak self] in self?.reload(animated: true) }
        reload(animated: false)
        // Text size changes: fixed row heights change with the category.
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (self: SessionListController, _) in
            var s = self.dataSource.snapshot()
            s.reconfigureItems(s.itemIdentifiers)
            self.dataSource.apply(s, animatingDifferences: false)
            self.collectionView.collectionViewLayout.invalidateLayout()
        }
        // Pull to re-probe sync (health + room redial) when a network looks stale.
        // (The control is captured directly: a primary action's `sender` is
        // nil, so ending via `action.sender` left the spinner running forever.)
        let refresh = UIRefreshControl()
        refresh.addAction(UIAction { [weak self, weak refresh] _ in
            guard let self else { return refresh?.endRefreshing() ?? () }
            self.app.refresh { refresh?.endRefreshing() }
        }, for: .valueChanged)
        refresh.accessibilityIdentifier = "pull-to-refresh"
        collectionView.refreshControl = refresh
    }

    /// Subclasses build their sections from the app model.
    func buildSections() -> [(id: String, header: String?, folders: [FolderRowVM], sessions: [SessionRowVM])] { [] }

    func reload(animated: Bool) {
        let sections = buildSections()
        var snapshot = NSDiffableDataSourceSnapshot<String, Item>()
        var nextSessions: [String: SessionRowVM] = [:]
        var nextFolders: [String: FolderRowVM] = [:]
        var changed: [Item] = []
        for s in sections {
            snapshot.appendSections([s.id])
            var items: [Item] = []
            if let header = s.header {
                headers[s.id] = header
                items.append(.header(s.id))
                if collapsible {
                    let state = SectionHeaderCell.State(
                        id: s.id,
                        title: header,
                        count: s.sessions.count,
                        collapsed: collapsed.contains(s.id),
                        live: s.sessions.contains { $0.status == .working } ? .spinner : s.sessions.contains { $0.status == .awaiting } ? .dot(StatusTone.input) : nil
                    )
                    if let old = headerStates[s.id], old != state { changed.append(.header(s.id)) }
                    headerStates[s.id] = state
                    if state.collapsed {
                        snapshot.appendItems(items, toSection: s.id)
                        continue
                    }
                }
            }
            for f in s.folders {
                if folders[f.id] != nil, folders[f.id] != f { changed.append(.folder(f.id)) }
                nextFolders[f.id] = f
                items.append(.folder(f.id))
            }
            for r in s.sessions where nextSessions[r.id] == nil {
                if let old = sessions[r.id], old != r { changed.append(.session(r.id)) }
                nextSessions[r.id] = r
                items.append(.session(r.id))
            }
            snapshot.appendItems(items, toSection: s.id)
        }
        appendSavedChats(to: &snapshot)
        changed += snapshot.itemIdentifiers.filter { if case let .header(id) = $0, id == "oa-saved-history" || id == "oa-shared-projects" { return dataSource.snapshot().itemIdentifiers.contains($0) }; if case .savedProject = $0 { return dataSource.snapshot().itemIdentifiers.contains($0) }; if case .savedChat = $0 { return dataSource.snapshot().itemIdentifiers.contains($0) }; if case .savedControl = $0 { return dataSource.snapshot().itemIdentifiers.contains($0) }; return false }
        sessions = nextSessions
        folders = nextFolders
        let present = Set(snapshot.itemIdentifiers)
        snapshot.reconfigureItems(changed.filter { present.contains($0) })
        dataSource.apply(snapshot, animatingDifferences: animated && view.window != nil)
    }

    /// Subclasses follow the scroll (the front page fades its wallpaper).
    func listDidScroll(_ scrollView: UIScrollView) {}

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        listDidScroll(scrollView)
    }

    private func appendSavedChats(to snapshot: inout NSDiffableDataSourceSnapshot<String, Item>) {
        savedRows = [:]
        savedProjects = [:]
        guard showsSavedChats, let state = app.savedSnapshot, state.signedIn else { return }
        let projectsSection = "oa-shared-projects"
        headers[projectsSection] = "Shared projects"
        headerStates[projectsSection] = .init(id: projectsSection, title: "Shared projects", count: state.projects.count, collapsed: collapsed.contains(projectsSection), live: nil)
        snapshot.appendSections([projectsSection])
        snapshot.appendItems([.header(projectsSection)], toSection: projectsSection)
        if !collapsed.contains(projectsSection) {
            savedProjects = Dictionary(uniqueKeysWithValues: state.projects.map { ($0.id, $0) })
            snapshot.appendItems(state.projects.map { .savedProject($0.id) }, toSection: projectsSection)
            snapshot.appendItems([.savedControl(.invitations)], toSection: projectsSection)
        }
        let section = "oa-saved-history"
        snapshot.appendSections([section])
        headers[section] = state.project ?? "OpenAgents chats"
        headerStates[section] = .init(id: section, title: state.project ?? "OpenAgents chats", count: state.rows.count, collapsed: collapsed.contains(section), live: nil)
        snapshot.appendItems([.header(section)], toSection: section)
        guard !collapsed.contains(section) else { return }
        var controls: [Item] = [.savedControl(.refresh), .savedControl(.archived), .savedControl(.projects)]
        if state.hasMore { controls.append(.savedControl(.more)) }
        if state.project != nil { controls.append(.savedControl(.myChats)) }
        if state.loginRequired { controls.append(.savedControl(.login)) }
        snapshot.appendItems(controls, toSection: section)
        savedRows = Dictionary(uniqueKeysWithValues: state.rows.map { ($0.id, $0) })
        snapshot.appendItems(state.rows.map { .savedChat($0.id) }, toSection: section)
        if case .loading = state.state { snapshot.appendItems([.savedControl(.notice)], toSection: section) }
        else if case .failed = state.state { snapshot.appendItems([.savedControl(.notice)], toSection: section) }
        else if state.rows.isEmpty { snapshot.appendItems([.savedControl(.notice)], toSection: section) }

    }

    private func savedControlTitle(_ item: SavedControl) -> String {
        switch item {
        case .refresh: return "Refresh chats"
        case .archived: return app.savedSnapshot?.archived == true ? "Active chats" : "Archived chats"
        case .projects: return "All shared projects"
        case .invitations: return "Project invitations"
        case .more: return "Load more chats"
        case .myChats: return "My chats"
        case .login: return "Try again"
        case .notice:
            if let state = app.savedSnapshot?.state {
                switch state {
                case .loading: return "Loading chats…"
                case let .failed(message, stale): return (stale ? "Couldn’t refresh. Showing saved results.\n" : "") + message
                default: break
                }
            }
            return "No chats here yet."
        }
    }

    private func savedControl(_ item: SavedControl) {
        do {
            switch item {
            case .refresh: try app.savedHistory?.action(action: .refreshChats)
            case .archived: try app.savedHistory?.action(action: .toggleArchived)
            case .more: try app.savedHistory?.action(action: .moreChats)
            case .myChats: try app.savedHistory?.action(action: .selectProject(id: nil))
            case .projects: navigationController?.pushViewController(SavedProjectsController(app: app), animated: true)
            case .invitations: app.openProjectInvitations()
            case .login: app.restoreAccount()
            case .notice: break
            }
        } catch { Toast.show("Couldn’t load your chats. Try again.", in: view.window) }
    }

    // MARK: Navigation

    func collectionView(_ collectionView: UICollectionView, didSelectItemAt indexPath: IndexPath) {
        collectionView.deselectItem(at: indexPath, animated: true)
        switch dataSource.itemIdentifier(for: indexPath) {
        case let .session(id):
            openSession(id)
        case let .folder(id):
            openFolder(id)
        case let .savedProject(id):
            do {
                try app.savedHistory?.action(action: .selectProject(id: id))
                collapsed.remove("oa-saved-history")
                UserDefaults.standard.set(Array(collapsed), forKey: "collapsedSections")
                reload(animated: true)
            }
            catch { Toast.show("Couldn’t open this project. Try again.", in: view.window) }
        case let .savedChat(id):
            do {
                try app.savedHistory?.action(action: .open(id: id))
                if let shell = drawerShell {
                    shell.showPage(SavedTranscriptController(app: app))
                } else {
                    navigationController?.pushViewController(SavedTranscriptController(app: app), animated: true)
                }
            } catch { Toast.show("Couldn’t open this chat. Refresh and try again.", in: view.window) }
        case let .savedControl(control):
            savedControl(control)
        case let .header(id) where collapsible:
            toggleSection(id)
        default:
            break
        }
    }

    func collectionView(_ collectionView: UICollectionView, shouldSelectItemAt indexPath: IndexPath) -> Bool {
        if case .savedControl(.notice) = dataSource.itemIdentifier(for: indexPath) { return false }
        if case .header = dataSource.itemIdentifier(for: indexPath) { return collapsible }
        return true
    }

    func toggleSection(_ id: String) {
        if collapsed.remove(id) == nil { collapsed.insert(id) }
        UserDefaults.standard.set(Array(collapsed), forKey: "collapsedSections")
        UISelectionFeedbackGenerator().selectionChanged()
        reload(animated: true)
    }

    /// Long-press actions for a collapsible section header.
    func headerMenu(_ id: String) -> UIMenu? { nil }

    func openSession(_ id: String) {
        // iPad: sessions open beside the sidebar.
        if let split = splitViewController as? SplitRootController, !split.isCollapsed {
            split.openSession(id)
            return
        }
        // iPhone: the session takes the page beside the drawer.
        if let shell = drawerShell { return shell.openSession(id) }
        guard let session = SessionViewController(app: app, chatId: id) else {
            Toast.show("Couldn't open session. Try again.", in: view.window)
            return
        }
        navigationController?.pushViewController(session, animated: true)
    }

    func openFolder(_ id: String) {
        guard let f = folders[id] ?? app.frontPage.folders.first(where: { $0.id == id }) else { return }
        navigationController?.pushViewController(FolderViewController(app: app, folder: f), animated: true)
    }

    // MARK: Actions

    private func sessionId(_ path: IndexPath) -> String? {
        if case let .session(id) = dataSource.itemIdentifier(for: path) { return id }
        return nil
    }

    private func leadingSwipe(_ path: IndexPath) -> UISwipeActionsConfiguration? {
        guard !archivedRows, let id = sessionId(path), let vm = sessions[id] else { return nil }
        let pin = UIContextualAction(style: .normal, title: vm.pinned ? "Unpin" : "Pin") { [weak self] _, _, done in
            self?.app.setPinned(id, !vm.pinned)
            done(true)
        }
        pin.image = UIImage(systemName: vm.pinned ? "pin.slash.fill" : "pin.fill")
        pin.backgroundColor = Palette.accent
        return UISwipeActionsConfiguration(actions: [pin])
    }

    private func trailingSwipe(_ path: IndexPath) -> UISwipeActionsConfiguration? {
        guard let id = sessionId(path) else { return nil }
        if archivedRows {
            let restore = UIContextualAction(style: .normal, title: "Unarchive") { [weak self] _, _, done in
                self?.app.unarchive(id)
                done(true)
            }
            restore.image = UIImage(systemName: "tray.and.arrow.up.fill")
            restore.backgroundColor = Palette.accent
            return UISwipeActionsConfiguration(actions: [restore])
        }
        let archive = UIContextualAction(style: .destructive, title: "Archive") { [weak self] _, _, done in
            self?.app.archive(id)
            done(true)
        }
        archive.image = UIImage(systemName: "archivebox.fill")
        archive.backgroundColor = Palette.secondary
        let move = UIContextualAction(style: .normal, title: "Move") { [weak self] _, view, done in
            self?.presentMoveMenu(id, from: view)
            done(true)
        }
        move.image = UIImage(systemName: "folder.fill")
        move.backgroundColor = UIColor(hex: 0x5E6AD2)
        return UISwipeActionsConfiguration(actions: [archive, move])
    }

    func collectionView(_ collectionView: UICollectionView, contextMenuConfigurationForItemsAt indexPaths: [IndexPath], point: CGPoint) -> UIContextMenuConfiguration? {
        if let path = indexPaths.first, case let .header(id) = dataSource.itemIdentifier(for: path), let menu = headerMenu(id) {
            return UIContextMenuConfiguration(identifier: nil, previewProvider: nil) { _ in menu }
        }
        guard let path = indexPaths.first, let id = sessionId(path), let vm = sessions[id] else { return nil }
        return UIContextMenuConfiguration(identifier: id as NSString, previewProvider: nil) { [weak self] _ in
            guard let self else { return nil }
            if self.archivedRows {
                return UIMenu(children: [
                    UIAction(title: "Unarchive", image: UIImage(systemName: "tray.and.arrow.up")) { _ in self.app.unarchive(id) },
                    UIAction(title: "Rename…", image: UIImage(systemName: "pencil")) { _ in self.rename(id) },
                ])
            }
            return UIMenu(children: [
                UIAction(title: vm.pinned ? "Unpin" : "Pin", image: UIImage(systemName: vm.pinned ? "pin.slash" : "pin")) { _ in self.app.setPinned(id, !vm.pinned) },
                self.moveMenu(id),
                UIAction(title: "Rename…", image: UIImage(systemName: "pencil")) { _ in self.rename(id) },
                UIAction(title: "Archive", image: UIImage(systemName: "archivebox"), attributes: .destructive) { _ in self.app.archive(id) },
            ])
        }
    }

    func moveMenu(_ id: String) -> UIMenu {
        var items: [UIMenuElement] = app.frontPage.folders.filter { $0.id != "pinned" }.map { f in
            UIAction(title: f.name, image: UIImage(systemName: "folder")) { [weak self] _ in self?.app.move(id, toSection: f.id) }
        }
        items.append(UIAction(title: "No Section", image: UIImage(systemName: "tray")) { [weak self] _ in self?.app.move(id, toSection: nil) })
        items.append(UIAction(title: "New Section…", image: UIImage(systemName: "folder.badge.plus")) { [weak self] _ in
            self?.promptForSection { name in self?.app.createSection(name) }
        })
        return UIMenu(title: "Move to Section", image: UIImage(systemName: "folder"), children: items)
    }

    private func presentMoveMenu(_ id: String, from view: UIView) {
        let sheet = UIAlertController(title: "Move to Section", message: nil, preferredStyle: .actionSheet)
        for f in app.frontPage.folders where f.id != "pinned" {
            sheet.addAction(UIAlertAction(title: f.name, style: .default) { [weak self] _ in self?.app.move(id, toSection: f.id) })
        }
        sheet.addAction(UIAlertAction(title: "No Section", style: .default) { [weak self] _ in self?.app.move(id, toSection: nil) })
        sheet.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        sheet.popoverPresentationController?.sourceView = view
        present(sheet, animated: true)
    }

    func promptForSection(_ done: @escaping (String) -> Void) {
        let alert = UIAlertController(title: "New Section", message: nil, preferredStyle: .alert)
        alert.addTextField { $0.placeholder = "Name"; $0.autocapitalizationType = .words }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        alert.addAction(UIAlertAction(title: "Create", style: .default) { _ in
            let name = alert.textFields?.first?.text?.trimmingCharacters(in: .whitespaces) ?? ""
            if !name.isEmpty { done(name) }
        })
        present(alert, animated: true)
    }

    private func rename(_ id: String) {
        let alert = UIAlertController(title: "Rename Session", message: nil, preferredStyle: .alert)
        alert.addTextField { [weak self] in $0.text = self?.sessions[id]?.title }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        alert.addAction(UIAlertAction(title: "Rename", style: .default) { [weak self] _ in
            let title = alert.textFields?.first?.text?.trimmingCharacters(in: .whitespaces) ?? ""
            if !title.isEmpty { self?.app.rename(id, title) }
        })
        present(alert, animated: true)
    }
}

// MARK: Drag to rearrange

/// The desktop sidebar's drag: a session can be dropped among the pins (at
/// that place), into a section, or into Recent. Sections and Recent stay in
/// recency order, so only Pinned keeps the drop position.
extension SessionListController: UICollectionViewDragDelegate, UICollectionViewDropDelegate {
    private enum DropTarget { case pinned(after: String?, before: String?), section(String), recent }

    func collectionView(_ collectionView: UICollectionView, itemsForBeginning session: UIDragSession, at indexPath: IndexPath) -> [UIDragItem] {
        guard dragRearranges, !archivedRows, case let .session(id) = dataSource.itemIdentifier(for: indexPath) else { return [] }
        let item = UIDragItem(itemProvider: NSItemProvider(object: (sessions[id]?.title ?? "") as NSString))
        item.localObject = id
        return [item]
    }

    func collectionView(_ collectionView: UICollectionView, dragSessionIsRestrictedToDraggingApplication session: UIDragSession) -> Bool { true }

    func collectionView(_ collectionView: UICollectionView, dragPreviewParametersForItemAt indexPath: IndexPath) -> UIDragPreviewParameters? {
        guard let cell = collectionView.cellForItem(at: indexPath) else { return nil }
        let parameters = UIDragPreviewParameters()
        parameters.visiblePath = UIBezierPath(roundedRect: cell.bounds.insetBy(dx: 8, dy: 2), cornerRadius: 14)
        return parameters
    }

    func collectionView(_ collectionView: UICollectionView, dragSessionWillBegin session: UIDragSession) {
        showsDropTargets = true
        reload(animated: true)
    }

    func collectionView(_ collectionView: UICollectionView, dragSessionDidEnd session: UIDragSession) {
        showsDropTargets = false
        reload(animated: true)
    }

    func collectionView(_ collectionView: UICollectionView, canHandle session: UIDropSession) -> Bool {
        dragRearranges && session.localDragSession != nil
    }

    func collectionView(_ collectionView: UICollectionView, dropSessionDidUpdate session: UIDropSession, withDestinationIndexPath destinationIndexPath: IndexPath?) -> UICollectionViewDropProposal {
        guard let id = session.localDragSession?.items.first?.localObject as? String,
              dropTarget(for: id, session: session, at: destinationIndexPath) != nil
        else { return UICollectionViewDropProposal(operation: .forbidden) }
        // Over a section's header: into that section, highlighted.
        let intent: UICollectionViewDropProposal.Intent = headerUnder(session) != nil ? .insertIntoDestinationIndexPath : .insertAtDestinationIndexPath
        return UICollectionViewDropProposal(operation: .move, intent: intent)
    }

    func collectionView(_ collectionView: UICollectionView, performDropWith coordinator: UICollectionViewDropCoordinator) {
        guard let id = coordinator.items.first?.dragItem.localObject as? String,
              let vm = sessions[id],
              let target = dropTarget(for: id, session: coordinator.session, at: coordinator.destinationIndexPath)
        else { return }
        let section = app.frontPage.folders.first { $0.id != "pinned" && app.sessions(inFolder: $0.id).contains { $0.id == id } }?.id
        var changed = true
        switch target {
        case let .pinned(after, before):
            let pins = app.sessions(inFolder: "pinned").map(\.id)
            if vm.pinned, let i = pins.firstIndex(of: id),
               (i > 0 ? pins[i - 1] : nil) == after, (i + 1 < pins.count ? pins[i + 1] : nil) == before {
                changed = false
            } else {
                if !vm.pinned { app.setPinned(id, true) }
                app.movePin(id, after: after, before: before)
            }
        case let .section(sectionId):
            if vm.pinned { app.setPinned(id, false) }
            if vm.pinned || section != sectionId { app.move(id, toSection: sectionId) } else { changed = false }
        case .recent:
            if vm.pinned { app.setPinned(id, false) }
            if section != nil { app.move(id, toSection: nil) } else if !vm.pinned { changed = false }
        }
        if changed { UIImpactFeedbackGenerator(style: .light).impactOccurred() }
    }

    /// The section header under the finger, if any.
    private func headerUnder(_ session: UIDropSession) -> String? {
        guard let path = collectionView.indexPathForItem(at: session.location(in: collectionView)),
              case let .header(id) = dataSource.itemIdentifier(for: path) else { return nil }
        return id
    }

    /// Where a drop lands: on a header, into that section (Pinned: first);
    /// otherwise by the insertion point: Pinned (between two pins), a
    /// section, or Recent. Nil for anything else (search results, saved chats).
    private func dropTarget(for id: String, session: UIDropSession, at path: IndexPath?) -> DropTarget? {
        guard dragRearranges else { return nil }
        if let header = headerUnder(session) {
            switch header {
            case "pinned":
                return .pinned(after: nil, before: app.sessions(inFolder: "pinned").map(\.id).first { $0 != id })
            case "recent":
                return .recent
            default:
                return app.frontPage.folders.contains { $0.id == header } ? .section(header) : nil
            }
        }
        guard let path else { return nil }
        let snapshot = dataSource.snapshot()
        guard path.section < snapshot.numberOfSections else { return nil }
        let sectionId = snapshot.sectionIdentifiers[path.section]
        switch sectionId {
        case "pinned":
            let items = snapshot.itemIdentifiers(inSection: sectionId).filter { $0 != .session(id) }
            let lead = items.prefix { if case .session = $0 { return false } else { return true } }.count
            let pins = items.compactMap { item -> String? in if case let .session(pin) = item { return pin } else { return nil } }
            let at = max(0, min(path.item - lead, pins.count))
            return .pinned(after: at > 0 ? pins[at - 1] : nil, before: at < pins.count ? pins[at] : nil)
        case "recent":
            return .recent
        default:
            return app.frontPage.folders.contains { $0.id == sectionId } ? .section(sectionId) : nil
        }
    }
}

/// Front page, laid out like the desktop sidebar: Pinned and the user's
/// sections inline as foldable groups, then everything else under Recent.
/// Fold state persists; long-press a header for its actions.
class SessionsViewController: SessionListController {
    override var showsSavedChats: Bool { true }
    override var dragRearranges: Bool { query.trimmingCharacters(in: .whitespaces).isEmpty && !app.groupByProject }
    /// Search from the iPad sidebar: non-empty shows matches instead of sections.
    var query = "" { didSet { if query != oldValue { reload(animated: true) } } }

    /// The chat wallpaper behind the top of the front page. Fixed behind the
    /// list, fading out as the list scrolls up over it.
    private let wallpaper = WallpaperView()

    override func viewDidLoad() {
        collapsible = true
        super.viewDidLoad()
        let backdrop = UIView()
        backdrop.addSubview(wallpaper)
        collectionView.backgroundView = backdrop
        title = "Sessions"
        navigationItem.largeTitleDisplayMode = .always
        navigationItem.rightBarButtonItem = UIBarButtonItem(image: UIImage(systemName: "ellipsis"), menu: optionsMenu())
    }

    override func buildSections() -> [(id: String, header: String?, folders: [FolderRowVM], sessions: [SessionRowVM])] {
        if !query.trimmingCharacters(in: .whitespaces).isEmpty {
            return [("results", nil, [], app.search(query))]
        }
        var out: [(id: String, header: String?, folders: [FolderRowVM], sessions: [SessionRowVM])] = []
        for f in app.frontPage.folders {
            out.append((f.id, f.name, [], app.sessions(inFolder: f.id)))
        }
        // Dragging with nothing pinned yet: Pinned shows as a place to drop.
        if showsDropTargets, !out.contains(where: { $0.id == "pinned" }) {
            out.insert(("pinned", "Pinned", [], []), at: 0)
        }
        if app.groupByProject {
            for g in app.frontPage.projectGroups {
                out.append((g.id, g.name, [], g.sessions))
            }
            return out
        }
        out.append(("recent", out.isEmpty ? nil : "Recent", [], app.frontPage.sessions))
        return out
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        // Desktop hero: 72% of the viewport, at most 760pt.
        wallpaper.frame = CGRect(x: 0, y: 0, width: view.bounds.width, height: min(view.bounds.height * 0.72, 760))
    }

    override func listDidScroll(_ scrollView: UIScrollView) {
        let travel = scrollView.contentOffset.y + scrollView.adjustedContentInset.top
        wallpaper.scrollFade = 1 - max(0, travel) / max(1, wallpaper.bounds.height * 0.6)
    }

    override func headerMenu(_ id: String) -> UIMenu? {
        if id == "recent" { return nil }
        guard let folder = app.frontPage.folders.first(where: { $0.id == id }) else { return nil }
        let open = UIAction(title: "Open", image: UIImage(systemName: "arrow.up.right")) { [weak self] _ in self?.openFolder(id) }
        if id == "pinned" {
            return UIMenu(children: [
                open,
                UIAction(title: "Reorder…", image: UIImage(systemName: "arrow.up.arrow.down")) { [weak self] _ in self?.openFolder(id) },
            ])
        }
        return UIMenu(children: [
            open,
            UIAction(title: "Rename Section…", image: UIImage(systemName: "pencil")) { [weak self] _ in self?.renameSection(folder) },
            UIAction(title: "Delete Section", image: UIImage(systemName: "trash"), attributes: .destructive) { [weak self] _ in self?.app.deleteSection(id) },
        ])
    }

    private func renameSection(_ folder: FolderRowVM) {
        let alert = UIAlertController(title: "Rename Section", message: nil, preferredStyle: .alert)
        alert.addTextField { $0.text = folder.name }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        alert.addAction(UIAlertAction(title: "Rename", style: .default) { [weak self] _ in
            guard let name = alert.textFields?.first?.text?.trimmingCharacters(in: .whitespaces), !name.isEmpty else { return }
            self?.app.renameSection(folder.id, name)
        })
        present(alert, animated: true)
    }

    func optionsMenu() -> UIMenu {
        UIMenu(children: [
            UIAction(title: "New Section…", image: UIImage(systemName: "folder.badge.plus")) { [weak self] _ in
                self?.promptForSection { name in self?.app.createSection(name) }
            },
            UIAction(title: "Collapse All", image: UIImage(systemName: "rectangle.compress.vertical")) { [weak self] _ in
                guard let self else { return }
                var ids = self.app.frontPage.folders.map(\.id)
                if self.app.groupByProject { ids += self.app.frontPage.projectGroups.map(\.id) }
                for id in ids where !self.collapsed.contains(id) { self.toggleSection(id) }
            },
            // Rebuilt on each open: Settings flips it too.
            UIDeferredMenuElement.uncached { [weak self] done in
                guard let app = self?.app else { return done([]) }
                done([UIAction(title: "Group by Project", image: UIImage(systemName: "square.stack.3d.up"), state: app.groupByProject ? .on : .off) { _ in
                    app.groupByProject.toggle()
                }])
            },
            UIAction(title: "Archived", image: UIImage(systemName: "archivebox")) { [weak self] _ in
                guard let self else { return }
                self.navigationController?.pushViewController(FolderViewController(app: self.app, folder: FolderRowVM(id: "archived", name: "Archived", count: 0, symbol: "archivebox")), animated: true)
            },
        ])
    }
}

/// One folder's sessions (Pinned, a user section, Archived).
final class FolderViewController: SessionListController {
    private let folder: FolderRowVM

    init(app: AppModel, folder: FolderRowVM) {
        self.folder = folder
        super.init(app: app)
    }

    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        archivedRows = folder.id == "archived"
        super.viewDidLoad()
        title = folder.name
        navigationItem.largeTitleDisplayMode = .always
        if folder.id == "pinned" {
            // Pins are an ordered list: drag to reorder (synced to desktop).
            navigationItem.rightBarButtonItem = editButtonItem
            reorderable = true
            reload(animated: false)
            dataSource.reorderingHandlers.canReorderItem = { _ in true }
            dataSource.reorderingHandlers.didReorder = { [weak self] tx in self?.pinsReordered(tx) }
        } else if folder.id != "archived" {
            navigationItem.rightBarButtonItem = UIBarButtonItem(image: UIImage(systemName: "ellipsis"), menu: UIMenu(children: [
                UIAction(title: "Rename Section…", image: UIImage(systemName: "pencil")) { [weak self] _ in self?.renameSection() },
                UIAction(title: "Delete Section", image: UIImage(systemName: "trash"), attributes: .destructive) { [weak self] _ in self?.deleteSection() },
            ]))
        }
    }

    override func setEditing(_ editing: Bool, animated: Bool) {
        super.setEditing(editing, animated: animated)
        collectionView.isEditing = editing
    }

    override func buildSections() -> [(id: String, header: String?, folders: [FolderRowVM], sessions: [SessionRowVM])] {
        [("sessions", nil, [], app.sessions(inFolder: folder.id))]
    }

    private func pinsReordered(_ tx: NSDiffableDataSourceTransaction<String, Item>) {
        let ids = tx.finalSnapshot.itemIdentifiers.compactMap { item -> String? in
            if case let .session(id) = item { return id }
            return nil
        }
        for change in tx.difference.insertions {
            guard case let .insert(offset, item, _) = change, case let .session(id) = item else { continue }
            let after = offset > 0 ? ids[offset - 1] : nil
            let before = offset + 1 < ids.count ? ids[offset + 1] : nil
            app.movePin(id, after: after, before: before)
        }
    }

    private func renameSection() {
        let alert = UIAlertController(title: "Rename Section", message: nil, preferredStyle: .alert)
        alert.addTextField { [folder] in $0.text = folder.name }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        alert.addAction(UIAlertAction(title: "Rename", style: .default) { [weak self] _ in
            guard let self, let name = alert.textFields?.first?.text?.trimmingCharacters(in: .whitespaces), !name.isEmpty else { return }
            self.app.renameSection(self.folder.id, name)
            self.title = name
        })
        present(alert, animated: true)
    }

    private func deleteSection() {
        let alert = UIAlertController(title: "Delete “\(folder.name)”?", message: "Its sessions move back to the main list.", preferredStyle: .actionSheet)
        alert.addAction(UIAlertAction(title: "Delete Section", style: .destructive) { [weak self] _ in
            guard let self else { return }
            self.app.deleteSection(self.folder.id)
            self.navigationController?.popViewController(animated: true)
        })
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        // iPad presents action sheets as popovers: anchor it to the ⋯ button.
        alert.popoverPresentationController?.barButtonItem = navigationItem.rightBarButtonItem
        present(alert, animated: true)
    }
}

/// Search tab: live results as you type (matching runs in the core).
final class SearchViewController: SessionListController, UISearchResultsUpdating {
    private var query = ""

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Search"
        navigationItem.largeTitleDisplayMode = .always
        let search = UISearchController(searchResultsController: nil)
        search.searchResultsUpdater = self
        search.obscuresBackgroundDuringPresentation = false
        search.searchBar.placeholder = "Sessions, projects, branches"
        navigationItem.searchController = search
        navigationItem.hidesSearchBarWhenScrolling = false
    }

    func updateSearchResults(for searchController: UISearchController) {
        query = searchController.searchBar.text ?? ""
        reload(animated: true)
    }

    override func buildSections() -> [(id: String, header: String?, folders: [FolderRowVM], sessions: [SessionRowVM])] {
        [("results", nil, [], app.search(query))]
    }
}
