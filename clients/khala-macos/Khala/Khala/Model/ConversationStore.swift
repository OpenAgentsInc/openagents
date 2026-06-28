import Foundation

@MainActor
final class ConversationStore: ObservableObject {
    @Published private(set) var conversations: [Conversation]
    @Published private(set) var selectedConversationID: Conversation.ID?
    @Published private(set) var persistenceError: String?

    private let storeURL: URL
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()

    init(storeURL: URL? = nil) {
        self.storeURL = storeURL ?? Self.defaultStoreURL()
        self.conversations = []
        encoder.dateEncodingStrategy = .iso8601
        decoder.dateDecodingStrategy = .iso8601
        load()
        if conversations.isEmpty { _ = createConversation() } else { selectedConversationID = conversations.first?.id }
    }

    var selectedConversation: Conversation? {
        guard let selectedConversationID else { return conversations.first }
        return conversations.first { $0.id == selectedConversationID } ?? conversations.first
    }

    @discardableResult
    func createConversation() -> Conversation {
        let conversation = Conversation()
        conversations.insert(conversation, at: 0)
        selectedConversationID = conversation.id
        persist()
        return conversation
    }

    func select(_ conversation: Conversation) { selectedConversationID = conversation.id }

    func delete(_ conversation: Conversation) {
        conversations.removeAll { $0.id == conversation.id }
        if conversations.isEmpty { _ = createConversation() } else if selectedConversationID == conversation.id { selectedConversationID = conversations.first?.id }
        persist()
    }

    @discardableResult
    func appendMessage(_ role: MessageRole, content: String, to conversationID: Conversation.ID) -> ChatMessage? {
        guard let index = conversations.firstIndex(where: { $0.id == conversationID }) else { return nil }
        let message = ChatMessage(role: role, content: content)
        conversations[index].messages.append(message)
        conversations[index].updatedAt = Date()
        if conversations[index].title == Conversation.defaultTitle, role == .user { conversations[index].title = Conversation.derivedTitle(from: content) }
        resort()
        selectedConversationID = conversationID
        persist()
        return message
    }

    func updateMessage(_ messageID: ChatMessage.ID, in conversationID: Conversation.ID, content: String) {
        guard let conversationIndex = conversations.firstIndex(where: { $0.id == conversationID }), let messageIndex = conversations[conversationIndex].messages.firstIndex(where: { $0.id == messageID }) else { return }
        conversations[conversationIndex].messages[messageIndex].content = content
        conversations[conversationIndex].updatedAt = Date()
        resort()
        selectedConversationID = conversationID
        persist()
    }

    private func resort() { conversations.sort { $0.updatedAt > $1.updatedAt } }

    private func load() {
        do {
            let data = try Data(contentsOf: storeURL)
            conversations = try decoder.decode([Conversation].self, from: data)
            resort()
            persistenceError = nil
        } catch CocoaError.fileReadNoSuchFile {
            conversations = []
        } catch {
            conversations = []
            persistenceError = "Could not read local chat history. A fresh session was opened."
        }
    }

    private func persist() {
        do {
            try FileManager.default.createDirectory(at: storeURL.deletingLastPathComponent(), withIntermediateDirectories: true)
            let data = try encoder.encode(conversations)
            try data.write(to: storeURL, options: [.atomic])
            persistenceError = nil
        } catch {
            persistenceError = "Could not save local chat history."
        }
    }

    private static func defaultStoreURL() -> URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? FileManager.default.temporaryDirectory
        return base.appendingPathComponent("OpenAgents", isDirectory: true).appendingPathComponent("KhalaDesktop", isDirectory: true).appendingPathComponent("conversations.json")
    }
}
