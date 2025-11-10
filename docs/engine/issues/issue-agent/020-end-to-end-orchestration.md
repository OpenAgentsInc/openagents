# Issue #020: Implement End-to-End Issue Processing Orchestration

**Component:** `component:issue-agent`
**Priority:** `priority:p0`
**Status:** `status:proposed`
**Effort:** 5-7 days
**Assignee:** TBD
**Created:** 2025-11-10
**Depends On:** #016, #017, #018

## Description

Implement the `IssueAgentService` actor that orchestrates the complete issue → patches → PR workflow. This is the top-level service that ties together enhancement, retrieval, patching, and PR creation.

## Goals

1. Coordinate all IssueAgent sub-services
2. Implement complete issue processing flow
3. Handle errors and retries gracefully
4. Stream progress updates via ACP
5. Support human-in-the-loop review gates
6. Track metrics and outcomes

## Implementation Details

### Files to Create

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
└── IssueAgent/
    ├── IssueAgentService.swift          # Main orchestration actor
    ├── Core/
    │   ├── WorkspaceManager.swift       # Git operations
    │   └── GitHubClient.swift           # GitHub API
    └── PR/
        ├── PRBuilder.swift              # Build PR metadata
        └── PRSubmitter.swift            # Submit PR
```

### IssueAgentService Actor

```swift
public actor IssueAgentService {
    private let enhancer: IssueEnhancer
    private let retrieval: RetrievalOrchestrator
    private let patchOrchestrator: PatchOrchestrator
    private let prBuilder: PRBuilder
    private let githubClient: GitHubClient
    private let workspaceManager: WorkspaceManager
    private let config: IssueAgentConfig

    public func processIssue(
        _ issueURL: String,
        workspace: String
    ) async throws -> IssueProcessingResult

    public func createPR(
        result: IssueProcessingResult,
        workspace: String
    ) async throws -> CreatedPR
}
```

### Complete Workflow

```swift
public func processIssue(
    _ issueURL: String,
    workspace: String
) async throws -> IssueProcessingResult {

    // 1. Fetch issue from GitHub
    sendProgress("fetching_issue", 0.1)
    let issue = try await githubClient.fetchIssue(url: issueURL)

    // 2. Enhance issue with Foundation Models
    sendProgress("enhancing_issue", 0.2)
    let enhanced = try await enhancer.enhance(issue)

    // 3. Retrieve relevant files via hybrid search
    sendProgress("retrieving_files", 0.4)
    let files = try await retrieval.findRelevantFiles(
        for: enhanced,
        workspace: workspace,
        topK: config.topKFiles
    )

    // 4. Generate patches via agent delegation
    sendProgress("generating_patches", 0.7)
    let patches = try await patchOrchestrator.generatePatches(
        issue: enhanced,
        files: files,
        workspace: workspace
    )

    // 5. Validate patches
    sendProgress("validating_patches", 0.9)
    let validatedPatches = try await validatePatches(patches, workspace: workspace)

    // 6. Return result (don't create PR yet)
    sendProgress("completed", 1.0)
    return IssueProcessingResult(
        issue: issue,
        enhancement: enhanced,
        files: files,
        patches: validatedPatches,
        processedAt: Date()
    )
}
```

### Result Types

```swift
public struct IssueProcessingResult: Codable, Sendable {
    public var issue: Issue
    public var enhancement: EnhancedIssue
    public var files: [RankedFile]
    public var patches: [ValidatedPatch]
    public var processedAt: Date

    public var successfulPatches: [ValidatedPatch] {
        patches.filter { $0.isValid && $0.validationResult == .passed }
    }
}

public struct ValidatedPatch: Codable, Sendable {
    public var proposal: PatchProposal
    public var validationResult: ValidationResult
    public var appliedContent: String?      // New file content after patch
    public var syntaxValid: Bool
    public var applicabilityScore: Float    // 0.0-1.0

    public enum ValidationResult: String, Codable {
        case passed
        case failed
        case warning
    }

    public var isValid: Bool {
        validationResult == .passed && syntaxValid
    }
}

public struct CreatedPR: Codable, Sendable {
    public var prNumber: Int
    public var prURL: String
    public var branch: String
    public var patches: [ValidatedPatch]
    public var createdAt: Date
}
```

### Progress Streaming

```swift
private func sendProgress(_ step: String, _ progress: Double) {
    // Stream via ACP session/update
    let update = ToolCallUpdate(
        callId: currentCallId,
        status: .started,
        output: [
            "step": step,
            "progress": progress
        ]
    )

    sessionUpdateHub.sendUpdate(update)
}
```

## Acceptance Criteria

- [ ] `IssueAgentService` orchestrates complete workflow
- [ ] Fetches issue from GitHub API
- [ ] Enhances issue with Foundation Models
- [ ] Retrieves top-K files via hybrid search
- [ ] Generates patches via agent delegation
- [ ] Validates patches before returning
- [ ] Streams progress updates (10%, 20%, 40%, 70%, 90%, 100%)
- [ ] Returns structured result for review
- [ ] Optionally creates PR after approval
- [ ] Error handling for each step
- [ ] Integration tests:
  - `testEndToEndProcessing()`
  - `testErrorHandling()`
  - `testProgressStreaming()`
  - `testPRCreation()`
- [ ] Performance: <60s for full workflow

## Dependencies

- Issue #016 (IssueEnhancer)
- Issue #017 (RetrievalOrchestrator)
- Issue #018 (AgentDelegator)
- GitHub API client
- Git workspace management

## References

- [IssueAgent Architecture](../../../plans/issue-agent-architecture.md) § 4.1 (Data Flow)
- [Pierrebhat Spec](../../../../pierrebhat/docs/SPEC.md) § "Operational Flow"

## Example Usage

```swift
let service = IssueAgentService(
    enhancer: enhancer,
    retrieval: retrieval,
    patchOrchestrator: patchOrchestrator,
    prBuilder: prBuilder,
    githubClient: githubClient,
    workspaceManager: workspaceManager,
    config: config
)

// Process issue
let result = try await service.processIssue(
    "https://github.com/org/repo/issues/123",
    workspace: "/path/to/workspace"
)

print("Enhancement: \(result.enhancement.keyFocus)")
print("Files: \(result.files.count)")
print("Valid patches: \(result.successfulPatches.count)")

// Review patches...

// Create PR
if userApproved {
    let pr = try await service.createPR(result: result, workspace: workspace)
    print("Created PR: \(pr.prURL)")
}
```

## PR Creation Flow

```swift
public func createPR(
    result: IssueProcessingResult,
    workspace: String
) async throws -> CreatedPR {

    // 1. Generate PR metadata (title, body)
    let metadata = try await prBuilder.generateMetadata(
        issue: result.issue,
        patches: result.successfulPatches
    )

    // 2. Create branch
    let branchName = "issue-\(result.issue.number)-\(UUID().uuidString.prefix(8))"
    try await workspaceManager.createBranch(branchName, in: workspace)

    // 3. Apply patches
    for patch in result.successfulPatches {
        guard let content = patch.appliedContent else { continue }
        try await workspaceManager.writeFile(
            path: patch.proposal.filePath,
            content: content,
            workspace: workspace
        )
    }

    // 4. Commit
    let commitMessage = buildCommitMessage(
        issue: result.issue,
        metadata: metadata
    )
    try await workspaceManager.commit(message: commitMessage, workspace: workspace)

    // 5. Push
    try await workspaceManager.push(branch: branchName, workspace: workspace)

    // 6. Create PR via GitHub API
    let pr = try await githubClient.createPullRequest(
        repo: config.targetRepo,
        head: branchName,
        base: config.baseBranch,
        title: metadata.title,
        body: metadata.body,
        draft: config.createDraftPRs
    )

    return CreatedPR(
        prNumber: pr.number,
        prURL: pr.url,
        branch: branchName,
        patches: result.successfulPatches,
        createdAt: Date()
    )
}
```

## Configuration

```swift
public struct IssueAgentConfig: Codable, Sendable {
    // Retrieval
    public var topKFiles: Int = 20
    public var minSimilarity: Float = 0.7

    // Patching
    public var preferredAgent: AgentType = .claudeCode
    public var maxConcurrentPatches: Int = 5

    // PR
    public var targetRepo: String           // e.g., "org/repo"
    public var baseBranch: String = "main"
    public var createDraftPRs: Bool = true
    public var requireHumanReview: Bool = false

    // Workflow
    public var autoCreatePR: Bool = false   // Default: return for review
}
```

## Error Handling

```swift
public enum IssueAgentError: Error, LocalizedError {
    case issueNotFound(String)
    case enhancementFailed(Error)
    case noFilesFound
    case noPatchesGenerated
    case allPatchesFailed
    case prCreationFailed(Error)

    public var errorDescription: String? {
        switch self {
        case .issueNotFound(let url):
            return "Issue not found: \(url)"
        case .enhancementFailed(let error):
            return "Failed to enhance issue: \(error.localizedDescription)"
        case .noFilesFound:
            return "No relevant files found"
        case .noPatchesGenerated:
            return "Failed to generate any patches"
        case .allPatchesFailed:
            return "All patches failed validation"
        case .prCreationFailed(let error):
            return "Failed to create PR: \(error.localizedDescription)"
        }
    }
}
```

## Progress Steps

```swift
public enum ProcessingStep: String, Codable {
    case fetchingIssue = "fetching_issue"
    case enhancingIssue = "enhancing_issue"
    case retrievingFiles = "retrieving_files"
    case generatingPatches = "generating_patches"
    case validatingPatches = "validating_patches"
    case creatingBranch = "creating_branch"
    case applyingPatches = "applying_patches"
    case committingChanges = "committing_changes"
    case pushingToFork = "pushing_to_fork"
    case creatingPR = "creating_pr"
    case completed = "completed"
}
```

## Performance Targets

- Complete workflow: <60s (without PR creation)
- PR creation: <30s
- Total end-to-end: <90s

## Testing

```bash
cd ios
xcodebuild test -workspace OpenAgents.xcworkspace \
  -scheme OpenAgents -sdk macosx \
  -only-testing:OpenAgentsCoreTests/IssueAgentServiceTests
```

## Integration Test

```swift
func testEndToEndIssueProcessing() async throws {
    // Setup test workspace
    let workspace = try createTestWorkspace(
        files: [
            "src/Settings.swift": settingsContent,
            "src/Theme.swift": themeContent
        ]
    )

    // Create mock issue
    let issue = Issue(
        number: 1,
        title: "Fix typo in settings",
        body: "The word 'prefrences' should be 'preferences' in Settings.swift",
        url: "https://github.com/test/repo/issues/1",
        comments: [],
        labels: ["bug"],
        createdAt: Date()
    )

    // Mock GitHub client
    let githubClient = MockGitHubClient()
    githubClient.mockIssue = issue

    // Process
    let service = IssueAgentService(/* ... */)
    let result = try await service.processIssue(
        issue.url,
        workspace: workspace
    )

    // Assertions
    XCTAssertEqual(result.issue.number, 1)
    XCTAssertFalse(result.enhancement.keyFocus.isEmpty)
    XCTAssertGreaterThan(result.files.count, 0)
    XCTAssertGreaterThan(result.successfulPatches.count, 0)

    // Verify patch content
    let patch = result.successfulPatches.first!
    XCTAssertEqual(patch.proposal.filePath, "src/Settings.swift")
    XCTAssertTrue(patch.proposal.beforeBlock.contains("prefrences"))
    XCTAssertTrue(patch.proposal.afterBlock.contains("preferences"))
    XCTAssertTrue(patch.isValid)
}
```

## Notes

- Default behavior: return proposals for review (don't auto-create PR)
- Support `autoCreatePR` flag for fully automated mode
- Stream progress for long-running operations
- Handle partial failures (some patches succeed, some fail)
- Log detailed metrics for debugging

## Metrics Tracking

```swift
public struct IssueProcessingMetrics: Codable {
    public var issueNumber: Int
    public var totalDuration: TimeInterval
    public var enhancementDuration: TimeInterval
    public var retrievalDuration: TimeInterval
    public var patchingDuration: TimeInterval
    public var filesRetrieved: Int
    public var patchesGenerated: Int
    public var patchesValid: Int
    public var agentsUsed: [AgentType]
}
```

## Definition of Done

- [ ] Code implemented and reviewed
- [ ] Unit tests passing
- [ ] Integration tests passing
- [ ] End-to-end test with real issue passing
- [ ] Performance targets met
- [ ] Progress streaming working
- [ ] Error handling robust
- [ ] Metrics tracking implemented
- [ ] Documentation complete
- [ ] Merged to main branch
