import Foundation

#if canImport(FoundationModels)
import FoundationModels
#endif

struct FMAnalysis {
    struct Result {
        enum Source: String { case sessionAnalyze, fm }
        let text: String
        let source: Source
        let topFiles: [String]
        let goalPatterns: [String]
        let avgConversationLength: Double?
    }

    @available(iOS 26.0, macOS 26.0, *)
    static func compute(workspaceRoot: String, operationResults: [AgentOp: any Encodable]) async -> Result? {
        #if canImport(FoundationModels)
        var analyze: SessionAnalyzeResult?
        for (op, result) in operationResults { if case .sessionAnalyze = op.kind, let r = result as? SessionAnalyzeResult { analyze = r } }
        guard let analyze = analyze else { return nil }
        if let intent = analyze.userIntent?.trimmingCharacters(in: .whitespacesAndNewlines), !intent.isEmpty {
            let lines = intent.replacingOccurrences(of: "\r", with: "\n").components(separatedBy: "\n").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
            if let first = lines.first, first.hasSuffix(":") {
                let label = first.dropLast().trimmingCharacters(in: .whitespaces)
                let items = lines.dropFirst().map { raw -> String in
                    let stripped = raw.replacingOccurrences(of: "^[\\s]*[-*•]+\\s*", with: "", options: .regularExpression)
                    let rel = PathUtils.normalizeToWorkspaceRelative(workspaceRoot: workspaceRoot, inputPath: stripped)
                    if rel != "." && rel != stripped { return rel }
                    return stripped
                }
                let nonEmpty = items.filter { !$0.isEmpty }
                if nonEmpty.isEmpty { return Result(text: String(label), source: .sessionAnalyze, topFiles: [], goalPatterns: [], avgConversationLength: nil) }
                let maxItems = Array(nonEmpty.prefix(4))
                let joined: String = {
                    if maxItems.count == 1 { return maxItems[0] }
                    if maxItems.count == 2 { return "\(maxItems[0]) and \(maxItems[1])" }
                    let head = maxItems.dropLast().joined(separator: ", ")
                    if let last = maxItems.last { return "\(head), and \(last)" }
                    return head
                }()
                let sentence = "User intends to \(label.lowercased()) \(joined)."
                let topFilesFromAnalyze: [String] = {
                    let pairs = (analyze.fileFrequency ?? [:]).sorted { $0.value > $1.value }
                    return pairs.prefix(5).map { k, _ in PathUtils.normalizeToWorkspaceRelative(workspaceRoot: workspaceRoot, inputPath: k) }
                }()
                let goals: [String] = (analyze.goalPatterns ?? []).prefix(3).map { $0 }
                let avg = analyze.avgConversationLength
                return Result(text: sentence, source: .sessionAnalyze, topFiles: topFilesFromAnalyze, goalPatterns: goals, avgConversationLength: avg)
            }
            let sentence = lines.joined(separator: " ")
            let topFilesFromAnalyze: [String] = {
                let pairs = (analyze.fileFrequency ?? [:]).sorted { $0.value > $1.value }
                return pairs.prefix(5).map { k, _ in PathUtils.normalizeToWorkspaceRelative(workspaceRoot: workspaceRoot, inputPath: k) }
            }()
            let goals: [String] = (analyze.goalPatterns ?? []).prefix(3).map { $0 }
            let avg = analyze.avgConversationLength
            return Result(text: sentence, source: .sessionAnalyze, topFiles: topFilesFromAnalyze, goalPatterns: goals, avgConversationLength: avg)
        }
        return nil
        #else
        return nil
        #endif
    }
}
