import Foundation

// MARK: - Thread

public struct ACPThread: Equatable, Codable {
    public var id: String
    public var title: String?
    public var created_at: Int64?
    public var updated_at: Int64?
    public var events: [ACPEvent]

    public init(id: String, title: String? = nil, created_at: Int64? = nil, updated_at: Int64? = nil, events: [ACPEvent]) {
        self.id = id
        self.title = title
        self.created_at = created_at
        self.updated_at = updated_at
        self.events = events
    }
}

