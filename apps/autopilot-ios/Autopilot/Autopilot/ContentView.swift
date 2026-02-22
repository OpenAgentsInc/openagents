import SwiftUI

struct ContentView: View {
    @StateObject private var model = CodexHandshakeViewModel()
    @Environment(\.scenePhase) private var scenePhase
    @State private var showDebugSurface = false

    var body: some View {
        WgpuiBackgroundView(model: model)
            .overlay(alignment: .topTrailing) {
                Color.clear
                    .frame(width: 44, height: 44)
                    .contentShape(Rectangle())
                    .onLongPressGesture(minimumDuration: 1.2) {
                        showDebugSurface = true
                    }
                    .accessibilityHidden(true)
            }
            .sheet(isPresented: $showDebugSurface) {
                CodexDebugView(model: model)
            }
            .task {
                await model.autoConnectOnLaunch()
            }
            .onAppear {
                model.handleScenePhaseChange(scenePhase)
            }
            .onChange(of: scenePhase) { _, newPhase in
                model.handleScenePhaseChange(newPhase)
            }
            .preferredColorScheme(.dark)
    }
}

private struct CodexDebugView: View {
    @ObservedObject var model: CodexHandshakeViewModel

    var body: some View {
        NavigationStack {
            Form {
                Section("OpenAgents") {
                    HStack {
                        Text("Environment")
                        Spacer()
                        Text(model.environmentHost)
                            .foregroundStyle(OATheme.mutedForeground)
                    }

                    Text("Auth: \(authDescription(model.authState))")
                        .font(.footnote)

                    if model.isAuthenticated {
                        Button("Sign Out") {
                            model.signOut()
                        }
                    } else {
                        TextField("Email", text: $model.email)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled(true)
                            .keyboardType(.emailAddress)

                        TextField("Verification Code", text: $model.verificationCode)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled(true)
                            .keyboardType(.numberPad)

                        HStack {
                            Button(model.isSendingCode ? "Sending..." : "Send Code") {
                                Task {
                                    await model.sendEmailCode()
                                }
                            }
                            .disabled(!model.canSendAuthCode)

                            Button(model.isVerifyingCode ? "Verifying..." : "Verify") {
                                Task {
                                    await model.verifyEmailCode()
                                }
                            }
                            .disabled(!model.canVerifyAuthCode)
                        }
                    }
                }

                Section("Worker") {
                    Button("Load Workers + Handshake") {
                        Task {
                            await model.refreshWorkers()
                        }
                    }
                    .disabled(!model.isAuthenticated)

                    if model.workers.isEmpty {
                        Text("No workers loaded")
                            .foregroundStyle(OATheme.mutedForeground)
                    } else {
                        if let selectedWorkerID = model.selectedWorkerID {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Active Worker")
                                Text("\(selectedWorkerID) (\(model.latestSnapshot?.status ?? "unknown"))")
                                    .foregroundStyle(OATheme.mutedForeground)
                                    .font(.footnote)
                            }
                        }

                        if model.workers.count > 1 {
                            Text("Auto-selecting freshest running desktop worker (\(model.workers.count) candidates found).")
                                .font(.footnote)
                                .foregroundStyle(OATheme.mutedForeground)
                        }

                        if let snapshot = model.latestSnapshot {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Snapshot: \(snapshot.status)")
                                Text("Latest seq: \(snapshot.latestSeq)")
                                    .foregroundStyle(OATheme.mutedForeground)
                                    .font(.footnote)
                            }
                        }
                    }
                }

                Section("Handshake") {
                    Text("Device ID: \(model.deviceID)")
                        .font(.footnote)
                        .foregroundStyle(OATheme.mutedForeground)
                        .textSelection(.enabled)

                    Text("Stream: \(streamDescription(model.streamState))")
                        .font(.footnote)

                    Text("Handshake: \(handshakeDescription(model.handshakeState))")
                        .font(.footnote)

                    HStack {
                        Button("Connect Stream") {
                            model.connectStream()
                        }
                        .disabled(!model.isAuthenticated)

                        Button("Disconnect") {
                            model.disconnectStream()
                        }
                    }

                    Button("Send Handshake") {
                        Task {
                            await model.sendHandshake()
                        }
                    }
                    .disabled(!model.isAuthenticated || model.selectedWorkerID == nil)
                }

                Section("Reconnect Telemetry") {
                    LabeledContent("Connect attempts", value: "\(model.streamLifecycle.connectAttempts)")
                    LabeledContent("Reconnect attempts", value: "\(model.streamLifecycle.reconnectAttempts)")
                    LabeledContent("Successful sessions", value: "\(model.streamLifecycle.successfulSessions)")
                    LabeledContent("Recovered sessions", value: "\(model.streamLifecycle.recoveredSessions)")
                    LabeledContent("Last backoff", value: "\(model.streamLifecycle.lastBackoffMs)ms")
                    LabeledContent(
                        "Last recovery latency",
                        value: "\(model.streamLifecycle.lastRecoveryLatencyMs)ms"
                    )
                    LabeledContent(
                        "Last disconnect",
                        value: model.streamLifecycle.lastDisconnectReason?.rawValue ?? "n/a"
                    )
                }

                Section("Reconnect Events") {
                    if model.streamLifecycleEvents.isEmpty {
                        Text("No lifecycle events yet")
                            .foregroundStyle(OATheme.mutedForeground)
                    } else {
                        ForEach(Array(model.streamLifecycleEvents.enumerated()), id: \.offset) { _, line in
                            Text(line)
                                .font(.caption2)
                                .textSelection(.enabled)
                        }
                    }
                }

                Section("Recent Events") {
                    if model.recentEvents.isEmpty {
                        Text("No events yet")
                            .foregroundStyle(OATheme.mutedForeground)
                    } else {
                        ForEach(Array(model.recentEvents.enumerated()), id: \.offset) { _, event in
                            VStack(alignment: .leading, spacing: 4) {
                                Text(event.event)
                                    .font(.caption)
                                Text(event.rawData)
                                    .font(.caption2)
                                    .foregroundStyle(OATheme.mutedForeground)
                                    .lineLimit(3)
                            }
                        }
                    }
                }

                Section("Control Requests") {
                    if model.controlRequests.isEmpty {
                        Text("No control requests yet")
                            .foregroundStyle(OATheme.mutedForeground)
                    } else {
                        ForEach(model.controlRequests.prefix(20)) { request in
                            VStack(alignment: .leading, spacing: 4) {
                                Text("\(request.request.method.rawValue) [\(request.state.rawValue)]")
                                    .font(.caption)
                                Text(request.requestID)
                                    .font(.caption2)
                                    .foregroundStyle(OATheme.mutedForeground)
                                    .textSelection(.enabled)
                                if let message = request.errorMessage, !message.isEmpty {
                                    Text(message)
                                        .font(.caption2)
                                        .foregroundStyle(OATheme.destructive)
                                }
                            }
                        }
                    }
                }

                if let status = model.statusMessage {
                    Section("Status") {
                        Text(status)
                    }
                }

                if let error = model.errorMessage {
                    Section("Error") {
                        Text(error)
                            .foregroundStyle(OATheme.destructive)
                    }
                }
            }
            .scrollContentBackground(.hidden)
            .background(OATheme.background)
            .tint(OATheme.ring)
            .navigationTitle("Codex Debug")
        }
        .foregroundStyle(OATheme.foreground)
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
        return "connecting (Khala websocket)"
    case .live:
        return "live"
    case .reconnecting:
        return "reconnecting (Khala websocket)"
    }
}

private func handshakeDescription(_ state: HandshakeState) -> String {
    switch state {
    case .idle:
        return "idle"
    case .sending:
        return "sending"
    case .waitingAck(let handshakeID):
        return "waiting ack (\(handshakeID))"
    case .success(let handshakeID):
        return "success (\(handshakeID))"
    case .timedOut(let handshakeID):
        return "timed out (\(handshakeID))"
    case .failed(let message):
        return "failed (\(message))"
    }
}

#Preview {
    ContentView()
}

private enum OATheme {
    static let background = Color(red: 16 / 255, green: 16 / 255, blue: 17 / 255)
    static let foreground = Color(red: 216 / 255, green: 222 / 255, blue: 233 / 255)
    static let mutedForeground = Color(red: 153 / 255, green: 153 / 255, blue: 153 / 255)
    static let destructive = Color(red: 191 / 255, green: 97 / 255, blue: 106 / 255)
    static let ring = Color(red: 136 / 255, green: 192 / 255, blue: 208 / 255)
}
