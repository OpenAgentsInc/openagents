import Foundation
import SwiftUI

/// Owns the live, multi-turn streaming chat loop for one conversation.
///
/// This is the seam #6346 fills: it replaces the old single-shot
/// `VoiceController.complete(...)` chat path with token-by-token streaming over
/// the FULL conversation history.
///
/// Flow for a send (typed OR voice):
///   1. persist the user `Message` (via `ConversationStore`),
///   2. create an empty assistant `Message` and mark it the streaming turn,
///   3. stream `KhalaClient.streamCompletion` over the whole transcript,
///      appending each delta to the assistant message so the bubble grows live,
///   4. settle on `[DONE]` (persist), or surface an inline, retryable error.
///
/// Cancellation: a `stop()` call (or starting a new turn) cancels the in-flight
/// `Task`; whatever streamed so far is kept and settled.
///
/// Voice and text share this one path: the voice controller transcribes and
/// hands the final transcript to `send(_:)`, so a spoken turn becomes a normal
/// user turn that streams a reply into the same transcript.
@MainActor
final class ChatViewModel: ObservableObject {
    /// Which conversational channel this turn streams over.
    /// `.khala` is the public collective-intelligence model; `.artanis` is the
    /// owner-only operator channel (#6363, epic #6359) that talks to the real
    /// Artanis operator persona via `POST /api/operator/artanis/chat`. Both
    /// surfaces (this app and the Khala CLI) call the SAME endpoint, so the
    /// operator logic is shared.
    enum Channel: Equatable {
        case khala
        case artanis

        /// The name shown on the assistant's bubbles for this channel.
        var speaker: String {
            switch self {
            case .khala: return "Khala"
            case .artanis: return "Artanis"
            }
        }
    }

    struct ChatError: Equatable {
        let title: String
        let message: String
        let isRetryable: Bool
    }

    /// The assistant message currently being streamed, if any. Views compare
    /// against this to drive `MessageBubble(isStreaming:)`.
    @Published private(set) var streamingMessageID: UUID?
    /// True while a request is in flight (request sent, awaiting/receiving
    /// tokens). Drives the composer stop control and disabled state.
    @Published private(set) var isStreaming = false
    /// Inline error for the active turn (402 / network / HTTP). Cleared on the
    /// next send or retry.
    @Published private(set) var error: ChatError?
    /// Monotonic counter bumped on every streamed delta so the chat view can
    /// auto-scroll as the assistant turn grows (SwiftData content mutations do
    /// not always re-fire `onChange` on a stable identity).
    @Published private(set) var streamTick = 0

    private let store: ConversationStore
    private let conversation: Conversation
    private var task: Task<Void, Never>?

    /// The active channel for this conversation. Fixed for the lifetime of the
    /// model: switching channels recreates the model (and starts a fresh
    /// conversation) so the operator persona and the public model never share
    /// transcript context.
    let channel: Channel

    /// The last user prompt sent, kept so a transport/HTTP error can be retried
    /// without re-reading the transcript.
    private var lastUserPrompt: String?

    init(store: ConversationStore, conversation: Conversation, channel: Channel = .khala) {
        self.store = store
        self.conversation = conversation
        self.channel = channel
    }

    /// The conversation this model is bound to, so the shell can tell whether the
    /// active model matches the active conversation.
    var conversationID: UUID { conversation.id }

    /// True when this turn can be cancelled / a stop control should show.
    var canStop: Bool { isStreaming }

    // MARK: - Send

    /// Send a user turn and stream the reply. `alreadyPersisted` is set when the
    /// caller (e.g. the composer or empty-state suggestion) has already written
    /// the user `Message`, so we do not duplicate it.
    func send(_ text: String, alreadyPersisted: Bool = false) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        guard !isStreaming else { return }

        guard let key = KeychainStore.loadAPIKey() else {
            error = ChatError(
                title: KhalaClient.KhalaError.missingKey.recoveryTitle,
                message: KhalaClient.KhalaError.missingKey.recoveryMessage,
                isRetryable: false
            )
            return
        }

        error = nil
        lastUserPrompt = trimmed
        if !alreadyPersisted {
            store.appendMessage(.user, content: trimmed, to: conversation)
        }

        startStream(apiKey: key)
    }

    /// Re-stream the last user turn: drop the most recent assistant turn (if any)
    /// and stream a fresh reply over the prior history. Wired to the
    /// `MessageBubble` regenerate hook.
    func regenerate() {
        guard !isStreaming else { return }
        guard let key = KeychainStore.loadAPIKey() else {
            error = ChatError(
                title: KhalaClient.KhalaError.missingKey.recoveryTitle,
                message: KhalaClient.KhalaError.missingKey.recoveryMessage,
                isRetryable: false
            )
            return
        }
        // Require a trailing user turn to regenerate against.
        guard conversation.sortedMessages.contains(where: { $0.role == .user }) else { return }

        error = nil
        // Remove the last assistant turn so the new stream is the reply to the
        // most recent user turn.
        if let lastAssistant = conversation.sortedMessages.last(where: { $0.role == .assistant }) {
            store.deleteMessage(lastAssistant, from: conversation)
        }
        startStream(apiKey: key)
    }

    /// Retry after a retryable error (network / 5xx). The user turn is already
    /// persisted, so we re-stream the reply over the prior history. Any partial
    /// assistant turn that arrived before the failure is dropped first so the
    /// retry produces a single clean reply.
    func retry() {
        guard !isStreaming, error?.isRetryable == true else { return }
        guard let key = KeychainStore.loadAPIKey() else { return }
        error = nil
        if let trailingAssistant = conversation.sortedMessages.last,
           trailingAssistant.role == .assistant {
            store.deleteMessage(trailingAssistant, from: conversation)
        }
        startStream(apiKey: key)
    }

    /// Cancel the in-flight stream. Whatever streamed so far is kept and the
    /// assistant turn is settled.
    func stop() {
        task?.cancel()
        // `task`'s `defer`/completion settles state.
    }

    // MARK: - Streaming core

    private func startStream(apiKey: String) {
        // Build the outgoing history from the persisted transcript (multi-turn).
        let history = conversation.sortedMessages
            .filter { $0.role != .system && !$0.content.isEmpty }
            .map { KhalaClient.OutgoingMessage(role: $0.role.rawValue, content: $0.content) }
        guard !history.isEmpty else { return }

        // Create the empty assistant turn we stream into.
        let assistant = store.appendMessage(.assistant, content: "", to: conversation)
        streamingMessageID = assistant.id
        isStreaming = true
        streamTick &+= 1

        let activeChannel = channel
        task = Task { [weak self] in
            guard let self else { return }
            do {
                let stream = activeChannel == .artanis
                    ? KhalaClient.streamArtanisCompletion(messages: history, apiKey: apiKey)
                    : KhalaClient.streamCompletion(messages: history, apiKey: apiKey)
                for try await delta in stream {
                    // Append live; the SwiftData model is observable so the
                    // bubble grows token-by-token.
                    assistant.content += delta
                    self.streamTick &+= 1
                }
                self.settle(assistant)
            } catch is CancellationError {
                self.settle(assistant)
            } catch let khala as KhalaClient.KhalaError {
                self.fail(khala, assistant: assistant)
            } catch {
                self.fail(.transport(error), assistant: assistant)
            }
        }
    }

    /// Finish a turn: persist whatever streamed, clear streaming flags. An empty
    /// assistant turn (e.g. immediate cancel) is removed so it does not leave a
    /// blank bubble.
    private func settle(_ assistant: Message) {
        if assistant.content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            store.deleteMessage(assistant, from: conversation)
        } else {
            store.persist()
        }
        streamingMessageID = nil
        isStreaming = false
        task = nil
    }

    /// Surface an error for the turn and remove the (empty) assistant bubble so
    /// the error notice stands alone with a retry affordance.
    private func fail(_ khala: KhalaClient.KhalaError, assistant: Message) {
        // Keep any partial tokens that arrived before the failure; only drop a
        // wholly empty bubble.
        if assistant.content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            store.deleteMessage(assistant, from: conversation)
        } else {
            store.persist()
        }
        error = ChatError(
            title: khala.recoveryTitle,
            message: khala.recoveryMessage,
            isRetryable: khala.isRetryable
        )
        streamingMessageID = nil
        isStreaming = false
        task = nil
    }
}
