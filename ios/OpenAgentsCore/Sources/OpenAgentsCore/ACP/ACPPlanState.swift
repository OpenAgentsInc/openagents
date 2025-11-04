import Foundation

// MARK: - Plan state updates

public enum ACPPlanStatus: String, Codable, CaseIterable, Equatable {
    case idle
    case running
    case completed
    case failed
}

public struct ACPPlanState: Equatable, Codable {
    public let type: String = "plan_state"
    public var status: ACPPlanStatus
    public var summary: String?
    public var steps: [String]?
    public var ts: Int64?

    public init(status: ACPPlanStatus, summary: String? = nil, steps: [String]? = nil, ts: Int64? = nil) {
        self.status = status
        self.summary = summary
        self.steps = steps
        self.ts = ts
    }
}

