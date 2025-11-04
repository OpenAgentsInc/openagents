import Foundation

public struct ThreadSummary: Codable, Equatable, Hashable {
    public let id: String
    public let title: String?
    public let source: String // "codex" | "claude_code"
    public let created_at: Int64?
    public let updated_at: Int64
    public let last_message_ts: Int64?
    public let message_count: Int?

    public init(id: String, title: String?, source: String, created_at: Int64?, updated_at: Int64, last_message_ts: Int64?, message_count: Int?) {
        self.id = id
        self.title = title
        self.source = source
        self.created_at = created_at
        self.updated_at = updated_at
        self.last_message_ts = last_message_ts
        self.message_count = message_count
    }
}
