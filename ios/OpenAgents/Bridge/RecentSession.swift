import Foundation

struct RecentSession: Codable, Identifiable, Equatable {
    let session_id: String
    let last_ts: Int64
    let message_count: Int64
    let mode: String?
    var id: String { session_id }
}

