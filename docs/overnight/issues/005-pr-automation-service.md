# Issue #005: Implement PRAutomationService

**Component**: GitHub Integration Layer
**Priority**: P0 (Critical Path)
**Estimated Effort**: 2-3 days
**Dependencies**: #004 (AgentCoordinator)
**Assignee**: TBD

---

## Overview

Create GitHub PRs from agent work using `gh` CLI: branch management, commit generation, PR creation with templated body.

**Key Change from Audit**: Use `findBinary()` strategy (not hardcoded `/opt/homebrew/bin/gh`). For demo: commit working tree, don't parse tool calls for file changes.

**Location**: `ios/OpenAgensCore/Sources/OpenAgentsCore/GitHubIntegration/PRAutomationService.swift`

**References**:
- Binary finding strategy: ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/CLIAgentProvider.swift:246

---

## Requirements

1. Create branches (`agent/session-{id}`)
2. Generate commits from ACPToolCallWire array
3. Push to remote
4. Create PR via `gh pr create`
5. Generate PR body with session context

---

## Implementation

```swift
actor PRAutomationService {
    func createBranch(baseBranch: String, sessionId: String) async throws -> String {
        let branchName = "agent/\(sessionId)"

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        process.arguments = ["checkout", "-b", branchName, baseBranch]

        try process.run()
        process.waitUntilExit()

        guard process.terminationStatus == 0 else {
            throw GitError.branchCreationFailed(process.terminationStatus)
        }

        return branchName
    }

    func commitFromToolCalls(_ sessionId: String, toolCalls: [ACPToolCallWire]) async throws {
        // Group tool calls by file
        var fileGroups: [String: [ACPToolCallWire]] = [:]
        for call in toolCalls where call.name == "edit_file" || call.name == "write_file" {
            if let path = call.arguments["path"] as? String {
                fileGroups[path, default: []].append(call)
            }
        }

        // Create commit message
        let message = """
        Agent work: \(sessionId)

        - Modified \(fileGroups.count) files
        - \(toolCalls.count) tool calls executed

        Generated with OpenAgents Overnight Orchestration
        """

        // Git add + commit
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        process.arguments = ["commit", "-am", message]

        try process.run()
        process.waitUntilExit()

        guard process.terminationStatus == 0 else {
            throw GitError.commitFailed(process.terminationStatus)
        }
    }

    func createPR(title: String, body: String, branch: String, baseBranch: String, draft: Bool) async throws -> Int {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/opt/homebrew/bin/gh")
        process.arguments = [
            "pr", "create",
            "--title", title,
            "--body", body,
            "--base", baseBranch,
            "--head", branch
        ] + (draft ? ["--draft"] : [])

        let pipe = Pipe()
        process.standardOutput = pipe

        try process.run()
        process.waitUntilExit()

        guard process.terminationStatus == 0 else {
            throw GitHubError.prCreationFailed(process.terminationStatus)
        }

        // Parse PR number from output
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let output = String(data: data, encoding: .utf8) ?? ""
        let regex = try NSRegularExpression(pattern: #"/pull/(\d+)"#)
        guard let match = regex.firstMatch(in: output, range: NSRange(output.startIndex..., in: output)),
              let range = Range(match.range(at: 1), in: output),
              let prNumber = Int(output[range]) else {
            throw GitHubError.couldNotParsePRNumber
        }

        return prNumber
    }

    func generatePRBody(task: OvernightTask, result: AgentSessionResult) async throws -> String {
        """
        ## Autonomous Agent Work

        **Task**: \(task.decision.task)

        **Agent**: \(task.decision.agent.rawValue)

        **Rationale**: \(task.decision.rationale)

        **Session**: `\(result.sessionId)`

        **Duration**: \(formatDuration(result.totalDuration))

        **Tool Calls**: \(result.toolCalls.count)

        ---

        *Generated with [OpenAgents Overnight Orchestration](https://github.com/OpenAgentsInc/openagents)*
        """
    }

    private func formatDuration(_ seconds: TimeInterval) -> String {
        let minutes = Int(seconds / 60)
        let secs = Int(seconds.truncatingRemainder(dividingBy: 60))
        return "\(minutes)m \(secs)s"
    }
}
```

---

## Testing

1. `testCreateBranch()` - Creates git branch
2. `testCommitFromToolCalls()` - Generates commit
3. `testGeneratePRBody()` - Template rendering
4. `testCreatePR()` - Requires `gh` CLI auth (skip in CI)

---

## Acceptance Criteria

- [ ] Branch creation works
- [ ] Commits generated from tool calls
- [ ] PR body template includes all context
- [ ] `gh pr create` integration works
- [ ] Tests pass (≥80% coverage)

---

## References

- Architecture: `architecture.md` - PRAutomationService
