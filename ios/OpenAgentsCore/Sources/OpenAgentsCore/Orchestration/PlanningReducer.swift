import Foundation

/// Handles planning-related transformations and plan streaming state.
final class PlanningReducer {
    private var currentACPPlan: ACPPlan?
    private var planIndexByOpId: [String: Int] = [:]
    private let stream: ACPUpdateStreamHandler

    init(stream: @escaping ACPUpdateStreamHandler) {
        self.stream = stream
    }

    /// Convert an `ExplorePlan` to `ACPPlan`, cache indices, and stream.
    func streamPlan(_ plan: ExplorePlan) async {
        planIndexByOpId.removeAll()
        var entries: [ACPPlanEntry] = []
        for (i, op) in plan.nextOps.enumerated() {
            planIndexByOpId[op.opId.uuidString] = i
            entries.append(ACPPlanEntry(content: op.description, priority: .medium, status: .pending, _meta: nil))
        }
        let acpPlan = ACPPlan(entries: entries, _meta: nil)
        currentACPPlan = acpPlan
        await stream(.plan(acpPlan))
    }

    /// Update a specific plan entry status and re-stream the plan.
    func updateEntry(opId: String, to status: ACPPlanEntryStatus, error: String? = nil) async {
        guard var plan = currentACPPlan, let idx = planIndexByOpId[opId], idx < plan.entries.count else { return }
        var entry = plan.entries[idx]
        entry.status = status
        if let err = error {
            var meta = entry._meta ?? [:]
            meta["error"] = AnyEncodable(err)
            entry._meta = meta
        }
        plan.entries[idx] = entry
        currentACPPlan = plan
        await stream(.plan(plan))
    }

    // MARK: - Static helpers

    /// If the plan contains session ops, append a sessionAnalyze op at the end.
    static func addAnalysisIfNeeded(_ ops: [AgentOp]) -> [AgentOp] {
        let hasSessionOps = ops.contains { op in
            switch op.kind {
            case .sessionList, .sessionSearch:
                return true
            default:
                return false
            }
        }
        guard hasSessionOps else { return ops }
        var out = ops
        out.append(AgentOp(opId: UUID(), kind: .sessionAnalyze(SessionAnalyzeParams(sessionIds: [], provider: nil, metrics: nil))))
        return out
    }

    /// Parse FM text response into a sequence of AgentOps (legacy path).
    static func parseOperationsFromResponse(_ response: String) throws -> [AgentOp] {
        var ops: [AgentOp] = []
        let lines = response.split(separator: "\n").map(String.init)
        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty { continue }
            // Very simple parser: op and the rest of the line as parameter
            let parts = trimmed.split(separator: " ", maxSplits: 1).map(String.init)
            let opName = parts.first?.lowercased() ?? ""
            let rest = parts.count > 1 ? parts[1] : ""
            switch opName {
            case let n where n.contains("sessionlist"):
                ops.append(AgentOp(opId: UUID(), kind: .sessionList(.init(provider: nil))))
            case let n where n.contains("sessionsearch"):
                let pat = rest.isEmpty ? "TODO" : rest
                ops.append(AgentOp(opId: UUID(), kind: .sessionSearch(.init(pattern: pat, provider: nil))))
            case let n where n.contains("sessionread"):
                let sid = rest.isEmpty ? "latest" : rest
                // Default provider if not inferred from FM output
                ops.append(AgentOp(opId: UUID(), kind: .sessionRead(.init(sessionId: sid, provider: "claude-code"))))
            case let n where n.contains("listdir"):
                let path = rest.isEmpty ? "." : rest
                ops.append(AgentOp(opId: UUID(), kind: .listDir(.init(path: path))))
            case let n where n.contains("readspan"):
                let path = rest.isEmpty ? "." : rest
                ops.append(AgentOp(opId: UUID(), kind: .readSpan(.init(path: path, startLine: 0, endLine: 512))))
            case let n where n.contains("grep"):
                let pat = rest.isEmpty ? "TODO" : rest
                ops.append(AgentOp(opId: UUID(), kind: .grep(.init(pattern: pat, pathPrefix: nil, caseInsensitive: false, maxResults: 200))))
            case let n where n.contains("sessionanalyze"):
                ops.append(AgentOp(opId: UUID(), kind: .sessionAnalyze(SessionAnalyzeParams(sessionIds: [], provider: nil, metrics: nil))))
            default:
                continue
            }
        }
        if ops.isEmpty {
            throw OrchestrationError.executionFailed("Unable to parse operations from FM response")
        }
        return ops
    }
}

#if canImport(FoundationModels)
import FoundationModels
extension PlanningReducer {
    /// Rough token estimator for guarded prompts.
    @available(iOS 26.0, macOS 26.0, *)
    static func estimateTokenCount(instructions: Instructions, prompt: String) -> Int {
        // Very rough heuristic: 1 token ≈ 4 chars; add 20% headroom
        let instrChars = String(describing: instructions).count
        let totalChars = instrChars + prompt.count
        return Int(Double(totalChars) / 4.0 * 1.2)
    }
}
#endif
