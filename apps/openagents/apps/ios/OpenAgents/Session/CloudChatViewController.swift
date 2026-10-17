import UIKit

/// The chat answered by OpenAgents (#11978, #11979), over the core's
/// `CoreCloudChat`. Signed out it runs as a visitor with a few messages
/// before a sign-in; after a sign-in the chat moves to the account and
/// keeps going. One per app, so the conversation survives the sign-in
/// swap of the window's root.
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

/// The phone's first screen signed out, and the page New session opens when
/// no computer is online: a message list, a composer with send and stop,
/// and a quiet line about messages left before a sign-in.
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
    private let demo = UIButton(type: .system)
    /// Cards shown above the composer (#11982).
    let cardSlot = UIStackView()
    private var observation: AnyObject?
    private var appObservation: AnyObject?
    private var rows: [CloudChatLine] = []
    private var promptedForSignIn = false
    private var ownComputerCard: OwnComputerCardView?
    /// The demo's copy of the signed-out first screen (`-demo -route welcome`).
    private let signedOutLook: Bool

    init(app: AppModel, chat: CloudChatModel, signedOutLook: Bool = false) {
        self.app = app
        self.chat = chat
        self.signedOutLook = signedOutLook
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

        demo.setTitle("See a demo", for: .normal)
        demo.titleLabel?.font = Fonts.ui(.sans, 13)
        demo.tintColor = Palette.secondary
        demo.accessibilityIdentifier = "open-demo"
        demo.addAction(UIAction { [weak self] _ in self?.app.openDemo() }, for: .touchUpInside)

        cardSlot.axis = .vertical
        cardSlot.spacing = 8
        cardSlot.isHidden = true

        composer.useCloudControls()
        composer.placeholder = "Ask OpenAgents"
        composer.onSend = { [weak self] text, _, _ in self?.send(text) ?? .kept }
        composer.onStop = { [weak self] in self?.chat.stop() }

        let footer = UIStackView(arrangedSubviews: [note, demo])
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
        appObservation = app.observe { [weak self] in self?.renderBar() }
        renderBar()
        render()
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        if rows.isEmpty { _ = composer.becomeFirstResponder() }
    }

    private var signedIn: Bool { app.isSignedIn && !signedOutLook }

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

    private func renderBar() {
        var items: [UIBarButtonItem] = []
        if !signedIn {
            let signIn = UIBarButtonItem(title: "Sign in", primaryAction: UIAction { [weak self] _ in self?.signIn() })
            signIn.accessibilityIdentifier = "account-sign-in"
            items.append(signIn)
        }
        if !chat.snapshot.lines.isEmpty {
            let fresh = UIBarButtonItem(image: UIImage(systemName: "square.and.pencil"), primaryAction: UIAction { [weak self] _ in
                self?.chat.newChat()
            })
            fresh.accessibilityLabel = "New chat"
            fresh.accessibilityIdentifier = "cloud-chat-new"
            items.append(fresh)
        }
        navigationItem.rightBarButtonItems = items
        demo.isHidden = signedIn
        renderOwnComputerCard()
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
        if s.signInRequired, !signedIn, !promptedForSignIn {
            promptedForSignIn = true
            promptSignIn()
        }
        if !s.signInRequired { promptedForSignIn = false }
    }

    private func noteText(_ s: CloudChatSnapshot) -> String? {
        if let error = s.error { return error }
        guard !signedIn, let left = s.left else { return nil }
        if left == 0 { return "Sign in to keep chatting. It's free." }
        if left <= 3 { return left == 1 ? "1 message left before you sign in" : "\(left) messages left before you sign in" }
        return nil
    }

    private func send(_ text: String) -> ComposerBar.SendResult {
        if chat.snapshot.signInRequired, !signedIn {
            promptSignIn()
            return .kept
        }
        chat.send(text)
        return .sent
    }

    // MARK: Sign-in

    /// The account sign-in: the code's approval page opens in the in-app
    /// sheet, with Sign in with Apple beside it (#11980).
    func signIn() {
        if presentedViewController != nil { dismiss(animated: false) }
        app.beginAccountSignIn()
        let nav = UINavigationController(rootViewController: SignInViewController(app: app))
        nav.topViewController?.navigationItem.leftBarButtonItem = UIBarButtonItem(systemItem: .close, primaryAction: UIAction { [weak nav, weak app] _ in
            app?.cancelAccountSignIn()
            nav?.dismiss(animated: true)
        })
        present(nav, animated: true)
    }

    private func promptSignIn() {
        guard presentedViewController == nil else { return }
        view.endEditing(true)
        let prompt = SignInPromptController(app: app) { [weak self] in self?.signIn() }
        if let sheet = prompt.sheetPresentationController {
            sheet.detents = [.custom { _ in 290 }]
            sheet.prefersGrabberVisible = true
        }
        present(prompt, animated: true)
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
    /// Signed out: the window's root.
    static func signedOutRoot(app: AppModel) -> UIViewController? {
        guard let chat = app.cloudChat else { return nil }
        return DrawerShellController.nav(CloudChatViewController(app: app, chat: chat))
    }

    /// The offline demo (#11984): `-route welcome` is the signed-out first
    /// screen, `-route cloud` the chat in the shell.
    static func demoRoot(app: AppModel) -> UIViewController? {
        let args = ProcessInfo.processInfo.arguments
        guard app.isDemo, let i = args.firstIndex(of: "-route"), i + 1 < args.count, args[i + 1] == "welcome", let chat = app.cloudChat else { return nil }
        return DrawerShellController.nav(CloudChatViewController(app: app, chat: chat, signedOutLook: true))
    }

    static func openDemo(in root: UIViewController?, app: AppModel) {
        guard app.isDemo, let root, let chat = app.cloudChat else { return }
        show(CloudChatViewController(app: app, chat: chat), in: root)
    }

    /// Just signed in from the cloud chat: keep the conversation open in the
    /// new shell, now as the account's.
    static func continueAfterSignIn(in root: UIViewController, app: AppModel) {
        guard let chat = app.cloudChat, !chat.snapshot.lines.isEmpty else { return }
        DispatchQueue.main.async {
            show(CloudChatViewController(app: app, chat: chat), in: root)
        }
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

/// "Sign in to keep chatting. It's free." with Sign in and Not now.
private final class SignInPromptController: UIViewController {
    private let onSignIn: () -> Void
    private let app: AppModel

    init(app: AppModel, onSignIn: @escaping () -> Void) {
        self.app = app
        self.onSignIn = onSignIn
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = Palette.background
        view.accessibilityIdentifier = "sign-in-prompt"
        let message = UILabel()
        message.text = "Sign in to keep chatting. It's free."
        message.font = Fonts.ui(.sansSemibold, 20)
        message.textColor = Palette.text
        message.numberOfLines = 0
        message.textAlignment = .center
        var config = UIButton.Configuration.filled()
        config.title = "Sign in"
        config.baseBackgroundColor = Palette.text
        config.baseForegroundColor = Palette.background
        config.cornerStyle = .capsule
        config.contentInsets = NSDirectionalEdgeInsets(top: 12, leading: 32, bottom: 12, trailing: 32)
        let signIn = UIButton(configuration: config)
        signIn.accessibilityIdentifier = "sign-in-prompt-sign-in"
        signIn.addAction(UIAction { [weak self] _ in
            guard let self else { return }
            let action = self.onSignIn
            self.dismiss(animated: true) { action() }
        }, for: .touchUpInside)
        let later = UIButton(type: .system)
        later.setTitle("Not now", for: .normal)
        later.tintColor = Palette.secondary
        later.accessibilityIdentifier = "sign-in-prompt-later"
        later.addAction(UIAction { [weak self] _ in self?.dismiss(animated: true) }, for: .touchUpInside)
        let apple = SignInSheet.appleButton(app: app, traits: traitCollection)
        let stack = UIStackView(arrangedSubviews: [message, apple, signIn, later])
        stack.axis = .vertical
        stack.alignment = .center
        stack.spacing = 16
        stack.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.centerYAnchor.constraint(equalTo: view.safeAreaLayoutGuide.centerYAnchor),
            stack.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -24),
        ])
    }
}
