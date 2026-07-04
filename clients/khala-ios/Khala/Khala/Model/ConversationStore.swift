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

    /// True when the persistent on-disk store could not be opened and the app is
    /// running on an in-memory fallback (history will not survive relaunch). The
    /// UI can surface a gentle notice instead of silently losing persistence.
    @Published private(set) var isUsingEphemeralFallback = false

    init(inMemory: Bool = false) {
        let schema = Schema([Conversation.self, Message.self])
        let config = ModelConfiguration(
            schema: schema,
            isStoredInMemoryOnly: inMemory
        )
        do {
            container = try ModelContainer(for: schema, configurations: [config])
        } catch {
            // A corrupt or migration-broken on-disk store must never
            // black-screen the app. First try to RECOVER persistence by moving
            // the corrupt store aside and opening a fresh on-disk store (so a
            // one-time corruption doesn't permanently disable history); only if
            // that also fails do we fall back to a non-persistent in-memory
            // store so the UI still launches.
            if !inMemory, let recovered = Self.recoverByResettingStore(schema: schema) {
                container = recovered
            } else {
                let fallback = ModelConfiguration(schema: schema, isStoredInMemoryOnly: true)
                container = (try? ModelContainer(for: schema, configurations: [fallback]))
                    ?? {
                        // Last-resort empty container; SwiftData guarantees this
                        // path succeeds for an in-memory store.
                        // swiftlint:disable:next force_try
                        return try! ModelContainer(for: schema, configurations: [fallback])
                    }()
                isUsingEphemeralFallback = true
            }
        }
        context = ModelContext(container)
        refresh()
    }

    /// Move the default SwiftData store files aside and open a fresh on-disk
    /// store. Returns the new container, or `nil` if recovery is not possible
    /// (caller then falls back to in-memory). Best-effort and non-throwing.
    private static func recoverByResettingStore(schema: Schema) -> ModelContainer? {
        let fileManager = FileManager.default
        guard let appSupport = try? fileManager.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        ) else { return nil }

        // SwiftData's default store is `default.store` (+ `-wal` / `-shm`).
        let stamp = Int(Date().timeIntervalSince1970)
        for suffix in ["", "-wal", "-shm"] {
            let url = appSupport.appendingPathComponent("default.store\(suffix)")
            guard fileManager.fileExists(atPath: url.path) else { continue }
            let quarantined = appSupport.appendingPathComponent(
                "default.corrupt-\(stamp).store\(suffix)"
            )
            // Prefer moving (keeps the corrupt data for diagnostics); if the move
            // fails, delete so the fresh store can be created.
            if (try? fileManager.moveItem(at: url, to: quarantined)) == nil {
                try? fileManager.removeItem(at: url)
            }
        }

        let fresh = ModelConfiguration(schema: schema, isStoredInMemoryOnly: false)
        return try? ModelContainer(for: schema, configurations: [fresh])
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

    func bindSyncThreadId(_ syncThreadId: String, to conversation: Conversation) {
        guard conversation.syncThreadId != syncThreadId else { return }
        conversation.syncThreadId = syncThreadId
        save()
        refresh()
    }

    @discardableResult
    func mergeSyncedThread(_ thread: KhalaClient.SyncedChatThread) -> Conversation {
        if let existing = conversations.first(where: { $0.syncThreadId == thread.threadId }) {
            existing.title = thread.title.isEmpty ? Conversation.defaultTitle : thread.title
            existing.updatedAt = Self.date(from: thread.updatedAt) ?? existing.updatedAt
            save()
            refresh()
            return existing
        }

        let conversation = Conversation(
            title: thread.title.isEmpty ? Conversation.defaultTitle : thread.title,
            syncThreadId: thread.threadId,
            createdAt: Self.date(from: thread.createdAt) ?? Date(),
            updatedAt: Self.date(from: thread.updatedAt) ?? Date()
        )
        context.insert(conversation)
        save()
        refresh()
        return conversation
    }

    @discardableResult
    func mergeSyncedThreads(_ threads: [KhalaClient.SyncedChatThread]) -> Int {
        var changed = 0
        for thread in threads {
            _ = mergeSyncedThread(thread)
            changed += 1
        }
        return changed
    }

    /// Append a message to a conversation and bump `updatedAt`. If the
    /// conversation is still untitled and this is the first user message, derive
    /// a title from it. Returns the inserted `Message` so streaming callers can
    /// mutate its `content` live (token-by-token) before the final `persist()`.
    @discardableResult
    func appendMessage(
        _ role: MessageRole,
        content: String,
        to conversation: Conversation
    ) -> Message {
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
        return message
    }

    /// Delete a single message from a conversation (e.g. an empty/failed
    /// streamed assistant turn, or the assistant turn being regenerated).
    func deleteMessage(_ message: Message, from conversation: Conversation) {
        conversation.messages.removeAll { $0.id == message.id }
        context.delete(message)
        conversation.updatedAt = Date()
        save()
        refresh()
    }

    /// Persist in-place edits to already-inserted models (e.g. an assistant
    /// `Message` whose `content` was grown live during streaming) and re-sort
    /// Recents. Cheaper than re-inserting; used at stream settle.
    func persist() {
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

    private static func date(from iso: String) -> Date? {
        let fractional = ISO8601DateFormatter()
        fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = fractional.date(from: iso) { return date }
        return ISO8601DateFormatter().date(from: iso)
    }
}
