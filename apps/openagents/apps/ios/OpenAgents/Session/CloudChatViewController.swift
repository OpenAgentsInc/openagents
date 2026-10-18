import UIKit

/// The chat answered by OpenAgents (#11978, #11979), over the core's
/// `CoreCloudChat`. It is always the account's chat: the phone signs in with
/// its own key on first launch (#11995).
/// One per app, so the conversation survives the swap of the window's root.
final class CloudChatModel {
    private let core: CoreCloudChat
    private let bridge: CloudChatBridge
    private(set) var snapshot: CloudChatSnapshot
    private var observers: [UUID: () -> Void] = [:]

    convenience init(account: CoreAccount) {
        self.init { account.cloudChat(listener: $0) }
    }

    /// The offline demo's chat: fixed replies, no server (#11984).
    static func demo() -> CloudChatModel { CloudChatModel { CoreCloudChat.demo(listener: $0) } }

    private init(_ make: (CloudChatBridge) -> CoreCloudChat) {
        let bridge = CloudChatBridge()
        self.bridge = bridge
        core = make(bridge)
        snapshot = core.snapshot()
        bridge.model = self
    }

    /// Whether the own-computer card shows under the open chat (#11982).
    func ownComputerCardShows(_ offer: CoreOwnComputerOffer, computers: Int) -> Bool {
        offer.shows(chat: core, computers: UInt32(computers))
    }

    fileprivate func changed(_ snapshot: CloudChatSnapshot) {
        self.snapshot = snapshot
        observers.values.forEach { $0() }
    }

    func send(_ text: String) { core.send(text: text) }
    func stop() { core.stop() }
    func newChat() { core.newChat() }
    /// Pin or unpin the open chat; `done` gets nil, or a line to show.
    func setPinned(_ pinned: Bool, done: @escaping (String?) -> Void) {
        core.setPinned(pinned: pinned, done: CloudChatDoneBridge(done))
    }
    /// Archive the open chat and start a new one; `done` as for `setPinned`.
    func archive(done: @escaping (String?) -> Void) {
        core.archive(done: CloudChatDoneBridge(done))
    }
    /// The chat as plain text, for Copy Transcript.
    func transcript() -> String { core.transcript() }
    /// Start a new chat with `text` (New session, with no computer online).
    func start(_ text: String) {
        core.newChat()
        core.send(text: text)
    }

    func observe(_ handler: @escaping () -> Void) -> AnyObject {
        let id = UUID()
        observers[id] = handler
        return ObserverToken { [weak self] in self?.observers[id] = nil }
    }

    private final class ObserverToken {
        let cancel: () -> Void
        init(cancel: @escaping () -> Void) { self.cancel = cancel }
        deinit { cancel() }
    }
}

private final class CloudChatBridge: CloudChatListener, @unchecked Sendable {
    weak var model: CloudChatModel?
    func onCloudChatChanged(snapshot: CloudChatSnapshot) {
        DispatchQueue.main.async { [weak self] in self?.model?.changed(snapshot) }
    }
}

/// Hands a pin or archive result back on the main thread.
private final class CloudChatDoneBridge: CloudChatDone, @unchecked Sendable {
    private let done: (String?) -> Void
    init(_ done: @escaping (String?) -> Void) { self.done = done }
    func onDone(error: String?) {
        let done = self.done
        DispatchQueue.main.async { done(error) }
    }
}

/// The phone's landing chat, and the page New session opens when no
/// computer is online: a message list, a composer with send and stop, and a
/// quiet line for errors. Nothing here asks anyone to sign in (#11996).
///
/// `cardSlot` sits between the messages and the composer for cards such as
/// "Run this on your own computer" (#11982): add views with `addCard(_:)`.
final class CloudChatViewController: UIViewController, UITableViewDataSource {
    private let app: AppModel
    private let chat: CloudChatModel
    private let table = UITableView(frame: .zero, style: .plain)
    private let composer = ComposerBar()
    private let note = UILabel()
    private let empty = UILabel()
    /// Cards shown above the composer (#11982).
    let cardSlot = UIStackView()
    private var observation: AnyObject?
    private var appObservation: AnyObject?
    private var rows: [CloudChatLine] = []
    private var ownComputerCard: OwnComputerCardView?
    private var voiceBar: VoiceBarItem?
    private var voiceObservation: AnyObject?

    init(app: AppModel, chat: CloudChatModel) {
        self.app = app
        self.chat = chat
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError() }

    func addCard(_ card: UIView) {
        cardSlot.addArrangedSubview(card)
        cardSlot.isHidden = false
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = Palette.background
        title = "OpenAgents"
        navigationItem.largeTitleDisplayMode = .never
        view.accessibilityIdentifier = "cloud-chat"

        table.dataSource = self
        table.separatorStyle = .none
        table.backgroundColor = .clear
        table.keyboardDismissMode = .interactive
        table.allowsSelection = false
        table.register(CloudChatCell.self, forCellReuseIdentifier: "line")
        table.accessibilityIdentifier = "cloud-chat-lines"

        empty.text = "Ask anything, or start a coding task. Your computer can join later."
        empty.font = Fonts.ui(.sans, 17)
        empty.textColor = Palette.secondary
        empty.numberOfLines = 0
        empty.textAlignment = .center
        empty.accessibilityIdentifier = "cloud-chat-empty"

        note.font = Fonts.ui(.sans, 13)
        note.textColor = Palette.secondary
        note.numberOfLines = 0
        note.textAlignment = .center
        note.accessibilityIdentifier = "cloud-chat-note"

        cardSlot.axis = .vertical
        cardSlot.spacing = 8
        cardSlot.isHidden = true

        composer.useCloudControls()
        composer.placeholder = "Ask OpenAgents"
        composer.onSend = { [weak self] text, _, _ in self?.send(text) ?? .kept }
        composer.onStop = { [weak self] in self?.chat.stop() }

        let footer = UIStackView(arrangedSubviews: [note])
        footer.axis = .vertical
        footer.alignment = .center
        footer.spacing = 2
        for v in [table, empty, cardSlot, composer, footer] as [UIView] {
            v.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview(v)
        }
        let guide = view.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            table.topAnchor.constraint(equalTo: guide.topAnchor),
            table.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            table.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            table.bottomAnchor.constraint(equalTo: cardSlot.topAnchor, constant: -8),
            empty.centerYAnchor.constraint(equalTo: table.centerYAnchor),
            empty.leadingAnchor.constraint(equalTo: guide.leadingAnchor, constant: 32),
            empty.trailingAnchor.constraint(equalTo: guide.trailingAnchor, constant: -32),
            cardSlot.leadingAnchor.constraint(equalTo: guide.leadingAnchor, constant: 12),
            cardSlot.trailingAnchor.constraint(equalTo: guide.trailingAnchor, constant: -12),
            cardSlot.bottomAnchor.constraint(equalTo: composer.topAnchor, constant: -8),
            composer.centerXAnchor.constraint(equalTo: guide.centerXAnchor),
            composer.leadingAnchor.constraint(greaterThanOrEqualTo: guide.leadingAnchor, constant: 12),
            composer.widthAnchor.constraint(lessThanOrEqualToConstant: 768),
            composer.leadingAnchor.constraint(equalTo: guide.leadingAnchor, constant: 12).withPriority(.defaultHigh),
            composer.bottomAnchor.constraint(equalTo: footer.topAnchor, constant: -4),
            footer.leadingAnchor.constraint(equalTo: guide.leadingAnchor, constant: 16),
            footer.trailingAnchor.constraint(equalTo: guide.trailingAnchor, constant: -16),
            footer.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor, constant: -4),
        ])
        observation = chat.observe { [weak self] in self?.render() }
        // The chat has its own composer, so the shell hides the capsule's
        // voice strip here; the call (or starting one) stays a tap away up top.
        voiceBar = VoiceBarItem(app: app, host: self)
        voiceObservation = app.voice.observe { [weak self] in self?.syncBarItems() }
        appObservation = app.observe { [weak self] in self?.renderBar() }
        renderBar()
        render()
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        if rows.isEmpty { _ = composer.becomeFirstResponder() }
    }

    private var signedIn: Bool { app.isSignedIn }

    /// "Run this on your own computer" (#11982): Rust decides; it goes away
    /// once closed or when one of the person's computers joins.
    private func renderOwnComputerCard() {
        guard let offer = app.ownComputerOffer else { return }
        let computers = signedIn ? app.hostOptions.filter { $0.name != AppModel.cloudHostName }.count : 0
        let shows = chat.ownComputerCardShows(offer, computers: computers)
        if shows, ownComputerCard == nil {
            let card = OwnComputerCardView(card: offer.card())
            card.onClose = { [weak self] in
                offer.close()
                self?.renderOwnComputerCard()
            }
            ownComputerCard = card
            addCard(card)
        } else if !shows, let card = ownComputerCard {
            card.removeFromSuperview()
            ownComputerCard = nil
            cardSlot.isHidden = cardSlot.arrangedSubviews.isEmpty
        }
    }

    private lazy var freshItem: UIBarButtonItem = {
        let fresh = UIBarButtonItem(image: UIImage(systemName: "square.and.pencil"), primaryAction: UIAction { [weak self] _ in
            self?.chat.newChat()
        })
        fresh.accessibilityLabel = "New chat"
        fresh.accessibilityIdentifier = "cloud-chat-new"
        return fresh
    }()

    /// The ⋯ menu, made once so a streaming answer doesn't close it.
    private lazy var moreItem: UIBarButtonItem = {
        let more = UIBarButtonItem(image: UIImage(systemName: "ellipsis"), menu: chatMenu())
        more.accessibilityLabel = "More"
        more.accessibilityIdentifier = "cloud-chat-menu"
        return more
    }()

    private func renderBar() {
        syncBarItems()
        renderOwnComputerCard()
    }

    private func syncBarItems() {
        var items: [UIBarButtonItem] = []
        if !chat.snapshot.lines.isEmpty { items.append(contentsOf: [moreItem, freshItem]) }
        if let voiceBar, voiceBar.visible { items.append(voiceBar.item) }
        // Voice level frames land here too: only touch the bar when it changes.
        let shown = navigationItem.rightBarButtonItems ?? []
        if shown.count != items.count || !zip(shown, items).allSatisfy({ $0 === $1 }) {
            navigationItem.rightBarButtonItems = items
        }
    }

    /// The session's ⋯ menu, for a cloud chat: Pin, Copy Transcript, and
    /// Archive. Runs on… comes later (#12020).
    private func chatMenu() -> UIMenu {
        UIMenu(children: [UIDeferredMenuElement.uncached { [weak self] done in
            guard let self else { return done([]) }
            let pinned = self.chat.snapshot.pinned
            done([
                UIAction(title: pinned ? "Unpin" : "Pin", image: UIImage(systemName: pinned ? "pin.slash" : "pin")) { [weak self] _ in
                    self?.chat.setPinned(!pinned) { error in
                        if let error { Toast.show(error, in: self?.view.window) }
                    }
                },
                UIAction(title: "Copy Transcript", image: UIImage(systemName: "doc.on.doc")) { [weak self] _ in
                    guard let self else { return }
                    UIPasteboard.general.string = self.chat.transcript()
                },
                UIAction(title: "Archive", image: UIImage(systemName: "archivebox"), attributes: .destructive) { [weak self] _ in
                    self?.chat.archive { error in
                        if let error { Toast.show(error, in: self?.view.window) }
                    }
                },
            ])
        }])
    }

    private func render() {
        let s = chat.snapshot
        rows = s.lines.filter { !$0.text.isEmpty }
        // The answer is on its way but hasn't started: a quiet placeholder.
        if s.answering, rows.last?.role == .user {
            rows.append(CloudChatLine(role: .assistant, text: "…"))
        }
        table.reloadData()
        if !rows.isEmpty {
            table.scrollToRow(at: IndexPath(row: rows.count - 1, section: 0), at: .bottom, animated: false)
        }
        empty.isHidden = !rows.isEmpty
        composer.running = s.answering
        if let unsent = s.unsent, composer.text.isEmpty { composer.text = unsent }
        note.text = noteText(s)
        note.textColor = s.error == nil ? Palette.secondary : Palette.danger
        note.isHidden = note.text == nil
        renderBar()
    }

    private func noteText(_ s: CloudChatSnapshot) -> String? {
        if let error = s.error { return error }
        return nil
    }

    private func send(_ text: String) -> ComposerBar.SendResult {
        chat.send(text)
        return .sent
    }


    // MARK: Table

    func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { rows.count }

    func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "line", for: indexPath) as! CloudChatCell
        cell.show(rows[indexPath.row])
        return cell
    }
}

// MARK: Opening the chat

extension CloudChatViewController {
    /// The shell's first page (#12009): the cloud chat, or an empty page
    /// when there is none (a local dev stack).
    static func landingPage(app: AppModel) -> UIViewController {
        if let chat = app.cloudChat { return CloudChatViewController(app: app, chat: chat) }
        let page = UIViewController()
        page.view.backgroundColor = Palette.background
        page.title = "OpenAgents"
        return page
    }

    /// New session with no computer online: start a chat on OpenAgents.
    static func start(_ text: String, from presenter: UIViewController, app: AppModel) -> Bool {
        guard let chat = app.cloudChat else { return false }
        chat.start(text)
        let root = presenter.view.window?.rootViewController
        presenter.dismiss(animated: true)
        if let root { show(CloudChatViewController(app: app, chat: chat), in: root) }
        return true
    }

    private static func show(_ page: CloudChatViewController, in root: UIViewController) {
        if let shell = root as? DrawerShellController {
            shell.showPage(page)
        } else if let shell = (root as? UISplitViewController)?.viewController(for: .compact) as? DrawerShellController, root.traitCollection.horizontalSizeClass == .compact {
            shell.showPage(page)
        } else {
            let nav = DrawerShellController.nav(page)
            page.navigationItem.leftBarButtonItem = UIBarButtonItem(systemItem: .close, primaryAction: UIAction { [weak nav] _ in nav?.dismiss(animated: true) })
            nav.modalPresentationStyle = .pageSheet
            root.present(nav, animated: true)
        }
    }
}

private extension NSLayoutConstraint {
    func withPriority(_ priority: UILayoutPriority) -> NSLayoutConstraint {
        self.priority = priority
        return self
    }
}

/// One line: the person's messages in a bubble on the right, answers as
/// text across the page.
private final class CloudChatCell: UITableViewCell {
    private let label = UILabel()
    private let bubble = UIView()
    private var userConstraints: [NSLayoutConstraint] = []
    private var answerConstraints: [NSLayoutConstraint] = []

    override init(style: UITableViewCell.CellStyle, reuseIdentifier: String?) {
        super.init(style: style, reuseIdentifier: reuseIdentifier)
        backgroundColor = .clear
        label.numberOfLines = 0
        bubble.layer.cornerRadius = 18
        bubble.layer.cornerCurve = .continuous
        for v in [bubble, label] {
            v.translatesAutoresizingMaskIntoConstraints = false
            contentView.addSubview(v)
        }
        let margins = contentView.layoutMarginsGuide
        userConstraints = [
            label.trailingAnchor.constraint(equalTo: margins.trailingAnchor, constant: -12),
            label.leadingAnchor.constraint(greaterThanOrEqualTo: margins.leadingAnchor, constant: 56),
        ]
        answerConstraints = [
            label.leadingAnchor.constraint(equalTo: margins.leadingAnchor),
            label.trailingAnchor.constraint(equalTo: margins.trailingAnchor),
        ]
        NSLayoutConstraint.activate([
            label.topAnchor.constraint(equalTo: contentView.topAnchor, constant: 14),
            label.bottomAnchor.constraint(equalTo: contentView.bottomAnchor, constant: -14),
            bubble.topAnchor.constraint(equalTo: label.topAnchor, constant: -9),
            bubble.bottomAnchor.constraint(equalTo: label.bottomAnchor, constant: 9),
            bubble.leadingAnchor.constraint(equalTo: label.leadingAnchor, constant: -14),
            bubble.trailingAnchor.constraint(equalTo: label.trailingAnchor, constant: 14),
        ])
    }
    required init?(coder: NSCoder) { fatalError() }

    func show(_ line: CloudChatLine) {
        let user = line.role == .user
        NSLayoutConstraint.deactivate(user ? answerConstraints : userConstraints)
        NSLayoutConstraint.activate(user ? userConstraints : answerConstraints)
        bubble.isHidden = !user
        bubble.backgroundColor = Palette.userBubble
        let size: CGFloat = line.role == .tool ? 13 : 17
        let color = line.role == .tool ? Palette.secondary : Palette.text
        label.attributedText = Self.markdown(line.text, size: size, color: color)
        label.accessibilityIdentifier = user ? "cloud-chat-user" : "cloud-chat-answer"
    }

    static func markdown(_ text: String, size: CGFloat, color: UIColor) -> NSAttributedString {
        let font = Fonts.ui(.sans, size)
        let options = AttributedString.MarkdownParsingOptions(interpretedSyntax: .inlineOnlyPreservingWhitespace, failurePolicy: .returnPartiallyParsedIfPossible)
        guard let parsed = try? AttributedString(markdown: text, options: options) else {
            return NSAttributedString(string: text, attributes: [.font: font, .foregroundColor: color])
        }
        let out = NSMutableAttributedString(parsed)
        let whole = NSRange(location: 0, length: out.length)
        out.addAttribute(.foregroundColor, value: color, range: whole)
        out.enumerateAttribute(.inlinePresentationIntent, in: whole) { value, range, _ in
            let intent = (value as? NSNumber).map { InlinePresentationIntent(rawValue: $0.uintValue) } ?? []
            var traits: UIFontDescriptor.SymbolicTraits = []
            if intent.contains(.stronglyEmphasized) { traits.insert(.traitBold) }
            if intent.contains(.emphasized) { traits.insert(.traitItalic) }
            var styled = font
            if intent.contains(.code) {
                styled = .monospacedSystemFont(ofSize: size - 1, weight: .regular)
            } else if !traits.isEmpty, let descriptor = font.fontDescriptor.withSymbolicTraits(traits) {
                styled = UIFont(descriptor: descriptor, size: size)
            }
            out.addAttribute(.font, value: styled, range: range)
        }
        return out
    }
}
