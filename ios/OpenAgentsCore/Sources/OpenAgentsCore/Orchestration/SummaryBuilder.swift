import Foundation

struct SummaryBuilder {
    static func generate(workspaceRoot: String, operationResults: [AgentOp: any Encodable]) async -> ExploreSummary {
        let workspaceName = (workspaceRoot as NSString).lastPathComponent

        var topFiles: [String] = []
        var followups: [String] = []
        var fileFrequency: [String: Int] = [:]
        var allSessions: [(provider: String, sessions: [SessionMetadata])] = []

        // First pass: collect all sessions
        for (op, result) in operationResults {
            if case .sessionList(let params) = op.kind, let listResult = result as? SessionListResult {
                let provider = params.provider ?? "all"
                allSessions.append((provider: provider, sessions: listResult.sessions))
            }
        }

        // Second pass: aggregate session insights
        if !allSessions.isEmpty {
            let totalCount = allSessions.reduce(0) { $0 + $1.sessions.count }
            followups.append("Found \(totalCount) total sessions across \(allSessions.count) providers")

            var seenTitles = Set<String>()
            var interestingTitles: [String] = []
            for (_, sessions) in allSessions {
                for session in sessions {
                    guard let title = session.title else { continue }
                    if title == "Warmup" || title.isEmpty { continue }
                    let cleanTitle = title
                        .replacingOccurrences(of: "\n", with: " ")
                        .trimmingCharacters(in: CharacterSet.whitespacesAndNewlines)
                        .components(separatedBy: CharacterSet.whitespacesAndNewlines)
                        .filter { !$0.isEmpty }
                        .joined(separator: " ")
                    if seenTitles.contains(cleanTitle) { continue }
                    seenTitles.insert(cleanTitle)
                    let truncated = cleanTitle.count > 70 ? String(cleanTitle.prefix(70)) + "..." : cleanTitle
                    interestingTitles.append(truncated)
                    if interestingTitles.count >= 8 { break }
                }
                if interestingTitles.count >= 8 { break }
            }
            if !interestingTitles.isEmpty {
                followups.append("**Recent work:**")
                followups.append(contentsOf: interestingTitles)
            }
        }

        // Third pass: process other operation types
        for (op, result) in operationResults {
            switch op.kind {
            case .sessionList:
                break
            case .sessionSearch(let params):
                if let searchResult = result as? SessionSearchResult {
                    followups.append("Search '\(params.pattern)' found \(searchResult.matches.count) matches")
                }
            case .sessionRead(let params):
                if let readResult = result as? SessionReadResult {
                    topFiles.append(contentsOf: readResult.fileReferences.prefix(10))
                    for file in readResult.fileReferences { fileFrequency[file, default: 0] += 1 }
                    followups.append("Session \(params.sessionId): \(readResult.events.count) events, \(readResult.fileReferences.count) files referenced")
                }
            case .sessionAnalyze:
                if let analyzeResult = result as? SessionAnalyzeResult {
                    if let freq = analyzeResult.fileFrequency { for (file, count) in freq { fileFrequency[file, default: 0] += count } }
                    if let goals = analyzeResult.goalPatterns { followups.append("Common goals: \(goals.prefix(3).joined(separator: ", "))") }
                    if let avg = analyzeResult.avgConversationLength { followups.append("Average conversation: \(Int(avg)) events") }
                }
            case .readSpan:
                if let span = result as? ContentSpanResult { topFiles.append(span.path) }
            case .grep:
                if let grep = result as? GrepResult {
                    let uniqueFiles = Set(grep.matches.map { $0.path })
                    topFiles.append(contentsOf: uniqueFiles)
                    followups.append("Pattern '\(grep.pattern)' found in \(uniqueFiles.count) files")
                }
            default:
                break
            }
        }

        let sortedByFrequency = fileFrequency.sorted { $0.value > $1.value }
        let mostFrequentFiles = sortedByFrequency.prefix(10).map { $0.key }
        let finalTopFiles = mostFrequentFiles + topFiles.filter { !mostFrequentFiles.contains($0) }
        let uniqueTopFiles = Array(Set(finalTopFiles)).prefix(10)
        if !sortedByFrequency.isEmpty {
            let top3 = sortedByFrequency.prefix(3).map { "\($0.key) (\($0.value)x)" }
            followups.insert("Most modified files: \(top3.joined(separator: ", "))", at: 0)
        }

        return ExploreSummary(
            repo_name: workspaceRoot.lastPathComponent,
            languages: [:],
            entrypoints: [],
            top_files: Array(uniqueTopFiles),
            followups: Array(followups.prefix(5))
        )
    }
}

private extension String {
    var lastPathComponent: String { (self as NSString).lastPathComponent }
}

