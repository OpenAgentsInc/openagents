import Foundation
import SwiftData

/// A role in a chat turn. Mirrors the OpenAI-compatible roles the Khala API
/// understands. Stored as a raw `String` so the SwiftData model stays simple
/// and forward-compatible.
enum MessageRole: String, Codable, CaseIterable, Sendable {
    case system
    case user
    case assistant
}

/// One message in a conversation transcript.
///
/// SwiftData `@Model` on iOS 17+. The store owns persistence; views read and
/// mutate through `ConversationStore`.
@Model
final class Message {
    @Attribute(.unique) var id: UUID
    /// Stored as the raw role string; use `messageRole` for the typed value.
    var roleRaw: String
    var content: String
    var createdAt: Date

    /// Back-reference to the owning conversation (set by SwiftData).
    var conversation: Conversation?

    init(
        id: UUID = UUID(),
        role: MessageRole,
        content: String,
        createdAt: Date = Date()
    ) {
        self.id = id
        self.roleRaw = role.rawValue
        self.content = content
        self.createdAt = createdAt
    }

    var role: MessageRole {
        get { MessageRole(rawValue: roleRaw) ?? .user }
        set { roleRaw = newValue.rawValue }
    }
}

/// A locally-persisted chat conversation. v1 keeps history on-device only
/// (no server sync). Recents is sorted by `updatedAt` descending.
@Model
final class Conversation {
    @Attribute(.unique) var id: UUID
    var title: String
    var createdAt: Date
    var updatedAt: Date

    /// Cascade-delete the transcript when the conversation is removed.
    @Relationship(deleteRule: .cascade, inverse: \Message.conversation)
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

    /// Messages in stable chronological order for rendering.
    var sortedMessages: [Message] {
        messages.sorted { $0.createdAt < $1.createdAt }
    }

    /// Derive a short title from the first user message (truncated). Used when
    /// the conversation is still untitled.
    static func derivedTitle(from text: String, maxLength: Int = 48) -> String {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return defaultTitle }
        let firstLine = trimmed.split(whereSeparator: \.isNewline).first.map(String.init) ?? trimmed
        if firstLine.count <= maxLength { return firstLine }
        let end = firstLine.index(firstLine.startIndex, offsetBy: maxLength)
        return String(firstLine[..<end]).trimmingCharacters(in: .whitespaces) + "…"
    }
}
