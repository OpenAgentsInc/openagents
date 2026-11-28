// Firm reason: 6 (UIKit Metal view host, CADisplayLink loop, UIScrollView native scroll driver, ComposerEditor text overlay, VoiceOver KitAccess bridge, and Photos/Files pickers over the compiled Coax GPU surface in 100% OpenAgents Cloud mode).
import Combine
import PhotosUI
import SwiftUI
import UIKit
import UniformTypeIdentifiers

struct GpuRootHostView: UIViewControllerRepresentable {
    @ObservedObject var account: MobileAccountController
    @ObservedObject var cloudChat: MobileCloudChat
    @ObservedObject var wallet: MobileWalletController
    @ObservedObject var shell: MobileShellState

    func makeUIViewController(context: Context) -> GpuRootViewController {
        GpuRootViewController(
            account: account,
            cloudChat: cloudChat,
            wallet: wallet,
            shell: shell
        )
    }

    func updateUIViewController(_ uiViewController: GpuRootViewController, context: Context) {
        uiViewController.markDirtyAndWake()
    }
}

@MainActor
final class GpuRootViewController: UIViewController, UITextViewDelegate, UIScrollViewDelegate, KitAccessSource, PHPickerViewControllerDelegate, UIDocumentPickerDelegate {
    private let account: MobileAccountController
    private let cloudChat: MobileCloudChat
    private let wallet: MobileWalletController
    private let shell: MobileShellState
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
    private var lastSyncedViewKey: String?
    private var lastFocusNonce: UInt64 = 0
    private var access: KitAccess?
    private var pressedAtLaunch = false

    private lazy var editor: ComposerEditor = ComposerEditor(style: editorStyle())

    init(
        account: MobileAccountController,
        cloudChat: MobileCloudChat,
        wallet: MobileWalletController,
        shell: MobileShellState
    ) {
        self.account = account
        self.cloudChat = cloudChat
        self.wallet = wallet
        self.shell = shell
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

        dictation.speechRate = cloudChat.voiceSpeechRate
        dictation.vadSilenceCommitMs = Int64(cloudChat.voiceVadSilenceMs)
        cloudChat.onAssistantStreamUpdate = { [weak self] itemID, text, complete in
            guard let self, self.dictation.isDuplexActive else { return }
            self.dictation.feedAssistantStream(itemID: itemID, fullText: text, complete: complete)
            self.markDirtyAndWake()
        }

        dictation.objectWillChange
            .sink { [weak self] _ in
                Task { @MainActor [weak self] in self?.markDirtyAndWake() }
            }
            .store(in: &cancellables)

        account.objectWillChange
            .sink { [weak self] _ in
                Task { @MainActor [weak self] in self?.markDirtyAndWake() }
            }
            .store(in: &cancellables)

        cloudChat.objectWillChange
            .sink { [weak self] _ in
                Task { @MainActor [weak self] in self?.markDirtyAndWake() }
            }
            .store(in: &cancellables)

        wallet.objectWillChange
            .sink { [weak self] _ in
                Task { @MainActor [weak self] in self?.markDirtyAndWake() }
            }
            .store(in: &cancellables)

        shell.objectWillChange
            .sink { [weak self] _ in
                Task { @MainActor [weak self] in self?.markDirtyAndWake() }
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
            syncCloudToGpu(surface)
        }

        if cloudChat.composerFocusNonce != lastFocusNonce {
            lastFocusNonce = cloudChat.composerFocusNonce
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

    // MARK: - OpenAgents Cloud + Account + Wallet -> Coax GPU Surface Sync

    private func syncCloudToGpu(_ surface: UnsafeMutableRawPointer) {
        let signedIn = account.isLoggedIn
        let reachCode: Int32 = {
            if !signedIn { return OA_IOS_REACH_UNKNOWN }
            return cloudChat.isStreamConnected ? OA_IOS_REACH_OK : OA_IOS_REACH_CHECKING
        }()

        let activeConv = cloudChat.activeConversation
        let headerTarget = signedIn ? cloudChat.selectedTarget.displayLabel : "OpenAgents Cloud"
        let sessionTitle = activeConv?.title ?? ""
        let projectName = cloudChat.selectedProject?.name ?? (activeConv?.workspace ?? "")

        oa_ios_set_header(
            surface,
            headerTarget,
            sessionTitle,
            projectName,
            reachCode,
            activeConv != nil ? 1 : 0,
            0,
            0
        )

        oa_ios_set_account_wallet(
            surface,
            account.sidebarLabel,
            signedIn ? 1 : 0,
            wallet.sidebarLabel,
            wallet.ready ? 1 : 0
        )

        let bannerErr = dictation.errorMessage ?? cloudChat.lastError ?? ""
        let toast = cloudChat.statusToast ?? ""
        oa_ios_set_banners(surface, bannerErr, 0, toast)

        oa_ios_set_full_auto(surface, cloudChat.fullAutoEnabled ? 1 : 0)
        oa_ios_set_usage(surface, cloudChat.usageStripSummary, 0)
        oa_ios_set_voice_duplex(surface, dictation.duplexState.rawValue, dictation.audioLevel)
        oa_ios_set_approval(surface, 0, "", "")
        oa_ios_approval_options_clear(surface)
        oa_ios_set_queued(surface, "")

        // Slash command autocomplete suggestions
        let slashes = cloudChat.matchingSlashCommands
        oa_ios_suggestions_clear(surface, 0)
        for cmd in slashes.prefix(8) {
            oa_ios_suggestions_push(surface, cmd.id, cmd.command, cmd.subtitle)
        }

        oa_ios_todos_clear(surface)
        oa_ios_folders_clear(surface)

        let modelOpt = cloudChat.selectedModelOption
        let effortLabel = modelOpt.supportsThinking ? cloudChat.selectedEffort.label : ""
        oa_ios_set_composer(
            surface,
            modelOpt.chipLabel,
            effortLabel,
            cloudChat.selectedTarget.shortChipLabel,
            Int32(cloudChat.pendingAttachments.count),
            dictation.isListening ? 1 : 0
        )

        let elapsedSecs: Int32 = {
            if let started = cloudChat.activeTurnStartedAt {
                return Int32(max(0, Date().timeIntervalSince(started)))
            }
            return 0
        }()
        oa_ios_set_working(
            surface,
            cloudChat.isSending ? 1 : 0,
            cloudChat.isSending ? 1 : 0,
            elapsedSecs,
            cloudChat.activePhaseLabel
        )

        // Sidebar: Synced OpenAgents Cloud conversations
        oa_ios_sessions_clear(surface)
        let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
        for conv in cloudChat.conversations.prefix(64) where !conv.archived {
            let isCurrent = cloudChat.activeConversationID == conv.id
            let isWorking = !conv.activeTurnID.isEmpty || (isCurrent && cloudChat.isSending)
            let age = Self.relativeAge(nowMs: nowMs, atMs: conv.updatedAtMs)
            oa_ios_sessions_push(
                surface,
                conv.id,
                conv.title,
                conv.workspace.isEmpty ? "Cloud" : conv.workspace,
                conv.model,
                age,
                isWorking ? 1 : 0,
                isCurrent ? 1 : 0,
                0,
                0
            )
            if let last = conv.turns.last {
                oa_ios_session_set_preview(surface, conv.id, last.userText, "", last.output)
            }
        }

        // Sidebar: Cloud Projects (pushed into section 2 via intro_items)
        oa_ios_intro_clear(surface)
        for project in cloudChat.projects.prefix(24) {
            oa_ios_intro_push(
                surface,
                OA_IOS_INTRO_TUI,
                project.id,
                project.name,
                project.repoCanonical.isEmpty ? "Cloud Project" : project.repoCanonical,
                "",
                Self.relativeAge(nowMs: nowMs, atMs: project.updatedAtMs),
                0,
                cloudChat.selectedProjectID == project.id ? 1 : 0
            )
        }

        // Sidebar: Execution Targets & Enrolled Computers (section 3 via machines)
        oa_ios_machines_clear(surface)
        if signedIn {
            oa_ios_machines_push(
                surface,
                CloudExecutionTarget.autoMachineID,
                "Auto Placement",
                "Cloud Tools + Enrolled ($0)",
                cloudChat.isStreamConnected ? OA_IOS_REACH_OK : OA_IOS_REACH_CHECKING,
                0,
                cloudChat.selectedTarget == .auto ? 1 : 0
            )
            oa_ios_machines_push(
                surface,
                CloudExecutionTarget.cloudMachineID,
                "OpenAgents Cloud",
                account.email.isEmpty ? "openagents.com" : account.email,
                cloudChat.isStreamConnected ? OA_IOS_REACH_OK : OA_IOS_REACH_CHECKING,
                0,
                cloudChat.selectedTarget == .cloud ? 1 : 0
            )
            let isSandboxSelected: Bool = {
                if case .sandbox = cloudChat.selectedTarget { return true }
                return false
            }()
            oa_ios_machines_push(
                surface,
                CloudExecutionTarget.sandboxMachineID,
                "Cloud Sandbox",
                "Linux container",
                OA_IOS_REACH_OK,
                0,
                isSandboxSelected ? 1 : 0
            )
            for env in cloudChat.environments.prefix(12) {
                let isSelected: Bool = {
                    if case .computer(let host, _) = cloudChat.selectedTarget {
                        return host == env.hostPubkey
                    }
                    return false
                }()
                let wsDetail = env.workspaces.first?.name ?? env.osLabel
                oa_ios_machines_push(
                    surface,
                    "host:\(env.hostPubkey)",
                    env.displayName,
                    wsDetail,
                    env.online ? OA_IOS_REACH_OK : OA_IOS_REACH_BAD,
                    Int32(env.workspaces.count),
                    isSelected ? 1 : 0
                )
            }
        }

        // Transcript items
        let viewKey = cloudChat.activeConversationID ?? "cloud:new"
        let sessionChanged = viewKey != lastSyncedViewKey
        lastSyncedViewKey = viewKey
        oa_ios_items_clear(surface, sessionChanged ? 1 : 0)
        for item in cloudChat.activeItems.suffix(192) {
            let roleCode: Int32 = {
                switch item.role {
                case .user: return OA_IOS_ROLE_USER
                case .assistant: return OA_IOS_ROLE_ASSISTANT
                case .thought: return OA_IOS_ROLE_THOUGHT
                case .tool: return OA_IOS_ROLE_TOOL
                case .notice: return OA_IOS_ROLE_NOTICE
                }
            }()
            let elapsed: Int32 = item.startedAtMs > 0 ? Int32(max(0, (nowMs - item.startedAtMs) / 1000)) : 0
            oa_ios_items_push(
                surface,
                roleCode,
                item.id,
                item.text,
                item.toolName,
                item.path,
                item.diff,
                item.complete ? 1 : 0,
                item.failed ? 1 : 0,
                0,
                item.diffCut ? 1 : 0,
                0,
                elapsed
            )
        }

        // Picker catalogs: Models, Efforts, Execution Targets (Modes chip)
        oa_ios_models_clear(surface)
        for m in CloudModelOption.catalog {
            oa_ios_models_push(surface, m.id, m.label, m.detail, m.id == cloudChat.selectedModelID ? 1 : 0)
        }

        oa_ios_efforts_clear(surface)
        for e in CloudThinkingEffort.allCases {
            oa_ios_efforts_push(surface, e.rawValue, e.label, e.detail, e == cloudChat.selectedEffort ? 1 : 0)
        }

        oa_ios_modes_clear(surface)
        oa_ios_modes_push(surface, CloudExecutionTarget.autoMachineID, "Auto", "Cloud Tools + Enrolled Computer ($0) + Sandbox", cloudChat.selectedTarget == .auto ? 1 : 0)
        oa_ios_modes_push(surface, CloudExecutionTarget.cloudMachineID, "Cloud", "Instant streaming + Web & GitHub tools", cloudChat.selectedTarget == .cloud ? 1 : 0)
        let sandboxSel: Int32 = {
            if case .sandbox = cloudChat.selectedTarget { return 1 }
            return 0
        }()
        oa_ios_modes_push(surface, CloudExecutionTarget.sandboxMachineID, "Sandbox", "Isolated Cloud Linux sandbox", sandboxSel)
        for env in cloudChat.environments.prefix(3) {
            let sel: Int32 = {
                if case .computer(let h, _) = cloudChat.selectedTarget, h == env.hostPubkey { return 1 }
                return 0
            }()
            oa_ios_modes_push(surface, "host:\(env.hostPubkey)", env.displayName, env.online ? "Online · $0 local" : "Offline", sel)
        }

        // Sync draft from cloudChat (e.g. voice dictation, slash command, or cleared after send)
        if editor.markedTextRange == nil {
            let gpuDraft = currentGpuDraft(surface)
            if gpuDraft != cloudChat.draftText {
                _ = oa_ios_draft_set(surface, cloudChat.draftText)
            }
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
            cloudChat.draftText = text
            if editor.isFirstResponder {
                _ = editor.resignFirstResponder()
            }
            oa_ios_blur(surface)
            if !account.isLoggedIn {
                shell.isAccountPresented = true
                break
            }
            let prompt = text.trimmingCharacters(in: .whitespacesAndNewlines)
            if !prompt.isEmpty || !cloudChat.pendingAttachments.isEmpty {
                _ = oa_ios_draft_set(surface, "")
                editor.set("")
                cloudChat.sendMessage()
            }
        case OA_IOS_ACT_STOP:
            dictation.stopSpeakingImmediate()
            Task { await cloudChat.stopActiveTurn() }
        case OA_IOS_ACT_NEW_CHAT:
            cloudChat.startNewChat()
            _ = focusComposer()
        case OA_IOS_ACT_SELECT_SESSION:
            cloudChat.selectConversation(arg)
        case OA_IOS_ACT_OPEN_COMPUTERS, OA_IOS_ACT_OPEN_WORKSPACE:
            dismissKeyboard(surface)
            shell.isEnvironmentsPresented = true
        case OA_IOS_ACT_OPEN_ATTACH:
            dismissKeyboard(surface)
            presentAttachActionSheet()
        case OA_IOS_ACT_CLEAR_ATTACH:
            cloudChat.pendingAttachments.removeAll()
        case OA_IOS_ACT_CHOSE_MODEL:
            cloudChat.chooseModel(arg)
        case OA_IOS_ACT_CHOSE_EFFORT:
            if let eff = CloudThinkingEffort(rawValue: arg) {
                cloudChat.chooseEffort(eff)
            }
        case OA_IOS_ACT_CHOSE_MODE:
            cloudChat.selectTargetByMachineID(arg)
        case OA_IOS_ACT_TOGGLE_MIC:
            dictation.toggle(existingText: cloudChat.draftText) { [weak self] spoken in
                self?.cloudChat.draftText = spoken
                self?.markDirtyAndWake()
            }
        case OA_IOS_ACT_DISMISS_ERROR:
            cloudChat.lastError = nil
            dictation.errorMessage = nil
        case OA_IOS_ACT_COPY_TEXT:
            var buf = [CChar](repeating: 0, count: 16385)
            _ = oa_ios_copy_get(surface, &buf, Int32(buf.count))
            let copied = String(cString: buf)
            if !copied.isEmpty {
                UIPasteboard.general.string = copied
                let gen = UIImpactFeedbackGenerator(style: .light)
                gen.impactOccurred()
            }
        case OA_IOS_ACT_APPLY_SUGGESTION:
            if let cmd = cloudChat.matchingSlashCommands.first(where: { $0.id == arg }) {
                cloudChat.applySlashCommand(cmd)
                _ = oa_ios_draft_set(surface, cloudChat.draftText)
                editor.set(cloudChat.draftText)
                _ = focusComposer()
            }
        case OA_IOS_ACT_DASHBOARD_CHANGED:
            if arg == "1" {
                dismissKeyboard(surface)
            }
        case OA_IOS_ACT_TOGGLE_FULL_AUTO:
            cloudChat.toggleFullAuto()
        case OA_IOS_ACT_OPEN_USAGE:
            dismissKeyboard(surface)
            shell.isUsagePresented = true
            if account.isLoggedIn {
                Task { await cloudChat.refreshUsage() }
            }
        case OA_IOS_ACT_TOGGLE_LIVE_VOICE:
            dismissKeyboard(surface)
            dictation.speechRate = cloudChat.voiceSpeechRate
            dictation.vadSilenceCommitMs = Int64(cloudChat.voiceVadSilenceMs)
            dictation.toggleLiveVoice(
                onPartial: { [weak self] spoken in
                    self?.cloudChat.draftText = spoken
                    self?.markDirtyAndWake()
                },
                onAutoSend: { [weak self] spoken in
                    guard let self else { return }
                    self.cloudChat.draftText = ""
                    self.markDirtyAndWake()
                    self.cloudChat.sendMessage(overrideText: spoken)
                },
                onBargeIn: { [weak self] in
                    guard let self else { return }
                    self.markDirtyAndWake()
                    Task { await self.cloudChat.stopActiveTurn() }
                }
            )
        case OA_IOS_ACT_SELECT_INTRO:
            if cloudChat.selectedProjectID == arg {
                cloudChat.selectProject(nil)
            } else {
                cloudChat.selectProject(arg)
            }
        case OA_IOS_ACT_SELECT_MACHINE:
            cloudChat.selectTargetByMachineID(arg)
        case OA_IOS_ACT_OPEN_ACCOUNT:
            dismissKeyboard(surface)
            shell.isAccountPresented = true
        case OA_IOS_ACT_OPEN_WALLET:
            dismissKeyboard(surface)
            shell.isWalletPresented = true
        default:
            break
        }
        markDirtyAndWake()
    }

    // MARK: - Hardware Keyboard Shortcuts (Cmd+D, Cmd+N, Cmd+\, Cmd+Up/Down, Cmd+1..9)

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
        cloudChat.startNewChat()
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
        let active = cloudChat.conversations.filter { !$0.archived }
        guard idx - 1 < active.count else { return }
        oa_ios_dashboard_set(surface, 0)
        oa_ios_drawer_set(surface, 0)
        cloudChat.selectConversation(active[idx - 1].id)
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
        sheet.addAction(UIAlertAction(title: "Environments & Projects", style: .default) { [weak self] _ in
            self?.shell.isEnvironmentsPresented = true
        })
        sheet.addAction(UIAlertAction(title: "Account & Cloud", style: .default) { [weak self] _ in
            self?.shell.isAccountPresented = true
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
                let att = CloudAttachment.prepareImage(data: data, name: "photo.jpg")
                Task { @MainActor [weak self] in
                    self?.cloudChat.addAttachment(att)
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
            cloudChat.addAttachment(CloudAttachment.prepareImage(data: data, name: url.lastPathComponent))
        } else if let text = String(data: data, encoding: .utf8)?.nilIfContainsBinaryNUL() {
            cloudChat.addAttachment(CloudAttachment(
                name: url.lastPathComponent,
                mimeType: "text/plain",
                textPreview: text.truncatedToUTF8Bytes(4_000),
                imageData: nil,
                byteCount: data.count
            ))
        } else {
            cloudChat.addAttachment(CloudAttachment(
                name: url.lastPathComponent,
                mimeType: UTType(filenameExtension: ext)?.preferredMIMEType ?? "application/octet-stream",
                textPreview: nil,
                imageData: nil,
                byteCount: data.count
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
        if cloudChat.draftText != txt {
            cloudChat.draftText = txt
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
        account.isLoggedIn = true
        account.email = "demo@openagents.com"
        account.creditMsat = 25_000_000
        account.promoMicros = 5_000_000
        account.promoExpiresAt = "2027-02-25T00:00:00Z"
        cloudChat.isStreamConnected = true
        cloudChat.usageSnapshot = CloudUsageSnapshot(
            todayUsdMicros: 420_000,
            monthUsdMicros: 2_850_000,
            totalUsdMicros: 2_850_000,
            todayTokens: 48_200,
            totalTokens: 320_000,
            todayRequests: 9,
            totalCalls: 42,
            creditMsat: 25_000_000,
            promoMicros: 5_000_000,
            promoExpiresAt: "2027-02-25T00:00:00Z",
            sandboxTurns: 0,
            sandboxSeconds: 0,
            sandboxMsat: 0,
            localTurns: 6
        )
        cloudChat.environments = [
            CloudEnvironment(
                hostPubkey: String(repeating: "a", count: 64),
                displayName: "Studio Mac",
                os: "macos",
                online: true,
                lastSeenMs: nowMs,
                workspaces: [
                    CloudWorkspace(workspaceID: "default", name: "openagents", localPath: "/Users/dev/openagents", repoRemote: "https://github.com/OpenAgentsInc/openagents", repoCanonical: "github.com/openagentsinc/openagents", branch: "main")
                ]
            )
        ]
        cloudChat.projects = [
            CloudProject(id: "01234567-89ab-4cde-8f01-23456789abcd", revision: 1, name: "OpenAgents Core", description: "Coax + Cloud platform", repoCanonical: "github.com/openagentsinc/openagents", updatedAtMs: nowMs - 60_000)
        ]
        let demoConvID = "11234567-89ab-4cde-8f01-23456789abcd"
        cloudChat.conversations = [
            MobileCloudConversation(
                id: demoConvID,
                title: "Verify OpenAgents Cloud Mode on iOS",
                model: "google/gemini-4-neon",
                workspace: "OpenAgents Core",
                revision: 2,
                updatedAtMs: nowMs - 30_000,
                pinned: false,
                archived: false,
                activeTurnID: "",
                compute: "cloud",
                sourceDevice: "ios-mobile",
                turns: [
                    MobileCloudTurn(id: "21234567-89ab-4cde-8f01-23456789abcd", userText: "Inspect Cloud Chat v2 sync and Tier 1 tools.", output: "OpenAgents Cloud streaming, Tier 1 tools, and enrolled computer placement are active.", complete: true)
                ],
                items: []
            )
        ]
        if UserDefaults.standard.bool(forKey: "OADemoConversation") {
            cloudChat.activeConversationID = demoConvID
            cloudChat.activeItems = [
                CloudTranscriptItem(id: "u1", role: .user, text: "Check the OpenAgents Cloud pipeline on iOS.", complete: true, startedAtMs: nowMs - 90_000),
                CloudTranscriptItem(id: "t1", role: .thought, text: "Verifying streaming `/api/v1/inference`, Tier 1 `/api/tools/cloud/v1`, and `/api/chat/v2/*` SSE sync.", complete: true, startedAtMs: nowMs - 85_000),
                CloudTranscriptItem(id: "tl1", role: .tool, text: "f apps/openagents-ios/Sources/MobileCloudChat.swift\nf apps/openagents-ios/Sources/EnvironmentsView.swift", toolName: "github_tree", complete: true, startedAtMs: nowMs - 82_000),
                CloudTranscriptItem(id: "a1", role: .assistant, text: "OpenAgents iOS is running 100% on OpenAgents Cloud:\n\n- **Metal shader**: `Components/components.metal` with Geist & SVG icons\n- **Cloud Sync**: Live SSE over `/api/chat/v2/stream`\n- **Tools & Placement**: Tier 1 Cloud Tools + Enrolled Computers ($0) + Cloud Sandbox\n\n```swift\nlet surface = oa_ios_open(resourcesPath)\noa_ios_frame(surface)\n```", complete: true, startedAtMs: nowMs - 70_000),
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
