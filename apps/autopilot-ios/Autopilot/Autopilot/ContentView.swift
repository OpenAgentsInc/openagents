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
        NavigationStack {
            VStack(spacing: 0) {
                header
                Divider()
                transcript
                Divider()
                controls
            }
            .navigationTitle("Codex")
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("OpenAgents • \(model.environmentHost)")
                .font(.footnote)
                .foregroundStyle(.secondary)

            Text("Auth: \(authDescription(model.authState))")
                .font(.footnote)
                .foregroundStyle(model.isAuthenticated ? Color.secondary : Color.orange)

            if let workerID = model.selectedWorkerID {
                Text("Worker: \(workerID)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .textSelection(.enabled)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(Color(.secondarySystemBackground))
    }

    private var transcript: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 10) {
                    if model.chatMessages.isEmpty {
                        ContentUnavailableView(
                            "No Codex Messages Yet",
                            systemImage: "message",
                            description: Text("Start the worker stream to render Codex events as chat.")
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

    private var controls: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Button("Load Worker") {
                    Task {
                        await model.refreshWorkers()
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(!model.isAuthenticated)

                Button(model.streamState == .idle ? "Connect" : "Disconnect") {
                    if model.streamState == .idle {
                        model.connectStream()
                    } else {
                        model.disconnectStream()
                    }
                }
                .buttonStyle(.bordered)
                .disabled(!model.isAuthenticated)

                Button("Handshake") {
                    Task {
                        await model.sendHandshake()
                    }
                }
                .buttonStyle(.bordered)
                .disabled(!model.isAuthenticated || model.selectedWorkerID == nil)
            }

            Text("Stream: \(streamDescription(model.streamState)) • Handshake: \(handshakeDescription(model.handshakeState))")
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(2)

            if !model.isAuthenticated {
                Text("Sign in on the Debug tab first.")
                    .font(.caption)
                    .foregroundStyle(.orange)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(Color(.secondarySystemBackground))
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
                            Button("Send Code") {
                                Task {
                                    await model.sendEmailCode()
                                }
                            }

                            Button("Verify") {
                                Task {
                                    await model.verifyEmailCode()
                                }
                            }
                            .disabled(model.verificationCode.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
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
