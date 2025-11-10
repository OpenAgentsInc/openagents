# Issue #018: Implement Agent Delegator for Patch Generation

**Component:** `component:issue-agent`
**Priority:** `priority:p0`
**Status:** `status:proposed`
**Effort:** 5-7 days
**Assignee:** TBD
**Created:** 2025-11-10
**Depends On:** #016, #017, Bridge setup

## Description

Implement the `AgentDelegator` actor that delegates patch generation to external agents (Claude Code, Codex) or Foundation Models based on complexity. This is the core intelligence layer that generates Before/After code patches.

## Goals

1. Support multiple agent backends (Foundation Models, Claude Code, Codex)
2. Build structured prompts for patch generation
3. Parse Before/After blocks from responses
4. Handle agent unavailability and fallbacks
5. Track agent performance metrics

## Implementation Details

### Files to Create

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
└── IssueAgent/
    └── Patching/
        ├── AgentDelegator.swift         # Main delegator
        ├── AgentClients.swift           # Agent client wrappers
        ├── PatchParser.swift            # Parse Before/After from responses
        └── PatchTypes.swift             # PatchProposal, AgentType
```

### AgentDelegator Actor

```swift
public actor AgentDelegator {
    private let fmSession: LanguageModelSession?
    private let claudeCodeClient: ClaudeCodeClient?
    private let codexClient: CodexClient?
    private let config: AgentDelegatorConfig

    public func requestPatch(
        issue: Issue,
        keyFocus: String,
        filePath: String,
        fileContent: String,
        agent: AgentType
    ) async throws -> PatchProposal
}
```

### Agent Types

```swift
public enum AgentType: String, Codable {
    case foundationModels   // On-device, fast, good for simple edits
    case claudeCode         // Cloud, powerful, good for moderate complexity
    case codex              // Cloud, specialized for code, good for complex changes
}

public struct AgentDelegatorConfig: Codable, Sendable {
    public var preferredAgent: AgentType = .claudeCode
    public var fallbackAgent: AgentType? = .foundationModels
    public var allowExternalAgents: Bool = true
    public var timeout: TimeInterval = 60.0  // seconds
}
```

### Patch Proposal

```swift
public struct PatchProposal: Codable, Sendable {
    public var filePath: String
    public var beforeBlock: String
    public var afterBlock: String
    public var needsPatch: Bool
    public var agent: AgentType
    public var confidence: Float?         // 0.0-1.0 (if agent provides)
    public var generatedAt: Date

    public var isValid: Bool {
        !beforeBlock.isEmpty && !afterBlock.isEmpty && needsPatch
    }
}
```

### Prompt Template

```swift
private func buildPatchPrompt(
    issue: Issue,
    keyFocus: String,
    filePath: String,
    fileContent: String
) -> String {
    return """
    You are tasked with fixing the following GitHub issue:

    Issue #\(issue.number): \(issue.title)
    \(issue.body)

    Key Focus: \(keyFocus)

    File to modify: \(filePath)

    Current file content:
    ```
    \(fileContent)
    ```

    Instructions:
    1. Analyze the issue and the file carefully
    2. Determine if this file needs changes (Yes/No)
    3. If Yes, provide the fix using EXACTLY this format:

    NEEDS_PATCH: Yes

    Before:
    ```
    <exact code block to replace, including all whitespace>
    ```

    After:
    ```
    <replacement code block>
    ```

    IMPORTANT:
    - The Before block must EXACTLY match existing code (including whitespace)
    - Only include the specific section that needs to change
    - Do not include line numbers or file paths in the code blocks
    - Be minimal - only change what's necessary
    """
}
```

## Acceptance Criteria

- [ ] `AgentDelegator` supports Foundation Models, Claude Code, Codex
- [ ] Builds structured prompts with issue + file context
- [ ] Parses Before/After blocks from responses
- [ ] Validates patch format (exact match possible)
- [ ] Handles agent timeouts and errors
- [ ] Falls back to alternate agent if primary fails
- [ ] Tracks metrics (latency, success rate per agent)
- [ ] Unit tests:
  - `testPatchPromptBuilding()`
  - `testPatchParsing()`
  - `testAgentSelection()`
  - `testFallbackBehavior()`
  - `testErrorHandling()`
- [ ] Integration tests with real agents
- [ ] Performance: <30s for patch generation

## Dependencies

- Issue #016 (IssueEnhancer - for issue types)
- Issue #017 (RetrievalOrchestrator - for file selection)
- Bridge setup for external agents (Claude Code, Codex)

## References

- [IssueAgent Architecture](../../../plans/issue-agent-architecture.md) § 4.4 (Patch Generation)
- [Pierrebhat Spec](../../../../pierrebhat/docs/SPEC.md) § "Patch Generation"

## Example Usage

```swift
let delegator = AgentDelegator(
    fmSession: fmSession,
    claudeCodeClient: claudeClient,
    codexClient: nil
)

let patch = try await delegator.requestPatch(
    issue: issue,
    keyFocus: "Fix typo in error message",
    filePath: "src/Error.swift",
    fileContent: fileContent,
    agent: .claudeCode
)

if patch.isValid {
    print("Before: \(patch.beforeBlock)")
    print("After: \(patch.afterBlock)")
    print("Agent: \(patch.agent)")
}
```

## Agent Client Wrappers

### Foundation Models Client

```swift
public actor FoundationModelsClient {
    private let session: LanguageModelSession

    public func generatePatch(_ prompt: String) async throws -> String {
        let response = try await session.respond(
            to: prompt,
            options: GenerationOptions(temperature: 0.2, maxTokens: 2048)
        )
        return response.content
    }
}
```

### Claude Code Client

```swift
public actor ClaudeCodeClient {
    private let bridgeServer: DesktopWebSocketServer

    public func generatePatch(_ prompt: String) async throws -> String {
        // Use existing ACP bridge to call Claude Code
        let sessionId = try await bridgeServer.createSession()
        let response = try await bridgeServer.sendPrompt(sessionId: sessionId, prompt: prompt)
        return extractTextFromResponse(response)
    }
}
```

### Codex Client

```swift
public actor CodexClient {
    private let bridgeServer: DesktopWebSocketServer

    public func generatePatch(_ prompt: String) async throws -> String {
        // Similar to Claude Code, but via Codex bridge
        let sessionId = try await bridgeServer.createSession()
        let response = try await bridgeServer.sendPrompt(sessionId: sessionId, prompt: prompt)
        return extractTextFromResponse(response)
    }
}
```

## Patch Parsing

```swift
public struct PatchParser {
    public static func parse(_ response: String) throws -> (needsPatch: Bool, before: String?, after: String?) {
        // Extract NEEDS_PATCH
        let needsPatch = response.contains("NEEDS_PATCH: Yes")

        guard needsPatch else {
            return (false, nil, nil)
        }

        // Extract Before block
        let beforeRegex = #"Before:\s*```[^\n]*\n(.*?)\n```"#
        let beforeMatch = response.range(of: beforeRegex, options: .regularExpression)
        let before = beforeMatch.map { String(response[$0]) }

        // Extract After block
        let afterRegex = #"After:\s*```[^\n]*\n(.*?)\n```"#
        let afterMatch = response.range(of: afterRegex, options: .regularExpression)
        let after = afterMatch.map { String(response[$0]) }

        guard let before = before, let after = after else {
            throw PatchError.invalidFormat
        }

        return (true, cleanCodeBlock(before), cleanCodeBlock(after))
    }

    private static func cleanCodeBlock(_ block: String) -> String {
        // Remove code fence markers, trim whitespace
        var cleaned = block
        cleaned = cleaned.replacingOccurrences(of: #"```[^\n]*\n"#, with: "", options: .regularExpression)
        cleaned = cleaned.replacingOccurrences(of: "```", with: "")
        return cleaned
    }
}
```

## Agent Selection Strategy

```swift
func selectAgent(complexity: EnhancedIssue.Complexity, config: AgentDelegatorConfig) -> AgentType {
    switch complexity {
    case .simple:
        // Try Foundation Models first (fast, local)
        return .foundationModels
    case .moderate:
        // Use Claude Code (good balance)
        return .claudeCode
    case .complex:
        // Use Codex (specialized for complex code)
        return .codex
    }
}
```

## Error Handling

```swift
public enum PatchError: Error, LocalizedError {
    case agentUnavailable(AgentType)
    case timeout
    case invalidFormat
    case beforeBlockNotFound
    case parsingFailed(String)

    public var errorDescription: String? {
        switch self {
        case .agentUnavailable(let agent):
            return "Agent \(agent) is unavailable"
        case .timeout:
            return "Patch generation timed out"
        case .invalidFormat:
            return "Response does not match expected format"
        case .beforeBlockNotFound:
            return "Before block not found in file"
        case .parsingFailed(let reason):
            return "Failed to parse patch: \(reason)"
        }
    }
}
```

## Performance Targets

- Foundation Models: <10s per patch
- Claude Code: <30s per patch
- Codex: <30s per patch
- Parse time: <10ms

## Testing

```bash
cd ios
xcodebuild test -workspace OpenAgents.xcworkspace \
  -scheme OpenAgents -sdk macosx \
  -only-testing:OpenAgentsCoreTests/AgentDelegatorTests
```

## Golden Tests

```
tests/golden/patches/
├── typo-fix/
│   ├── issue.json
│   ├── file_content.txt
│   └── expected_patch.json
├── feature-add/
│   ├── issue.json
│   ├── file_content.txt
│   └── expected_patch.json
└── refactor/
    ├── issue.json
    ├── file_content.txt
    └── expected_patch.json
```

## Notes

- Temperature should be low (0.2) for deterministic patches
- Before block must exactly match (whitespace-sensitive)
- Agents may refuse to patch (safety check)
- Track success rate per agent to improve selection

## Future Enhancements

- Multi-hunk patches (multiple Before/After in one file)
- Fuzzy matching for Before block (handle minor differences)
- Confidence scoring from agent responses
- Learn from PR outcomes to improve agent selection

## Definition of Done

- [ ] Code implemented and reviewed
- [ ] Unit tests passing
- [ ] Integration tests with real agents passing
- [ ] Golden tests passing (>70% applicability)
- [ ] Performance targets met
- [ ] Error handling robust
- [ ] Documentation complete
- [ ] Merged to main branch
