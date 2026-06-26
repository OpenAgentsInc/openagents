import SwiftUI

/// The main chat surface for a single conversation.
///
/// #6346 wires LIVE STREAMING into this view: typed and voice turns both append
/// a user `Message`, create an empty assistant `Message`, and stream the reply
/// token-by-token over the FULL conversation history (multi-turn). The assistant
/// bubble grows live (`MessageBubble(isStreaming:true)`) and settles into
/// rendered markdown/code on `[DONE]`. A stop control cancels the in-flight
/// stream; the regenerate hook re-streams the last user turn.
///
/// Voice and text share one transcript: the push-to-talk orb transcribes and
/// hands the transcript to the same streaming `send(_:)`, so a spoken turn is a
/// normal user turn that streams a reply. The voice visualization stays.
///
/// The streaming chat round-trip is owned by `ChatViewModel`; the explicit Codex
/// delegation panel keeps its separate, typed, non-streaming path on
/// `VoiceController`.
struct ChatView: View {
    @ObservedObject var store: ConversationStore
    @ObservedObject var voice: VoiceController
    @ObservedObject var model: ChatViewModel
    let conversation: Conversation
    @Binding var hasKey: Bool
    let onOpenSettings: () -> Void

    @State private var typedMessage = ""
    @AppStorage("khala.codex.pylonRef") private var codexPylonRef = ""
    @FocusState private var composerFocused: Bool

    /// A fresh chat with nothing in flight: show the empty-state suggestions in
    /// place of the scroll content. The composer inset below stays visible, so
    /// there is ALWAYS a text box to type what you want.
    private var isEmptyState: Bool {
        conversation.messages.isEmpty
            && !voice.state.isBusy
            && voice.response.isEmpty
            && voice.transcript.isEmpty
    }

    var body: some View {
        ZStack {
            AnimatedBackground(level: voice.level, accent: voice.state.accentColor)

            if isEmptyState {
                // Home/empty state: greeting + the voice orb (push-to-talk) as the
                // primary affordance. No canned suggestions — hold the orb to talk
                // or type in the composer below.
                VStack(spacing: 32) {
                    Spacer(minLength: 0)
                    emptyGreeting
                    voiceControls
                    Spacer(minLength: 0)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .padding(.horizontal)
            } else {
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
                        Color.clear.frame(height: 1).id("bottom")
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.horizontal)
                    .padding(.vertical, 18)
                }
                .scrollDismissesKeyboard(.interactively)
                // Auto-scroll to the streaming turn as it grows + on each delta.
                .onChange(of: model.streamTick) { _, _ in scroll(proxy) }
                .onChange(of: conversation.messages.count) { _, _ in scroll(proxy) }
                .onChange(of: model.error) { _, _ in scroll(proxy) }
                .onChange(of: voice.response) { _, _ in scroll(proxy) }
                .onChange(of: voice.requestError) { _, _ in scroll(proxy) }
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
        .onAppear {
            // Route push-to-talk transcripts into the shared streaming path.
            voice.onTranscript = { [weak model] transcript in
                model?.send(transcript)
            }
        }
    }

    // MARK: - Transcript

    private var transcriptPanel: some View {
        VStack(alignment: .leading, spacing: 12) {
            let persisted = conversation.sortedMessages.filter { $0.role != .system }
            if persisted.isEmpty && !model.isStreaming && model.error == nil
                && voice.response.isEmpty && voice.requestError == nil {
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
                        outgoing: message.role == .user,
                        isStreaming: message.id == model.streamingMessageID,
                        onRegenerate: regenerateHook(for: message)
                    )
                    .id(message.id)
                }

                // Inline streaming-chat error with retry.
                if let error = model.error {
                    chatErrorNotice(error)
                }

                // The separate Codex-delegation panel surfaces its result/error
                // through the voice controller; keep rendering those.
                if !voice.response.isEmpty {
                    MessageBubble(
                        title: "Khala",
                        text: voice.response,
                        outgoing: false,
                        isStreaming: voice.state.isBusy
                    )
                }
                if let requestError = voice.requestError {
                    codexErrorNotice(requestError)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .animation(.easeOut(duration: 0.18), value: model.streamTick)
        .animation(.easeOut(duration: 0.18), value: model.error)
        .animation(.easeOut(duration: 0.18), value: voice.response)
        .animation(.easeOut(duration: 0.18), value: voice.requestError)
    }

    /// Regenerate is offered only on the LAST assistant turn and only when no
    /// stream is in flight.
    private func regenerateHook(for message: Message) -> (() -> Void)? {
        guard message.role == .assistant, !model.isStreaming else { return nil }
        let lastAssistant = conversation.sortedMessages.last { $0.role == .assistant }
        guard message.id == lastAssistant?.id else { return nil }
        return { model.regenerate() }
    }

    /// Home/empty-state greeting shown above the voice orb.
    private var emptyGreeting: some View {
        VStack(spacing: 10) {
            Image(systemName: "sparkles")
                .font(.system(size: 34, weight: .semibold))
                .foregroundStyle(.tint)
                .accessibilityHidden(true)
            Text("Khala")
                .font(.largeTitle.weight(.semibold))
                .foregroundStyle(.primary)
            Text("Collective intelligence behind a free API — one mind, many models. Hold the orb to talk, or type below.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: 460)
        .padding(.horizontal, 12)
    }

    private var voiceControls: some View {
        VStack(spacing: 12) {
            Text(voiceLabel)
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

    /// While a chat stream is in flight the orb reads "Khala is replying…" so the
    /// voice label reflects the shared transcript state.
    private var voiceLabel: String {
        if model.isStreaming { return "Khala is replying…" }
        return voice.state.label
    }

    /// Inline error for the streaming chat path (402 / network / HTTP), with a
    /// retry affordance for retryable failures.
    private func chatErrorNotice(_ error: ChatViewModel.ChatError) -> some View {
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
                Button(action: model.retry) {
                    Label("Retry", systemImage: "arrow.clockwise")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .disabled(model.isStreaming)
            } else if !hasKey {
                Button("Open Settings", action: onOpenSettings)
                    .buttonStyle(.bordered)
                    .controlSize(.small)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 12))
    }

    private func codexErrorNotice(_ error: VoiceController.RequestError) -> some View {
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
                .disabled(model.isStreaming || voice.state.isBusy)

            // Stop while streaming, send otherwise.
            if model.isStreaming {
                Button(action: model.stop) {
                    Image(systemName: "stop.circle.fill")
                        .font(.title)
                        .frame(width: 34, height: 34)
                        .foregroundStyle(.red)
                }
                .accessibilityLabel("Stop generating")
            } else {
                Button(action: sendTyped) {
                    Image(systemName: "arrow.up.circle.fill")
                        .font(.title)
                        .frame(width: 34, height: 34)
                        .foregroundStyle(canSend ? voice.state.accentColor : Color.secondary)
                }
                .disabled(!canSend)
                .accessibilityLabel("Send message")
            }
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
        hasKey && !model.isStreaming && !voice.state.isBusy
            && !typedMessage.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var canSendCodexTask: Bool {
        hasKey && !model.isStreaming && !voice.state.isBusy
            && !typedMessage.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && codexPylonRef.trimmingCharacters(in: .whitespacesAndNewlines).count >= 3
    }

    private func sendTyped() {
        let text = typedMessage.trimmingCharacters(in: .whitespacesAndNewlines)
        guard canSend else { return }
        composerFocused = false
        typedMessage = ""
        model.send(text)
    }

    private func sendCodexTask() {
        let text = typedMessage.trimmingCharacters(in: .whitespacesAndNewlines)
        guard canSendCodexTask else { return }
        composerFocused = false
        typedMessage = ""
        store.appendMessage(.user, content: text, to: conversation)
        voice.sendCodexTask(text, pylonRef: codexPylonRef)
    }

    private func scroll(_ proxy: ScrollViewProxy) {
        withAnimation(.easeOut(duration: 0.2)) {
            proxy.scrollTo("bottom", anchor: .bottom)
        }
    }
}
