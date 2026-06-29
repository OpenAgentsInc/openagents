import Foundation

enum MessageRole: String, Codable, CaseIterable, Identifiable, Sendable {
    case user
    case assistant
    var id: String { rawValue }
}

struct ChatMessage: Codable, Equatable, Identifiable, Sendable {
    var id: UUID
    var role: MessageRole
    var content: String
    var createdAt: Date

    init(id: UUID = UUID(), role: MessageRole, content: String, createdAt: Date = Date()) {
        self.id = id
        self.role = role
        self.content = content
        self.createdAt = createdAt
    }
}

struct Conversation: Codable, Equatable, Identifiable, Sendable {
    var id: UUID
    var title: String
    var createdAt: Date
    var updatedAt: Date
    var messages: [ChatMessage]

    init(id: UUID = UUID(), title: String = Conversation.defaultTitle, createdAt: Date = Date(), updatedAt: Date = Date(), messages: [ChatMessage] = []) {
        self.id = id
        self.title = title
        self.createdAt = createdAt
        self.updatedAt = updatedAt
        self.messages = messages
    }

    static let defaultTitle = "New Chat"

    static func derivedTitle(from text: String, maxLength: Int = 48) -> String {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return defaultTitle }
        let firstLine = trimmed.split(whereSeparator: \.isNewline).first.map(String.init) ?? trimmed
        if firstLine.count <= maxLength { return firstLine }
        let end = firstLine.index(firstLine.startIndex, offsetBy: maxLength)
        return String(firstLine[..<end]).trimmingCharacters(in: .whitespaces) + "..."
    }
}
