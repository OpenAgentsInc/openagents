import SwiftUI

/// The single Khala screen: animated background, push-to-talk button, and the
/// transcript/response text. A gear opens Settings (key + voice options).
struct ContentView: View {
    @StateObject private var voice = VoiceController()
    @State private var showSettings = false
    @State private var hasKey = KeychainStore.hasAPIKey
    @State private var permissionsRequested = false
    @State private var typedMessage = ""
    @AppStorage("khala.codex.pylonRef") private var codexPylonRef = ""
    @FocusState private var composerFocused: Bool

    var body: some View {
        ZStack {
            AnimatedBackground(level: voice.level, accent: voice.state.accentColor)

            VStack(spacing: 0) {
                header
                    .padding(.horizontal)
                    .padding(.top)
                    .padding(.bottom, 10)

                ScrollViewReader { proxy in
                    ScrollView {
                        VStack(spacing: 18) {
                            conversationPanel
                                .id("conversation")

                            voiceControls

                            codexTaskPanel

                            if !hasKey {
                                Button("Add a Khala key to get started") { showSettings = true }
                                    .font(.footnote.weight(.semibold))
                                    .foregroundStyle(.orange)
                            }
                        }
                        .frame(maxWidth: .infinity)
                        .padding(.horizontal)
                        .padding(.bottom, 18)
                    }
                    .scrollDismissesKeyboard(.interactively)
                    .onChange(of: voice.response) { _, _ in
                        withAnimation(.easeOut(duration: 0.2)) {
                            proxy.scrollTo("conversation", anchor: .bottom)
                        }
                    }
                    .onChange(of: voice.requestError) { _, _ in
                        withAnimation(.easeOut(duration: 0.2)) {
                            proxy.scrollTo("conversation", anchor: .bottom)
                        }
                    }
                }
            }
        }
        .safeAreaInset(edge: .bottom) {
            composer
                .padding(.horizontal)
                .padding(.top, 10)
                .padding(.bottom, 8)
                .background(.thinMaterial)
        }
        .sheet(isPresented: $showSettings) {
            SettingsView(hasKey: $hasKey)
        }
        .task {
            guard !permissionsRequested else { return }
            permissionsRequested = true
            _ = await voice.requestPermissions()
            // Demo/test hook (env-gated; no-op in normal use): auto-send a prompt
            // on launch so the end-to-end Khala API round-trip is verifiable on a
            // simulator without driving the UI. Pair with KHALA_API_KEY.
            if let demo = ProcessInfo.processInfo.environment["KHALA_DEMO_PROMPT"],
               !demo.isEmpty, hasKey {
                voice.sendText(demo)
            }
        }
    }

    private var conversationPanel: some View {
        VStack(alignment: .leading, spacing: 12) {
            if voice.transcript.isEmpty && voice.response.isEmpty && voice.requestError == nil {
                Text("Type a message or hold to talk.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.vertical, 32)
            } else {
                if !voice.transcript.isEmpty {
                    messageBubble(title: "You", text: voice.transcript, outgoing: true)
                }

                if !voice.response.isEmpty {
                    messageBubble(title: "Khala", text: voice.response, outgoing: false)
                }

                if let requestError = voice.requestError {
                    errorNotice(requestError)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.top, 10)
        .animation(.easeOut(duration: 0.18), value: voice.transcript)
        .animation(.easeOut(duration: 0.18), value: voice.response)
        .animation(.easeOut(duration: 0.18), value: voice.requestError)
    }

    private var voiceControls: some View {
        VStack(spacing: 12) {
            Text(voice.state.label)
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(voice.state.accentColor)
                .multilineTextAlignment(.center)
                .lineLimit(2)
                .minimumScaleFactor(0.85)

            PushToTalkButton(
                state: voice.state,
                level: voice.level,
                onPressDown: { voice.pressDown() },
                onPressUp: { voice.pressUp() }
            )
            .padding(.bottom, 4)
        }
    }

    private func messageBubble(
        title: String,
        text: String,
        outgoing: Bool
    ) -> some View {
        HStack {
            if outgoing {
                Spacer(minLength: 34)
            }

            VStack(alignment: .leading, spacing: 6) {
                Text(title)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                Text(text)
                    .font(.body)
                    .foregroundStyle(.primary)
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(14)
            .frame(maxWidth: 520, alignment: .leading)
            .background(
                outgoing ? AnyShapeStyle(.thinMaterial) : AnyShapeStyle(.regularMaterial),
                in: RoundedRectangle(cornerRadius: 14)
            )

            if !outgoing {
                Spacer(minLength: 34)
            }
        }
    }

    private func errorNotice(_ error: VoiceController.RequestError) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Image(systemName: error.isRetryable ? "arrow.clockwise.circle.fill" : "exclamationmark.circle.fill")
                    .foregroundStyle(error.isRetryable ? .orange : .red)
                Text(error.title)
                    .font(.callout.weight(.semibold))
                    .foregroundStyle(.primary)
            }

            Text(error.message)
                .font(.footnote)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            if error.isRetryable {
                Button(action: voice.retryLastSubmission) {
                    Label("Retry", systemImage: "arrow.clockwise")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .disabled(voice.state.isBusy)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12))
    }

    /// Voice-free composer: type a message and send it through the same Khala
    /// round-trip the push-to-talk path uses. This is the minimal way to test
    /// the end-to-end handshake without microphone/speech permissions.
    private var composer: some View {
        HStack(spacing: 8) {
            TextField("Type a message…", text: $typedMessage, axis: .vertical)
                .lineLimit(1...4)
                .textFieldStyle(.plain)
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
                .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 12))
                .focused($composerFocused)
                .submitLabel(.send)
                .onSubmit(sendTyped)
                .disabled(voice.state.isBusy)

            Button(action: sendTyped) {
                Group {
                    if voice.state.isBusy {
                        ProgressView()
                            .controlSize(.small)
                    } else {
                        Image(systemName: "arrow.up.circle.fill")
                            .font(.title)
                    }
                }
                .frame(width: 34, height: 34)
                .foregroundStyle(canSend ? voice.state.accentColor : Color.secondary)
            }
            .disabled(!canSend)
            .accessibilityLabel("Send message")
        }
        .padding(.horizontal, 4)
    }

    /// Explicit coding-delegation path. The server still enforces owner-scoped
    /// Pylon authorization; this form keeps the mobile request typed, bounded,
    /// and separate from plain chat.
    private var codexTaskPanel: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Label("Codex task", systemImage: "hammer")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(.secondary)
                Spacer()
                Button(action: sendCodexTask) {
                    HStack(spacing: 6) {
                        Image(systemName: "arrow.triangle.branch")
                        Text("Delegate")
                    }
                    .font(.footnote.weight(.semibold))
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
                .disabled(!canSendCodexTask)
            }

            TextField("caller-owned Pylon ref", text: $codexPylonRef)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .font(.system(.footnote, design: .monospaced))
                .padding(.horizontal, 12)
                .padding(.vertical, 9)
                .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 10))
                .disabled(voice.state.isBusy)
        }
        .padding(12)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12))
        .padding(.horizontal, 4)
    }

    private var canSend: Bool {
        hasKey
            && !voice.state.isBusy
            && !typedMessage.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var canSendCodexTask: Bool {
        canSend
            && codexPylonRef.trimmingCharacters(in: .whitespacesAndNewlines).count >= 3
    }

    private func sendTyped() {
        let text = typedMessage
        guard canSend else { return }
        composerFocused = false
        typedMessage = ""
        voice.sendText(text)
    }

    private func sendCodexTask() {
        let text = typedMessage
        guard canSendCodexTask else { return }
        composerFocused = false
        typedMessage = ""
        voice.sendCodexTask(text, pylonRef: codexPylonRef)
    }

    private var header: some View {
        HStack {
            Text("Khala")
                .font(.title2.weight(.semibold))
                .foregroundStyle(.primary)
            Spacer()
            Button { showSettings = true } label: {
                Image(systemName: "gearshape")
                    .font(.title3)
                    .foregroundStyle(.secondary)
            }
        }
    }
}

#Preview {
    ContentView().preferredColorScheme(.dark)
}
