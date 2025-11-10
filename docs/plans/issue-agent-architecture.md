# IssueAgent Architecture - Automated Issue Resolution via Embeddings & Agent Delegation

**Status:** Proposed
**Date:** 2025-11-10
**Owner:** OpenAgents Core Team
**Related Docs:**
- [Embeddings Implementation Plan](./embeddings-implementation-plan.md)
- [SearchKit Spec v0.2.2](../engine/spec-v0.2.2.md)
- ADR-0006 (Foundation Models)

---

## 1. Executive Summary

**IssueAgent** is a high-level orchestration system that automates the GitHub issue → PR workflow using local embeddings, semantic retrieval, Foundation Models for simple tasks, and delegation to powerful agents (Claude Code, Codex) for complex patch generation.

### Key Differences from Pierrebhat

| Aspect | Pierrebhat (Python) | IssueAgent (Swift) |
|--------|---------------------|-------------------|
| **Embeddings** | OpenAI API (text-embedding-ada-002) | Local MLX (BGE-small-en-v1.5) |
| **Storage** | FAISS (in-memory) | SQLite (persistent) |
| **Search** | Pure semantic | Hybrid (FTS5 + semantic + RRF) |
| **Issue Enhancement** | OpenAI completion | Foundation Models (on-device) |
| **Patch Generation** | OpenAI (davinci-002) | Delegate to Claude Code/Codex |
| **Planning** | None (direct patch) | Foundation Models + agent delegation |
| **Platform** | Python script | Native Swift (macOS + iOS) |

### Architecture Philosophy

1. **Local-first intelligence**: Use on-device models for lightweight tasks (summarization, key focus extraction, similarity search)
2. **Strategic delegation**: Delegate complex reasoning (patch generation, multi-file refactors) to external agents
3. **Persistent knowledge**: Store embeddings and metadata in SQLite for incremental updates
4. **ACP-native**: All operations exposed as ACP tool calls and updates
5. **Human-in-the-loop**: Support review/approval gates before PR creation

---

## 2. System Architecture

### 2.1 Layered Design

```
┌─────────────────────────────────────────────────────────────┐
│                    IssueAgent (High-level)                   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ IssueOrchestrator                                    │   │
│  │  - Issue fetching & enhancement                      │   │
│  │  - Retrieval coordination                            │   │
│  │  - Patch orchestration & delegation                  │   │
│  │  - PR creation & management                          │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                           ↓ Uses
┌─────────────────────────────────────────────────────────────┐
│              SearchKitService (Mid-level)                    │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ HybridSearch                                         │   │
│  │  - Lexical (FTS5 BM25)                              │   │
│  │  - Semantic (cosine similarity)                     │   │
│  │  - RRF fusion                                       │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                           ↓ Uses
┌─────────────────────────────────────────────────────────────┐
│             EmbeddingService (Low-level)                     │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ MLXEmbeddingProvider                                 │   │
│  │ VectorStore (SQLite)                                 │   │
│  │ SimilaritySearch                                     │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Component Responsibilities

**IssueAgent Layer** (NEW - this plan)
- GitHub API integration (issue fetching, PR creation)
- Issue enhancement (extract key focus, allowed extensions)
- Retrieval orchestration (top-k file selection)
- Patch orchestration (delegate to agents)
- PR workflow management

**SearchKit Layer** (implements spec-v0.2.2)
- File indexing (chunking, FTS5, embeddings)
- Hybrid search (lexical + semantic + fusion)
- Span reading
- ACP tool calls

**Embedding Layer** (implements embeddings-implementation-plan.md)
- MLX embedding generation
- Vector storage
- Brute-force similarity search

---

## 3. Directory Structure

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
├── Embeddings/                          # Phase 1-3 (other agent)
│   ├── EmbeddingProvider.swift
│   ├── MLXEmbeddingProvider.swift
│   ├── EmbeddingService.swift
│   └── VectorStore.swift
│
├── SearchKit/                           # Phase 4 (integration)
│   ├── Indexing/
│   │   ├── FileScanner.swift
│   │   ├── Chunker.swift
│   │   └── IndexManager.swift
│   ├── Search/
│   │   ├── FTS5Engine.swift
│   │   ├── SemanticSearch.swift
│   │   ├── HybridSearch.swift
│   │   └── RRFFusion.swift
│   ├── SpanReader.swift
│   └── SearchKitService.swift
│
└── IssueAgent/                          # NEW (this plan)
    ├── Core/
    │   ├── IssueTypes.swift             # Issue, PR, PatchProposal types
    │   ├── GitHubClient.swift           # GitHub API wrapper
    │   └── WorkspaceManager.swift       # Git clone, branch management
    ├── Enhancement/
    │   ├── IssueEnhancer.swift          # FM-based key focus extraction
    │   └── ExtensionFilter.swift        # Parse allowed file types
    ├── Retrieval/
    │   ├── RetrievalOrchestrator.swift  # Coordinate hybrid search
    │   └── FileRanker.swift             # Post-retrieval ranking
    ├── Patching/
    │   ├── PatchOrchestrator.swift      # Delegate to agents
    │   ├── AgentDelegator.swift         # Call Claude Code/Codex
    │   └── PatchValidator.swift         # Validate patches before apply
    ├── PR/
    │   ├── PRBuilder.swift              # Create PR metadata
    │   └── PRSubmitter.swift            # Submit via GitHub API
    └── IssueAgentService.swift          # Main orchestration actor
```

---

## 4. Data Flow: Issue → PR

### 4.1 High-Level Flow

```
1. Fetch Issue
   ├─ GitHub API → Issue(title, body, comments)
   └─ Parse conversation history

2. Enhance Issue (Foundation Models)
   ├─ Extract "key focus" (natural language goal)
   ├─ Identify allowed file extensions (if mentioned)
   └─ Generate search queries

3. Retrieve Relevant Files (Hybrid Search)
   ├─ Lexical search (keywords from issue)
   ├─ Semantic search (key focus embedding)
   ├─ RRF fusion (combine rankings)
   └─ Return top-K file paths

4. Generate Patches (Agent Delegation)
   ├─ For each file:
   │   ├─ Load full content
   │   ├─ Delegate to Claude Code/Codex
   │   │   └─ Prompt: issue + file → Before/After
   │   └─ Validate patch (syntax, applicability)
   └─ Collect successful patches

5. Create Pull Request
   ├─ Generate PR title/body (Foundation Models)
   ├─ Apply patches to fork branch
   ├─ Push to remote
   └─ Open PR via GitHub API
```

### 4.2 Detailed Steps

#### Step 1: Fetch Issue

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

    // Derived
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
```

#### Step 2: Enhance Issue

**Input:** Raw issue
**Output:** Enhanced issue with metadata

```swift
public struct EnhancedIssue: Codable, Sendable {
    public var issue: Issue
    public var keyFocus: String              // Natural language goal
    public var allowedExtensions: [String]?  // e.g., [".swift", ".ts"]
    public var searchQueries: [String]       // Multiple query variations
    public var estimatedComplexity: Complexity  // .simple, .moderate, .complex

    public enum Complexity: String, Codable {
        case simple      // Foundation Models can handle
        case moderate    // Needs Claude Code
        case complex     // Needs Codex or manual intervention
    }
}
```

**Implementation:** Use Foundation Models with structured output

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

#### Step 3: Retrieve Relevant Files

**Orchestration:**

```swift
public actor RetrievalOrchestrator {
    private let searchService: SearchKitService
    private let embeddingService: EmbeddingService

    public func findRelevantFiles(
        for enhancement: EnhancedIssue,
        workspace: String,
        topK: Int = 20
    ) async throws -> [RankedFile] {

        // 1. Hybrid search for each query
        var allResults: [SearchResult] = []

        for query in enhancement.searchQueries {
            let hybridRequest = HybridSearchRequest(
                query: query,
                workspace: workspace,
                lexicalWeight: 0.4,
                semanticWeight: 0.6,
                limit: topK * 2  // Fetch extra for fusion
            )
            let results = try await searchService.hybridSearch(hybridRequest)
            allResults.append(contentsOf: results)
        }

        // 2. Deduplicate and re-rank
        let deduplicated = deduplicateByPath(allResults)

        // 3. Filter by allowed extensions (if specified)
        let filtered = enhancement.allowedExtensions.map { exts in
            deduplicated.filter { result in
                exts.contains { ext in result.path.hasSuffix(ext) }
            }
        } ?? deduplicated

        // 4. Boost by recency, file size, importance
        let ranked = rankFiles(filtered, workspace: workspace)

        // 5. Return top-K
        return Array(ranked.prefix(topK))
    }

    private func rankFiles(_ files: [SearchResult], workspace: String) -> [RankedFile] {
        // TODO: Implement ranking heuristics
        // - Recency (mtime)
        // - Size (prefer smaller files for focused changes)
        // - Depth (prefer files closer to root)
        // - Prior PR success (learn from history)
        return files.map { RankedFile(searchResult: $0, rank: $0.score) }
    }
}
```

#### Step 4: Generate Patches

**Delegation Model:**

```swift
public actor PatchOrchestrator {
    private let agentDelegator: AgentDelegator
    private let validator: PatchValidator

    public func generatePatches(
        issue: EnhancedIssue,
        files: [RankedFile],
        workspace: String
    ) async throws -> [PatchProposal] {

        var proposals: [PatchProposal] = []

        for file in files {
            do {
                // Load file content
                let content = try await loadFileContent(path: file.path, workspace: workspace)

                // Determine agent based on complexity
                let agent: AgentType = selectAgent(for: issue, file: file)

                // Delegate to agent
                let patch = try await agentDelegator.requestPatch(
                    issue: issue.issue,
                    keyFocus: issue.keyFocus,
                    filePath: file.path,
                    fileContent: content,
                    agent: agent
                )

                // Validate patch
                try validator.validate(patch, against: content)

                proposals.append(patch)

            } catch {
                // Log and continue with other files
                print("[PatchOrchestrator] Failed to patch \(file.path): \(error)")
            }
        }

        return proposals
    }

    private func selectAgent(for issue: EnhancedIssue, file: RankedFile) -> AgentType {
        switch issue.estimatedComplexity {
        case .simple:
            return .foundationModels  // Try FM first for simple edits
        case .moderate:
            return .claudeCode
        case .complex:
            return .codex
        }
    }
}
```

**Agent Delegator:**

```swift
public enum AgentType: String, Codable {
    case foundationModels
    case claudeCode
    case codex
}

public actor AgentDelegator {
    private let fmSession: LanguageModelSession?
    private let claudeCodeClient: ClaudeCodeClient?
    private let codexClient: CodexClient?

    public func requestPatch(
        issue: Issue,
        keyFocus: String,
        filePath: String,
        fileContent: String,
        agent: AgentType
    ) async throws -> PatchProposal {

        let prompt = buildPatchPrompt(
            issue: issue,
            keyFocus: keyFocus,
            filePath: filePath,
            fileContent: fileContent
        )

        switch agent {
        case .foundationModels:
            return try await requestFromFoundationModels(prompt)
        case .claudeCode:
            return try await requestFromClaudeCode(prompt)
        case .codex:
            return try await requestFromCodex(prompt)
        }
    }

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
        1. Analyze the issue and the file
        2. Determine if this file needs changes (Yes/No)
        3. If Yes, provide the fix using this format:

        Before:
        ```
        <exact code block to replace>
        ```

        After:
        ```
        <replacement code block>
        ```

        Ensure the Before block exactly matches the existing code, including whitespace.
        """
    }

    private func requestFromFoundationModels(_ prompt: String) async throws -> PatchProposal {
        guard let session = fmSession else {
            throw IssueAgentError.foundationModelsUnavailable
        }

        // Use structured output if possible
        let response = try await session.respond(to: prompt)
        return try parsePatchFromResponse(response.content)
    }

    private func requestFromClaudeCode(_ prompt: String) async throws -> PatchProposal {
        // Call Claude Code via ACP bridge
        guard let client = claudeCodeClient else {
            throw IssueAgentError.claudeCodeUnavailable
        }

        let response = try await client.prompt(prompt)
        return try parsePatchFromResponse(response)
    }

    private func requestFromCodex(_ prompt: String) async throws -> PatchProposal {
        // Call Codex via ACP bridge
        guard let client = codexClient else {
            throw IssueAgentError.codexUnavailable
        }

        let response = try await client.prompt(prompt)
        return try parsePatchFromResponse(response)
    }
}
```

**Patch Types:**

```swift
public struct PatchProposal: Codable, Sendable {
    public var filePath: String
    public var beforeBlock: String
    public var afterBlock: String
    public var needsPatch: Bool
    public var agent: AgentType
    public var confidence: Float?  // 0.0-1.0

    public var isValid: Bool {
        !beforeBlock.isEmpty && !afterBlock.isEmpty && needsPatch
    }
}

public struct AppliedPatch: Codable, Sendable {
    public var proposal: PatchProposal
    public var success: Bool
    public var error: String?
    public var newContent: String?
}
```

#### Step 5: Create Pull Request

```swift
public actor PRBuilder {
    private let fmSession: LanguageModelSession?
    private let githubClient: GitHubClient

    public func createPR(
        issue: Issue,
        patches: [AppliedPatch],
        workspace: String,
        targetRepo: String
    ) async throws -> CreatedPR {

        // 1. Generate PR title and body using Foundation Models
        let prMetadata = try await generatePRMetadata(issue: issue, patches: patches)

        // 2. Create branch
        let branchName = "issue-\(issue.number)-\(UUID().uuidString.prefix(8))"
        try await workspaceManager.createBranch(branchName, in: workspace)

        // 3. Apply patches
        for patch in patches where patch.success, let content = patch.newContent {
            try await workspaceManager.writeFile(
                path: patch.proposal.filePath,
                content: content,
                workspace: workspace
            )
        }

        // 4. Commit changes
        let commitMessage = """
        Fix #\(issue.number): \(issue.title)

        \(prMetadata.commitBody)

        🤖 Generated with OpenAgents IssueAgent
        Co-Authored-By: IssueAgent <noreply@openagents.com>
        """

        try await workspaceManager.commit(message: commitMessage, workspace: workspace)

        // 5. Push to fork
        try await workspaceManager.push(branch: branchName, workspace: workspace)

        // 6. Create PR
        let pr = try await githubClient.createPullRequest(
            repo: targetRepo,
            head: branchName,
            base: "main",
            title: prMetadata.title,
            body: prMetadata.body
        )

        return CreatedPR(pr: pr, branch: branchName, patches: patches)
    }

    @Generable
    private struct PRMetadata: Equatable {
        @Guide(description: "Concise PR title (max 72 chars)")
        var title: String

        @Guide(description: "PR body with summary, changes, test plan")
        var body: String

        @Guide(description: "Commit message body")
        var commitBody: String
    }

    private func generatePRMetadata(
        issue: Issue,
        patches: [AppliedPatch]
    ) async throws -> PRMetadata {

        let changedFiles = patches.filter { $0.success }.map { $0.proposal.filePath }

        let prompt = """
        Generate a pull request for the following GitHub issue:

        Issue #\(issue.number): \(issue.title)
        \(issue.body)

        Changed files:
        \(changedFiles.map { "- \($0)" }.joined(separator: "\n"))

        Provide:
        1. A concise PR title
        2. A PR body with:
           - Summary of changes
           - Test plan
        3. A commit message body
        """

        guard let session = fmSession else {
            // Fallback to template
            return PRMetadata(
                title: "Fix #\(issue.number): \(issue.title)",
                body: "Fixes #\(issue.number)\n\nChanged files:\n\(changedFiles.map { "- \($0)" }.joined(separator: "\n"))",
                commitBody: "Automated fix for issue #\(issue.number)"
            )
        }

        let response = try await session.respond(to: prompt, expectingType: PRMetadata.self)
        return response
    }
}
```

---

## 5. Integration Points

### 5.1 SearchKit Integration

IssueAgent relies on SearchKit for all retrieval operations:

```swift
// In IssueAgentService
public actor IssueAgentService {
    private let searchService: SearchKitService
    private let embeddingService: EmbeddingService
    private let orchestrator: RetrievalOrchestrator

    public init(
        searchService: SearchKitService,
        embeddingService: EmbeddingService
    ) {
        self.searchService = searchService
        self.embeddingService = embeddingService
        self.orchestrator = RetrievalOrchestrator(
            searchService: searchService,
            embeddingService: embeddingService
        )
    }
}
```

### 5.2 Agent Bridge Integration

Expose IssueAgent operations via ACP:

```swift
// In DesktopWebSocketServer+IssueAgent.swift
extension DesktopWebSocketServer {

    func registerIssueAgentMethods() {
        router.register(method: "issue_agent/process_issue") { [weak self] id, params, _ in
            await self?.handleProcessIssue(id: id, params: params)
        }

        router.register(method: "issue_agent/create_pr") { [weak self] id, params, _ in
            await self?.handleCreatePR(id: id, params: params)
        }

        router.register(method: "issue_agent/status") { [weak self] id, params, _ in
            await self?.handleIssueAgentStatus(id: id, params: params)
        }
    }

    private func handleProcessIssue(id: String, params: [String: Any]) async {
        guard let issueURL = params["issue_url"] as? String else {
            sendError(id: id, code: "ERR_MISSING_PARAMS", message: "Missing issue_url")
            return
        }

        do {
            // 1. Fetch issue
            let issue = try await githubClient.fetchIssue(url: issueURL)

            // 2. Enhance issue
            let enhanced = try await issueEnhancer.enhance(issue)

            // 3. Retrieve relevant files
            let files = try await retrievalOrchestrator.findRelevantFiles(
                for: enhanced,
                workspace: currentWorkspace,
                topK: 20
            )

            // 4. Generate patches
            let patches = try await patchOrchestrator.generatePatches(
                issue: enhanced,
                files: files,
                workspace: currentWorkspace
            )

            // 5. Return proposals (don't create PR yet)
            let response = IssueProcessingResult(
                issue: issue,
                enhancement: enhanced,
                files: files,
                patches: patches
            )

            sendResponse(id: id, result: response.toDictionary())

        } catch {
            sendError(id: id, code: "ERR_PROCESSING_FAILED", message: error.localizedDescription)
        }
    }
}
```

### 5.3 Foundation Models Integration

Use existing Foundation Models infrastructure:

```swift
// In IssueEnhancer.swift
#if canImport(FoundationModels)
import FoundationModels

@available(iOS 26.0, macOS 15.0, *)
public actor IssueEnhancer {
    private let model: SystemLanguageModel

    public init() {
        self.model = SystemLanguageModel.default
    }

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

        let prompt = """
        Analyze this GitHub issue:

        Title: \(issue.title)
        Body: \(issue.body)

        Extract the key focus, suggested file extensions, search queries, and complexity.
        """

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

---

## 6. ACP Alignment

All IssueAgent operations are exposed as ACP tool calls:

### 6.1 Tool Definitions

```
issue_agent.process_issue
  Input: { issue_url: String, workspace: String }
  Output: IssueProcessingResult (enhanced issue + files + patches)

issue_agent.create_pr
  Input: { issue_url: String, patches: [PatchProposal], workspace: String }
  Output: CreatedPR (PR URL, branch, status)

issue_agent.status
  Input: { job_id: UUID }
  Output: JobStatus (pending|processing|completed|failed)
```

### 6.2 Streaming Updates

All operations stream progress via `session/update`:

```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "session_id": "...",
    "updates": [{
      "type": "tool_call_update",
      "call_id": "...",
      "status": "started",
      "output": {
        "step": "fetching_issue",
        "progress": 0.1
      }
    }]
  }
}
```

Progress steps:
1. `fetching_issue` (10%)
2. `enhancing_issue` (20%)
3. `retrieving_files` (40%)
4. `generating_patches` (70%)
5. `creating_pr` (90%)
6. `completed` (100%)

---

## 7. Configuration & Policy

### 7.1 Configuration

```swift
public struct IssueAgentConfig: Codable, Sendable {
    // Agent selection
    public var preferredAgent: AgentType = .claudeCode
    public var fallbackAgent: AgentType? = .foundationModels
    public var allowExternalAgents: Bool = true

    // Retrieval
    public var topKFiles: Int = 20
    public var minSimilarity: Float = 0.7
    public var lexicalWeight: Float = 0.4
    public var semanticWeight: Float = 0.6

    // Patching
    public var maxConcurrentPatches: Int = 5
    public var validateBeforeApply: Bool = true
    public var requireHumanReview: Bool = false

    // GitHub
    public var autoCreatePR: Bool = false  // Default: return proposals for review
    public var prDraftMode: Bool = true    // Create as draft PR
    public var prLabels: [String] = ["automated", "issue-agent"]

    // Workspace
    public var defaultWorkspace: String = "/Users/christopherdavid/code/openagents"
    public var forkOwner: String = "openagents-bot"
}
```

### 7.2 Policy Gates

```swift
public struct IssueAgentPolicy: Codable, Sendable {
    // What can IssueAgent do?
    public var canFetchIssues: Bool = true
    public var canReadFiles: Bool = true
    public var canWriteFiles: Bool = false  // Requires explicit enable
    public var canCreateBranches: Bool = false
    public var canPushToFork: Bool = false
    public var canCreatePRs: Bool = false

    // What agents can be used?
    public var allowFoundationModels: Bool = true
    public var allowClaudeCode: Bool = true  // Requires credentials
    public var allowCodex: Bool = true       // Requires credentials

    // Rate limits
    public var maxIssuesPerHour: Int = 10
    public var maxPatchesPerIssue: Int = 50
    public var maxPRsPerDay: Int = 5
}
```

---

## 8. Testing Strategy

### 8.1 Unit Tests

```swift
// Test issue enhancement
func testIssueEnhancement() async throws {
    let issue = Issue(
        number: 123,
        title: "Add dark mode support",
        body: "We need dark mode for the settings screen",
        url: "...",
        comments: [],
        labels: ["enhancement"],
        createdAt: Date()
    )

    let enhancer = IssueEnhancer()
    let enhanced = try await enhancer.enhance(issue)

    XCTAssertFalse(enhanced.keyFocus.isEmpty)
    XCTAssertTrue(enhanced.searchQueries.count >= 3)
}

// Test retrieval orchestration
func testRetrievalOrchestration() async throws {
    let enhanced = EnhancedIssue(
        issue: mockIssue,
        keyFocus: "Add dark mode toggle to settings",
        allowedExtensions: [".swift"],
        searchQueries: ["settings dark mode", "theme switcher", "appearance preference"],
        estimatedComplexity: .moderate
    )

    let orchestrator = RetrievalOrchestrator(
        searchService: mockSearchService,
        embeddingService: mockEmbeddingService
    )

    let files = try await orchestrator.findRelevantFiles(
        for: enhanced,
        workspace: "/path/to/test/repo",
        topK: 5
    )

    XCTAssertEqual(files.count, 5)
    XCTAssertTrue(files.allSatisfy { $0.path.hasSuffix(".swift") })
}
```

### 8.2 Integration Tests

```swift
func testEndToEndIssueProcessing() async throws {
    // Setup test workspace
    let workspace = try createTestWorkspace(withFiles: [
        "src/Settings.swift": settingsFileContent,
        "src/Theme.swift": themeFileContent,
        "src/App.swift": appFileContent
    ])

    // Create mock issue
    let issue = Issue(
        number: 1,
        title: "Fix typo in settings",
        body: "The word 'prefrences' should be 'preferences' in Settings.swift",
        url: "...",
        comments: [],
        labels: ["bug"],
        createdAt: Date()
    )

    let service = IssueAgentService(
        searchService: searchService,
        embeddingService: embeddingService
    )

    // Process issue
    let result = try await service.processIssue(issue, workspace: workspace)

    XCTAssertEqual(result.patches.count, 1)
    XCTAssertEqual(result.patches[0].filePath, "src/Settings.swift")
    XCTAssertTrue(result.patches[0].beforeBlock.contains("prefrences"))
    XCTAssertTrue(result.patches[0].afterBlock.contains("preferences"))
}
```

### 8.3 Golden Tests

Create a corpus of real GitHub issues with expected outcomes:

```
ios/OpenAgentsCoreTests/IssueAgentTests/Fixtures/
├── issue-001-typo/
│   ├── issue.json
│   ├── workspace/
│   │   └── src/Settings.swift
│   └── expected/
│       ├── enhanced.json
│       ├── retrieved_files.json
│       └── patches.json
├── issue-002-feature/
│   └── ...
└── issue-003-refactor/
    └── ...
```

```swift
func testGoldenIssues() async throws {
    let fixtures = try loadGoldenFixtures()

    for fixture in fixtures {
        let result = try await service.processIssue(
            fixture.issue,
            workspace: fixture.workspace
        )

        // Assert enhancement quality
        XCTAssertEqual(result.enhancement.keyFocus, fixture.expected.keyFocus)

        // Assert retrieval recall (did we find the right files?)
        let expectedPaths = Set(fixture.expected.files.map { $0.path })
        let retrievedPaths = Set(result.files.map { $0.path })
        let recall = Float(expectedPaths.intersection(retrievedPaths).count) / Float(expectedPaths.count)
        XCTAssertGreaterThan(recall, 0.8)  // 80% recall target

        // Assert patch validity
        for expectedPatch in fixture.expected.patches {
            let actualPatch = result.patches.first { $0.filePath == expectedPatch.filePath }
            XCTAssertNotNil(actualPatch)
            // Could also compare patch semantics
        }
    }
}
```

---

## 9. Milestones

### Phase 1: Foundation (Week 1-2)
**Depends on:** Embeddings Plan Phase 1-3

- Core types (Issue, EnhancedIssue, PatchProposal)
- GitHub API client (fetch issues, create PRs)
- Workspace manager (git operations)
- Unit tests

### Phase 2: Enhancement & Retrieval (Week 2-4)
**Depends on:** SearchKit implementation (spec-v0.2.2)

- IssueEnhancer (Foundation Models)
- RetrievalOrchestrator (hybrid search)
- FileRanker (post-retrieval ranking)
- Integration tests

### Phase 3: Patch Generation (Week 4-6)
**Depends on:** Agent bridge setup

- PatchOrchestrator
- AgentDelegator (Claude Code, Codex)
- PatchValidator
- Patch parsing and application

### Phase 4: PR Creation (Week 6-7)

- PRBuilder (metadata generation)
- PRSubmitter (GitHub API)
- End-to-end workflow tests

### Phase 5: ACP Integration (Week 7-8)

- Bridge methods (process_issue, create_pr)
- Streaming progress updates
- Local RPC mappings for macOS app

### Phase 6: Polish & Deployment (Week 8-9)

- Golden test corpus
- Performance optimization
- Configuration UI in Settings
- Documentation

---

## 10. Success Metrics

### 10.1 Retrieval Quality

- **Recall@10:** ≥80% (top-10 includes correct files)
- **Precision@10:** ≥60% (top-10 files are relevant)
- **MRR (Mean Reciprocal Rank):** ≥0.7

### 10.2 Patch Quality

- **Syntax Validity:** ≥95% of patches are syntactically valid
- **Applicability:** ≥90% of patches apply cleanly
- **Semantic Correctness:** ≥70% of patches address the issue (manual review)

### 10.3 PR Quality

- **Acceptance Rate:** ≥40% of PRs merged (after review)
- **Time to Merge:** Median <3 days
- **Revisions Needed:** Median ≤2 iterations

### 10.4 Performance

- **Issue Processing:** <60 seconds end-to-end (fetch → patches)
- **PR Creation:** <30 seconds (patches → PR)
- **Retrieval Latency:** <3 seconds (hybrid search)

---

## 11. Comparison with Pierrebhat

| Feature | Pierrebhat | IssueAgent (OpenAgents) |
|---------|-----------|-------------------------|
| **Embeddings** | OpenAI API (1536-d) | Local MLX (384-d) |
| **Search** | Pure semantic (FAISS) | Hybrid (FTS5 + semantic + RRF) |
| **Issue Enhancement** | OpenAI completion | Foundation Models (structured) |
| **Patch Generation** | OpenAI davinci-002 | Delegate to Claude Code/Codex |
| **Planning** | None | Foundation Models + agent selection |
| **Storage** | In-memory FAISS | Persistent SQLite |
| **Platform** | Python script | Native Swift (macOS + iOS) |
| **Orchestration** | Sequential script | Actor-based async/await |
| **PR Strategy** | Fixed head branch | Per-issue branches |
| **Validation** | String match only | Syntax + semantic validation |
| **Human-in-loop** | No | Optional review gates |
| **Progress Tracking** | None | ACP streaming updates |

---

## 12. Future Extensions

### 12.1 Multi-Issue Resolution

Process multiple related issues in a single PR:

```swift
func processBatch(_ issues: [Issue]) async throws -> CreatedPR {
    // Find common files affected by multiple issues
    // Generate coordinated patches
    // Create single PR addressing all issues
}
```

### 12.2 Iterative Refinement

If initial patches fail tests:

```swift
func refinePatches(
    _ failedPatches: [PatchProposal],
    testErrors: [String]
) async throws -> [PatchProposal] {
    // Re-prompt agent with test failure context
    // Generate refined patches
    // Return for re-application
}
```

### 12.3 Learning from Feedback

Track PR outcomes to improve future patches:

```swift
struct PatchFeedback: Codable {
    var patchId: UUID
    var accepted: Bool
    var reviewerComments: String?
    var iterationsNeeded: Int
}

func updateRankingModel(feedback: [PatchFeedback]) async {
    // Fine-tune file ranking
    // Adjust agent selection heuristics
    // Update complexity estimation
}
```

### 12.4 Proactive Issue Detection

Scan codebase for potential issues:

```swift
func scanForIssues(workspace: String) async throws -> [DetectedIssue] {
    // Use Foundation Models to analyze code
    // Detect common patterns (TODOs, deprecations, anti-patterns)
    // Suggest improvements
    // Auto-create issues or PRs
}
```

---

## 13. Documentation Plan

### 13.1 ADR-0009: IssueAgent Architecture

Topics:
- Why hybrid retrieval over pure semantic
- Why agent delegation over single model
- Why Foundation Models for enhancement
- Why per-issue branches
- Why optional human review gates

### 13.2 User Guide

**Location:** `docs/issue-agent/`

Files:
- `overview.md` - What is IssueAgent, how it works
- `quickstart.md` - Process your first issue
- `configuration.md` - Config options, policy gates
- `agent-selection.md` - When to use which agent
- `troubleshooting.md` - Common issues

### 13.3 Developer Guide

**Location:** `docs/issue-agent/developer-guide.md`

Topics:
- Adding new agent providers
- Customizing retrieval ranking
- Implementing patch validators
- Testing strategies
- Golden test format

---

## 14. Open Questions

### 14.1 Agent Credentials

**Question:** How do we securely store Claude Code/Codex credentials?

**Options:**
1. macOS Keychain
2. Environment variables
3. Secure enclave
4. Per-session API keys

**Recommendation:** macOS Keychain with user opt-in.

### 14.2 Fork Management

**Question:** Use a shared bot account or user's personal fork?

**Options:**
1. Shared `openagents-bot` account (requires credentials)
2. User's personal GitHub account (OAuth flow)
3. Temporary forks (delete after PR merge)

**Recommendation:** User's personal account with OAuth.

### 14.3 Complexity Estimation

**Question:** How accurate is Foundation Models complexity estimation?

**Plan:**
- Start with FM estimates
- Track actual complexity (agent used, time taken)
- Build classifier from historical data
- Re-train periodically

### 14.4 Test Execution

**Question:** Should IssueAgent run tests before creating PRs?

**Options:**
1. Always run tests (slow, reliable)
2. Never run tests (fast, risky)
3. Run tests for complex changes only
4. Run tests as async PR check (GitHub Actions)

**Recommendation:** Option 4 (async PR check).

---

## 15. Summary

**IssueAgent** is a comprehensive issue-to-PR automation system that:

1. **Builds on local embeddings** (Phase 1-3 from embeddings plan)
2. **Leverages hybrid search** (SearchKit spec-v0.2.2)
3. **Uses Foundation Models for lightweight tasks** (enhancement, PR generation)
4. **Delegates complex reasoning to powerful agents** (Claude Code, Codex)
5. **Integrates natively with OpenAgents** (ACP, Bridge, Swift actors)
6. **Supports human-in-the-loop workflows** (review gates, draft PRs)

This design improves on pierrebhat by:
- Using persistent storage (SQLite)
- Supporting hybrid search (better recall)
- Implementing agent delegation (better patch quality)
- Providing streaming progress (better UX)
- Enabling human review (better safety)

**Next Steps:**
1. Review and approve this plan
2. Complete embeddings implementation (Phase 1-3)
3. Complete SearchKit implementation (spec-v0.2.2)
4. Begin IssueAgent Phase 1 (Core types + GitHub client)
5. Iterate based on golden test results
