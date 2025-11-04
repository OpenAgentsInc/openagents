import Foundation

/*!
 Execution plans mirroring ACP Rust SDK `plan.rs`.
*/

public struct ACPPlan: Codable {
    public var entries: [ACPPlanEntry]
    public var _meta: [String: AnyEncodable]?
    public init(entries: [ACPPlanEntry], _meta: [String: AnyEncodable]? = nil) { self.entries = entries; self._meta = _meta }
}

public struct ACPPlanEntry: Codable {
    public var content: String
    public var priority: ACPPlanEntryPriority
    public var status: ACPPlanEntryStatus
    public var _meta: [String: AnyEncodable]?
    public init(content: String, priority: ACPPlanEntryPriority, status: ACPPlanEntryStatus, _meta: [String: AnyEncodable]? = nil) {
        self.content = content; self.priority = priority; self.status = status; self._meta = _meta
    }
}

public enum ACPPlanEntryPriority: String, Codable { case high = "high", medium = "medium", low = "low" }
public enum ACPPlanEntryStatus: String, Codable { case pending = "pending", in_progress = "in_progress", completed = "completed" }
