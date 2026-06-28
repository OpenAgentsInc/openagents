import SwiftUI

@MainActor
final class ChatViewModel: ObservableObject {
    struct ChatError: Equatable {
        let title: String
        let message: String
        let isRetryable: Bool
    }

    @Published private(set) var streamingMessageID: UUID?
    @Published private(set) var isStreaming = false
    @Published private(set) var error: ChatError?
    @Published private(set) var streamTick = 0

    private let store: ConversationStore
    private let conversationID: UUID
    private var task: Task<Void, Never>?

    init(store: ConversationStore, conversationID: UUID) {
        self.store = store
        self.conversationID = conversationID
    }

    func send(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, !isStreaming else { return }
        guard let key = KeychainStore.loadAPIKey() else {
            error = ChatError(
                title: KhalaClient.KhalaError.missingKey.recoveryTitle,
                message: KhalaClient.KhalaError.missingKey.recoveryMessage,
                isRetryable: false
            )
            return
        }

        error = nil
        guard let conversation = store.conversation(id: conversationID) else { return }
        store.appendMessage(.user, content: trimmed, to: conversation)
        startStream(apiKey: key)
    }

    func retry() {
        guard !isStreaming, error?.isRetryable == true else { return }
        guard let key = KeychainStore.loadAPIKey() else { return }
        error = nil
        startStream(apiKey: key)
    }

    func stop() {
        task?.cancel()
    }

    private func startStream(apiKey: String) {
        guard let conversation = store.conversation(id: conversationID) else { return }
        let history = conversation.messages
            .sorted { $0.createdAt < $1.createdAt }
            .filter { $0.role != .system && !$0.content.isEmpty }
            .map { KhalaClient.OutgoingMessage(role: $0.role.rawValue, content: $0.content) }
        guard !history.isEmpty else { return }

        let assistant = store.appendMessage(.assistant, content: "", to: conversation)
        streamingMessageID = assistant.id
        isStreaming = true
        streamTick &+= 1

        task = Task { [weak self] in
            guard let self else { return }
            var assembled = ""
            do {
                for try await delta in KhalaClient.streamCompletion(messages: history, apiKey: apiKey) {
                    assembled += delta
                    self.store.updateMessage(assistant.id, in: self.conversationID, content: assembled)
                    self.streamTick &+= 1
                }
                self.settle(assistantID: assistant.id, content: assembled)
            } catch is CancellationError {
                self.settle(assistantID: assistant.id, content: assembled)
            } catch let khala as KhalaClient.KhalaError {
                self.fail(khala, assistantID: assistant.id, content: assembled)
            } catch {
                self.fail(.transport(error.localizedDescription), assistantID: assistant.id, content: assembled)
            }
        }
    }

    private func settle(assistantID: UUID, content: String) {
        if content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            store.deleteMessage(assistantID, from: conversationID)
        }
        streamingMessageID = nil
        isStreaming = false
        task = nil
    }

    private func fail(_ khala: KhalaClient.KhalaError, assistantID: UUID, content: String) {
        if content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            store.deleteMessage(assistantID, from: conversationID)
        }
        error = ChatError(title: khala.recoveryTitle, message: khala.recoveryMessage, isRetryable: khala.isRetryable)
        streamingMessageID = nil
        isStreaming = false
        task = nil
    }
}

struct ChatView: View {
    @ObservedObject var store: ConversationStore
    let conversationID: UUID?
    @Binding var hasKey: Bool
    let onOpenSettings: () -> Void

    @State private var typedMessage = ""
    @State private var model: ChatViewModel?

    var body: some View {
        VStack(spacing: 0) {
            if let conversation = conversation, let model = model {
                transcript(conversation: conversation, model: model)
                composer(model: model)
            } else {
                ContentUnavailableView("No conversation", systemImage: "bubble.left.and.bubble.right")
            }
        }
        .background(Color(nsColor: .windowBackgroundColor))
        .onAppear(perform: syncModel)
        .onChange(of: conversationID) { _, _ in syncModel() }
    }

    private var conversation: Conversation? {
        guard let conversationID else { return nil }
        return store.conversation(id: conversationID)
    }

    private func transcript(conversation: Conversation, model: ChatViewModel) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    if conversation.messages.isEmpty && model.error == nil {
                        emptyState
                    } else {
                        ForEach(conversation.messages.sorted { $0.createdAt < $1.createdAt }) { message in
                            if message.role != .system {
                                MessageBubble(
                                    title: message.role == .user ? "You" : "Khala",
                                    text: message.content,
                                    outgoing: message.role == .user,
                                    isStreaming: message.id == model.streamingMessageID
                                )
                                .id(message.id)
                            }
                        }
                    }

                    if let error = model.error {
                        chatErrorNotice(error, model: model)
                    }

                    Color.clear.frame(height: 1).id("bottom")
                }
                .frame(maxWidth: 860)
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 28)
                .padding(.vertical, 24)
            }
            .onChange(of: model.streamTick) { _, _ in scroll(proxy) }
            .onChange(of: conversation.messages.count) { _, _ in scroll(proxy) }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 10) {
            Text("Khala")
                .font(.largeTitle.weight(.semibold))
            Text("Collective intelligence behind a free API. Ask anything.")
                .font(.callout)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.top, 140)
    }

    private func chatErrorNotice(_ error: ChatViewModel.ChatError, model: ChatViewModel) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Label(error.title, systemImage: error.isRetryable ? "arrow.clockwise.circle.fill" : "exclamationmark.circle.fill")
                .font(.callout.weight(.semibold))
            Text(error.message)
                .font(.footnote)
                .foregroundStyle(.secondary)
            HStack {
                if error.isRetryable {
                    Button("Retry", action: model.retry)
                } else if !hasKey {
                    Button("Open Settings", action: onOpenSettings)
                }
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 8))
    }

    private func composer(model: ChatViewModel) -> some View {
        HStack(alignment: .bottom, spacing: 10) {
            TextField("Ask Khala", text: $typedMessage, axis: .vertical)
                .lineLimit(1...5)
                .textFieldStyle(.plain)
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
                .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 8))
                .onSubmit { send(model: model) }
                .disabled(model.isStreaming)

            if model.isStreaming {
                Button(action: model.stop) {
                    Image(systemName: "stop.circle.fill")
                }
                .font(.title2)
                .accessibilityLabel("Stop generating")
            } else {
                Button(action: { send(model: model) }) {
                    Image(systemName: "arrow.up.circle.fill")
                }
                .font(.title2)
                .disabled(!canSend(model: model))
                .accessibilityLabel("Send message")
            }
        }
        .padding(16)
        .background(.bar)
    }

    private func canSend(model: ChatViewModel) -> Bool {
        hasKey && !model.isStreaming && !typedMessage.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private func send(model: ChatViewModel) {
        guard canSend(model: model) else { return }
        let text = typedMessage
        typedMessage = ""
        model.send(text)
    }

    private func syncModel() {
        guard let conversationID else {
            model = nil
            return
        }
        model = ChatViewModel(store: store, conversationID: conversationID)
    }

    private func scroll(_ proxy: ScrollViewProxy) {
        withAnimation(.easeOut(duration: 0.2)) {
            proxy.scrollTo("bottom", anchor: .bottom)
        }
    }
}
