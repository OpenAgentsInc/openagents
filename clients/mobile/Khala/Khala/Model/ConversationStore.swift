import Foundation
import SwiftData

/// Local persistence façade over SwiftData for `Conversation`/`Message`.
///
/// v1 stores chat history on-device only (no server sync). The SwiftData store
/// file lives in Application Support. The store is `@MainActor`/`ObservableObject`
/// so views observe `conversations` for the drawer Recents list, and the feature
/// lanes (#6344 drawer/history, #6345 chat view) drive new/rename/delete/append
/// through these methods rather than touching the `ModelContext` directly.
@MainActor
final class ConversationStore: ObservableObject {
    let container: ModelContainer
    private let context: ModelContext

    /// All conversations, sorted by `updatedAt` descending (most recent first).
    @Published private(set) var conversations: [Conversation] = []

    init(inMemory: Bool = false) {
        let schema = Schema([Conversation.self, Message.self])
        let config = ModelConfiguration(
            schema: schema,
            isStoredInMemoryOnly: inMemory
        )
        do {
            container = try ModelContainer(for: schema, configurations: [config])
        } catch {
            // A corrupt or migration-broken store should never black-screen the
            // app. Fall back to an in-memory container so the UI still launches.
            let fallback = ModelConfiguration(schema: schema, isStoredInMemoryOnly: true)
            container = (try? ModelContainer(for: schema, configurations: [fallback]))
                ?? {
                    // Last-resort empty container; SwiftData guarantees this path
                    // succeeds for an in-memory store.
                    // swiftlint:disable:next force_try
                    return try! ModelContainer(for: schema, configurations: [fallback])
                }()
        }
        context = ModelContext(container)
        refresh()
    }

    // MARK: - Reads

    /// Reload `conversations` from the store, sorted by `updatedAt` desc.
    func refresh() {
        let descriptor = FetchDescriptor<Conversation>(
            sortBy: [SortDescriptor(\.updatedAt, order: .reverse)]
        )
        conversations = (try? context.fetch(descriptor)) ?? []
    }

    var mostRecent: Conversation? { conversations.first }

    // MARK: - Mutations

    /// Create a new, empty conversation, persist it, and return it.
    @discardableResult
    func createConversation() -> Conversation {
        let convo = Conversation()
        context.insert(convo)
        save()
        refresh()
        return convo
    }

    /// Append a message to a conversation and bump `updatedAt`. If the
    /// conversation is still untitled and this is the first user message, derive
    /// a title from it.
    func appendMessage(
        _ role: MessageRole,
        content: String,
        to conversation: Conversation
    ) {
        let message = Message(role: role, content: content)
        message.conversation = conversation
        conversation.messages.append(message)
        conversation.updatedAt = Date()

        if conversation.title == Conversation.defaultTitle,
           role == .user,
           conversation.messages.filter({ $0.role == .user }).count == 1 {
            conversation.title = Conversation.derivedTitle(from: content)
        }

        save()
        refresh()
    }

    /// Rename a conversation. Empty input falls back to the default title.
    func rename(_ conversation: Conversation, to title: String) {
        let trimmed = title.trimmingCharacters(in: .whitespacesAndNewlines)
        conversation.title = trimmed.isEmpty ? Conversation.defaultTitle : trimmed
        conversation.updatedAt = Date()
        save()
        refresh()
    }

    /// Delete a conversation and its transcript (cascade).
    func delete(_ conversation: Conversation) {
        context.delete(conversation)
        save()
        refresh()
    }

    /// Touch `updatedAt` (e.g. after editing in place) and re-sort Recents.
    func touch(_ conversation: Conversation) {
        conversation.updatedAt = Date()
        save()
        refresh()
    }

    private func save() {
        do {
            try context.save()
        } catch {
            // Persistence failures must not crash the chat surface; the in-memory
            // graph stays usable for the current session.
            #if DEBUG
            print("ConversationStore save failed: \(error)")
            #endif
        }
    }
}
