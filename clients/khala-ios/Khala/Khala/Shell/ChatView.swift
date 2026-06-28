import SwiftUI

/// Dead-simple chat surface for one conversation: a scrolling transcript of
/// markdown message bubbles + a text composer. Live streaming is owned by
/// `ChatViewModel`. No voice, no delegation panel, no model picker — just chat.
struct ChatView: View {
    @ObservedObject var store: ConversationStore
    @ObservedObject var model: ChatViewModel
    let conversation: Conversation
    @Binding var hasKey: Bool
    let onOpenSettings: () -> Void

    @State private var typedMessage = ""
    @FocusState private var composerFocused: Bool

    private var messages: [Message] {
        conversation.sortedMessages.filter { $0.role != .system }
    }

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()

            ScrollViewReader { proxy in
                ScrollView {
                    VStack(spacing: 18) {
                        if messages.isEmpty && model.error == nil {
                            emptyState
                        } else {
                            ForEach(messages) { message in
                                MessageBubble(
                                    title: message.role == .user ? "You" : model.channel.speaker,
                                    text: message.content,
                                    outgoing: message.role == .user,
                                    isStreaming: message.id == model.streamingMessageID
                                )
                                .id(message.id)
                            }
                            if let error = model.error {
                                chatErrorNotice(error)
                            }
                        }
                        Color.clear.frame(height: 1).id("bottom")
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.horizontal)
                    .padding(.vertical, 18)
                }
                .scrollDismissesKeyboard(.interactively)
                .onChange(of: model.streamTick) { _, _ in scroll(proxy) }
                .onChange(of: conversation.messages.count) { _, _ in scroll(proxy) }
                .onChange(of: model.error) { _, _ in scroll(proxy) }
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
            // New/empty chat: autofocus the composer so the keyboard opens
            // immediately. A short delay lets the view settle so focus reliably
            // takes and the keyboard animates up.
            guard messages.isEmpty else { return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) {
                composerFocused = true
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 8) {
            Text(model.channel.speaker)
                .font(.largeTitle.weight(.semibold))
                .foregroundStyle(.primary)
            Text(emptyStateSubtitle)
                .font(.callout)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            if model.channel == .artanis {
                Label("Owner-only operator channel", systemImage: "lock.shield")
                    .font(.caption.weight(.medium))
                    .foregroundStyle(.secondary)
                    .padding(.top, 4)
            }
            if !hasKey {
                Button {
                    onOpenSettings()
                } label: {
                    Label("Add API key", systemImage: "key")
                }
                .buttonStyle(.borderedProminent)
                .padding(.top, 8)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.top, 140)
        .padding(.horizontal, 24)
    }

    private var emptyStateSubtitle: String {
        switch model.channel {
        case .khala:
            return "Collective intelligence behind a free API. Ask anything."
        case .artanis:
            return "You are talking to the operator agent that runs the loop — not the public Khala collective. Ask what it's working on."
        }
    }

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

    private var composer: some View {
        HStack(spacing: 8) {
            TextField("Ask \(model.channel.speaker)", text: $typedMessage, axis: .vertical)
                .lineLimit(1...5)
                .textFieldStyle(.plain)
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
                .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 12))
                .focused($composerFocused)
                .submitLabel(.send)
                .onSubmit(sendTyped)
                .disabled(model.isStreaming)

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
                        .foregroundStyle(canSend ? Color.accentColor : Color.secondary)
                }
                .disabled(!canSend)
                .accessibilityLabel("Send message")
            }
        }
        .padding(.horizontal, 4)
    }

    private var canSend: Bool {
        hasKey && !model.isStreaming
            && !typedMessage.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private func sendTyped() {
        let text = typedMessage.trimmingCharacters(in: .whitespacesAndNewlines)
        guard canSend else { return }
        composerFocused = false
        typedMessage = ""
        model.send(text)
    }

    private func scroll(_ proxy: ScrollViewProxy) {
        withAnimation(.easeOut(duration: 0.2)) {
            proxy.scrollTo("bottom", anchor: .bottom)
        }
    }
}
