# Issue #016: Implement Issue Enhancer with Foundation Models

**Component:** `component:issue-agent`
**Priority:** `priority:p1`
**Status:** `status:proposed`
**Effort:** 3-4 days
**Assignee:** TBD
**Created:** 2025-11-10
**Depends On:** None (uses existing FM integration)

## Description

Implement the `IssueEnhancer` actor that uses Foundation Models to analyze GitHub issues and extract structured metadata: key focus, search queries, allowed file extensions, and estimated complexity. This enhanced metadata drives retrieval and agent selection.

## Goals

1. Use Foundation Models with structured output (`@Generable`)
2. Extract key focus (1-2 sentence summary)
3. Generate 3-5 alternative search queries
4. Identify allowed file extensions (if mentioned)
5. Estimate complexity (simple/moderate/complex)
6. Handle Foundation Models unavailability gracefully

## Implementation Details

### Files to Create

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
└── IssueAgent/
    ├── Core/
    │   ├── IssueTypes.swift             # Issue, EnhancedIssue types
    │   └── GitHubClient.swift           # GitHub API wrapper (stub)
    └── Enhancement/
        ├── IssueEnhancer.swift          # FM-based enhancer
        └── ExtensionFilter.swift        # Parse file extensions
```

### Structured Output Type

```swift
@Generable
public struct IssueEnhancement: Equatable {
    @Guide(description: "A concise 1-2 sentence summary of the core issue or feature request")
    public var keyFocus: String

    @Guide(description: "Specific file extensions to focus on, if mentioned (e.g., ['.swift', '.ts'])")
    public var allowedExtensions: [String]?

    @Guide(description: "3-5 alternative search queries to find relevant files")
    public var searchQueries: [String]

    @Guide(description: "Estimated complexity: 'simple' for typos/docs, 'moderate' for single-file logic, 'complex' for multi-file refactors")
    public var complexity: String
}
```

### IssueEnhancer Actor

```swift
#if canImport(FoundationModels)
import FoundationModels

@available(iOS 26.0, macOS 15.0, *)
public actor IssueEnhancer {
    private let model: SystemLanguageModel

    public func enhance(_ issue: Issue) async throws -> EnhancedIssue {
        guard case .available = model.availability else {
            throw IssueAgentError.foundationModelsUnavailable
        }

        let instructions = """
        You are an expert at analyzing GitHub issues and extracting key information for code retrieval.
        """

        let session = LanguageModelSession(
            model: model,
            instructions: instructions
        )

        let prompt = buildPrompt(issue)
        let response = try await session.respond(
            to: prompt,
            expectingType: IssueEnhancement.self,
            options: GenerationOptions(temperature: 0.3)
        )

        return EnhancedIssue(
            issue: issue,
            keyFocus: response.keyFocus,
            allowedExtensions: response.allowedExtensions,
            searchQueries: response.searchQueries,
            estimatedComplexity: parseComplexity(response.complexity)
        )
    }
}
#endif
```

### Result Types

```swift
public struct Issue: Codable, Sendable {
    public var number: Int
    public var title: String
    public var body: String
    public var url: String
    public var comments: [Comment]
    public var labels: [String]
    public var createdAt: Date

    public struct Comment: Codable, Sendable {
        public var author: String
        public var body: String
        public var createdAt: Date
    }

    public var fullText: String {
        let commentText = comments.map { "From \($0.author): \($0.body)" }.joined(separator: "\n")
        return """
        Issue: \(title)
        \(body)

        Responses:
        \(commentText)
        """
    }
}

public struct EnhancedIssue: Codable, Sendable {
    public var issue: Issue
    public var keyFocus: String
    public var allowedExtensions: [String]?
    public var searchQueries: [String]
    public var estimatedComplexity: Complexity

    public enum Complexity: String, Codable {
        case simple      // Typos, docs, trivial fixes
        case moderate    // Single-file logic changes
        case complex     // Multi-file refactors, architecture changes
    }
}
```

## Acceptance Criteria

- [ ] `IssueEnhancer` uses Foundation Models with structured output
- [ ] Generates key focus (1-2 sentences)
- [ ] Generates 3-5 search queries with variations
- [ ] Extracts file extensions when mentioned
- [ ] Estimates complexity (simple/moderate/complex)
- [ ] Handles FM unavailability (fallback or error)
- [ ] Unit tests:
  - `testIssueEnhancement()`
  - `testKeyFocusExtraction()`
  - `testSearchQueryGeneration()`
  - `testComplexityEstimation()`
  - `testFallbackBehavior()`
- [ ] Golden tests with real issues
- [ ] Performance: <2s for enhancement

## Dependencies

- Existing Foundation Models infrastructure (ADR-0006)
- `LanguageModelSession` API
- `@Generable` macro support

## References

- [IssueAgent Architecture](../../../plans/issue-agent-architecture.md) § 4.2 (Enhancement)
- [Pierrebhat Spec](../../../../pierrebhat/docs/SPEC.md) § "Issue Ingestion"
- ADR-0006 (Foundation Models)

## Example Usage

```swift
let issue = Issue(
    number: 123,
    title: "Add dark mode support to settings",
    body: "We need dark mode for the settings screen. Should work on both iOS and macOS.",
    url: "https://github.com/org/repo/issues/123",
    comments: [],
    labels: ["enhancement"],
    createdAt: Date()
)

let enhancer = IssueEnhancer()
let enhanced = try await enhancer.enhance(issue)

print("Key Focus: \(enhanced.keyFocus)")
// "Add dark mode toggle to settings screen for iOS and macOS"

print("Search Queries: \(enhanced.searchQueries)")
// ["settings dark mode", "theme switcher", "appearance preference", "dark mode toggle"]

print("Complexity: \(enhanced.estimatedComplexity)")
// .moderate

print("Extensions: \(enhanced.allowedExtensions ?? [])")
// [".swift"]
```

## Prompt Template

```swift
private func buildPrompt(_ issue: Issue) -> String {
    return """
    Analyze this GitHub issue:

    Title: \(issue.title)
    Body: \(issue.body)

    Comments:
    \(issue.comments.map { "\($0.author): \($0.body)" }.joined(separator: "\n"))

    Extract:
    1. Key focus (what is the core problem/feature?)
    2. Search queries (how would you search for relevant files?)
    3. File extensions (if specific languages/files mentioned)
    4. Complexity estimate (simple/moderate/complex)
    """
}
```

## Complexity Estimation Heuristics

- **Simple**: Typos, documentation, string changes, trivial fixes
- **Moderate**: Single-file logic changes, new functions, UI updates
- **Complex**: Multi-file refactors, architecture changes, new features with dependencies

## Fallback Behavior

If Foundation Models unavailable:

```swift
// Option 1: Return error
throw IssueAgentError.foundationModelsUnavailable

// Option 2: Use heuristics
func fallbackEnhance(_ issue: Issue) -> EnhancedIssue {
    return EnhancedIssue(
        issue: issue,
        keyFocus: issue.title,  // Use title as-is
        allowedExtensions: nil,
        searchQueries: [issue.title, extractKeywords(issue.body)],
        estimatedComplexity: .moderate  // Default to moderate
    )
}
```

## Testing

```bash
cd ios
xcodebuild test -workspace OpenAgents.xcworkspace \
  -scheme OpenAgents -sdk macosx \
  -only-testing:OpenAgentsCoreTests/IssueEnhancerTests
```

## Golden Test Corpus

```
tests/golden/issue-enhancement/
├── issue-001-typo.json
├── issue-002-feature.json
├── issue-003-refactor.json
└── issue-004-bug.json
```

Each file:
```json
{
  "issue": {
    "number": 1,
    "title": "Fix typo in README",
    "body": "The word 'teh' should be 'the' in README.md",
    "labels": ["documentation"]
  },
  "expected": {
    "keyFocus": "Fix typo in README.md",
    "searchQueries": ["README", "documentation"],
    "complexity": "simple",
    "allowedExtensions": [".md"]
  }
}
```

## Performance Targets

- Enhancement latency: <2s p95
- FM token usage: <500 tokens per issue

## Notes

- Use low temperature (0.3) for consistent structured output
- Search queries should be diverse (synonym expansion)
- Complexity estimation is guidance, not hard constraint
- Allow user override of complexity if needed

## Definition of Done

- [ ] Code implemented and reviewed
- [ ] Unit tests passing
- [ ] Golden tests passing (>80% accuracy)
- [ ] Performance targets met
- [ ] Fallback behavior implemented
- [ ] Documentation complete
- [ ] Merged to main branch
