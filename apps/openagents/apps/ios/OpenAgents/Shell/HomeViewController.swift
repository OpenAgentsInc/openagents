import UIKit
import UniformTypeIdentifiers

/// Mounts shared Rust Native cards. Rust owns dashboard state and actions.
final class HomeViewController: UIViewController, UIDocumentPickerDelegate, UIDragInteractionDelegate, UIDropInteractionDelegate {
    private let app: AppModel
    private let scroll = UIScrollView()
    private let content = UIStackView()
    private var token: AnyObject?
    private var home: CoreHome?
    private var revision: UInt64 = 0
    private var submission: UInt64 = 0
    private var instance = ""
    private var screenKey = ""
    private var dragFrames: [String: (String, UInt64)] = [:]
    private var fieldDrafts: [String: String] = [:]
    private var isHome = true

    init(app: AppModel) { self.app = app; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError() }
    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Home"
        view.backgroundColor = Palette.background
        content.axis = .vertical
        content.spacing = 16
        content.translatesAutoresizingMaskIntoConstraints = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        let refresh = UIRefreshControl()
        refresh.addTarget(self, action: #selector(pullToRefresh(_:)), for: .valueChanged)
        scroll.refreshControl = refresh
        view.addSubview(scroll)
        scroll.addSubview(content)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor), scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: view.topAnchor), scroll.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            content.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 20),
            content.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -20),
            content.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 16),
            content.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -24),
            content.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -40),
        ])
        token = app.observe { [weak self] in self?.render() }
        NotificationCenter.default.addObserver(self, selector: #selector(becameActive), name: UIApplication.didBecomeActiveNotification, object: nil)
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self, UITraitUserInterfaceStyle.self]) { (self: HomeViewController, _: UITraitCollection) in
            self.revision = 0
            self.render()
        }
        render()
    }
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        render()
        home?.setVisible(visible: true, foreground: true)
        Task { await home?.refresh() }
    }
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        home?.setVisible(visible: false, foreground: UIApplication.shared.applicationState == .active)
    }
    @objc private func pullToRefresh(_ sender: UIRefreshControl) {
        Task { @MainActor in
            await home?.refresh()
            sender.endRefreshing()
        }
    }
    @objc private func becameActive() {
        guard viewIfLoaded?.window != nil else { return }
        home?.setVisible(visible: true, foreground: true)
        Task { await home?.refresh() }
    }
    private func render() {
        if home == nil {
            home = app.home
            if home == nil, app.isDemo {
                home = CoreHome.demo(dataDir: FileManager.default.temporaryDirectory.appendingPathComponent("OpenAgentsDemoHome").path, listener: HomeBridge(app: app))
                if let client = app.client { home?.updateClient(client: client) }
            }
        }
        guard let home, let json = try? home.snapshot(), let data = json.data(using: .utf8),
              let document = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let root = document["root"] as? [String: Any], let next = document["revision"] as? UInt64,
              let identity = document["instance"] as? String, next != revision || identity != instance else { return }
        let nextScreen = root["key"] as? String ?? ""
        let changedScreen = screenKey != nextScreen
        if changedScreen { fieldDrafts.removeAll(); screenKey = nextScreen }
        isHome = nextScreen.hasPrefix("home-Home-")
        configureNavigation(root, instance: identity, revision: next)
        revision = next
        instance = identity
        let offset = scroll.contentOffset
        dragFrames.removeAll()
        for child in content.arrangedSubviews { child.removeFromSuperview() }
        content.addArrangedSubview(mount(root, instance: identity, revision: next))
        scroll.layoutIfNeeded()
        scroll.setContentOffset(changedScreen ? CGPoint(x: 0, y: -scroll.adjustedContentInset.top) : offset, animated: false)
    }
    private func configureNavigation(_ root: [String: Any], instance: String, revision: UInt64) {
        navigationItem.largeTitleDisplayMode = isHome ? .always : .never
        navigationItem.leftBarButtonItem = nil
        navigationItem.rightBarButtonItems = nil
        let children = ((root["element"] as? [String: Any])?["props"] as? [String: Any])?["children"] as? [[String: Any]] ?? []
        if !isHome {
            if screenKey.hasPrefix("home-Edit-") {
                title = "Edit widgets"
                navigationItem.leftBarButtonItem = UIBarButtonItem(title: "Cancel", primaryAction: UIAction { [weak self] _ in
                    self?.activate("cancel", instance: instance, revision: revision)
                })
                navigationItem.leftBarButtonItem?.accessibilityIdentifier = "cancel"
                let done = UIBarButtonItem(title: "Done", primaryAction: UIAction { [weak self] _ in
                    self?.view.endEditing(true)
                    self?.activate("done", instance: instance, revision: revision)
                })
                done.accessibilityIdentifier = "done"
                let group = children.first { $0["key"] as? String == "edit-actions" }
                let actions = (((group?["element"] as? [String: Any])?["props"] as? [String: Any])?["children"] as? [[String: Any]] ?? []).compactMap { node -> UIAction? in
                    guard let key = node["key"] as? String, !["done", "cancel"].contains(key),
                          let element = node["element"] as? [String: Any], let props = element["props"] as? [String: Any],
                          let label = props["label"] as? String else { return nil }
                    return UIAction(title: label, attributes: key == "delete-dashboard" ? .destructive : []) { [weak self] _ in
                        self?.activate(key, instance: instance, revision: revision)
                    }
                }
                let options = UIBarButtonItem(image: UIImage(systemName: "ellipsis"), menu: UIMenu(children: actions))
                options.accessibilityLabel = "Dashboard options"
                navigationItem.rightBarButtonItems = [done, options]
            } else if screenKey.hasPrefix("home-Manage-") {
                title = "Dashboards"
                navigationItem.leftBarButtonItem = UIBarButtonItem(title: "Home", primaryAction: UIAction { [weak self] _ in
                    self?.activate("back-home", instance: instance, revision: revision)
                })
                navigationItem.leftBarButtonItem?.accessibilityIdentifier = "back-home"
            } else { title = "Dashboard" }
            return
        }
        let name = children.first { $0["key"] as? String == "dashboard-name" }
        title = (((name?["element"] as? [String: Any])?["props"] as? [String: Any])?["value"] as? String) ?? "Home"
        let sessions = UIBarButtonItem(title: "Sessions", primaryAction: UIAction { [weak self] _ in
            self?.router?.showSessions()
        })
        sessions.accessibilityIdentifier = "home-sessions"
        navigationItem.leftBarButtonItem = sessions
        navigationItem.rightBarButtonItems = [("manage", "ellipsis", "Manage dashboards"), ("edit", "slider.horizontal.3", "Edit widgets")].map { key, icon, label in
            let item = UIBarButtonItem(image: UIImage(systemName: icon), primaryAction: UIAction { [weak self] _ in
                self?.activate(key, instance: instance, revision: revision)
            })
            item.accessibilityIdentifier = key
            item.accessibilityLabel = label
            return item
        }
    }
    private func font(_ size: CGFloat, weight: FaceRole = .sans) -> UIFont {
        UIFontMetrics(forTextStyle: .body).scaledFont(for: Fonts.ui(weight, size))
    }
    private func mount(_ node: [String: Any], instance: String, revision: UInt64) -> UIView {
        let key = node["key"] as? String ?? ""
        let element = node["element"] as? [String: Any] ?? [:]
        let props = element["props"] as? [String: Any] ?? [:]
        let style = node["style"] as? [String: Any] ?? [:]
        let result: UIView
        switch element["kind"] as? String {
        case "text":
            let label = UILabel()
            label.text = props["value"] as? String
            label.numberOfLines = 0
            let heading = props["role"] as? String == "heading"
            label.font = font(CGFloat(style["text_size"] as? Int ?? (heading ? 17 : 13)), weight: heading ? .sansMedium : .sans)
            label.adjustsFontForContentSizeCategory = true
            label.textColor = heading && (style["text_size"] as? Int ?? 17) >= 15 ? Palette.text : Palette.secondary
            if heading { label.accessibilityTraits.insert(.header) }
            result = label
        case "button":
            let button = UIButton(type: .system)
            var config = UIButton.Configuration.plain()
            let lines = (props["label"] as? String ?? "").split(separator: "\n", maxSplits: 1).map(String.init)
            config.title = lines.first
            config.baseForegroundColor = Palette.text
            config.contentInsets = NSDirectionalEdgeInsets(top: 8, leading: 0, bottom: 8, trailing: 0)
            config.titleAlignment = .leading
            config.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { [weak self] incoming in
                var attributes = incoming
                attributes.font = self?.font(CGFloat(style["text_size"] as? Int ?? 15), weight: .sansMedium)
                return attributes
            }
            if lines.count > 1 {
                config.subtitle = lines[1]
                config.titlePadding = 5
                config.contentInsets.trailing = 24
                let disclosure = UIImageView(image: UIImage(systemName: "chevron.right", withConfiguration: UIImage.SymbolConfiguration(pointSize: 11, weight: .medium)))
                disclosure.tintColor = Palette.tertiary
                disclosure.translatesAutoresizingMaskIntoConstraints = false
                button.addSubview(disclosure)
                NSLayoutConstraint.activate([
                    disclosure.trailingAnchor.constraint(equalTo: button.trailingAnchor),
                    disclosure.centerYAnchor.constraint(equalTo: button.centerYAnchor),
                ])
                config.subtitleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { [weak self] incoming in
                    var attributes = incoming
                    attributes.font = self?.font(12)
                    attributes.foregroundColor = Palette.secondary
                    return attributes
                }
            }
            if key == "scope" {
                config.image = UIImage(systemName: "folder", withConfiguration: UIImage.SymbolConfiguration(pointSize: 13))
                config.imagePadding = 8
                config.baseForegroundColor = Palette.secondary
            }
            button.configuration = config
            button.titleLabel?.numberOfLines = lines.count > 1 ? 2 : 0
            button.subtitleLabel?.numberOfLines = traitCollection.preferredContentSizeCategory.isAccessibilityCategory ? 0 : 1
            button.titleLabel?.adjustsFontForContentSizeCategory = true
            button.subtitleLabel?.adjustsFontForContentSizeCategory = true
            button.isEnabled = props["enabled"] as? Bool ?? false
            button.contentHorizontalAlignment = .leading
            button.heightAnchor.constraint(greaterThanOrEqualToConstant: CGFloat(style["min_height"] as? Int ?? 44)).isActive = true
            button.configurationUpdateHandler = { button in
                button.backgroundColor = button.isHighlighted ? Palette.controlFill : .clear
                button.layer.cornerRadius = 8
            }
            button.addAction(UIAction { [weak self] _ in self?.activate(key, instance: instance, revision: revision) }, for: .touchUpInside)
            result = button
        case "field":
            let stack = UIStackView()
            stack.axis = .vertical
            stack.spacing = 6
            let label = UILabel()
            label.text = props["label"] as? String
            label.font = .preferredFont(forTextStyle: .subheadline)
            label.adjustsFontForContentSizeCategory = true
            label.textColor = Palette.secondary
            label.numberOfLines = 0
            let field = UITextField()
            field.borderStyle = .roundedRect
            field.text = fieldDrafts[key] ?? props["value"] as? String
            field.accessibilityLabel = props["label"] as? String
            field.accessibilityIdentifier = key
            field.font = font(16)
            field.adjustsFontForContentSizeCategory = true
            field.autocorrectionType = .no
            field.autocapitalizationType = .none
            field.heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
            field.addAction(UIAction { [weak self, weak field] _ in
                self?.fieldDrafts[key] = field?.text ?? ""
            }, for: .editingChanged)
            stack.addArrangedSubview(label)
            stack.addArrangedSubview(field)
            result = stack
        case "stack", "list":
            let stack = UIStackView()
            stack.axis = props["axis"] as? String == "horizontal" && !traitCollection.preferredContentSizeCategory.isAccessibilityCategory ? .horizontal : .vertical
            stack.spacing = CGFloat(style["gap_points"] as? Int ?? 16)
            if stack.axis == .horizontal { stack.distribution = .fillEqually }
            let children = (props["children"] as? [[String: Any]] ?? []).filter {
                let key = $0["key"] as? String ?? ""
                if isHome && ["dashboard-name", "home-actions"].contains(key) { return false }
                if screenKey.hasPrefix("home-Edit-") && key == "edit-actions" { return false }
                if screenKey.hasPrefix("home-Manage-") && ["manage-title", "back-home"].contains(key) { return false }
                return true
            }
            if props["axis"] as? String == "wrap", !traitCollection.preferredContentSizeCategory.isAccessibilityCategory {
                let columns = max(1, Int(max(240, view.bounds.width - 64) / 140))
                for start in stride(from: 0, to: children.count, by: columns) {
                    let row = UIStackView()
                    row.axis = .horizontal
                    row.spacing = 8
                    row.distribution = .fillEqually
                    for child in children[start..<min(start + columns, children.count)] {
                        row.addArrangedSubview(mount(child, instance: instance, revision: revision))
                    }
                    stack.addArrangedSubview(row)
                }
            } else {
                for child in children { stack.addArrangedSubview(mount(child, instance: instance, revision: revision)) }
            }
            if let radius = style["radius"] as? Double {
                stack.isLayoutMarginsRelativeArrangement = true
                let padding = style["padding_points"] as? [Int] ?? [20, 20, 20, 20]
                stack.directionalLayoutMargins = NSDirectionalEdgeInsets(top: CGFloat(padding[0]), leading: CGFloat(padding[3]), bottom: CGFloat(padding[2]), trailing: CGFloat(padding[1]))
                stack.backgroundColor = Palette.card
                stack.layer.cornerRadius = radius
                stack.layer.cornerCurve = .continuous
                stack.layer.borderWidth = 1 / traitCollection.displayScale
                stack.layer.borderColor = Palette.hairline.resolvedColor(with: traitCollection).cgColor
            }
            result = stack
        default: result = UIView()
        }
        if key.hasPrefix("edit-") {
            let drag = UIDragInteraction(delegate: self)
            drag.isEnabled = true
            result.addInteraction(drag)
            result.addInteraction(UIDropInteraction(delegate: self))
            dragFrames[key] = (instance, revision)
        }
        result.accessibilityIdentifier = key
        return result
    }
    func dragInteraction(_ interaction: UIDragInteraction, itemsForBeginning session: UIDragSession) -> [UIDragItem] {
        guard let key = interaction.view?.accessibilityIdentifier, let frame = dragFrames[key] else { return [] }
        let item = UIDragItem(itemProvider: NSItemProvider(object: "Widget" as NSString))
        item.localObject = ["up-" + String(key.dropFirst(5)), frame.0, String(frame.1)]
        return [item]
    }
    func dropInteraction(_ interaction: UIDropInteraction, canHandle session: UIDropSession) -> Bool {
        session.localDragSession != nil && session.items.count == 1
    }
    func dropInteraction(_ interaction: UIDropInteraction, sessionDidUpdate session: UIDropSession) -> UIDropProposal {
        UIDropProposal(operation: session.localDragSession == nil ? .forbidden : .move)
    }
    func dropInteraction(_ interaction: UIDropInteraction, performDrop session: UIDropSession) {
        guard let key = interaction.view?.accessibilityIdentifier, let frame = dragFrames[key],
              let source = session.items.first?.localObject as? [String], source.count == 3,
              source[1] == frame.0, UInt64(source[2]) == frame.1 else { return }
        do {
            try home?.reorder(instance: frame.0, revision: frame.1, source: source[0], target: "up-" + String(key.dropFirst(5)))
            render()
        } catch { Toast.show(error.localizedDescription, in: view.window) }
    }
    private func activate(_ key: String, instance: String, revision: UInt64, value: String? = nil) {
        guard let home else { return }
        submission &+= 1
        let ticket = submission
        let submitted = fieldDrafts
        Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                let fields = String(data: try JSONSerialization.data(withJSONObject: submitted), encoding: .utf8) ?? "{}"
                let json = try await home.activateForm(instance: instance, revision: revision, node: key, fields: fields)
                if submission == ticket {
                    for (key, value) in submitted where fieldDrafts[key] == value { fieldDrafts.removeValue(forKey: key) }
                }
                if let data = json.data(using: .utf8), let effect = try JSONSerialization.jsonObject(with: data) as? [String: Any] { handle(effect) }
                render()
            } catch { Toast.show(error.localizedDescription, in: view.window); render() }
        }
    }
    private func handle(_ effect: [String: Any]) {
        switch effect["kind"] as? String {
        case "session": if let id = effect["id"] as? String { router?.openSession(id) }
        case "url": if let text = effect["url"] as? String, let url = URL(string: text), url.scheme == "https" { UIApplication.shared.open(url) }
        case "export":
            guard let name = effect["name"] as? String, let contents = effect["contents"] as? String else { return }
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
            do {
                try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                let url = directory.appendingPathComponent(name)
                try contents.write(to: url, atomically: true, encoding: .utf8)
                let sheet = UIActivityViewController(activityItems: [url], applicationActivities: nil)
                sheet.popoverPresentationController?.sourceView = view
                sheet.completionWithItemsHandler = { _, _, _, _ in try? FileManager.default.removeItem(at: directory) }
                present(sheet, animated: true)
            } catch { Toast.show("Couldn't export this layout.", in: view.window) }
        case "import":
            let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.json], asCopy: true)
            picker.delegate = self
            present(picker, animated: true)
        default: break
        }
    }
    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let url = urls.first else { return }
        let access = url.startAccessingSecurityScopedResource()
        defer { if access { url.stopAccessingSecurityScopedResource() } }
        do {
            let size = try url.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? Int.max
            guard size <= 256 * 1024 else { throw CocoaError(.fileReadTooLarge) }
            let bytes = try Data(contentsOf: url)
            Task { @MainActor in
                do { try await home?.importLayout(bytes: bytes); render() }
                catch { Toast.show("Couldn't import this layout.", in: view.window) }
            }
        } catch { Toast.show("Couldn't import this layout.", in: view.window) }
    }
}

final class HomeBridge: HomeListener, @unchecked Sendable {
    private weak var app: AppModel?
    init(app: AppModel) { self.app = app }
    func onHomeChanged() { DispatchQueue.main.async { [weak self] in self?.app?.homeDidChange() } }
}
