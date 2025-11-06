import Foundation

// MARK: - Message

public struct ACPMessage: Equatable, Codable {
    public var id: String
    public var thread_id: String?
    public var role: ACPRole
    public var parts: [ACPContentPart]
    public var ts: Int64
    /// Indicates if this message is internal thinking/reasoning (vs. user-facing response)
    public var isThinking: Bool?

    public init(id: String, thread_id: String? = nil, role: ACPRole, parts: [ACPContentPart], ts: Int64, isThinking: Bool? = nil) {
        self.id = id
        self.thread_id = thread_id
        self.role = role
        self.parts = parts
        self.ts = ts
        self.isThinking = isThinking
    }
}

