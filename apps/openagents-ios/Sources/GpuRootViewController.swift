// Firm reason: 6 (UIKit Metal view host, CADisplayLink loop, UIScrollView native scroll driver, ComposerEditor text overlay, VoiceOver KitAccess bridge, and Photos/Files pickers over the compiled Coax GPU surface).
import Combine
import PhotosUI
import SwiftUI
import UIKit
import UniformTypeIdentifiers

struct GpuRootHostView: UIViewControllerRepresentable {
    @ObservedObject var store: AppStore

    func makeUIViewController(context: Context) -> GpuRootViewController {
        GpuRootViewController(store: store)
    }

    func updateUIViewController(_ uiViewController: GpuRootViewController, context: Context) {
        uiViewController.markDirtyAndWake()
    }
}

@MainActor
final class GpuRootViewController: UIViewController, UITextViewDelegate, UIScrollViewDelegate, KitAccessSource, PHPickerViewControllerDelegate, UIDocumentPickerDelegate {
    private let store: AppStore
    private let dictation = VoiceDictationController()
    private var cancellables: Set<AnyCancellable> = []

    private var surface: UnsafeMutableRawPointer?
    private var canvas: UIView?
    private var displayLink: CADisplayLink?
    private let scrollDriver = GPUScrollDriver()
    private var syncingScroll = false

    private var keyboardHeight: CGFloat = 0
    private var zoom: Float = 1.0
    private var needsStateSync = true
    private var lastSyncedSessionID: String?
    private var lastFocusNonce: UInt64 = 0
    private var access: KitAccess?
    private var pressedAtLaunch = false

    private lazy var editor: ComposerEditor = ComposerEditor(style: editorStyle())

    init(store: AppStore) {
        self.store = store
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) not supported")
    }

    deinit {
        displayLink?.invalidate()
        if let surface {
            oa_ios_close(surface)
        }
    }

    private func editorStyle() -> [Float] {
        var s = [Float](repeating: 0, count: 14)
        _ = oa_ios_style(&s)
        s[0] *= zoom
        s[1] *= zoom
        return s
    }

    override var preferredStatusBarStyle: UIStatusBarStyle { .lightContent }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        view.accessibilityIdentifier = "chat.screen"

        guard let resources = Bundle.main.path(forResource: "Components", ofType: nil),
              let ptr = resources.withCString({ oa_ios_open($0) }),
              let rawView = oa_ios_view(ptr) else {
            let label = UILabel()
            label.text = "OpenAgents Metal renderer could not initialize. Please restart the app."
            label.numberOfLines = 0
            label.textAlignment = .center
            label.textColor = .white
            label.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview(label)
            NSLayoutConstraint.activate([
                label.centerYAnchor.constraint(equalTo: view.centerYAnchor),
                label.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 24),
                label.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -24),
            ])
            return
        }

        self.surface = ptr
        oa_ios_on_change(ptr, { ctx in
            guard let ctx else { return }
            let vc = Unmanaged<GpuRootViewController>.fromOpaque(ctx).takeUnretainedValue()
            MainActor.assumeIsolated { vc.wake() }
        }, Unmanaged.passUnretained(self).toOpaque())

        let metalView = Unmanaged<UIView>.fromOpaque(rawView).takeUnretainedValue()
        self.canvas = metalView

        scrollDriver.frame = view.bounds
        scrollDriver.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        scrollDriver.contentInsetAdjustmentBehavior = .never
        scrollDriver.delegate = self
        scrollDriver.delaysContentTouches = false
        scrollDriver.isDirectionalLockEnabled = true
        scrollDriver.showsVerticalScrollIndicator = false
        scrollDriver.backgroundColor = .clear
        view.addSubview(scrollDriver)

        scrollDriver.addSubview(metalView)
        metalView.frame = scrollDriver.bounds
        metalView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        metalView.isAccessibilityElement = false
        metalView.accessibilityElementsHidden = true
        oa_ios_native_scroll(ptr, 1)

        access = makeAccess()
        updateTextSize()

        editor.delegate = self
        editor.host = view
        editor.attach = { [weak self] in self?.attachEditor() }
        editor.activate = { [weak self] in self?.focusComposer() ?? false }

        #if DEBUG
        applyDebugDemoStateIfRequested()
        if UserDefaults.standard.bool(forKey: "OADemoPreview") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { [weak self] in
                self?.applyDebugDemoStateIfRequested()
                self?.markDirtyAndWake()
            }
        }
        #endif

        store.objectWillChange
            .sink { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.markDirtyAndWake()
                }
            }
            .store(in: &cancellables)

        dictation.objectWillChange
            .sink { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.markDirtyAndWake()
                }
            }
            .store(in: &cancellables)

        let nc = NotificationCenter.default
        nc.addObserver(self, selector: #selector(keyboardMoved(_:)), name: UIResponder.keyboardWillChangeFrameNotification, object: nil)
        nc.addObserver(self, selector: #selector(keyboardHid), name: UIResponder.keyboardWillHideNotification, object: nil)
        nc.addObserver(self, selector: #selector(onWillResignActive), name: UIApplication.willResignActiveNotification, object: nil)
        nc.addObserver(self, selector: #selector(onDidBecomeActive), name: UIApplication.didBecomeActiveNotification, object: nil)
        nc.addObserver(self, selector: #selector(wake), name: UIAccessibility.reduceMotionStatusDidChangeNotification, object: nil)
        nc.addObserver(self, selector: #selector(wake), name: UIAccessibility.reduceTransparencyStatusDidChangeNotification, object: nil)
        nc.addObserver(self, selector: #selector(updateTextSize), name: UIContentSizeCategory.didChangeNotification, object: nil)
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        markDirtyAndWake()
        #if DEBUG
        runLaunchPressScriptIfRequested()
        #endif
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        wake()
    }

    override func viewSafeAreaInsetsDidChange() {
        super.viewSafeAreaInsetsDidChange()
        wake()
    }

    func markDirtyAndWake() {
        needsStateSync = true
        wake()
    }

    @objc func wake() {
        guard surface != nil, viewIfLoaded?.window != nil, UIApplication.shared.applicationState != .background else { return }
        if displayLink == nil {
            let link = CADisplayLink(target: GpuTickProxy(self), selector: #selector(GpuTickProxy.tick))
            link.preferredFrameRateRange = CAFrameRateRange(minimum: 30, maximum: 60, preferred: 60)
            link.add(to: .main, forMode: .common)
            displayLink = link
        }
        displayLink?.isPaused = false
    }

    @objc private func onWillResignActive() {
        displayLink?.invalidate()
        displayLink = nil
    }

    @objc private func onDidBecomeActive() {
        markDirtyAndWake()
    }

    @objc private func keyboardMoved(_ note: Notification) {
        guard let end = (note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue,
              let window = view.window else { return }
        let inView = view.convert(end, from: window.screen.coordinateSpace)
        keyboardHeight = max(0, view.bounds.maxY - inView.minY)
        wake()
    }

    @objc private func keyboardHid() {
        keyboardHeight = 0
        wake()
    }

    @objc private func updateTextSize() {
        guard let surface else { return }
        let traits = UITraitCollection(preferredContentSizeCategory: UIApplication.shared.preferredContentSizeCategory)
        let want = Float(UIFontMetrics(forTextStyle: .body).scaledValue(for: 17, compatibleWith: traits) / 17)
        let now = oa_ios_text_zoom(surface, want)
        guard now != zoom else { return }
        zoom = now
        editor.restyle(editorStyle())
        wake()
    }

    // MARK: - Frame Loop

    fileprivate func step() {
        guard let surface else { return }

        if needsStateSync {
            needsStateSync = false
            syncStoreToGpu(surface)
        }

        if store.composerFocusNonce != lastFocusNonce {
            lastFocusNonce = store.composerFocusNonce
            oa_ios_set_focus(surface, "chat.composer")
            _ = focusComposer()
        }

        let insets = view.safeAreaInsets
        let composing = editor.isFirstResponder
        let reducedMotion = UIAccessibility.isReduceMotionEnabled || UserDefaults.standard.bool(forKey: "OAReduceMotion")
        oa_ios_env(
            surface,
            Float(insets.top),
            Float(insets.bottom),
            Float(keyboardHeight),
            composing ? 1 : 0,
            composing ? Float(editor.textHeight(width: editor.bounds.width)) : 0,
            reducedMotion ? 1 : 0
        )
        oa_ios_transparency(surface, UIAccessibility.isReduceTransparencyEnabled ? 1 : 0)

        oa_ios_frame(surface)
        syncScroll(surface)
        drainActions(surface)
        followFocus(surface)
        placeEditor(surface)
        access?.refresh()

        if oa_ios_moving(surface) == 0 && !needsStateSync {
            displayLink?.isPaused = true
        }
    }

    // MARK: - AntigravityStore -> Coax GPU Surface Sync

    private func syncStoreToGpu(_ surface: UnsafeMutableRawPointer) {
        let computer = store.activeComputer
        let reachCode: Int32 = {
            guard let computer else { return OA_IOS_REACH_UNKNOWN }
            switch computer.reachability {
            case .reachable: return OA_IOS_REACH_OK
            case .checking: return OA_IOS_REACH_CHECKING
            case .refused, .unreachable: return OA_IOS_REACH_BAD
            case .unknown: return computer.canSendRPC ? OA_IOS_REACH_CHECKING : OA_IOS_REACH_UNKNOWN
            }
        }()

        let activeSession = store.activeSession
        let sessionTitle = activeSession.map { store.displayTitle(for: $0) } ?? ""
        let computerName = computer?.name ?? ""
        let pairedCount = Int32(store.computers.filter { $0.paired != nil || $0.isLocalMac }.count)
        let workspaceName = store.workspaceName

        oa_ios_set_header(
            surface,
            computerName,
            sessionTitle,
            workspaceName,
            reachCode,
            pairedCount,
            activeSession != nil ? 1 : 0,
            store.hasOlderMessages ? 1 : 0,
            store.hasMoreSessions ? 1 : 0
        )

        let bannerErr = dictation.errorMessage ?? store.bannerError ?? store.rateLimitBanner?.message ?? ""
        let toast = store.statusToast ?? ""
        oa_ios_set_banners(surface, bannerErr, Int32(store.pendingOutbox.count), toast)

        if let pending = store.pendingInteraction {
            oa_ios_set_approval(surface, 1, pending.prompt, pending.warning)
            oa_ios_approval_options_clear(surface)
            for opt in pending.options.prefix(6) {
                oa_ios_approval_options_push(surface, opt.id, opt.label, opt.kind)
            }
        } else {
            oa_ios_set_approval(surface, 0, "", "")
            oa_ios_approval_options_clear(surface)
        }

        if let usage = store.activeUsage, usage.totalTokens > 0 {
            let pct = Int32(max(0, min(100, Int(usage.contextPercent.rounded()))))
            oa_ios_set_usage(surface, "\(pct)% · \(usage.summaryLabel)", pct)
        } else {
            oa_ios_set_usage(surface, "", 0)
        }
        oa_ios_set_queued(surface, store.queuedDraft ?? "")

        // Slash (/) and @ mention autocomplete suggestions above composer
        let mentions = store.matchingMentions
        let slashes = mentions.isEmpty ? store.matchingSlashCommands : []
        if !mentions.isEmpty {
            oa_ios_suggestions_clear(surface, 1)
            for m in mentions.prefix(8) {
                oa_ios_suggestions_push(surface, m.id, m.label, m.subtitle)
            }
        } else if !slashes.isEmpty {
            oa_ios_suggestions_clear(surface, 0)
            for cmd in slashes.prefix(8) {
                oa_ios_suggestions_push(surface, cmd.id, cmd.command, cmd.subtitle)
            }
        } else {
            oa_ios_suggestions_clear(surface, 0)
        }

        // Inline Todos / Task Progress card
        oa_ios_todos_clear(surface)
        for todo in store.activeTodos.prefix(16) {
            let st: Int32 = todo.isCompleted ? OA_IOS_TODO_COMPLETED : (todo.isInProgress ? OA_IOS_TODO_IN_PROGRESS : OA_IOS_TODO_PENDING)
            oa_ios_todos_push(surface, todo.content, st)
        }

        // Workspace folders picker catalog
        oa_ios_folders_clear(surface)
        for folder in store.folders.prefix(32) {
            let lbl = folder.isEmpty ? (workspaceName.isEmpty ? "Workspace root" : workspaceName) : folder
            oa_ios_folders_push(surface, folder, lbl, folder == store.selectedFolder ? 1 : 0)
        }

        let modelOpt = store.selectedModelOption
        let effortLabel = store.selectedModelSupportsThinking ? store.selectedEffort.label : ""
        oa_ios_set_composer(
            surface,
            modelOpt.label,
            effortLabel,
            store.reviewMode.label,
            Int32(store.pendingAttachments.count),
            dictation.isListening ? 1 : 0
        )

        let isBusy = activeSession?.busy == true
        let isAwaiting = store.isAwaitingTurnStart || store.isSending
        let liveStatus = store.activeTurnLiveStatus
        let elapsedSecs: Int32 = {
            if let started = liveStatus?.startedAt {
                return Int32(max(0, Date().timeIntervalSince(started)))
            }
            return 0
        }()
        let workLabel = liveStatus?.phaseLabel ?? (isAwaiting && !isBusy ? "Sending…" : "Working")
        oa_ios_set_working(surface, isBusy ? 1 : 0, isAwaiting ? 1 : 0, elapsedSecs, workLabel)

        // Sessions list for GPU sidebar (Working + Recents)
        oa_ios_sessions_clear(surface)
        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        let orderedSessions = store.workingSessions + store.recentSessions
        for session in orderedSessions.prefix(64) {
            let title = store.displayTitle(for: session)
            let repo = store.sessionMetaByID[session.id]?.folder ?? workspaceName
            let branch = session.model.isEmpty ? "antigravity" : session.model
            let attention = !session.interactionID.isEmpty
            let statusText = attention ? "Needs you" : Self.relativeAge(nowMs: nowMs, atMs: session.updatedAtMs)
            oa_ios_sessions_push(
                surface,
                session.id,
                title,
                repo,
                branch,
                statusText,
                session.busy ? 1 : 0,
                session.id == store.activeSessionID ? 1 : 0,
                attention ? 1 : 0,
                0
            )
            if session.id == store.activeSessionID {
                let lastAsk = store.items.last(where: { $0.role == .user }).map { ParsedUserMessage.parse($0.text).text } ?? ""
                let lastTool = store.items.last(where: { $0.role == .tool }).map {
                    let target = $0.path.isEmpty ? $0.text : $0.path
                    return "\($0.toolName.isEmpty ? "Tool" : $0.toolName) \(target)"
                } ?? ""
                let lastReply = store.items.last(where: { $0.role == .assistant })?.text ?? ""
                oa_ios_session_set_preview(surface, session.id, lastAsk, lastTool, lastReply)
            } else if !session.note.isEmpty {
                oa_ios_session_set_preview(surface, session.id, "", "", session.note)
            }
        }

        // Transcript items
        let sessionChanged = store.activeSessionID != lastSyncedSessionID
        lastSyncedSessionID = store.activeSessionID
        oa_ios_items_clear(surface, sessionChanged ? 1 : 0)
        for item in store.items.suffix(192) {
            let roleCode: Int32
            let displayText: String
            switch item.role {
            case .user:
                roleCode = OA_IOS_ROLE_USER
                let parsed = ParsedUserMessage.parse(item.text)
                if parsed.images.isEmpty {
                    displayText = parsed.text
                } else {
                    let imgTag = parsed.images.map { "[\($0.name)]" }.joined(separator: " ")
                    displayText = parsed.text.isEmpty ? imgTag : "\(parsed.text)\n\(imgTag)"
                }
            case .assistant:
                roleCode = OA_IOS_ROLE_ASSISTANT
                displayText = item.text
            case .thought:
                roleCode = OA_IOS_ROLE_THOUGHT
                displayText = item.text
            case .tool:
                roleCode = OA_IOS_ROLE_TOOL
                displayText = item.text
            case .notice, .unknown:
                roleCode = OA_IOS_ROLE_NOTICE
                displayText = item.text
            }
            let failed = item.noticeKind == "error" || item.noticeKind == "failed"
            let elapsed: Int32 = item.startedAtMs > 0 ? Int32(max(0, (nowMs - item.startedAtMs) / 1000)) : 0
            let toolLabel: String
            if item.isSubagent && !item.subagentType.isEmpty {
                var parts = [item.subagentType]
                if item.isBackground { parts.append("bg") }
                if !item.worktreeBranch.isEmpty { parts.append(item.worktreeBranch) }
                toolLabel = parts.joined(separator: " · ")
            } else {
                toolLabel = item.toolName
            }
            oa_ios_items_push(
                surface,
                roleCode,
                item.id,
                displayText,
                toolLabel,
                item.path,
                item.diff,
                item.complete ? 1 : 0,
                failed ? 1 : 0,
                item.isSubagent ? 1 : 0,
                item.diffCut ? 1 : 0,
                Int32(item.subagentCalls),
                elapsed
            )
        }

        // Picker catalogs: Models, Efforts, Modes
        oa_ios_models_clear(surface)
        for m in store.availableModels.prefix(24) {
            let detail = m.supportsThinking ? "\(m.family) · Reasoning" : m.family
            oa_ios_models_push(surface, m.id, m.label, detail, m.id == store.selectedModelID ? 1 : 0)
        }

        oa_ios_efforts_clear(surface)
        for e in store.selectedModelOption.supportedEfforts.prefix(8) {
            oa_ios_efforts_push(surface, e.rawValue, e.label, Self.effortDetail(e), e == store.selectedEffort ? 1 : 0)
        }

        oa_ios_modes_clear(surface)
        for mode in ReviewMode.allCases.prefix(6) {
            oa_ios_modes_push(surface, mode.rawValue, mode.label, mode.description, mode == store.reviewMode ? 1 : 0)
        }

        // Sync draft from store (e.g. voice dictation or slash command or cleared after send)
        if editor.markedTextRange == nil {
            let gpuDraft = currentGpuDraft(surface)
            if gpuDraft != store.draftText {
                _ = oa_ios_draft_set(surface, store.draftText)
            }
        }
    }

    private static func effortDetail(_ e: ThinkingEffort) -> String {
        switch e {
        case .none: return "No reasoning tokens"
        case .minimal: return "Minimal thinking overhead"
        case .low: return "Fastest replies"
        case .medium: return "Balanced speed and reasoning"
        case .high: return "Deeper multi-step reasoning"
        case .xhigh: return "Maximum thinking depth"
        case .max: return "Full reasoning budget"
        }
    }

    private static func relativeAge(nowMs: Int64, atMs: Int64) -> String {
        guard atMs > 0 else { return "" }
        let sec = max(0, (nowMs - atMs) / 1000)
        if sec < 60 { return "now" }
        if sec < 3600 { return "\(sec / 60)m" }
        if sec < 86400 { return "\(sec / 3600)h" }
        if sec < 14 * 86400 { return "\(sec / 86400)d" }
        return "\(sec / (7 * 86400))w"
    }

    // MARK: - Actions from Coax GPU Surface

    private func drainActions(_ surface: UnsafeMutableRawPointer) {
        while true {
            let act = oa_ios_take_action(surface)
            if act == OA_IOS_ACT_NONE { break }
            var argBuf = [CChar](repeating: 0, count: 512)
            _ = oa_ios_action_arg(surface, &argBuf, Int32(argBuf.count))
            let arg = String(cString: argBuf)
            handleAction(act, arg: arg, surface: surface)
        }
    }

    private func handleAction(_ act: Int32, arg: String, surface: UnsafeMutableRawPointer) {
        switch act {
        case OA_IOS_ACT_FOCUS_COMPOSER:
            _ = focusComposer()
        case OA_IOS_ACT_SEND:
            let text = currentGpuDraft(surface)
            store.draftText = text
            if editor.isFirstResponder {
                _ = editor.resignFirstResponder()
            }
            oa_ios_blur(surface)
            Task { await store.sendMessage() }
        case OA_IOS_ACT_STOP:
            Task { await store.stopActiveTurn() }
        case OA_IOS_ACT_NEW_CHAT:
            store.startNewChat()
            _ = focusComposer()
        case OA_IOS_ACT_SELECT_SESSION:
            if let session = store.sessions.first(where: { $0.id == arg }) {
                store.selectSession(session)
            }
        case OA_IOS_ACT_OPEN_COMPUTERS:
            dismissKeyboard(surface)
            store.isComputersPresented = true
        case OA_IOS_ACT_OPEN_WORKSPACE:
            dismissKeyboard(surface)
            store.isWorkspacePresented = true
        case OA_IOS_ACT_OPEN_SUBAGENT:
            dismissKeyboard(surface)
            if let item = store.items.first(where: { $0.id == arg }) {
                store.openSubagent(item)
            }
        case OA_IOS_ACT_OPEN_HUB:
            dismissKeyboard(surface)
            store.isHubPresented = true
        case OA_IOS_ACT_OPEN_ATTACH:
            dismissKeyboard(surface)
            presentAttachActionSheet()
        case OA_IOS_ACT_CLEAR_ATTACH:
            store.pendingAttachments.removeAll()
        case OA_IOS_ACT_CHOSE_MODEL:
            store.chooseModel(arg)
        case OA_IOS_ACT_CHOSE_EFFORT:
            if let eff = ThinkingEffort(rawValue: arg) {
                store.chooseEffort(eff)
            }
        case OA_IOS_ACT_CHOSE_MODE:
            if let mode = ReviewMode(rawValue: arg) {
                store.chooseReviewMode(mode)
            }
        case OA_IOS_ACT_TOGGLE_MIC:
            dictation.toggle(existingText: store.draftText) { [weak self] spoken in
                self?.store.draftText = spoken
                self?.markDirtyAndWake()
            }
        case OA_IOS_ACT_APPROVE:
            Task { await store.respondToInteraction(allow: true) }
        case OA_IOS_ACT_DENY:
            Task { await store.respondToInteraction(allow: false) }
        case OA_IOS_ACT_DISMISS_ERROR:
            store.bannerError = nil
            dictation.errorMessage = nil
        case OA_IOS_ACT_RETRY_OUTBOX:
            Task { await store.flushPendingOutbox(interactive: true) }
        case OA_IOS_ACT_COPY_TEXT:
            var buf = [CChar](repeating: 0, count: 16385)
            _ = oa_ios_copy_get(surface, &buf, Int32(buf.count))
            let copied = String(cString: buf)
            if !copied.isEmpty {
                UIPasteboard.general.string = copied
                let gen = UIImpactFeedbackGenerator(style: .light)
                gen.impactOccurred()
            }
        case OA_IOS_ACT_OPEN_FILE:
            dismissKeyboard(surface)
            store.openFileFromLink(arg)
        case OA_IOS_ACT_LOAD_OLDER:
            Task { await store.loadOlderMessages() }
        case OA_IOS_ACT_LOAD_MORE_SESSIONS:
            Task { await store.loadMoreSessions() }
        case OA_IOS_ACT_APPLY_SUGGESTION:
            if let mention = store.matchingMentions.first(where: { $0.id == arg }) {
                applyMentionInsertion(mention.insertion, surface: surface)
            } else if let cmd = store.matchingSlashCommands.first(where: { $0.id == arg }) {
                store.applySlashCommand(cmd)
                _ = oa_ios_draft_set(surface, store.draftText)
                editor.set(store.draftText)
                _ = focusComposer()
            }
        case OA_IOS_ACT_APPROVAL_OPTION:
            let opt = store.pendingInteraction?.options.first(where: { $0.id == arg })
            let kindLower = (opt?.kind ?? arg).lowercased()
            let allow = !kindLower.contains("reject") && !kindLower.contains("deny")
            Task { await store.respondToInteraction(allow: allow, optionID: arg) }
        case OA_IOS_ACT_CLEAR_QUEUED:
            store.clearQueuedDraft()
        case OA_IOS_ACT_CHOSE_FOLDER:
            store.selectedFolder = arg
        case OA_IOS_ACT_DASHBOARD_CHANGED:
            if arg == "1" {
                dismissKeyboard(surface)
            }
        default:
            break
        }
        markDirtyAndWake()
    }

    private func applyMentionInsertion(_ insertion: String, surface: UnsafeMutableRawPointer) {
        let current = store.draftText
        if let range = current.range(of: "@[^\\s]*$", options: .regularExpression) {
            store.draftText = current.replacingCharacters(in: range, with: "\(insertion) ")
        } else {
            store.draftText = current.isEmpty ? "\(insertion) " : "\(current) \(insertion) "
        }
        _ = oa_ios_draft_set(surface, store.draftText)
        editor.set(store.draftText)
        _ = focusComposer()
    }

    // MARK: - Hardware Keyboard Shortcuts (Desktop Parity: Cmd+D, Cmd+N, Cmd+Up/Down, Cmd+1..9)

    override var keyCommands: [UIKeyCommand]? {
        var cmds: [UIKeyCommand] = [
            UIKeyCommand(title: "Dashboard", action: #selector(cmdToggleDashboard), input: "d", modifierFlags: .command),
            UIKeyCommand(title: "New Chat", action: #selector(cmdNewChat), input: "n", modifierFlags: .command),
            UIKeyCommand(title: "Toggle Sidebar", action: #selector(cmdToggleSidebar), input: "\\", modifierFlags: .command),
            UIKeyCommand(title: "Jump to Top", action: #selector(cmdJumpTop), input: UIKeyCommand.inputUpArrow, modifierFlags: .command),
            UIKeyCommand(title: "Jump to Bottom", action: #selector(cmdJumpBottom), input: UIKeyCommand.inputDownArrow, modifierFlags: .command),
        ]
        for n in 1...9 {
            cmds.append(UIKeyCommand(title: "Chat \(n)", action: #selector(cmdSelectChatByIndex(_:)), input: "\(n)", modifierFlags: .command))
        }
        return cmds
    }

    @objc private func cmdToggleDashboard() {
        guard let surface else { return }
        let next: Int32 = oa_ios_dashboard_open(surface) != 0 ? 0 : 1
        if next != 0 { dismissKeyboard(surface) }
        oa_ios_dashboard_set(surface, next)
        markDirtyAndWake()
    }

    @objc private func cmdNewChat() {
        guard let surface else { return }
        oa_ios_dashboard_set(surface, 0)
        oa_ios_drawer_set(surface, 0)
        store.startNewChat()
        _ = focusComposer()
        markDirtyAndWake()
    }

    @objc private func cmdToggleSidebar() {
        guard let surface else { return }
        let next: Int32 = oa_ios_drawer_open(surface) != 0 ? 0 : 1
        if next != 0 { dismissKeyboard(surface) }
        oa_ios_drawer_set(surface, next)
        markDirtyAndWake()
    }

    @objc private func cmdJumpTop() {
        guard let surface else { return }
        oa_ios_scroll_top(surface)
        wake()
    }

    @objc private func cmdJumpBottom() {
        guard let surface else { return }
        oa_ios_scroll_bottom(surface)
        wake()
    }

    @objc private func cmdSelectChatByIndex(_ sender: UIKeyCommand) {
        guard let surface, let input = sender.input, let idx = Int(input), idx >= 1 else { return }
        let ordered = store.workingSessions + store.recentSessions
        guard idx - 1 < ordered.count else { return }
        oa_ios_dashboard_set(surface, 0)
        oa_ios_drawer_set(surface, 0)
        store.selectSession(ordered[idx - 1])
        markDirtyAndWake()
    }

    private func dismissKeyboard(_ surface: UnsafeMutableRawPointer) {
        if editor.isFirstResponder {
            _ = editor.resignFirstResponder()
        }
        editor.removeFromSuperview()
        oa_ios_blur(surface)
    }

    // MARK: - Native Attachments & Tools Sheet

    private func presentAttachActionSheet() {
        let sheet = UIAlertController(title: nil, message: nil, preferredStyle: .actionSheet)
        sheet.overrideUserInterfaceStyle = .dark
        sheet.addAction(UIAlertAction(title: "Photo Library", style: .default) { [weak self] _ in
            self?.presentPhotoPicker()
        })
        sheet.addAction(UIAlertAction(title: "Attach File", style: .default) { [weak self] _ in
            self?.presentDocumentPicker()
        })
        sheet.addAction(UIAlertAction(title: "Side Question (/btw)", style: .default) { [weak self] _ in
            self?.store.isSideQuestionPresented = true
        })
        if (store.activeSession?.busy == true || store.isAwaitingTurnStart), !store.draftText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            sheet.addAction(UIAlertAction(title: "Queue Follow-Up", style: .default) { [weak self] _ in
                self?.store.queueCurrentDraft()
                self?.markDirtyAndWake()
            })
        }
        sheet.addAction(UIAlertAction(title: "Antigravity Hub", style: .default) { [weak self] _ in
            self?.store.isHubPresented = true
        })
        sheet.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        if let popover = sheet.popoverPresentationController {
            popover.sourceView = view
            popover.sourceRect = CGRect(x: 32, y: view.bounds.maxY - 90, width: 44, height: 44)
        }
        present(sheet, animated: true)
    }

    private func presentPhotoPicker() {
        var config = PHPickerConfiguration(photoLibrary: .shared())
        config.filter = .images
        config.selectionLimit = 1
        let picker = PHPickerViewController(configuration: config)
        picker.delegate = self
        picker.overrideUserInterfaceStyle = .dark
        present(picker, animated: true)
    }

    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        picker.dismiss(animated: true)
        guard let provider = results.first?.itemProvider else { return }
        if provider.canLoadObject(ofClass: UIImage.self) {
            provider.loadDataRepresentation(forTypeIdentifier: UTType.image.identifier) { [weak self] data, _ in
                guard let data else { return }
                let att = FileAttachment.prepareImage(data: data, name: "photo.jpg")
                Task { @MainActor [weak self] in
                    self?.store.addAttachment(att)
                    self?.markDirtyAndWake()
                }
            }
        }
    }

    private func presentDocumentPicker() {
        let types: [UTType] = [.plainText, .sourceCode, .json, .image, .pdf, .data]
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: types, asCopy: true)
        picker.delegate = self
        picker.overrideUserInterfaceStyle = .dark
        present(picker, animated: true)
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let url = urls.first, let data = try? Data(contentsOf: url) else { return }
        let ext = url.pathExtension.lowercased()
        if ["png", "jpg", "jpeg", "heic", "webp", "gif"].contains(ext) {
            store.addAttachment(FileAttachment.prepareImage(data: data, name: url.lastPathComponent))
        } else if let text = String(data: data, encoding: .utf8)?.nilIfContainsBinaryNUL() {
            store.addAttachment(FileAttachment(
                name: url.lastPathComponent,
                mimeType: "text/plain",
                textPreview: text.truncatedToUTF8Bytes(4_000),
                byteCount: data.count
            ))
        } else {
            let b64 = data.count <= 24_000 ? data.base64EncodedString() : nil
            store.addAttachment(FileAttachment(
                name: url.lastPathComponent,
                mimeType: UTType(filenameExtension: ext)?.preferredMIMEType ?? "application/octet-stream",
                textPreview: nil,
                byteCount: data.count,
                binaryBase64: b64
            ))
        }
        markDirtyAndWake()
    }

    // MARK: - Native Scroll Driver

    private func syncScroll(_ surface: UnsafeMutableRawPointer) {
        var facts = [Float](repeating: 0, count: 3)
        guard oa_ios_scroll_info(surface, &facts) != 0 else { return }
        syncingScroll = true
        defer { syncingScroll = false }
        scrollDriver.isScrollEnabled = facts[0] != 0 && facts[2] > 0.5
        scrollDriver.contentSize = CGSize(
            width: scrollDriver.bounds.width,
            height: scrollDriver.bounds.height + CGFloat(max(0, facts[2]))
        )
        if !scrollDriver.isTracking && !scrollDriver.isDragging && !scrollDriver.isDecelerating {
            let target = CGPoint(x: 0, y: CGFloat(facts[1]))
            if abs(scrollDriver.contentOffset.y - target.y) > 0.5 {
                scrollDriver.contentOffset = target
            }
        }
        canvas?.frame = CGRect(origin: scrollDriver.contentOffset, size: scrollDriver.bounds.size)
    }

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        canvas?.frame = CGRect(origin: scrollView.contentOffset, size: scrollView.bounds.size)
        if !syncingScroll, let surface {
            oa_ios_scroll_to(surface, Float(scrollView.contentOffset.y))
            wake()
        }
    }

    func scrollViewShouldScrollToTop(_ scrollView: UIScrollView) -> Bool {
        if let surface {
            oa_ios_scroll_top(surface)
            wake()
        }
        return false
    }

    // MARK: - Platform Composer Editor (UITextView over GPU Composer)

    private func currentGpuDraft(_ surface: UnsafeMutableRawPointer) -> String {
        var buf = [CChar](repeating: 0, count: 16385)
        _ = oa_ios_draft_get(surface, &buf, Int32(buf.count))
        return String(cString: buf)
    }

    private func focusID(_ surface: UnsafeMutableRawPointer) -> String {
        var buf = [CChar](repeating: 0, count: 64)
        _ = oa_ios_focus(surface, &buf, Int32(buf.count))
        return String(cString: buf)
    }

    private func editorBox(_ surface: UnsafeMutableRawPointer) -> CGRect? {
        var b = [Float](repeating: 0, count: 6)
        guard oa_ios_editor(surface, &b) != 0 else { return nil }
        return CGRect(x: CGFloat(b[0]), y: CGFloat(b[1]), width: CGFloat(b[2]), height: CGFloat(b[3]))
    }

    private func attachEditor() {
        guard let surface else { return }
        if editor.superview == nil {
            view.addSubview(editor)
        }
        if let frame = editorBox(surface) {
            editor.frame = frame
        }
    }

    @discardableResult
    private func focusComposer() -> Bool {
        attachEditor()
        let ok = editor.becomeFirstResponder()
        wake()
        return ok
    }

    private func followFocus(_ surface: UnsafeMutableRawPointer) {
        let id = focusID(surface)
        if id == "chat.composer" {
            if editor.accessibilityIdentifier != id {
                editor.accessibilityIdentifier = id
            }
            attachEditor()
            placeEditor(surface)
            if !editor.isFirstResponder {
                _ = editor.becomeFirstResponder()
            }
        } else {
            if editor.isFirstResponder {
                _ = editor.resignFirstResponder()
            }
            if editor.superview != nil {
                editor.removeFromSuperview()
            }
        }
    }

    private func placeEditor(_ surface: UnsafeMutableRawPointer) {
        let held = currentGpuDraft(surface)
        if editor.markedTextRange == nil && editor.text != held {
            editor.set(held)
        }
        guard editor.superview != nil, let frame = editorBox(surface) else { return }
        if editor.frame != frame {
            editor.frame = frame
        }
        editor.isScrollEnabled = editor.textHeight(width: frame.width) + 12 > frame.height + 1
    }

    func textViewDidBeginEditing(_ textView: UITextView) {
        wake()
    }

    func textViewDidEndEditing(_ textView: UITextView) {
        editor.removeFromSuperview()
        if let surface, focusID(surface) == "chat.composer" {
            oa_ios_blur(surface)
        }
        wake()
    }

    func textViewDidChange(_ textView: UITextView) {
        guard let surface, textView.markedTextRange == nil else { return }
        let txt = textView.text ?? ""
        _ = oa_ios_draft_set(surface, txt)
        if store.draftText != txt {
            store.draftText = txt
        }
        markDirtyAndWake()
    }

    // MARK: - VoiceOver (KitAccessSource)

    var accessStamp: Int32 { surface.map { oa_ios_rows_stamp($0) } ?? 0 }
    var accessLayer: Int { surface.map { Int(oa_ios_layer($0)) } ?? 0 }
    var accessCount: Int { surface.map { Int(oa_ios_rows($0)) } ?? 0 }

    func accessRow(_ i: Int) -> KitRow? {
        guard let surface else { return nil }
        var row = [Float](repeating: 0, count: 8)
        var id = [CChar](repeating: 0, count: 64)
        var label = [CChar](repeating: 0, count: 4100)
        var value = [CChar](repeating: 0, count: 256)
        guard oa_ios_row(surface, Int32(i), &row, &id, Int32(id.count), &label, Int32(label.count)) != 0 else { return nil }
        _ = oa_ios_row_value(surface, Int32(i), &value, Int32(value.count))
        return KitRow(
            id: String(cString: id),
            label: String(cString: label),
            value: String(cString: value),
            role: Int(row[4]),
            state: Int(row[5]),
            parent: Int(row[6]),
            focus: Int(row[7]),
            frame: CGRect(x: CGFloat(row[0]), y: CGFloat(row[1]), width: CGFloat(row[2]), height: CGFloat(row[3]))
        )
    }

    func accessActivate(_ id: String) -> Bool {
        guard let surface else { return false }
        let pressed = oa_ios_activate(surface, id) != 0
        wake()
        return pressed
    }

    private func makeAccess() -> KitAccess {
        let bridge = KitAccess(container: view, source: self)
        bridge.stand = { [weak self] row in
            guard let self else { return .skip }
            if row.id == "chat.composer" {
                self.editor.box = row.frame
                self.editor.accessibilityIdentifier = row.id
                self.editor.accessibilityLabel = row.label
                self.editor.placeholder = row.label
                return .view(self.editor)
            }
            return .own
        }
        bridge.scroll = { [weak self] dir in
            guard let self, let surface = self.surface else { return false }
            let d: Int32 = dir == .up ? 1 : (dir == .down ? -1 : 0)
            guard d != 0, oa_ios_scroll_page(surface, d) != 0 else { return false }
            self.wake()
            UIAccessibility.post(notification: .pageScrolled, argument: nil)
            return true
        }
        bridge.escape = { [weak self] in
            guard let self, let surface = self.surface else { return false }
            for id in ["picker.scrim", "drawer.scrim"] where oa_ios_activate(surface, id) != 0 {
                self.wake()
                return true
            }
            return false
        }
        return bridge
    }

    // MARK: - Debug Simulator Inspection Helpers

    #if DEBUG
    private func applyDebugDemoStateIfRequested() {
        guard UserDefaults.standard.bool(forKey: "OADemoPreview") else { return }
        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        let hostID = "demo-macbook-pro"
        let demoComputer = ComputerEntry(
            id: hostID,
            hostKey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            name: "Christopher's MacBook Pro",
            ip: "100.127.107.31",
            dns: "macbook-pro-m5.ts.net",
            port: 47109,
            paired: nil,
            reachability: .reachable,
            isLocalMac: true
        )
        store.computers = [demoComputer]
        store.activeComputerID = hostID
        store.workspaceName = "openagents"
        store.sessions = [
            ChatSessionSummary(
                id: "s-live-1", workspaceID: "openagents",
                title: "Port iOS UI to shared Metal GPU components",
                managed: true, control: "available", generation: 1, revision: 4,
                busy: true, interactionID: "", updatedAtMs: nowMs - 30_000,
                model: "gemini-4-neon", effort: "medium", mode: "auto",
                desktopSurface: false, note: ""
            ),
            ChatSessionSummary(
                id: "s-recent-1", workspaceID: "openagents",
                title: "Verify Direct Tailscale push & grants",
                managed: true, control: "available", generation: 1, revision: 9,
                busy: false, interactionID: "", updatedAtMs: nowMs - 1_200_000,
                model: "gemini-4-neon", effort: "high", mode: "auto",
                desktopSurface: false, note: ""
            ),
            ChatSessionSummary(
                id: "s-recent-2", workspaceID: "openagents",
                title: "Publish signed macOS GPU build to DMG",
                managed: true, control: "available", generation: 1, revision: 6,
                busy: false, interactionID: "", updatedAtMs: nowMs - 7_200_000,
                model: "gemini-4-carbon", effort: "medium", mode: "plan",
                desktopSurface: false, note: ""
            ),
        ]
        if UserDefaults.standard.bool(forKey: "OADemoConversation") {
            store.activeSessionID = "s-recent-1"
            store.items = [
                TranscriptItem(
                    id: "u1", role: .user,
                    text: "Check the iOS Metal GPU shader and confirm zero stack frames exceed 512 KB.",
                    toolName: "", complete: true, startedAtMs: nowMs - 90_000,
                    subagentCalls: 0, subagentNow: "", path: "", diff: "", diffCut: false, noticeKind: ""
                ),
                TranscriptItem(
                    id: "t1", role: .thought,
                    text: "Inspecting `apps/openagents-ios/gpu-page.coax` and verifying `-Wframe-larger-than=524288` on `arm64-apple-ios18.0`.",
                    toolName: "", complete: true, startedAtMs: nowMs - 85_000,
                    subagentCalls: 0, subagentNow: "", path: "", diff: "", diffCut: false, noticeKind: ""
                ),
                TranscriptItem(
                    id: "tl1", role: .tool,
                    text: "apps/openagents-ios/generate.sh",
                    toolName: "Bash", complete: true, startedAtMs: nowMs - 82_000,
                    subagentCalls: 0, subagentNow: "", path: "apps/openagents-ios/generate.sh",
                    diff: "", diffCut: false, noticeKind: ""
                ),
                TranscriptItem(
                    id: "tl2", role: .tool,
                    text: "Updated gpu-page.coax",
                    toolName: "Edit", complete: true, startedAtMs: nowMs - 78_000,
                    subagentCalls: 0, subagentNow: "", path: "apps/openagents-ios/gpu-page.coax",
                    diff: "@@ -1,2 +1,3 @@\n-let old = SwiftUI()\n+let ui = MetalGpuSurface()\n+ui.render(noirPhoneSkin)",
                    diffCut: false, noticeKind: ""
                ),
                TranscriptItem(
                    id: "a1", role: .assistant,
                    text: "Everything is compiled and verified on `feat/gpu-ios-tailscale`:\n\n- **Metal shader**: `Components/components.metal` (27 KB) with Geist & 70 SVG icons\n- **Stack frame budget**: Passed `-Os` and `-Os -fno-inline` under 512 KB\n- **Transport**: Direct Tailscale grants + live push active (`100.127.107.31:47109`)\n\n```swift\nlet surface = oa_ios_open(resourcesPath)\noa_ios_frame(surface)\n```",
                    toolName: "", complete: true, startedAtMs: nowMs - 70_000,
                    subagentCalls: 0, subagentNow: "", path: "", diff: "", diffCut: false, noticeKind: ""
                ),
            ]
        }
    }

    private func runLaunchPressScriptIfRequested() {
        guard !pressedAtLaunch, let ids = UserDefaults.standard.string(forKey: "OAPress"), !ids.isEmpty else { return }
        pressedAtLaunch = true
        for (i, id) in ids.split(separator: ",").enumerated() {
            let trimmed = String(id).trimmingCharacters(in: .whitespacesAndNewlines)
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.7 * Double(i + 1)) { [weak self] in
                guard let self, let surface = self.surface else { return }
                _ = oa_ios_activate(surface, trimmed)
                self.wake()
            }
        }
    }
    #endif
}

@MainActor
private final class GpuTickProxy: NSObject {
    weak var controller: GpuRootViewController?
    init(_ controller: GpuRootViewController) { self.controller = controller }
    @objc func tick() { controller?.step() }
}

private final class GPUScrollDriver: UIScrollView {
    override func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        if gestureRecognizer === panGestureRecognizer {
            let v = panGestureRecognizer.velocity(in: self)
            if abs(v.x) > abs(v.y) { return false }
        }
        return super.gestureRecognizerShouldBegin(gestureRecognizer)
    }
}
