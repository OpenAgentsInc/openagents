import SwiftUI

struct ContentView: View {
    @StateObject private var model = CodexHandshakeViewModel()

    var body: some View {
        TabView {
            CodexChatView(model: model)
                .tabItem {
                    Label("Chat", systemImage: "message.fill")
                }

            CodexDebugView(model: model)
                .tabItem {
                    Label("Debug", systemImage: "ladybug.fill")
                }
        }
    }
}

private struct CodexChatView: View {
    @ObservedObject var model: CodexHandshakeViewModel

    var body: some View {
        VStack(spacing: 0) {
            transcript
            composer
        }
            .task {
                await model.autoConnectOnLaunch()
            }
    }

    private var transcript: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 10) {
                    if model.chatMessages.isEmpty {
                        let emptyDescription: String = {
                            if !model.isAuthenticated {
                                return "Sign in on the Debug tab."
                            }

                            switch model.streamState {
                            case .connecting, .reconnecting:
                                return "Connecting to your desktop Codex stream..."
                            default:
                                return "Waiting for Codex events from desktop."
                            }
                        }()

                        ContentUnavailableView(
                            "No Codex Messages Yet",
                            systemImage: "message",
                            description: Text(emptyDescription)
                        )
                        .padding(.top, 60)
                    } else {
                        ForEach(model.chatMessages) { message in
                            CodexMessageBubble(message: message)
                                .id(message.id)
                        }
                    }
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
            }
            .background(Color(.systemGroupedBackground))
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .onChange(of: model.chatMessages.count) { _, _ in
                guard let last = model.chatMessages.last else {
                    return
                }
                withAnimation(.easeOut(duration: 0.2)) {
                    proxy.scrollTo(last.id, anchor: .bottom)
                }
            }
        }
    }

    private var composer: some View {
        HStack(alignment: .bottom, spacing: 8) {
            TextField("Message Codex", text: $model.messageDraft, axis: .vertical)
                .lineLimit(1...5)
                .textFieldStyle(.roundedBorder)
                .textInputAutocapitalization(.sentences)
                .autocorrectionDisabled(false)
                .submitLabel(.send)
                .onSubmit {
                    Task {
                        await model.sendUserMessage()
                    }
                }

            Button {
                Task {
                    await model.sendUserMessage()
                }
            } label: {
                if model.isSendingMessage {
                    ProgressView()
                        .progressViewStyle(.circular)
                } else {
                    Text("Send")
                }
            }
            .buttonStyle(.borderedProminent)
            .disabled(!model.canSendMessage)
        }
        .padding(.horizontal, 10)
        .padding(.top, 8)
        .padding(.bottom, 10)
        .background(Color(.systemBackground))
    }
}

private struct CodexMessageBubble: View {
    let message: CodexChatMessage

    var body: some View {
        HStack(alignment: .bottom) {
            if isUser {
                Spacer(minLength: 40)
            }

            VStack(alignment: .leading, spacing: 4) {
                if showRoleLabel {
                    Text(roleLabel(for: message.role))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .textCase(.uppercase)
                }

                Text(displayText)
                    .font(.body)
                    .textSelection(.enabled)

                if message.isStreaming {
                    Text("streaming")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(bubbleBackground)
            .foregroundStyle(bubbleForeground)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))

            if !isUser {
                Spacer(minLength: 40)
            }
        }
    }

    private var isUser: Bool {
        message.role == .user
    }

    private var showRoleLabel: Bool {
        message.role != .user && message.role != .assistant
    }

    private var displayText: String {
        let trimmed = message.text.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? "…" : message.text
    }

    private var bubbleBackground: Color {
        switch message.role {
        case .user:
            return .blue
        case .assistant:
            return Color(.secondarySystemBackground)
        case .reasoning:
            return Color(.tertiarySystemBackground)
        case .tool:
            return Color(.systemGray5)
        case .system:
            return Color(.secondarySystemBackground)
        case .error:
            return Color.red.opacity(0.2)
        }
    }

    private var bubbleForeground: Color {
        switch message.role {
        case .user:
            return .white
        case .error:
            return .red
        default:
            return .primary
        }
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
                            .foregroundStyle(.secondary)
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
                            .foregroundStyle(.secondary)
                    } else {
                        if let selectedWorkerID = model.selectedWorkerID {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Active Worker")
                                Text("\(selectedWorkerID) (\(model.latestSnapshot?.status ?? "unknown"))")
                                    .foregroundStyle(.secondary)
                                    .font(.footnote)
                            }
                        }

                        if model.workers.count > 1 {
                            Text("Auto-selecting freshest running desktop worker (\(model.workers.count) candidates found).")
                                .font(.footnote)
                                .foregroundStyle(.secondary)
                        }

                        if let snapshot = model.latestSnapshot {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Snapshot: \(snapshot.status)")
                                Text("Latest seq: \(snapshot.latestSeq)")
                                    .foregroundStyle(.secondary)
                                    .font(.footnote)
                            }
                        }
                    }
                }

                Section("Handshake") {
                    Text("Device ID: \(model.deviceID)")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
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

                Section("Recent Events") {
                    if model.recentEvents.isEmpty {
                        Text("No events yet")
                            .foregroundStyle(.secondary)
                    } else {
                        ForEach(Array(model.recentEvents.enumerated()), id: \.offset) { _, event in
                            VStack(alignment: .leading, spacing: 4) {
                                Text(event.event)
                                    .font(.caption)
                                Text(event.rawData)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(3)
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
                            .foregroundStyle(.red)
                    }
                }
            }
            .navigationTitle("Codex Debug")
        }
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
        return "connecting (waiting for first poll)"
    case .live:
        return "live"
    case .reconnecting:
        return "reconnecting (retrying)"
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

private func roleLabel(for role: CodexChatRole) -> String {
    switch role {
    case .user:
        return "User"
    case .assistant:
        return "Assistant"
    case .reasoning:
        return "Reasoning"
    case .tool:
        return "Tool"
    case .system:
        return "System"
    case .error:
        return "Error"
    }
}

#Preview {
    ContentView()
}
