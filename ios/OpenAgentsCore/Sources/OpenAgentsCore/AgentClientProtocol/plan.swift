import Foundation

/*!
 Agent planning mirroring ACP Rust SDK `plan.rs` (simplified stub; extend as needed).
*/

public struct ACPPlanWire: Codable {
    public var steps: [Step]
    public var _meta: [String: AnyEncodable]? = nil
    public init(steps: [Step], _meta: [String: AnyEncodable]? = nil) {
        self.steps = steps; self._meta = _meta
    }

    public struct Step: Codable {
        public var title: String
        public var status: String? // e.g., pending/in_progress/completed
        public init(title: String, status: String? = nil) { self.title = title; self.status = status }
    }
}

