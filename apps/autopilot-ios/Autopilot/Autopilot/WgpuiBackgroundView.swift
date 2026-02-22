import SwiftUI
import UIKit
import QuartzCore
import os.log

private let wgpuiLog = OSLog(subsystem: "com.openagents.Autopilot", category: "WGPUI")

/// UIView that hosts the WGPUI Codex renderer on a CAMetalLayer.
private final class WgpuiBackgroundUIView: UIView, UITextFieldDelegate {
    override class var layerClass: AnyClass { CAMetalLayer.self }

    var onSendRequested: (() -> Void)?
    var onNewThreadRequested: (() -> Void)?
    var onInterruptRequested: (() -> Void)?
    var onModelCycleRequested: (() -> Void)?
    var onReasoningCycleRequested: (() -> Void)?
    var onComposerChanged: ((String) -> Void)?

    private var statePtr: UnsafeMutableRawPointer?
    private var displayLink: CADisplayLink?
    private var renderTickCount: Int = 0
    private var composerDraft: String = ""
    private let keyboardProxyField = UITextField(frame: .zero)

    private var effectiveScale: CGFloat {
        window?.screen.nativeScale ?? UIScreen.main.nativeScale
    }

    override init(frame: CGRect) {
        super.init(frame: frame)
        configureView()
        os_log("[WGPUI] WgpuiBackgroundUIView init frame=%@", log: wgpuiLog, type: .default, String(describing: frame))
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        configureView()
        os_log("[WGPUI] WgpuiBackgroundUIView init(coder)", log: wgpuiLog, type: .default)
    }

    private func configureView() {
        isOpaque = true
        backgroundColor = .black
        let tap = UITapGestureRecognizer(target: self, action: #selector(handleTapGesture(_:)))
        addGestureRecognizer(tap)
        isUserInteractionEnabled = true
        configureKeyboardProxyField()
    }

    private func configureKeyboardProxyField() {
        keyboardProxyField.translatesAutoresizingMaskIntoConstraints = false
        keyboardProxyField.autocapitalizationType = .sentences
        keyboardProxyField.autocorrectionType = .yes
        keyboardProxyField.spellCheckingType = .yes
        keyboardProxyField.keyboardType = .default
        keyboardProxyField.returnKeyType = .send
        keyboardProxyField.textContentType = .none
        keyboardProxyField.textColor = .clear
        keyboardProxyField.tintColor = .clear
        keyboardProxyField.backgroundColor = .clear
        keyboardProxyField.alpha = 0.01
        keyboardProxyField.delegate = self
        keyboardProxyField.addTarget(self, action: #selector(keyboardProxyChanged(_:)), for: .editingChanged)
        addSubview(keyboardProxyField)
        NSLayoutConstraint.activate([
            keyboardProxyField.widthAnchor.constraint(equalToConstant: 1),
            keyboardProxyField.heightAnchor.constraint(equalToConstant: 1),
            keyboardProxyField.leadingAnchor.constraint(equalTo: leadingAnchor),
            keyboardProxyField.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])
    }

    @objc private func handleTapGesture(_ recognizer: UITapGestureRecognizer) {
        guard let statePtr else { return }
        let location = recognizer.location(in: self)
        WgpuiBackgroundBridge.handleTap(state: statePtr, x: Float(location.x), y: Float(location.y))
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let bounds = self.bounds
        let scale = effectiveScale
        guard let metalLayer = layer as? CAMetalLayer else {
            os_log("[WGPUI] layoutSubviews layer is not CAMetalLayer", log: wgpuiLog, type: .error)
            return
        }

        contentScaleFactor = scale
        metalLayer.contentsScale = scale
        metalLayer.drawableSize = CGSize(width: bounds.width * scale, height: bounds.height * scale)

        let logicalW = UInt32(max(1.0, bounds.width.rounded(.toNearestOrAwayFromZero)))
        let logicalH = UInt32(max(1.0, bounds.height.rounded(.toNearestOrAwayFromZero)))

        if statePtr == nil {
            guard WgpuiBackgroundBridge.isAvailable else {
                os_log("[WGPUI] bridge unavailable", log: wgpuiLog, type: .error)
                return
            }

            statePtr = WgpuiBackgroundBridge.create(
                layerPtr: Unmanaged.passUnretained(metalLayer).toOpaque(),
                width: logicalW,
                height: logicalH,
                scale: Float(scale)
            )

            if statePtr != nil {
                displayLink = CADisplayLink(target: self, selector: #selector(tick))
                displayLink?.add(to: .main, forMode: .common)
            }
        } else {
            WgpuiBackgroundBridge.resize(state: statePtr, width: logicalW, height: logicalH)
        }
    }

    func sync(model: CodexHandshakeViewModel) {
        guard let statePtr else { return }

        WgpuiBackgroundBridge.clearCodexMessages(state: statePtr)
        for message in model.chatMessages.suffix(220) {
            WgpuiBackgroundBridge.pushCodexMessage(
                state: statePtr,
                role: mapRole(message.role),
                text: message.text,
                streaming: message.isStreaming
            )
        }

        let modelLabel = model.selectedModelOverride == "default" ? "model:auto" : model.selectedModelOverride
        let reasoningLabel = model.selectedReasoningEffort == "default" ? "reasoning:auto" : model.selectedReasoningEffort
        WgpuiBackgroundBridge.setCodexContext(
            state: statePtr,
            thread: "thread: \(model.activeThreadID ?? "none")",
            turn: "turn: \(model.activeTurnID ?? "none")",
            model: modelLabel,
            reasoning: reasoningLabel
        )
        let emptyState = resolveEmptyState(model: model)
        WgpuiBackgroundBridge.setEmptyState(
            state: statePtr,
            title: emptyState.title,
            detail: emptyState.detail
        )

        composerDraft = model.messageDraft
        WgpuiBackgroundBridge.setComposerText(state: statePtr, composerDraft)
        if keyboardProxyField.text != composerDraft {
            keyboardProxyField.text = composerDraft
        }
    }

    @objc private func tick() {
        guard let statePtr else { return }
        _ = WgpuiBackgroundBridge.render(state: statePtr, logFirstFrame: renderTickCount == 0)
        renderTickCount += 1

        if WgpuiBackgroundBridge.consumeSendRequested(state: statePtr) {
            onSendRequested?()
        }
        if WgpuiBackgroundBridge.consumeNewThreadRequested(state: statePtr) {
            onNewThreadRequested?()
        }
        if WgpuiBackgroundBridge.consumeInterruptRequested(state: statePtr) {
            onInterruptRequested?()
        }
        if WgpuiBackgroundBridge.consumeModelCycleRequested(state: statePtr) {
            onModelCycleRequested?()
        }
        if WgpuiBackgroundBridge.consumeReasoningCycleRequested(state: statePtr) {
            onReasoningCycleRequested?()
        }

        if WgpuiBackgroundBridge.composerFocused(state: statePtr) {
            WgpuiBackgroundBridge.setComposerFocused(state: statePtr, focused: false)
            DispatchQueue.main.async { [weak self] in
                self?.beginComposerEditing()
            }
        }
    }

    private func beginComposerEditing() {
        guard statePtr != nil else { return }
        keyboardProxyField.text = composerDraft
        if !keyboardProxyField.isFirstResponder {
            keyboardProxyField.becomeFirstResponder()
        }
    }

    @objc private func keyboardProxyChanged(_ textField: UITextField) {
        composerDraft = textField.text ?? ""
        WgpuiBackgroundBridge.setComposerText(state: statePtr, composerDraft)
        onComposerChanged?(composerDraft)
    }

    func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        onSendRequested?()
        return false
    }

    func textFieldDidEndEditing(_ textField: UITextField) {
        WgpuiBackgroundBridge.setComposerFocused(state: statePtr, focused: false)
    }

    private func mapRole(_ role: CodexChatRole) -> WgpuiCodexRole {
        switch role {
        case .user:
            return .user
        case .assistant:
            return .assistant
        case .reasoning:
            return .reasoning
        case .tool:
            return .tool
        case .system:
            return .system
        case .error:
            return .error
        }
    }

    private func resolveEmptyState(model: CodexHandshakeViewModel) -> (title: String, detail: String) {
        if let error = model.errorMessage?.trimmingCharacters(in: .whitespacesAndNewlines), !error.isEmpty {
            return ("Codex Error", error)
        }
        if !model.isAuthenticated {
            return ("Sign In Required", "Open the hidden debug panel to sign in.")
        }
        switch model.streamState {
        case .connecting:
            return ("Connecting", "Connecting to your desktop Codex stream...")
        case .reconnecting:
            return ("Reconnecting", "Recovering your desktop Codex stream...")
        default:
            return ("No Codex Messages Yet", "Waiting for Codex events from desktop.")
        }
    }

    deinit {
        os_log("[WGPUI] WgpuiBackgroundUIView deinit", log: wgpuiLog, type: .default)
        keyboardProxyField.delegate = nil
        displayLink?.invalidate()
        displayLink = nil
        if let statePtr {
            WgpuiBackgroundBridge.destroy(state: statePtr)
        }
    }
}

/// SwiftUI wrapper around the WGPUI iOS Codex surface.
struct WgpuiBackgroundView: View {
    @ObservedObject var model: CodexHandshakeViewModel

    var body: some View {
        Group {
            if WgpuiBackgroundBridge.isAvailable {
                Representable(model: model)
                    .ignoresSafeArea()
            } else {
                Color.black.ignoresSafeArea()
            }
        }
        .onAppear {
            let available = WgpuiBackgroundBridge.logAvailability()
            let msg = "[WGPUI] WgpuiBackgroundView onAppear -> using \(available ? "WGPUI renderer" : "fallback Color.black")"
            os_log("%{public}@", log: wgpuiLog, type: .default, msg)
            print(msg)
        }
    }

    private struct Representable: UIViewRepresentable {
        @ObservedObject var model: CodexHandshakeViewModel

        func makeUIView(context: Context) -> WgpuiBackgroundUIView {
            let view = WgpuiBackgroundUIView()
            view.onSendRequested = { [weak model] in
                guard let model else { return }
                Task { await model.sendUserMessage() }
            }
            view.onNewThreadRequested = { [weak model] in
                guard let model else { return }
                Task { await model.startThread() }
            }
            view.onInterruptRequested = { [weak model] in
                guard let model else { return }
                Task { await model.interruptActiveTurn() }
            }
            view.onModelCycleRequested = { [weak model] in
                guard let model else { return }
                model.selectedModelOverride = cycleSelection(
                    current: model.selectedModelOverride,
                    options: model.modelOverrideOptions
                )
            }
            view.onReasoningCycleRequested = { [weak model] in
                guard let model else { return }
                model.selectedReasoningEffort = cycleSelection(
                    current: model.selectedReasoningEffort,
                    options: model.reasoningEffortOptions
                )
            }
            view.onComposerChanged = { [weak model] text in
                model?.messageDraft = text
            }
            return view
        }

        func updateUIView(_ uiView: WgpuiBackgroundUIView, context: Context) {
            uiView.onComposerChanged = { [weak model] text in
                model?.messageDraft = text
            }
            uiView.sync(model: model)
        }

        private func cycleSelection(current: String, options: [String]) -> String {
            guard !options.isEmpty else {
                return current
            }
            guard let index = options.firstIndex(of: current) else {
                return options[0]
            }
            return options[(index + 1) % options.count]
        }
    }
}
