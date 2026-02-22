import Foundation
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
    var onAuthEmailChanged: ((String) -> Void)?
    var onAuthCodeChanged: ((String) -> Void)?
    var onSendCodeRequested: (() -> Void)?
    var onVerifyCodeRequested: (() -> Void)?
    var onSignOutRequested: (() -> Void)?
    var onRefreshWorkersRequested: (() -> Void)?
    var onConnectStreamRequested: (() -> Void)?
    var onDisconnectStreamRequested: (() -> Void)?
    var onSendHandshakeRequested: (() -> Void)?

    private var statePtr: UnsafeMutableRawPointer?
    private var displayLink: CADisplayLink?
    private var renderTickCount: Int = 0
    private var composerDraft: String = ""
    private var authEmailDraft: String = ""
    private var authCodeDraft: String = ""
    private var activeInputTarget: WgpuiInputTarget = .none
    private var pendingInputTargetAfterForeground: WgpuiInputTarget?
    private var configuredScale: CGFloat?
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
        registerLifecycleObservers()
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

    private func registerLifecycleObservers() {
        let center = NotificationCenter.default
        center.addObserver(
            self,
            selector: #selector(handleWillResignActive),
            name: UIApplication.willResignActiveNotification,
            object: nil
        )
        center.addObserver(
            self,
            selector: #selector(handleDidEnterBackground),
            name: UIApplication.didEnterBackgroundNotification,
            object: nil
        )
        center.addObserver(
            self,
            selector: #selector(handleWillEnterForeground),
            name: UIApplication.willEnterForegroundNotification,
            object: nil
        )
        center.addObserver(
            self,
            selector: #selector(handleDidBecomeActive),
            name: UIApplication.didBecomeActiveNotification,
            object: nil
        )
    }

    @objc private func handleWillResignActive() {
        displayLink?.isPaused = true
    }

    @objc private func handleDidEnterBackground() {
        displayLink?.isPaused = true
        if activeInputTarget != .none {
            pendingInputTargetAfterForeground = activeInputTarget
        }
        keyboardProxyField.resignFirstResponder()
    }

    @objc private func handleWillEnterForeground() {
        setNeedsLayout()
    }

    @objc private func handleDidBecomeActive() {
        displayLink?.isPaused = false
        setNeedsLayout()
        guard let target = pendingInputTargetAfterForeground else {
            return
        }
        pendingInputTargetAfterForeground = nil
        beginEditing(target: target)
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

        if statePtr != nil, configuredScale != scale {
            teardownRenderer()
        }

        if statePtr == nil {
            setupRenderer(
                layerPtr: Unmanaged.passUnretained(metalLayer).toOpaque(),
                width: logicalW,
                height: logicalH,
                scale: Float(scale)
            )
        } else {
            WgpuiBackgroundBridge.resize(state: statePtr, width: logicalW, height: logicalH)
        }
    }

    private func setupRenderer(
        layerPtr: UnsafeMutableRawPointer,
        width: UInt32,
        height: UInt32,
        scale: Float
    ) {
        guard WgpuiBackgroundBridge.isAvailable else {
            os_log("[WGPUI] bridge unavailable", log: wgpuiLog, type: .error)
            return
        }

        statePtr = WgpuiBackgroundBridge.create(
            layerPtr: layerPtr,
            width: width,
            height: height,
            scale: scale
        )
        configuredScale = CGFloat(scale)

        if statePtr != nil {
            if displayLink == nil {
                displayLink = CADisplayLink(target: self, selector: #selector(tick))
                displayLink?.add(to: .main, forMode: .common)
            }
            displayLink?.isPaused = false
        }
    }

    private func teardownRenderer() {
        if let statePtr {
            WgpuiBackgroundBridge.destroy(state: statePtr)
            self.statePtr = nil
        }
        configuredScale = nil
        renderTickCount = 0
        displayLink?.invalidate()
        displayLink = nil
    }

    func sync(model: CodexHandshakeViewModel) {
        guard let statePtr else { return }

        WgpuiBackgroundBridge.clearCodexMessages(state: statePtr)
        for message in missionOverviewRows(model: model).suffix(220) {
            WgpuiBackgroundBridge.pushCodexMessage(
                state: statePtr,
                role: mapRole(message.role),
                text: message.text,
                streaming: message.streaming
            )
        }

        let modelLabel = model.selectedModelOverride == "default" ? "model:auto" : model.selectedModelOverride
        let reasoningLabel = model.selectedReasoningEffort == "default" ? "reasoning:auto" : model.selectedReasoningEffort
        let projection = model.missionControlProjection
        WgpuiBackgroundBridge.setCodexContext(
            state: statePtr,
            thread: "workers: \(projection.workers.count)",
            turn: "events: \(projection.events.count)",
            model: modelLabel,
            reasoning: reasoningLabel
        )
        let emptyState = resolveEmptyState(model: model)
        WgpuiBackgroundBridge.setEmptyState(
            state: statePtr,
            title: emptyState.title,
            detail: emptyState.detail
        )
        WgpuiBackgroundBridge.setAuthFields(
            state: statePtr,
            email: model.email,
            code: model.verificationCode,
            authStatus: authDescription(model.authState)
        )
        WgpuiBackgroundBridge.setOperatorStatus(
            state: statePtr,
            workerStatus: workerStatusText(model: model),
            streamStatus: streamDescription(model.streamState),
            handshakeStatus: handshakeDescription(model.handshakeState),
            deviceStatus: "device: \(model.deviceID)",
            telemetry: telemetrySummary(model: model),
            events: eventsSummary(model: model),
            control: controlRequestSummary(model: model)
        )
        syncMissionControlProjection(state: statePtr, projection: projection)

        composerDraft = model.messageDraft
        WgpuiBackgroundBridge.setComposerText(state: statePtr, composerDraft)
        authEmailDraft = model.email
        authCodeDraft = model.verificationCode
        WgpuiBackgroundBridge.setAuthEmail(state: statePtr, authEmailDraft)
        WgpuiBackgroundBridge.setAuthCode(state: statePtr, authCodeDraft)
        syncKeyboardProxyText()
    }

    private func syncKeyboardProxyText() {
        let text: String
        switch activeInputTarget {
        case .composer:
            text = composerDraft
        case .authEmail:
            text = authEmailDraft
        case .authCode:
            text = authCodeDraft
        case .none:
            text = ""
        }
        if keyboardProxyField.text != text {
            keyboardProxyField.text = text
        }
    }

    private func syncMissionControlProjection(
        state: UnsafeMutableRawPointer,
        projection: RuntimeMissionControlProjection
    ) {
        WgpuiBackgroundBridge.clearMissionData(state: state)

        for worker in projection.workers {
            WgpuiBackgroundBridge.pushMissionWorker(
                state: state,
                workerID: worker.workerID,
                status: worker.status,
                heartbeatState: worker.heartbeatState,
                latestSeq: worker.latestSeq,
                lagEvents: worker.lagEvents,
                reconnectState: worker.reconnectState,
                lastEventAt: worker.lastEventAt,
                runningTurns: UInt64(max(0, worker.runningTurns)),
                queuedRequests: UInt64(max(0, worker.queuedRequests)),
                failedRequests: UInt64(max(0, worker.failedRequests))
            )
        }

        for thread in projection.threads {
            WgpuiBackgroundBridge.pushMissionThread(
                state: state,
                workerID: thread.workerID,
                threadID: thread.threadID,
                activeTurnID: thread.activeTurnID,
                lastSummary: thread.lastSummary,
                lastEventAt: thread.lastEventAt,
                freshnessSeq: thread.freshnessSeq,
                unreadCount: UInt64(max(0, thread.unreadCount)),
                muted: thread.muted
            )
        }

        for timeline in projection.timelines {
            for entry in timeline.entries {
                WgpuiBackgroundBridge.pushMissionTimelineEntry(
                    state: state,
                    workerID: entry.workerID,
                    threadID: entry.threadID,
                    role: entry.role,
                    text: entry.text,
                    isStreaming: entry.isStreaming,
                    turnID: entry.turnID,
                    itemID: entry.itemID,
                    occurredAt: entry.occurredAt
                )
            }
        }

        for event in projection.events {
            WgpuiBackgroundBridge.pushMissionEvent(
                state: state,
                id: UInt64(max(0, event.id)),
                topic: event.topic,
                seq: event.seq,
                workerID: event.workerID,
                threadID: event.threadID,
                turnID: event.turnID,
                requestID: event.requestID,
                eventType: event.eventType,
                method: event.method,
                summary: event.summary,
                severity: missionSeverityValue(event.severity),
                occurredAt: event.occurredAt,
                payloadJSON: jsonString(from: event.payload) ?? "{}",
                resyncMarker: event.resyncMarker
            )
        }

        for request in projection.requests {
            WgpuiBackgroundBridge.pushMissionRequest(
                state: state,
                requestID: request.requestID,
                workerID: request.workerID,
                threadID: request.threadID,
                method: request.method,
                requestState: request.state,
                occurredAt: request.occurredAt,
                errorCode: request.errorCode,
                errorMessage: request.errorMessage,
                retryable: request.retryable,
                responseJSON: jsonString(from: request.response)
            )
        }
    }

    private func missionSeverityValue(_ severity: RuntimeMissionControlEventSeverity) -> UInt8 {
        switch severity {
        case .info:
            return 0
        case .warning:
            return 1
        case .error:
            return 2
        }
    }

    private func jsonString(from value: JSONValue?) -> String? {
        guard let value else {
            return nil
        }
        guard let data = try? JSONEncoder().encode(value) else {
            return nil
        }
        return String(data: data, encoding: .utf8)
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
        if WgpuiBackgroundBridge.consumeSendCodeRequested(state: statePtr) {
            onSendCodeRequested?()
        }
        if WgpuiBackgroundBridge.consumeVerifyCodeRequested(state: statePtr) {
            onVerifyCodeRequested?()
        }
        if WgpuiBackgroundBridge.consumeSignOutRequested(state: statePtr) {
            onSignOutRequested?()
        }
        if WgpuiBackgroundBridge.consumeRefreshWorkersRequested(state: statePtr) {
            onRefreshWorkersRequested?()
        }
        if WgpuiBackgroundBridge.consumeConnectStreamRequested(state: statePtr) {
            onConnectStreamRequested?()
        }
        if WgpuiBackgroundBridge.consumeDisconnectStreamRequested(state: statePtr) {
            onDisconnectStreamRequested?()
        }
        if WgpuiBackgroundBridge.consumeSendHandshakeRequested(state: statePtr) {
            onSendHandshakeRequested?()
        }

        let target = WgpuiBackgroundBridge.activeInputTarget(state: statePtr)
        if target == .none {
            if activeInputTarget != .none {
                activeInputTarget = .none
                DispatchQueue.main.async { [weak self] in
                    self?.keyboardProxyField.resignFirstResponder()
                    self?.syncKeyboardProxyText()
                }
            }
        } else {
            WgpuiBackgroundBridge.setActiveInputTarget(state: statePtr, .none)
            DispatchQueue.main.async { [weak self] in
                self?.beginEditing(target: target)
            }
        }
    }

    private func beginEditing(target: WgpuiInputTarget) {
        guard statePtr != nil else { return }
        activeInputTarget = target
        syncKeyboardProxyText()

        switch target {
        case .authCode:
            keyboardProxyField.keyboardType = .numberPad
            keyboardProxyField.returnKeyType = .done
            keyboardProxyField.textContentType = .oneTimeCode
            keyboardProxyField.autocapitalizationType = .none
            keyboardProxyField.autocorrectionType = .no
            keyboardProxyField.spellCheckingType = .no
        case .authEmail:
            keyboardProxyField.keyboardType = .emailAddress
            keyboardProxyField.returnKeyType = .next
            keyboardProxyField.textContentType = .username
            keyboardProxyField.autocapitalizationType = .none
            keyboardProxyField.autocorrectionType = .no
            keyboardProxyField.spellCheckingType = .no
        case .composer:
            keyboardProxyField.keyboardType = .default
            keyboardProxyField.returnKeyType = .send
            keyboardProxyField.textContentType = .none
            keyboardProxyField.autocapitalizationType = .sentences
            keyboardProxyField.autocorrectionType = .yes
            keyboardProxyField.spellCheckingType = .yes
        case .none:
            break
        }
        keyboardProxyField.reloadInputViews()
        if !keyboardProxyField.isFirstResponder {
            keyboardProxyField.becomeFirstResponder()
        }
    }

    @objc private func keyboardProxyChanged(_ textField: UITextField) {
        let text = textField.text ?? ""
        switch activeInputTarget {
        case .composer:
            composerDraft = text
            WgpuiBackgroundBridge.setComposerText(state: statePtr, composerDraft)
            onComposerChanged?(composerDraft)
        case .authEmail:
            authEmailDraft = text
            WgpuiBackgroundBridge.setAuthEmail(state: statePtr, authEmailDraft)
            onAuthEmailChanged?(authEmailDraft)
        case .authCode:
            authCodeDraft = text
            WgpuiBackgroundBridge.setAuthCode(state: statePtr, authCodeDraft)
            onAuthCodeChanged?(authCodeDraft)
        case .none:
            break
        }
    }

    func textFieldShouldReturn(_ textField: UITextField) -> Bool {
        switch activeInputTarget {
        case .composer:
            onSendRequested?()
        case .authEmail:
            onSendCodeRequested?()
        case .authCode:
            onVerifyCodeRequested?()
        case .none:
            break
        }
        return false
    }

    func textFieldDidEndEditing(_ textField: UITextField) {
        activeInputTarget = .none
        WgpuiBackgroundBridge.setActiveInputTarget(state: statePtr, .none)
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

    private func missionOverviewRows(model: CodexHandshakeViewModel) -> [(role: CodexChatRole, text: String, streaming: Bool)] {
        let projection = model.missionControlProjection
        var rows: [(role: CodexChatRole, text: String, streaming: Bool)] = []

        let workerRows = projection.workers.sorted { lhs, rhs in
            if lhs.status != rhs.status {
                return lhs.status < rhs.status
            }
            return lhs.workerID < rhs.workerID
        }
        for worker in workerRows.prefix(18) {
            let seq = worker.latestSeq.map(String.init) ?? "n/a"
            let heartbeat = worker.heartbeatState ?? "unknown"
            let turns = worker.runningTurns
            let queued = worker.queuedRequests
            let failed = worker.failedRequests
            rows.append((
                role: worker.status == "running" ? .assistant : .system,
                text: "[lane] \(worker.workerID) | status=\(worker.status) seq=\(seq) hb=\(heartbeat) turns=\(turns) queued=\(queued) failed=\(failed)",
                streaming: false
            ))
        }

        for event in projection.events.suffix(120) {
            let worker = event.workerID ?? "unknown-worker"
            let thread = event.threadID ?? "thread:none"
            let label = event.resyncMarker ? "resync" : "event"
            let role: CodexChatRole = {
                if event.severity == .error {
                    return .error
                }
                if event.resyncMarker {
                    return .system
                }
                return .tool
            }()
            rows.append((
                role: role,
                text: "[\(label)] \(worker) | \(thread) | \(event.summary)",
                streaming: false
            ))
        }

        return rows
    }

    private func resolveEmptyState(model: CodexHandshakeViewModel) -> (title: String, detail: String) {
        if let error = model.errorMessage?.trimmingCharacters(in: .whitespacesAndNewlines), !error.isEmpty {
            return ("Codex Error", error)
        }
        if !model.isAuthenticated {
            return ("Sign In Required", "Tap Open Ops and sign in with email code.")
        }
        switch model.streamState {
        case .connecting:
            return ("Mission Control Connecting", "Connecting to your desktop Codex stream...")
        case .reconnecting:
            return ("Mission Control Reconnecting", "Recovering your desktop Codex stream...")
        default:
            return ("Mission Control Empty", "Waiting for Codex worker lanes and events.")
        }
    }

    private func authDescription(_ state: AuthState) -> String {
        switch state {
        case .signedOut:
            return "signed out"
        case .sendingCode:
            return "sending code"
        case .codeSent(let email):
            return "code sent to \(email)"
        case .verifying:
            return "verifying"
        case .authenticated(let email):
            if let email, !email.isEmpty {
                return "signed in as \(email)"
            }
            return "signed in"
        }
    }

    private func streamDescription(_ state: StreamState) -> String {
        switch state {
        case .idle:
            return "idle"
        case .connecting:
            return "connecting"
        case .live:
            return "live"
        case .reconnecting:
            return "reconnecting"
        }
    }

    private func handshakeDescription(_ state: HandshakeState) -> String {
        switch state {
        case .idle:
            return "idle"
        case .sending:
            return "sending"
        case .waitingAck(let handshakeID):
            return "waiting (\(handshakeID))"
        case .success(let handshakeID):
            return "success (\(handshakeID))"
        case .timedOut(let handshakeID):
            return "timed out (\(handshakeID))"
        case .failed(let message):
            return "failed (\(message))"
        }
    }

    private func workerStatusText(model: CodexHandshakeViewModel) -> String {
        let projection = model.missionControlProjection
        if !projection.workers.isEmpty {
            let running = projection.workers.filter { $0.status == "running" }.count
            let totalTurns = projection.workers.reduce(0) { $0 + Int($1.runningTurns) }
            let queued = projection.workers.reduce(0) { $0 + Int($1.queuedRequests) }
            let failed = projection.workers.reduce(0) { $0 + Int($1.failedRequests) }
            return "workers=\(projection.workers.count) running=\(running) turns=\(totalTurns) queued=\(queued) failed=\(failed)"
        }
        if let selected = model.selectedWorkerID {
            let snapshotStatus = model.latestSnapshot?.status ?? "unknown"
            return "\(selected) (\(snapshotStatus))"
        }
        return model.workers.isEmpty ? "none" : "candidates: \(model.workers.count)"
    }

    private func telemetrySummary(model: CodexHandshakeViewModel) -> String {
        let snapshot = model.streamLifecycle
        return [
            "connect=\(snapshot.connectAttempts)",
            "reconnect=\(snapshot.reconnectAttempts)",
            "sessions=\(snapshot.successfulSessions)",
            "recovered=\(snapshot.recoveredSessions)",
            "backoff=\(snapshot.lastBackoffMs)ms",
            "recovery=\(snapshot.lastRecoveryLatencyMs)ms",
            "last=\(snapshot.lastDisconnectReason?.rawValue ?? "n/a")"
        ].joined(separator: " | ")
    }

    private func eventsSummary(model: CodexHandshakeViewModel) -> String {
        if !model.missionControlProjection.events.isEmpty {
            return model.missionControlProjection.events
                .suffix(4)
                .map { record in
                    if let workerID = record.workerID {
                        return "\(workerID):\(record.summary)"
                    }
                    return record.summary
                }
                .joined(separator: " | ")
        }
        if model.recentEvents.isEmpty {
            return "none"
        }
        return model.recentEvents.prefix(4).map(\.event).joined(separator: " | ")
    }

    private func controlRequestSummary(model: CodexHandshakeViewModel) -> String {
        if !model.missionControlProjection.requests.isEmpty {
            return model.missionControlProjection.requests
                .suffix(6)
                .map { request in
                    let shortID = String(request.requestID.suffix(6))
                    if request.state == "error", let code = request.errorCode, !code.isEmpty {
                        return "\(request.method)#\(shortID)[error:\(code)]"
                    }
                    return "\(request.method)#\(shortID)[\(request.state)]"
                }
                .joined(separator: " | ")
        }
        if model.controlRequests.isEmpty {
            return "none"
        }
        return model.controlRequests.prefix(6).map { tracker in
            let shortID = String(tracker.requestID.suffix(6))
            switch tracker.state {
            case .queued:
                return "\(tracker.request.method.rawValue)#\(shortID)[queued]"
            case .running:
                return "\(tracker.request.method.rawValue)#\(shortID)[running]"
            case .success:
                return "\(tracker.request.method.rawValue)#\(shortID)[success]"
            case .error:
                if let code = tracker.errorCode, !code.isEmpty {
                    return "\(tracker.request.method.rawValue)#\(shortID)[error:\(code)]"
                }
                return "\(tracker.request.method.rawValue)#\(shortID)[error]"
            }
        }.joined(separator: " | ")
    }

    deinit {
        os_log("[WGPUI] WgpuiBackgroundUIView deinit", log: wgpuiLog, type: .default)
        NotificationCenter.default.removeObserver(self)
        keyboardProxyField.delegate = nil
        teardownRenderer()
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
                model?.cycleModelOverrideSelection()
            }
            view.onReasoningCycleRequested = { [weak model] in
                model?.cycleReasoningEffortSelection()
            }
            view.onComposerChanged = { [weak model] text in
                model?.messageDraft = text
            }
            view.onAuthEmailChanged = { [weak model] text in
                model?.email = text
            }
            view.onAuthCodeChanged = { [weak model] text in
                model?.verificationCode = text
            }
            view.onSendCodeRequested = { [weak model] in
                guard let model else { return }
                Task { await model.sendEmailCode() }
            }
            view.onVerifyCodeRequested = { [weak model] in
                guard let model else { return }
                Task { await model.verifyEmailCode() }
            }
            view.onSignOutRequested = { [weak model] in
                model?.signOut()
            }
            view.onRefreshWorkersRequested = { [weak model] in
                guard let model else { return }
                Task { await model.refreshWorkers() }
            }
            view.onConnectStreamRequested = { [weak model] in
                model?.connectStream()
            }
            view.onDisconnectStreamRequested = { [weak model] in
                model?.disconnectStream()
            }
            view.onSendHandshakeRequested = { [weak model] in
                guard let model else { return }
                Task { await model.sendHandshake() }
            }
            return view
        }

        func updateUIView(_ uiView: WgpuiBackgroundUIView, context: Context) {
            uiView.onComposerChanged = { [weak model] text in
                model?.messageDraft = text
            }
            uiView.onAuthEmailChanged = { [weak model] text in
                model?.email = text
            }
            uiView.onAuthCodeChanged = { [weak model] text in
                model?.verificationCode = text
            }
            uiView.sync(model: model)
        }

    }
}
