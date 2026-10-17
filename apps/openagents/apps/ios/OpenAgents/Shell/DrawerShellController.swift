import UIKit

/// iPhone shell: one page at a time (Home, a session, Settings) under a top
/// bar whose menu button opens a drawer from the left. The drawer is the iPad
/// sidebar: Sessions with search, and Home, Settings and New session in its
/// toolbar. A left-edge swipe opens it; a tap or swipe on the dimmed page
/// closes it. Over pages without their own composer, a floating "New
/// session" capsule carries the live summary and voice.
final class DrawerShellController: UIViewController, AppRouter, UINavigationControllerDelegate, UIGestureRecognizerDelegate {
    private let app: AppModel
    private let content: UINavigationController
    private let sidebar: SidebarViewController
    private let drawer = UIView()
    private let scrim = UIControl()
    private let capsule = Glass.surface()
    /// 0 closed … 1 open; a pan sets it as the finger moves.
    private var progress: CGFloat = 0
    private(set) var isDrawerOpen = false
    private var accessoryShown = true
    private var liveToken: AnyObject?
    private var voiceToken: AnyObject?

    static let capsuleHeight: CGFloat = 50

    init(app: AppModel) {
        self.app = app
        self.content = Self.nav(HomeViewController(app: app))
        self.sidebar = SidebarViewController(app: app)
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { fatalError() }

    private var drawerWidth: CGFloat { min(view.bounds.width * 0.86, 380) }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = Palette.background
        addChild(content)
        content.view.frame = view.bounds
        content.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(content.view)
        content.didMove(toParent: self)
        content.delegate = self
        if let root = content.viewControllers.first { addMenuButton(to: root) }

        capsule.translatesAutoresizingMaskIntoConstraints = false
        accessoryContent.translatesAutoresizingMaskIntoConstraints = false
        capsule.contentView.addSubview(accessoryContent)
        view.addSubview(capsule)
        NSLayoutConstraint.activate([
            capsule.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 16),
            capsule.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -16),
            capsule.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -8),
            capsule.heightAnchor.constraint(equalToConstant: Self.capsuleHeight),
            accessoryContent.leadingAnchor.constraint(equalTo: capsule.contentView.leadingAnchor),
            accessoryContent.trailingAnchor.constraint(equalTo: capsule.contentView.trailingAnchor),
            accessoryContent.topAnchor.constraint(equalTo: capsule.contentView.topAnchor),
            accessoryContent.bottomAnchor.constraint(equalTo: capsule.contentView.bottomAnchor),
        ])
        content.additionalSafeAreaInsets.bottom = Self.capsuleHeight + 8

        scrim.backgroundColor = UIColor.black.withAlphaComponent(0.32)
        scrim.alpha = 0
        scrim.isHidden = true
        scrim.frame = view.bounds
        scrim.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        scrim.isAccessibilityElement = true
        scrim.accessibilityLabel = "Close menu"
        scrim.accessibilityTraits = .button
        scrim.accessibilityIdentifier = "drawer-scrim"
        scrim.addAction(UIAction { [weak self] _ in self?.closeDrawer() }, for: .touchUpInside)
        view.addSubview(scrim)

        drawer.isHidden = true
        drawer.accessibilityViewIsModal = true
        drawer.layer.shadowColor = UIColor.black.cgColor
        drawer.layer.shadowRadius = 24
        drawer.layer.shadowOpacity = 0
        addChild(sidebar)
        sidebar.view.frame = drawer.bounds
        sidebar.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        drawer.addSubview(sidebar.view)
        view.addSubview(drawer)
        sidebar.didMove(toParent: self)

        let edge = UIScreenEdgePanGestureRecognizer(target: self, action: #selector(edgePan(_:)))
        edge.edges = .left
        edge.delegate = self
        view.addGestureRecognizer(edge)
        let close = UIPanGestureRecognizer(target: self, action: #selector(closePan(_:)))
        close.delegate = self
        scrim.addGestureRecognizer(close)

        accessoryContent.update(app.live)
        liveToken = app.observe { [weak self] in
            guard let self else { return }
            self.accessoryContent.update(self.app.live)
            // Hosts come and go with the workspace (online, capabilities).
            self.accessoryContent.updateVoice(self.app.voice)
        }
        let voice = accessoryContent.voice
        voice.onStart = { [weak self] source in
            guard let self else { return }
            self.openVoice(app: self.app, source: source)
        }
        voice.onOpen = { [weak self] source in
            guard let self else { return }
            self.presentVoiceStage(app: self.app, source: source)
        }
        voice.onMute = { [weak self] in self?.app.voice.toggleMute() }
        voice.onEnd = { [weak self] in self?.app.voice.stop() }
        voice.startButton.menu = voiceMenu(app: app)
        accessoryContent.updateVoice(app.voice)
        voiceToken = app.voice.observe { [weak self] in
            guard let self else { return }
            self.accessoryContent.updateVoice(self.app.voice)
        }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        layoutDrawer()
    }

    private func layoutDrawer() {
        let width = drawerWidth
        drawer.frame = CGRect(x: (progress - 1) * width, y: 0, width: width, height: view.bounds.height)
    }

    // MARK: Drawer

    func openDrawer(animated: Bool = true) { setDrawer(open: true, animated: animated) }

    func closeDrawer(animated: Bool = true) { setDrawer(open: false, animated: animated) }

    private func setProgress(_ value: CGFloat) {
        progress = min(1, max(0, value))
        if progress > 0 {
            drawer.isHidden = false
            scrim.isHidden = false
        }
        scrim.alpha = progress
        // While the drawer is out, the page behind it is out of reach.
        content.view.accessibilityElementsHidden = progress > 0
        capsule.accessibilityElementsHidden = progress > 0
        drawer.layer.shadowOpacity = Float(progress * 0.18)
        layoutDrawer()
    }

    private func setDrawer(open: Bool, animated: Bool) {
        if open {
            // The page's keyboard and any sheet give way to the drawer.
            content.view.endEditing(true)
        } else if !isDrawerOpen, progress == 0, drawer.isHidden {
            return
        }
        isDrawerOpen = open
        view.layoutIfNeeded()
        let finish = { [weak self] in
            guard let self, self.isDrawerOpen == open else { return }
            if !open {
                self.drawer.isHidden = true
                self.scrim.isHidden = true
                self.sidebar.view.endEditing(true)
            }
            UIAccessibility.post(notification: .screenChanged, argument: open ? self.drawer : nil)
        }
        if open { drawer.isHidden = false; scrim.isHidden = false }
        guard animated, !UIAccessibility.isReduceMotionEnabled, view.window != nil else {
            setProgress(open ? 1 : 0)
            return finish()
        }
        UIView.animate(withDuration: 0.42, delay: 0, usingSpringWithDamping: 1, initialSpringVelocity: 0,
                       options: [.beginFromCurrentState, .allowUserInteraction]) {
            self.setProgress(open ? 1 : 0)
        } completion: { _ in finish() }
    }

    @objc private func edgePan(_ pan: UIScreenEdgePanGestureRecognizer) {
        let width = max(1, drawerWidth)
        switch pan.state {
        case .began:
            content.view.endEditing(true)
            drawer.isHidden = false
            scrim.isHidden = false
        case .changed:
            setProgress(pan.translation(in: view).x / width)
        case .ended, .cancelled, .failed:
            let v = pan.velocity(in: view).x
            setDrawer(open: pan.state == .ended && (v > 300 || (progress > 0.5 && v > -300)), animated: true)
        default: break
        }
    }

    @objc private func closePan(_ pan: UIPanGestureRecognizer) {
        let width = max(1, drawerWidth)
        switch pan.state {
        case .changed:
            setProgress(1 + min(0, pan.translation(in: view).x) / width)
        case .ended, .cancelled, .failed:
            let v = pan.velocity(in: view).x
            setDrawer(open: !(pan.state == .ended && (v < -300 || (progress < 0.5 && v < 300))), animated: true)
        default: break
        }
    }

    func gestureRecognizerShouldBegin(_ gesture: UIGestureRecognizer) -> Bool {
        if gesture is UIScreenEdgePanGestureRecognizer {
            // Deeper pages keep the system swipe-back.
            return !isDrawerOpen && content.viewControllers.count == 1 && presentedViewController == nil
        }
        guard let pan = gesture as? UIPanGestureRecognizer else { return true }
        let v = pan.velocity(in: view)
        return isDrawerOpen && abs(v.x) > abs(v.y)
    }

    override func accessibilityPerformEscape() -> Bool {
        guard isDrawerOpen else { return false }
        closeDrawer()
        return true
    }

    func menuButton() -> UIBarButtonItem {
        let item = UIBarButtonItem(image: UIImage(systemName: "line.3.horizontal"), primaryAction: UIAction { [weak self] _ in
            self?.openDrawer()
        })
        item.accessibilityLabel = "Menu"
        item.accessibilityIdentifier = "drawer-menu"
        return item
    }

    /// Pages that manage their own leading item (Home) put `menuButton()`
    /// there themselves.
    private func addMenuButton(to page: UIViewController) {
        guard !(page is HomeViewController) else { return }
        page.navigationItem.leftBarButtonItem = menuButton()
    }

    /// One page at a time: the drawer swaps the page rather than stacking.
    func setPage(_ page: UIViewController) {
        addMenuButton(to: page)
        content.setViewControllers([page], animated: false)
        syncAccessory()
    }

    /// A screen the drawer opened (a saved chat) takes the page.
    func showPage(_ page: UIViewController) {
        sidebar.currentChatId = nil
        setPage(page)
        closeDrawer()
    }

    // MARK: Accessory (the floating New session capsule)

    private lazy var accessoryContent = AskAnythingAccessory { [weak self] in self?.presentNewSession() }

    /// Hidden over a session or a saved chat (they have their own composer).
    func syncAccessory(animated: Bool = false) {
        let top = content.topViewController
        setAccessoryVisible(!(top is SessionViewController || top is SavedTranscriptController), animated: animated)
    }

    /// Kept for screens that fade it with their own transitions.
    func prepareAccessoryForReveal() {
        guard !accessoryShown else { return }
        accessoryContent.alpha = 0
        setAccessoryVisible(true, animated: false)
    }

    func accessoryFrame(in window: UIWindow) -> CGRect? {
        guard accessoryShown, capsule.window != nil, !capsule.isHidden else { return nil }
        return capsule.convert(capsule.bounds, to: window)
    }

    func setAccessoryContentAlpha(_ alpha: CGFloat) {
        accessoryContent.alpha = alpha
    }

    func setAccessoryVisible(_ visible: Bool, animated: Bool) {
        guard visible != accessoryShown else { return }
        accessoryShown = visible
        content.additionalSafeAreaInsets.bottom = visible ? Self.capsuleHeight + 8 : 0
        if visible { capsule.isHidden = false }
        guard animated, !UIAccessibility.isReduceMotionEnabled else {
            capsule.alpha = visible ? 1 : 0
            capsule.isHidden = !visible
            return
        }
        UIView.animate(withDuration: 0.25, delay: 0, options: [.beginFromCurrentState, .allowUserInteraction]) {
            self.capsule.alpha = visible ? 1 : 0
        } completion: { _ in
            if !self.accessoryShown { self.capsule.isHidden = true }
        }
    }

    func navigationController(_ navigationController: UINavigationController, didShow viewController: UIViewController, animated: Bool) {
        syncAccessory()
    }

    static func nav(_ root: UIViewController) -> UINavigationController {
        let nav = UINavigationController(rootViewController: root)
        nav.navigationBar.prefersLargeTitles = true
        // Setting an appearance object drops iOS 26's scroll-edge effect;
        // style titles through the bar's own attributes instead.
        nav.navigationBar.largeTitleTextAttributes = [.font: Fonts.ui(.sansSemibold, 30), .foregroundColor: Palette.text]
        nav.navigationBar.titleTextAttributes = [.font: Fonts.ui(.sansSemibold, 17), .foregroundColor: Palette.text]
        nav.navigationBar.tintColor = Palette.text
        return nav
    }

    // MARK: AppRouter

    func presentNewSession(prompt: String? = nil) {
        closeDrawer(animated: false)
        let vc = NewSessionViewController(app: app, prompt: prompt) { [weak self] chatId, handoff in
            guard let self else { return }
            self.openSession(chatId, handoff: handoff)
        }
        let nav = UINavigationController(rootViewController: vc)
        nav.modalPresentationStyle = .pageSheet
        if let sheet = nav.sheetPresentationController {
            sheet.detents = [.large()]
            sheet.prefersGrabberVisible = true
        }
        present(nav, animated: true)
    }

    func showSettings() {
        if presentedViewController != nil { dismiss(animated: true) }
        sidebar.currentChatId = nil
        if content.viewControllers.first is MoreViewController {
            content.popToRootViewController(animated: false)
        } else {
            setPage(MoreViewController(app: app))
        }
        closeDrawer()
    }

    /// The sessions list lives in the drawer.
    func showSessions() {
        if presentedViewController != nil { dismiss(animated: true) }
        sidebar.showSessions()
        openDrawer()
    }

    func showHome() {
        if presentedViewController != nil { dismiss(animated: true) }
        sidebar.currentChatId = nil
        if content.viewControllers.first is HomeViewController {
            content.popToRootViewController(animated: false)
        } else {
            setPage(HomeViewController(app: app))
        }
        closeDrawer()
    }

    func showSearch() {
        if presentedViewController != nil { dismiss(animated: true) }
        openDrawer()
        sidebar.focusSearch()
    }

    /// From the new-session sheet: the chat goes in under the sheet at once,
    /// the sheet leaves without its slide, and the handoff animation carries
    /// the draft (page, composer, message) into the chat.
    func openSession(_ chatId: String, handoff: DraftHandoff?) {
        guard let handoff, let window = view.window else { return openSession(chatId) }
        guard let session = SessionViewController(app: app, chatId: chatId) else {
            return showSessionOpenError()
        }
        UIView.performWithoutAnimation {
            setPage(session)
            sidebar.currentChatId = chatId
            view.layoutIfNeeded()
            session.prepareArrival()
            // Sending puts the keyboard away; it slides down with the handoff.
            presentedViewController?.view.endEditing(true)
            // Hide the sheet now; tear it down only once the motion is done.
            // Dismissal (plus the keyboard re-hosting with it) is a heavy
            // compositor frame — mid-animation it swallowed the motion.
            if let sheet = presentedViewController?.presentationController as? UISheetPresentationController {
                // Drop the sheet's background dimming with it (it would sit over
                // the chat until the deferred dismissal, then snap off).
                sheet.largestUndimmedDetentIdentifier = .large
            }
            presentedViewController?.presentationController?.containerView?.alpha = 0
        }
        DraftHandoffAnimator.run(handoff, into: session, window: window) { [weak self] in
            if self?.presentedViewController != nil { self?.dismiss(animated: false) }
        }
    }

    /// Release any open session (the iPad split takes it back into its own
    /// column when it widens).
    func popToFrontPage() {
        guard content.viewControllers.contains(where: { $0 is SessionViewController || $0 is SavedTranscriptController }) else { return }
        sidebar.currentChatId = nil
        setPage(HomeViewController(app: app))
    }

    func showSessionOpenError() {
        if presentedViewController != nil { dismiss(animated: false) }
        view.endEditing(true)
        syncAccessory()
        sidebar.showSessions()
        openDrawer()
        Toast.show("Couldn't open session. Try again.", in: view.window)
    }

    /// Open a session as the page (from the drawer, deep links, search).
    func openSession(_ chatId: String) {
        if (content.viewControllers.first as? SessionViewController)?.chatId == chatId, content.viewControllers.count == 1 {
            if presentedViewController != nil { dismiss(animated: true) }
            return closeDrawer()
        }
        guard let session = SessionViewController(app: app, chatId: chatId) else {
            return showSessionOpenError()
        }
        if presentedViewController != nil { dismiss(animated: true) }
        setPage(session)
        sidebar.currentChatId = chatId
        closeDrawer()
    }
}

extension UIViewController {
    /// The iPhone drawer shell this controller sits in (also the iPad's
    /// compact shell), if any.
    var drawerShell: DrawerShellController? {
        sequence(first: self, next: { $0.parent }).lazy.compactMap { $0 as? DrawerShellController }.first
    }
}

/// The floating capsule at the bottom of the iPhone shell (and the trait-aware
/// content of the iPad's compact shell): a plus, "New session", and a live
/// summary of what's running ("2 working · 1 needs you"), with voice at its
/// trailing end. Tapping the plus side opens the new-session composer; the
/// waveform calls Codex, and during a call the strip shows it (orb, clock,
/// mute, hang-up) and opens the call stage.
final class AskAnythingAccessory: UIView {
    private let newSession = PressableControl()
    private let label = UILabel()
    private let summary = UILabel()
    private let cells = StatusGlyph()
    private let mark = UIImageView(image: UIImage(systemName: "plus", withConfiguration: UIImage.SymbolConfiguration(pointSize: 15, weight: .semibold)))
    private let plate = UIView()
    let voice = VoiceAccessoryView()
    private var voiceShown = false
    private var voiceLive = false
    private var restConstraints: [NSLayoutConstraint] = []
    private var idleVoiceConstraints: [NSLayoutConstraint] = []
    private var liveVoiceConstraints: [NSLayoutConstraint] = []

    init(onTap: @escaping () -> Void) {
        super.init(frame: .zero)
        newSession.accessibilityIdentifier = "new-session"
        newSession.accessibilityTraits = .button
        newSession.isAccessibilityElement = true
        newSession.addAction(UIAction { _ in onTap() }, for: .touchUpInside)

        mark.tintColor = Palette.accent
        mark.contentMode = .center
        plate.backgroundColor = Palette.accentSoft
        // A circle, concentric with the accessory capsule (7pt inset).
        plate.layer.cornerRadius = 17
        plate.layer.cornerCurve = .continuous
        plate.isUserInteractionEnabled = false
        plate.addSubview(mark)
        label.text = "New session"
        label.font = Fonts.ui(.sansMedium, 16)
        label.textColor = Palette.text
        summary.font = Fonts.ui(.sansMedium, 13)
        summary.textColor = Palette.secondary
        summary.textAlignment = .right
        cells.isHidden = true
        for v in [newSession, plate, label, mark, summary, cells, voice] as [UIView] { v.translatesAutoresizingMaskIntoConstraints = false }
        for v in [plate, label, summary, cells] as [UIView] { newSession.addSubview(v) }
        addSubview(newSession)
        addSubview(voice)
        voice.isHidden = true
        summary.setContentCompressionResistancePriority(.required, for: .horizontal)
        NSLayoutConstraint.activate([
            newSession.leadingAnchor.constraint(equalTo: leadingAnchor),
            newSession.topAnchor.constraint(equalTo: topAnchor),
            newSession.bottomAnchor.constraint(equalTo: bottomAnchor),
            plate.leadingAnchor.constraint(equalTo: newSession.leadingAnchor, constant: 7),
            plate.centerYAnchor.constraint(equalTo: newSession.centerYAnchor),
            plate.widthAnchor.constraint(equalToConstant: 34),
            plate.heightAnchor.constraint(equalToConstant: 34),
            mark.centerXAnchor.constraint(equalTo: plate.centerXAnchor),
            mark.centerYAnchor.constraint(equalTo: plate.centerYAnchor),
            label.leadingAnchor.constraint(equalTo: plate.trailingAnchor, constant: 11),
            label.centerYAnchor.constraint(equalTo: newSession.centerYAnchor),
            summary.trailingAnchor.constraint(equalTo: newSession.trailingAnchor, constant: -16),
            summary.centerYAnchor.constraint(equalTo: newSession.centerYAnchor),
            cells.trailingAnchor.constraint(equalTo: summary.leadingAnchor, constant: -7),
            cells.centerYAnchor.constraint(equalTo: newSession.centerYAnchor),
            cells.widthAnchor.constraint(equalToConstant: 12),
            cells.heightAnchor.constraint(equalToConstant: 12),
            voice.topAnchor.constraint(equalTo: topAnchor),
            voice.bottomAnchor.constraint(equalTo: bottomAnchor),
            voice.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -7),
        ])
        // Yields while the call strip narrows this side to the plus.
        let labelEnd = label.trailingAnchor.constraint(lessThanOrEqualTo: cells.leadingAnchor, constant: -10)
        labelEnd.priority = .defaultHigh
        labelEnd.isActive = true
        // No voice: "New session" spans the capsule.
        restConstraints = [newSession.trailingAnchor.constraint(equalTo: trailingAnchor)]
        // Idle voice: a 34pt waveform at the trailing end.
        idleVoiceConstraints = [
            voice.widthAnchor.constraint(equalToConstant: 34),
            newSession.trailingAnchor.constraint(equalTo: voice.leadingAnchor, constant: -2),
        ]
        // Live: the call strip takes everything after the plus.
        liveVoiceConstraints = [
            newSession.trailingAnchor.constraint(equalTo: plate.trailingAnchor, constant: 4),
            voice.leadingAnchor.constraint(equalTo: plate.trailingAnchor, constant: 12),
        ]
        NSLayoutConstraint.activate(restConstraints)
        registerForTraitChanges([UITraitTabAccessoryEnvironment.self]) { (self: AskAnythingAccessory, _) in
            // Inline (minimized tab bar): the plus and the live summary only.
            let inline = self.traitCollection.tabAccessoryEnvironment == .inline
            self.label.alpha = inline || self.voiceLive ? 0 : 1
            self.voice.setInline(inline)
        }
        update(AppModel.LiveCounts())
    }

    required init?(coder: NSCoder) { fatalError() }

    func update(_ live: AppModel.LiveCounts) {
        var parts: [String] = []
        if live.working > 0 { parts.append("\(live.working) working") }
        if live.awaiting > 0 { parts.append("\(live.awaiting) need\(live.awaiting == 1 ? "s" : "") you") }
        summary.text = parts.joined(separator: " · ")
        cells.isHidden = parts.isEmpty || voiceLive
        summary.isHidden = voiceLive
        cells.kind = live.working > 0 ? .spinner : .dot(StatusTone.input)
        newSession.accessibilityLabel = parts.isEmpty ? "New session" : "New session, " + parts.joined(separator: ", ")
    }

    /// Show voice when a host can run it, and the call strip while one runs.
    func updateVoice(_ controller: RemoteVoiceController) {
        let shown = controller.live || controller.available
        let live = controller.live
        voice.update(controller)
        guard shown != voiceShown || live != voiceLive else { return }
        voiceShown = shown
        voiceLive = live
        NSLayoutConstraint.deactivate(restConstraints + idleVoiceConstraints + liveVoiceConstraints)
        NSLayoutConstraint.activate(!shown ? restConstraints : live ? liveVoiceConstraints : idleVoiceConstraints)
        voice.isHidden = !shown
        let inline = traitCollection.tabAccessoryEnvironment == .inline
        let apply = {
            self.label.alpha = inline || live ? 0 : 1
            self.summary.isHidden = live
            self.cells.isHidden = live || self.summary.text?.isEmpty != false
            self.layoutIfNeeded()
        }
        if window == nil || UIAccessibility.isReduceMotionEnabled {
            apply()
        } else {
            UIView.animate(withDuration: 0.35, delay: 0, usingSpringWithDamping: 0.9, initialSpringVelocity: 0, options: [.beginFromCurrentState, .allowUserInteraction], animations: apply)
        }
    }
}
