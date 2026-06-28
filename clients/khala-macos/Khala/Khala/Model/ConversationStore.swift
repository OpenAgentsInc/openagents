import Foundation

@MainActor
final class ConversationStore: ObservableObject {
    @Published private(set) var conversations: [Conversation] = []
    @Published private(set) var isUsingEphemeralFallback = false

    private let encoder: JSONEncoder
    private let decoder: JSONDecoder
    private let fileURL: URL?

    init() {
        encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601

        let fileManager = FileManager.default
        if let appSupport = try? fileManager.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        ) {
            let directory = appSupport.appendingPathComponent("com.openagents.khala-macos", isDirectory: true)
            try? fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
            fileURL = directory.appendingPathComponent("conversations.json")
        } else {
            fileURL = nil
            isUsingEphemeralFallback = true
        }

        load()
    }

    var mostRecent: Conversation? {
        conversations.sorted { $0.updatedAt > $1.updatedAt }.first
    }

    @discardableResult
    func createConversation() -> Conversation {
        let conversation = Conversation()
        conversations.insert(conversation, at: 0)
        persist()
        return conversation
    }

    func delete(_ conversation: Conversation) {
        conversations.removeAll { $0.id == conversation.id }
        persist()
    }

    func rename(_ conversation: Conversation, to title: String) {
        update(conversation.id) { item in
            let trimmed = title.trimmingCharacters(in: .whitespacesAndNewlines)
            item.title = trimmed.isEmpty ? Conversation.defaultTitle : trimmed
            item.updatedAt = Date()
        }
    }

    @discardableResult
    func appendMessage(_ role: MessageRole, content: String, to conversation: Conversation) -> Message {
        let message = Message(role: role, content: content)
        update(conversation.id) { item in
            item.messages.append(message)
            item.updatedAt = Date()
            if item.title == Conversation.defaultTitle,
               role == .user,
               item.messages.filter({ $0.role == .user }).count == 1 {
                item.title = Conversation.derivedTitle(from: content)
            }
        }
        return message
    }

    func updateMessage(_ messageID: UUID, in conversationID: UUID, content: String) {
        update(conversationID) { item in
            guard let index = item.messages.firstIndex(where: { $0.id == messageID }) else { return }
            item.messages[index].content = content
            item.updatedAt = Date()
        }
    }

    func deleteMessage(_ messageID: UUID, from conversationID: UUID) {
        update(conversationID) { item in
            item.messages.removeAll { $0.id == messageID }
            item.updatedAt = Date()
        }
    }

    func conversation(id: UUID) -> Conversation? {
        conversations.first { $0.id == id }
    }

    private func update(_ id: UUID, mutate: (inout Conversation) -> Void) {
        guard let index = conversations.firstIndex(where: { $0.id == id }) else { return }
        mutate(&conversations[index])
        conversations.sort { $0.updatedAt > $1.updatedAt }
        persist()
    }

    private func load() {
        guard let fileURL, let data = try? Data(contentsOf: fileURL), !data.isEmpty else {
            conversations = []
            return
        }
        conversations = ((try? decoder.decode([Conversation].self, from: data)) ?? [])
            .sorted { $0.updatedAt > $1.updatedAt }
    }

    private func persist() {
        guard let fileURL else {
            isUsingEphemeralFallback = true
            return
        }
        do {
            let data = try encoder.encode(conversations)
            try data.write(to: fileURL, options: [.atomic])
        } catch {
            isUsingEphemeralFallback = true
        }
    }
}
