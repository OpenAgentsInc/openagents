import SwiftUI

struct ContentView: View {
    @StateObject private var model = CodexHandshakeViewModel()

    var body: some View {
        ZStack {
            OATheme.background.ignoresSafeArea()

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
        .tint(OATheme.ring)
        .preferredColorScheme(.dark)
    }
}

private struct CodexChatView: View {
    @ObservedObject var model: CodexHandshakeViewModel
    @FocusState private var isComposerFocused: Bool

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
            .scrollDismissesKeyboard(.interactively)
            .contentShape(Rectangle())
            .onTapGesture {
                dismissKeyboard()
            }
            .background(OATheme.background)
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
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
                .textInputAutocapitalization(.sentences)
                .autocorrectionDisabled(false)
                .focused($isComposerFocused)
                .submitLabel(.send)
                .background(
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .fill(OATheme.input)
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(OATheme.border, lineWidth: 1)
                )
                .foregroundStyle(OATheme.foreground)
                .tint(OATheme.ring)
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
            .tint(OATheme.primary)
            .disabled(!model.canSendMessage)
        }
        .padding(.horizontal, 10)
        .padding(.top, 8)
        .padding(.bottom, 10)
        .background(OATheme.background)
    }

    private func dismissKeyboard() {
        isComposerFocused = false
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
                        .foregroundStyle(OATheme.mutedForeground)
                        .textCase(.uppercase)
                }

                Text(displayText)
                    .font(.body)
                    .textSelection(.enabled)

                if message.isStreaming {
                    Text("streaming")
                        .font(.caption2)
                        .foregroundStyle(OATheme.mutedForeground)
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
            return OATheme.primary
        case .assistant:
            return OATheme.card
        case .reasoning:
            return OATheme.muted
        case .tool:
            return OATheme.accent
        case .system:
            return OATheme.card
        case .error:
            return OATheme.destructive.opacity(0.25)
        }
    }

    private var bubbleForeground: Color {
        switch message.role {
        case .user:
            return .white
        case .error:
            return OATheme.destructive
        default:
            return OATheme.foreground
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

private enum OATheme {
    static let background = Color(red: 16 / 255, green: 16 / 255, blue: 17 / 255)
    static let foreground = Color(red: 216 / 255, green: 222 / 255, blue: 233 / 255)
    static let card = Color(red: 26 / 255, green: 26 / 255, blue: 26 / 255)
    static let muted = Color(red: 42 / 255, green: 42 / 255, blue: 42 / 255)
    static let mutedForeground = Color(red: 153 / 255, green: 153 / 255, blue: 153 / 255)
    static let accent = Color(red: 80 / 255, green: 80 / 255, blue: 80 / 255)
    static let primary = Color(red: 79 / 255, green: 79 / 255, blue: 85 / 255)
    static let destructive = Color(red: 191 / 255, green: 97 / 255, blue: 106 / 255)
    static let border = Color(red: 42 / 255, green: 42 / 255, blue: 42 / 255)
    static let input = Color(red: 42 / 255, green: 42 / 255, blue: 42 / 255)
    static let ring = Color(red: 136 / 255, green: 192 / 255, blue: 208 / 255)
}
