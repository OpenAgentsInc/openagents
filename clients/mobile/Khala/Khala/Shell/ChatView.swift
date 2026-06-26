import SwiftUI

/// The main chat surface for a single conversation.
///
/// FOUNDATION SEAM (issue #6345 fills this out): rich markdown / fenced
/// code-block rendering, response action rows (copy, regenerate), and the
/// polished ChatGPT-style bubble layout. Issue #6344 owns composer streaming
/// refinements.
///
/// This foundation version is functional today: it renders the persisted
/// transcript, the existing voice visualization + push-to-talk orb, a text
/// composer, and the folded-in Codex/Delegate panel. Completed turns are
/// persisted to the active `Conversation` through `ConversationStore` so the
/// chat survives relaunch and shows up in Recents.
struct ChatView: View {
    @ObservedObject var store: ConversationStore
    @ObservedObject var voice: VoiceController
    let conversation: Conversation
    @Binding var hasKey: Bool
    let onOpenSettings: () -> Void

    @State private var typedMessage = ""
    @AppStorage("khala.codex.pylonRef") private var codexPylonRef = ""
    @FocusState private var composerFocused: Bool

    var body: some View {
        ZStack {
            AnimatedBackground(level: voice.level, accent: voice.state.accentColor)

            ScrollViewReader { proxy in
                ScrollView {
                    VStack(spacing: 18) {
                        transcriptPanel
                            .id("transcript")
                        voiceControls
                        codexTaskPanel
                        if !hasKey {
                            Button("Add a Khala key to get started", action: onOpenSettings)
                                .font(.footnote.weight(.semibold))
                                .foregroundStyle(.orange)
                        }
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.horizontal)
                    .padding(.vertical, 18)
                }
                .scrollDismissesKeyboard(.interactively)
                .onChange(of: voice.response) { _, _ in scroll(proxy) }
                .onChange(of: voice.requestError) { _, _ in scroll(proxy) }
                .onChange(of: conversation.messages.count) { _, _ in scroll(proxy) }
            }
        }
        .safeAreaInset(edge: .bottom) {
            composer
                .padding(.horizontal)
                .padding(.top, 10)
                .padding(.bottom, 8)
                .background(.thinMaterial)
        }
    }

    // MARK: - Transcript

    private var transcriptPanel: some View {
        VStack(alignment: .leading, spacing: 12) {
            let persisted = conversation.sortedMessages.filter { $0.role != .system }
            if persisted.isEmpty && voice.transcript.isEmpty && voice.response.isEmpty && voice.requestError == nil {
                Text("Type a message or hold to talk.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.vertical, 32)
            } else {
                ForEach(persisted) { message in
                    MessageBubble(
                        title: message.role == .user ? "You" : "Khala",
                        text: message.content,
                        outgoing: message.role == .user
                    )
                }

                // Live in-flight turn (not yet persisted).
                if voice.state.isBusy || !voice.response.isEmpty || voice.requestError != nil {
                    if !voice.transcript.isEmpty, lastPersistedUserDiffers(voice.transcript) {
                        MessageBubble(title: "You", text: voice.transcript, outgoing: true)
                    }
                    if !voice.response.isEmpty {
                        MessageBubble(
                            title: "Khala",
                            text: voice.response,
                            outgoing: false,
                            isStreaming: voice.state.isBusy
                        )
                    }
                    if let requestError = voice.requestError {
                        errorNotice(requestError)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .animation(.easeOut(duration: 0.18), value: voice.transcript)
        .animation(.easeOut(duration: 0.18), value: voice.response)
        .animation(.easeOut(duration: 0.18), value: voice.requestError)
        // Persist completed assistant turns into the conversation.
        .onChange(of: voice.state) { _, newValue in
            persistCompletedTurnIfNeeded(state: newValue)
        }
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

    private var composer: some View {
        HStack(spacing: 8) {
            TextField("Ask Khala", text: $typedMessage, axis: .vertical)
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
                        ProgressView().controlSize(.small)
                    } else {
                        Image(systemName: "arrow.up.circle.fill").font(.title)
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

    /// Explicit coding-delegation path, folded in from the prior ContentView.
    /// The server enforces owner-scoped Pylon authorization; this keeps the
    /// mobile request typed, bounded, and separate from plain chat.
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

    // MARK: - Send

    private var canSend: Bool {
        hasKey && !voice.state.isBusy
            && !typedMessage.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var canSendCodexTask: Bool {
        canSend && codexPylonRef.trimmingCharacters(in: .whitespacesAndNewlines).count >= 3
    }

    private func sendTyped() {
        let text = typedMessage.trimmingCharacters(in: .whitespacesAndNewlines)
        guard canSend else { return }
        composerFocused = false
        typedMessage = ""
        store.appendMessage(.user, content: text, to: conversation)
        voice.sendText(text)
    }

    private func sendCodexTask() {
        let text = typedMessage.trimmingCharacters(in: .whitespacesAndNewlines)
        guard canSendCodexTask else { return }
        composerFocused = false
        typedMessage = ""
        store.appendMessage(.user, content: text, to: conversation)
        voice.sendCodexTask(text, pylonRef: codexPylonRef)
    }

    // MARK: - Persistence of completed turns

    @State private var persistedResponseForState = false

    /// When a turn finishes (state leaves `.thinking` for `.idle`/`.success`)
    /// with a non-empty response, persist the assistant turn. Voice turns also
    /// persist the user transcript first if it was not already recorded.
    private func persistCompletedTurnIfNeeded(state: VoiceState) {
        switch state {
        case .thinking, .recording, .transcribing:
            persistedResponseForState = false
        case .idle, .success:
            guard !persistedResponseForState else { return }
            // Persist a voice user turn that bypassed the text composer.
            let trimmedTranscript = voice.transcript.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmedTranscript.isEmpty, lastPersistedUserDiffers(trimmedTranscript) {
                store.appendMessage(.user, content: trimmedTranscript, to: conversation)
            }
            let reply = voice.response.trimmingCharacters(in: .whitespacesAndNewlines)
            if !reply.isEmpty {
                store.appendMessage(.assistant, content: reply, to: conversation)
                persistedResponseForState = true
            }
        case .error:
            persistedResponseForState = false
        }
    }

    /// True when the latest persisted user message differs from `text`, so a
    /// composer-sent message (already persisted in `sendTyped`) is not duplicated.
    private func lastPersistedUserDiffers(_ text: String) -> Bool {
        let lastUser = conversation.sortedMessages.last { $0.role == .user }
        return lastUser?.content.trimmingCharacters(in: .whitespacesAndNewlines)
            != text.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func scroll(_ proxy: ScrollViewProxy) {
        withAnimation(.easeOut(duration: 0.2)) {
            proxy.scrollTo("transcript", anchor: .bottom)
        }
    }
}
