import Foundation

enum MessageRole: String, Codable, CaseIterable, Sendable {
    case system
    case user
    case assistant
}

struct Message: Identifiable, Codable, Equatable, Sendable {
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

struct Conversation: Identifiable, Codable, Equatable, Sendable {
    var id: UUID
    var title: String
    var createdAt: Date
    var updatedAt: Date
    var messages: [Message]

    init(
        id: UUID = UUID(),
        title: String = Conversation.defaultTitle,
        createdAt: Date = Date(),
        updatedAt: Date = Date(),
        messages: [Message] = []
    ) {
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
        guard firstLine.count > maxLength else { return firstLine }
        let end = firstLine.index(firstLine.startIndex, offsetBy: maxLength)
        return String(firstLine[..<end]).trimmingCharacters(in: .whitespaces) + "..."
    }
}
