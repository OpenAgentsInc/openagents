# Issue #002: Implement DecisionOrchestrator

**Component**: Orchestration Layer - Decision Engine
**Priority**: P0 (Critical Path)
**Estimated Effort**: 2-3 days (using existing components)
**Dependencies**: None (uses existing ExploreOrchestrator + SessionTools)
**Assignee**: TBD

---

## Overview

Implement `DecisionOrchestrator`, a Foundation Models-powered decision engine that analyzes session history to decide what tasks to work on next and which agent to assign.

**Key Change from Audit**: Reuse `ExploreOrchestrator` and `SessionAnalyzeTool` instead of creating new FM wrapper. Defer `repo.status` for demo.

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/DecisionOrchestrator.swift`

**References**:
- `ExploreOrchestrator`: ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift:1
- `SessionAnalyzeTool`: ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SessionTools.swift:145

---

## Requirements

### Functional Requirements

1. **Foundation Models Integration**
   - Use existing `ExploreOrchestrator` with its `fmAnalysis()` method
   - Check availability: `SystemLanguageModel.default.availability`
   - Fallback to heuristic decision-making if FM unavailable
   - Stream responses for responsiveness

2. **Session History Analysis**
   - Use existing `SessionAnalyzeTool` from `SessionTools`
   - Extract: file frequency, tool usage, user intents, avg duration
   - Support multiple providers (Claude Code, Codex)
   - Cache results (expensive to recompute)

3. **Repository Status (Post-Demo)**
   - Defer `repo.status`, `repo.coverage`, `repo.complexity` for post-demo
   - For demo: use session insights only

4. **Task Decision Logic**
   - Input: `OrchestrationContext` (session insights + repo status + recent tasks)
   - Output: `TaskDecision` (task description + agent + priority + rationale)
   - Prompt template with clear instructions
   - JSON output parsing

5. **Agent Selection**
   - Claude Code: refactoring, documentation, complex reasoning
   - Codex: test generation, boilerplate, repetitive tasks
   - Based on task type and user patterns

6. **Confidence Scoring**
   - Return confidence level (0.0-1.0)
   - Skip low-confidence decisions (< 0.5 threshold)
   - Log reasoning for audit trail

---

## Implementation Spec

### Interface

```swift
@available(macOS 26.0, *)
actor DecisionOrchestrator {
    private let fmOrchestrator: FMOrchestrator?
    private var sessionInsightsCache: [AgentType: [SessionInsight]]?
    private var cacheTimestamp: Date?
    private let cacheExpiry: TimeInterval = 3600  // 1 hour

    init(fmOrchestrator: FMOrchestrator? = nil) {
        if let fm = fmOrchestrator {
            self.fmOrchestrator = fm
        } else {
            // Try to initialize FM if available
            if SystemLanguageModel.default.availability.status == .available {
                self.fmOrchestrator = NativeFMOrchestrator()
            } else {
                self.fmOrchestrator = nil
            }
        }
    }

    /// Decide next task based on context
    func decideNextTask(context: OrchestrationContext) async throws -> TaskDecision

    /// Analyze session history for patterns
    func analyzeSessionHistory(providers: [AgentType], topK: Int, since: TimeInterval?) async throws -> [SessionInsight]

    /// Get repository status
    func getRepoStatus(workingDir: URL) async throws -> RepoStatus

    /// Prioritize candidate tasks (if multiple decisions needed)
    func prioritizeTasks(_ candidates: [TaskCandidate]) async throws -> [TaskDecision]
}

struct OrchestrationContext {
    let sessionInsights: [SessionInsight]
    let repoStatus: RepoStatus
    let recentTasks: [OvernightTask]
    let availableAgents: [AgentType]
    let timeBudget: TimeInterval
    let userPreferences: [String: String]
}

struct TaskDecision: Codable {
    let task: String  // Clear, specific task description
    let agent: AgentType  // .claude_code or .codex
    let priority: Priority
    let estimatedDuration: TimeInterval
    let rationale: String  // FM explanation or heuristic reason
    let confidence: Double  // 0.0-1.0
    let metadata: [String: String]
}

enum Priority: String, Codable {
    case high, medium, low
}

struct SessionInsight: Codable {
    let provider: AgentType
    let fileFrequency: [String: Int]
    let toolUsage: [String: Int]
    let userIntents: [String]
    let avgSessionDuration: TimeInterval
}

struct RepoStatus: Codable {
    let branch: String
    let ahead: Int
    let behind: Int
    let modifiedFiles: [String]
    let untrackedFiles: [String]
    let recentCommits: [GitCommit]
    let testCoverage: Double?
}

struct GitCommit: Codable {
    let sha: String
    let message: String
    let author: String
    let date: Date
}
```

### Decision Prompt Template

```swift
let promptTemplate = """
You are an autonomous code quality agent deciding what to work on next.

# Context

## Recent Development Patterns
\(formatSessionInsights(context.sessionInsights))

## Repository State
Branch: \(context.repoStatus.branch)
Modified files: \(context.repoStatus.modifiedFiles.count)
Test coverage: \(context.repoStatus.testCoverage ?? 0.0)
Recent commits: \(context.repoStatus.recentCommits.map { $0.message }.joined(separator: "\n"))

## Recent Tasks (Last 24h)
\(formatRecentTasks(context.recentTasks))

## Available Agents
- **claude-code**: Best for refactoring, documentation, complex reasoning, error handling
- **codex**: Best for test generation, boilerplate code, repetitive tasks

## Time Budget
\(Int(context.timeBudget / 60)) minutes

# Task

Decide the highest-impact task to work on right now. Consider:
1. **Files touched frequently** - Need refactoring or better error handling
2. **Low test coverage areas** - Need comprehensive tests
3. **User intents from past sessions** - What they care about most
4. **Time budget** - Can we realistically finish in \(Int(context.timeBudget / 60)) minutes?
5. **Avoid duplicating recent work** - Don't repeat tasks from last 24h

# Output Format

Return valid JSON:
{
  "task": "Clear, specific, actionable task description (1-2 sentences)",
  "agent": "claude-code" | "codex",
  "priority": "high" | "medium" | "low",
  "estimated_duration": <seconds>,
  "rationale": "Why this task now? Reference specific files/patterns from context.",
  "confidence": <0.0-1.0>
}

# Examples

{
  "task": "Refactor BridgeManager error handling to use Swift Result types with proper error propagation",
  "agent": "claude-code",
  "priority": "high",
  "estimated_duration": 1800,
  "rationale": "BridgeManager.swift touched 25 times in recent sessions with user frequently requesting 'improve error handling'. Current implementation uses optional returns which hides error context.",
  "confidence": 0.85
}

{
  "task": "Generate comprehensive unit tests for DesktopWebSocketServer covering all JSON-RPC methods",
  "agent": "codex",
  "priority": "high",
  "estimated_duration": 2400,
  "rationale": "Test coverage is 65%, WebSocketServer has 0% coverage. User frequently runs tests and mentions 'need more tests'. Codex excels at test generation.",
  "confidence": 0.9
}

Now decide the next task:
"""
```

### FM Decision Logic

```swift
func decideNextTask(context: OrchestrationContext) async throws -> TaskDecision {
    // Try FM first
    if let fm = fmOrchestrator {
        do {
            let decision = try await decidewithFM(context: context, fm: fm)
            if decision.confidence >= 0.5 {
                return decision
            }
            // Low confidence, fall back to heuristic
        } catch {
            // FM failed, fall back to heuristic
            print("FM decision failed: \(error), using heuristic")
        }
    }

    // Fallback: heuristic decision
    return try await decideWithHeuristic(context: context)
}

private func decideWithFM(context: OrchestrationContext, fm: FMOrchestrator) async throws -> TaskDecision {
    let prompt = generatePrompt(context: context)

    let response = try await fm.orchestrateWithPrompt(
        prompt: prompt,
        tools: [],  // No tool calling needed for this
        outputFormat: .json
    )

    // Parse JSON response
    guard let jsonData = response.data(using: .utf8),
          let decision = try? JSONDecoder().decode(TaskDecision.self, from: jsonData) else {
        throw DecisionError.invalidFMResponse(response)
    }

    return decision
}

private func decideWithHeuristic(context: OrchestrationContext) async throws -> TaskDecision {
    // Simple heuristic: pick most frequently touched file, suggest refactoring
    let allFiles = context.sessionInsights.flatMap { $0.fileFrequency }
    guard let mostTouched = allFiles.max(by: { $0.value < $1.value }) else {
        throw DecisionError.noInsightsAvailable
    }

    return TaskDecision(
        task: "Refactor \(mostTouched.key) for improved code quality and error handling",
        agent: .claude_code,
        priority: .medium,
        estimatedDuration: 1800,
        rationale: "Heuristic: Most frequently touched file (\(mostTouched.value) times)",
        confidence: 0.6,
        metadata: ["decision_method": "heuristic"]
    )
}
```

### Session History Analysis

```swift
func analyzeSessionHistory(providers: [AgentType], topK: Int, since: TimeInterval? = nil) async throws -> [SessionInsight] {
    // Check cache
    if let cached = sessionInsightsCache,
       let timestamp = cacheTimestamp,
       Date().timeIntervalSince(timestamp) < cacheExpiry {
        return cached.filter { providers.contains($0.provider) }
    }

    // Read .jsonl files for each provider
    var insights: [SessionInsight] = []

    for provider in providers {
        let sessionDir = getSessionDirectory(for: provider)
        let sessionFiles = try FileManager.default.contentsOfDirectory(at: sessionDir, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "jsonl" }
            .sorted { $0.lastPathComponent > $1.lastPathComponent }  // Most recent first
            .prefix(topK)

        var fileFrequency: [String: Int] = [:]
        var toolUsage: [String: Int] = [:]
        var durations: [TimeInterval] = []

        for file in sessionFiles {
            let session = try parseSessionFile(file)

            // Extract file references from tool calls
            for tool in session.toolCalls {
                if let path = tool.arguments["path"] as? String {
                    fileFrequency[path, default: 0] += 1
                }
            }

            // Count tool usage
            for tool in session.toolCalls {
                toolUsage[tool.name, default: 0] += 1
            }

            // Track duration
            if let duration = session.duration {
                durations.append(duration)
            }
        }

        let avgDuration = durations.isEmpty ? 0 : durations.reduce(0, +) / Double(durations.count)

        insights.append(SessionInsight(
            provider: provider,
            fileFrequency: fileFrequency,
            toolUsage: toolUsage,
            userIntents: inferIntents(from: toolUsage),  // Heuristic
            avgSessionDuration: avgDuration
        ))
    }

    // Cache results
    sessionInsightsCache = insights
    cacheTimestamp = Date()

    return insights
}

private func inferIntents(from toolUsage: [String: Int]) -> [String] {
    var intents: [String] = []

    if toolUsage["edit_file", default: 0] > 10 {
        intents.append("refactor")
    }
    if toolUsage["run_bash", default: 0] > 5 && toolUsage["run_bash"]! > toolUsage["edit_file", default: 0] {
        intents.append("debug")
    }
    if toolUsage["write_file", default: 0] > 5 {
        intents.append("new feature")
    }
    // Add more heuristics

    return intents
}
```

---

## Testing

### Unit Tests

1. `testDecideNextTask_WithMockFM()` - Parse FM response correctly
2. `testFallbackWhenFMUnavailable()` - Heuristic works
3. `testSessionHistoryAnalysis()` - Parse .jsonl files
4. `testCaching()` - Cache expires correctly
5. `testAgentSelection()` - Claude vs Codex logic
6. `testLowConfidenceHandling()` - Skip decisions < 0.5

### Integration Tests

1. `testRealFMDecision()` - On macOS 26+ with FM available
2. `testReasonableTaskSelection()` - Manual review of decisions

---

## Acceptance Criteria

- [ ] FM integration works on macOS 26+ with Apple Intelligence
- [ ] Fallback heuristic works when FM unavailable
- [ ] Session history parsing handles both Claude Code and Codex .jsonl formats
- [ ] Caching reduces redundant file I/O
- [ ] Agent selection logic is sound (Claude for refactoring, Codex for tests)
- [ ] Confidence scoring prevents bad decisions
- [ ] All unit tests pass with ≥85% coverage
- [ ] Integration test with real FM produces reasonable decisions
- [ ] No crashes or hangs

---

## References

- Architecture: `architecture.md` - DecisionOrchestrator section
- Testing: `testing-plan.md` - FM decision quality tests
- Existing: `FMOrchestrator.swift` for Foundation Models integration
