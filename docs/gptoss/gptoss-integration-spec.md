# GPTOSS 20B Integration Specification

**Status:** Proposed
**Date:** 2025-11-10
**Owner:** OpenAgents Core Team
**Related Docs:**
- [GPTOSS Research](./research.md)
- [Embeddings Implementation Plan](../plans/embeddings-implementation-plan.md)
- [Issue Agent Architecture](../plans/issue-agent-architecture.md)
- ADR-0006 (Foundation Models)

---

## 1. Executive Summary

We're integrating **GPT-OSS 20B** (via MLX Swift) as a new native agent provider in OpenAgents to enable powerful on-device code generation and complex reasoning on Apple Silicon Macs. This complements Foundation Models (lightweight tasks) and external agents (Codex, Claude Code) by providing a middle ground: production-quality LLM capabilities without API costs or network dependency.

### Key Design Principles

1. **Task-appropriate allocation**: Foundation Models for routing/summaries, GPTOSS for code generation, external agents for specialized tasks
2. **Native integration**: Swift actor-based provider, not external CLI
3. **Proven patterns**: Follow MLXEmbeddingProvider (downloads, caching) and OpenAgentsLocalProvider (delegation) patterns
4. **Memory-aware**: Intelligent loading/unloading, user warnings, automatic management
5. **Incremental adoption**: Users can invoke GPTOSS directly OR let Foundation Models route automatically

### Primary Use Cases

- **Code Generation**: Functions, classes, modules, refactors
- **Documentation**: README files, API docs, code comments
- **Complex Reasoning**: Architectural decisions, multi-step planning
- **Repository Analysis**: Codebase understanding, pattern detection
- **Autonomous Work**: Overnight tasks requiring sustained reasoning

---

## 2. Architecture Overview

### 2.1 Agent Provider Hierarchy

```
┌─────────────────────────────────────────────────────────────────┐
│                    AgentProvider Protocol                        │
│  (id, displayName, capabilities, start/resume/cancel)            │
└─────────────────────────────────────────────────────────────────┘
                            ↑
                            │ Implements
        ┌───────────────────┼───────────────────┐
        │                   │                   │
┌───────────────┐  ┌────────────────┐  ┌──────────────────┐
│ OpenAgents    │  │ GPTOSS         │  │ External Agents  │
│ LocalProvider │  │ AgentProvider  │  │ (Codex, Claude)  │
│               │  │                │  │                  │
│ Foundation    │  │ MLX Swift      │  │ CLI Process      │
│ Models        │  │ Native Actor   │  │ JSON-RPC         │
│               │  │                │  │                  │
│ Role:         │  │ Role:          │  │ Role:            │
│ - Routing     │  │ - Code gen     │  │ - Specialized    │
│ - Summaries   │  │ - Reasoning    │  │ - Fallback       │
│ - Meta Q&A    │  │ - Analysis     │  │ - External tools │
│ - Tool calls  │  │ - Long-form    │  │                  │
└───────────────┘  └────────────────┘  └──────────────────┘
        │                   ↑                   ↑
        └───────────────────┴───────────────────┘
                    Delegation Flow
         (FM decides → invokes GPTOSS/Codex/Claude)
```

### 2.2 Component Structure

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
├── Agents/
│   ├── AgentProvider.swift                  # Protocol
│   ├── AgentCapabilities.swift              # Capability flags
│   ├── AgentRegistry.swift                  # Provider lookup
│   ├── OpenAgentsLocalProvider.swift        # FM-based router
│   ├── GPTOSSAgentProvider.swift            # NEW: GPTOSS 20B
│   ├── CodexAgentProvider.swift             # External: Codex
│   └── ClaudeCodeAgentProvider.swift        # External: Claude Code
│
├── GPTOSS/                                   # NEW MODULE
│   ├── GPTOSSTypes.swift                    # Request/response types
│   ├── GPTOSSModelManager.swift             # Model download/loading
│   ├── GPTOSSChatSession.swift              # Conversation management
│   ├── GPTOSSStreamAdapter.swift            # MLX → ACP conversion
│   └── GPTOSSMemoryManager.swift            # Load/unload policies
│
└── DesktopBridge/
    ├── DesktopWebSocketServer+Session.swift # Agent invocation
    └── DesktopWebSocketServer+Local.swift   # Local RPC for FM delegation
```

---

## 3. Model Specification

### 3.1 Model Selection

**Primary Model:** `mlx-community/gpt-oss-20b-MXFP4-Q8`

**Characteristics:**
- **Architecture**: Mixture-of-Experts (MoE)
- **Total Parameters**: ~21 billion
- **Active Parameters**: ~3.6 billion (per token)
- **Quantization**: MXFP4-Q8 (mixed precision: 4-bit + 8-bit)
- **Download Size**: ~12.1 GB (3 safetensors shards)
- **License**: Apache 2.0
- **Chat Template**: Harmony format (built into tokenizer)

**Performance Expectations:**
- **First Token Latency**: 1-3 seconds (varies by chip)
- **Throughput**: 10-30 tokens/sec (M1/M2/M3/M4)
- **Context Window**: Standard Harmony limit (check tokenizer config)
- **Quality**: Production-grade, suitable for code generation

### 3.2 System Requirements

**Minimum:**
- **Platform**: macOS 13.0+ (Ventura)
- **Chip**: Apple Silicon (M1 or later)
- **Memory**: 16 GB unified memory
- **Disk Space**: 25 GB free (12 GB model + 13 GB working)

**Recommended:**
- **Memory**: 24 GB+ (for long contexts, KV cache)
- **Disk**: 50 GB+ free
- **Chip**: M2 Pro/Max or M3/M4 (better performance)

**Not Supported:**
- iOS/iPadOS (model too large)
- Intel Macs (MLX optimized for Apple Silicon)

### 3.3 Model Files

Files to download from Hugging Face Hub:

```
mlx-community/gpt-oss-20b-MXFP4-Q8/
├── model-00001-of-00003.safetensors  (~4.0 GB)
├── model-00002-of-00003.safetensors  (~4.0 GB)
├── model-00003-of-00003.safetensors  (~4.1 GB)
├── config.json                        (model config)
├── tokenizer.json                     (tokenizer data)
├── tokenizer_config.json              (tokenizer config)
├── generation_config.json             (generation defaults)
└── README.md                          (model card)
```

**Cache Location:** `~/.cache/huggingface/hub/models--mlx-community--gpt-oss-20b-MXFP4-Q8/`

---

## 4. Task Allocation Strategy

### 4.1 Foundation Models (Keep)

**Characteristics:**
- **Speed**: <1 second response time
- **Token Limit**: <140 tokens
- **Temperature**: 0.1-0.15 (deterministic)
- **Availability**: macOS 15.0+, iOS 26.0+ with Apple Intelligence

**Tasks:**
- Conversation title generation (3-5 words)
- Session summaries (1-2 sentences)
- Intent extraction (structured output)
- Classification/tagging
- Meta questions ("who are you?", "what can you do?")
- Routing decisions (which agent to invoke)

**Example Prompts:**
```
Q: who are you?
A: We are OpenAgents. Ready to assist.

Q: Generate a title for this conversation: [...]
A: Fix Authentication Error

Q: Should this task use GPTOSS? [task description]
A: Yes, code generation requires GPTOSS.
```

### 4.2 GPTOSS 20B (New)

**Characteristics:**
- **Speed**: 1-3 sec first token, 10-30 tok/sec
- **Token Limit**: Unlimited (context window permitting)
- **Temperature**: 0.7 (configurable 0.0-1.0)
- **Availability**: macOS 13.0+, Apple Silicon, 16 GB+ RAM

**Tasks:**
- Code generation (functions, classes, modules)
- Code refactoring (single/multi-file)
- Documentation generation (README, API docs, comments)
- Complex reasoning (architectural decisions, planning)
- Repository analysis (codebase understanding)
- Long-form explanations (tutorials, guides)
- Autonomous work (multi-step tasks)

**Example Prompts:**
```
Generate a Swift function that validates email addresses using regex.

Refactor this authentication module to use async/await instead of callbacks.

Write a comprehensive README for the OpenAgents project explaining architecture and usage.

Analyze the repository and suggest performance optimizations for the agent coordination layer.
```

### 4.3 External Agents (Existing)

**Codex:**
- Specialized for coding tasks
- External CLI process
- Fallback when GPTOSS unavailable
- Use when: User explicitly requests Codex, or GPTOSS fails

**Claude Code:**
- Advanced reasoning and analysis
- External CLI process
- Fallback for complex tasks
- Use when: User explicitly requests Claude, or task requires Claude-specific features

### 4.4 Routing Decision Tree

```
User Prompt
    │
    ├─> Meta question? ────> Foundation Models (inline answer)
    │
    ├─> Title/summary? ────> Foundation Models
    │
    ├─> Code generation? ──> GPTOSS (if available, else Codex)
    │
    ├─> Long-form? ────────> GPTOSS (if available, else Claude Code)
    │
    ├─> Explicit "use X"? ─> Route to requested agent
    │
    └─> Complex reasoning? ─> GPTOSS (if available, else delegate)
```

**Routing Implementation:**
- Foundation Models session in OpenAgentsLocalProvider makes decision
- Uses `gptoss.generate` tool (similar to `codex.run`)
- Falls back gracefully if GPTOSS unavailable

---

## 5. Integration Pattern

### 5.1 MLX Swift Integration (Following Embeddings Pattern)

**Package Dependencies** (Package.swift):

```swift
dependencies: [
    .package(url: "https://github.com/ml-explore/mlx-swift-examples.git", from: "2.29.0"),
],
targets: [
    .target(
        name: "OpenAgentsCore",
        dependencies: [
            .product(name: "MLXEmbedders", package: "mlx-swift-examples"),  // Existing
            .product(name: "MLXLLM", package: "mlx-swift-examples"),        // NEW
            .product(name: "MLXLMCommon", package: "mlx-swift-examples"),   // NEW
            .product(name: "Tokenizers", package: "mlx-swift-examples"),    // NEW (explicit)
        ]
    ),
]
```

**Model Loading Pattern:**

```swift
import Hub
import MLXLLM
import MLXLMCommon

public actor GPTOSSModelManager {
    private var model: LLM?
    private var chatSession: ChatSession?

    public func loadModel(
        modelID: String = "mlx-community/gpt-oss-20b-MXFP4-Q8",
        progressHandler: @escaping (Double) -> Void
    ) async throws {
        guard model == nil else { return }  // Already loaded

        // Option A: Simple (auto-download)
        let configuration = ModelConfiguration(id: modelID)
        let container = try await loadModelContainer(configuration: configuration)
        // Extract model and create chat session

        // Option B: Advanced (progress tracking)
        let repo = Hub.Repo(id: modelID)
        let files = ["*.safetensors", "config.json", "tokenizer.json",
                     "tokenizer_config.json", "generation_config.json"]
        let modelDir = try await Hub.snapshot(
            from: repo,
            matching: files,
            progressHandler: { progress in
                progressHandler(progress.fractionCompleted)
            }
        )
        let loadedModel = try await loadModel(url: modelDir)
        chatSession = ChatSession(loadedModel)
        model = loadedModel
    }

    public func unloadModel() async {
        chatSession = nil
        model = nil
        print("[GPTOSS] Model unloaded, memory freed")
    }
}
```

**Streaming Pattern:**

```swift
public func streamResponse(
    prompt: String,
    sessionId: ACPSessionId,
    updateHub: SessionUpdateHub
) async throws {
    guard let chat = chatSession else {
        throw GPTOSSError.modelNotLoaded
    }

    // Generate streaming response
    let options = GenerationOptions(temperature: 0.7, maximumResponseTokens: nil)
    let stream = chat.streamResponse(to: prompt, options: options)

    // Convert to ACP format and send
    for try await token in stream {
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: token)))
        await updateHub.sendSessionUpdate(
            sessionId: sessionId,
            update: .agentMessageChunk(chunk)
        )
    }
}
```

### 5.2 Agent Provider Implementation

**GPTOSSAgentProvider.swift:**

```swift
#if os(macOS)
import Foundation
import MLXLLM
import MLXLMCommon

public final class GPTOSSAgentProvider: AgentProvider, @unchecked Sendable {
    // MARK: - Properties

    public let id: ACPSessionModeId = .gptoss_20b  // NEW enum case
    public let displayName: String = "GPTOSS 20B"
    public let capabilities: AgentCapabilities = .init(
        executionMode: .native,          // In-process Swift
        streamingMode: .acp,             // ACP format
        supportsResume: true,            // Conversation history
        supportsWorkingDirectory: true,  // Workspace awareness
        requiresExternalBinary: false,   // No CLI needed
        supportsMCP: false               // Not yet
    )

    private let modelManager: GPTOSSModelManager
    private var cancelled: Set<String> = []

    // MARK: - Initialization

    public init() {
        self.modelManager = GPTOSSModelManager()
    }

    // MARK: - Availability

    public func isAvailable() async -> Bool {
        #if os(macOS)
        // Check memory
        let memory = ProcessInfo.processInfo.physicalMemory
        guard memory >= 16_000_000_000 else {  // 16 GB minimum
            print("[GPTOSS] Insufficient memory: \(memory / 1_000_000_000) GB")
            return false
        }

        // Check if model is loaded or can be loaded
        return true
        #else
        return false  // iOS not supported
        #endif
    }

    // MARK: - Lifecycle

    public func start(
        sessionId: ACPSessionId,
        prompt: String,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws -> AgentHandle {
        // Ensure model is loaded
        try await modelManager.loadModel { progress in
            // Optionally send loading progress to UI
        }

        // Stream response
        try await modelManager.streamResponse(
            prompt: prompt,
            sessionId: sessionId,
            updateHub: updateHub
        )

        return AgentHandle(sessionId: sessionId, mode: id, isStarted: true)
    }

    public func resume(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws {
        // Resume with conversation history
        try await modelManager.streamResponse(
            prompt: prompt,
            sessionId: sessionId,
            updateHub: updateHub
        )
    }

    public func cancel(sessionId: ACPSessionId, handle: AgentHandle) async {
        cancelled.insert(sessionId.value)
        // Cancel streaming if in progress
    }
}
#endif
```

### 5.3 Foundation Models Delegation

**Add gptoss.generate Tool to OpenAgentsLocalProvider:**

```swift
#if canImport(FoundationModels)
@available(iOS 26.0, macOS 26.0, *)
extension OpenAgentsLocalProvider {
    struct FMTool_GPTOSSGenerate: Tool {
        let name = "gptoss.generate"
        let description = "Delegate a code generation or complex reasoning task to GPTOSS 20B. Use for multi-line code, documentation, or analysis requiring >140 tokens."
        typealias Output = String

        private let sessionId: ACPSessionId
        private let updateHub: SessionUpdateHub
        private let workspaceRoot: String?
        private weak var server: DesktopWebSocketServer?

        @Generable
        struct Arguments {
            @Guide(description: "The user's prompt to pass to GPTOSS")
            var user_prompt: String

            @Guide(description: "Task description (e.g., 'code generation', 'documentation', 'analysis')")
            var task_type: String?

            @Guide(description: "Workspace root directory")
            var workspace_root: String?

            @Guide(description: "Temperature (0.0-1.0, default 0.7)")
            var temperature: Double?

            @Guide(description: "Maximum tokens to generate")
            var max_tokens: Int?
        }

        func call(arguments a: Arguments) async throws -> Output {
            guard let server = self.server else {
                throw GPTOSSError.serverUnavailable
            }

            let callId = UUID().uuidString

            // Emit ACP tool_call for UI
            var args: [String: AnyEncodable] = [:]
            args["provider"] = AnyEncodable("gptoss")
            args["user_prompt"] = AnyEncodable(a.user_prompt)
            if let t = a.task_type { args["task_type"] = AnyEncodable(t) }
            if let wr = a.workspace_root ?? workspaceRoot { args["workspace_root"] = AnyEncodable(wr) }
            if let temp = a.temperature { args["temperature"] = AnyEncodable(temp) }
            if let max = a.max_tokens { args["max_tokens"] = AnyEncodable(max) }

            let call = ACPToolCallWire(call_id: callId, name: name, arguments: args)
            await updateHub.sendSessionUpdate(sessionId: sessionId, update: .toolCall(call))

            // Switch to GPTOSS mode and invoke
            await server.localSessionSetMode(sessionId: sessionId, mode: .gptoss_20b)
            let req = ACP.Agent.SessionPromptRequest(
                session_id: sessionId,
                content: [.text(.init(text: a.user_prompt))]
            )
            try? await server.localSessionPrompt(request: req)

            return "gptoss.generate dispatched"
        }
    }
}
#endif
```

---

## 6. Memory Management

### 6.1 Memory Requirements

**Baseline (Model Loaded):**
- Model weights: ~12 GB (quantized)
- KV cache: 1-4 GB (grows with context length)
- Overhead: ~500 MB (tokenizer, buffers)
- **Total**: 14-17 GB

**During Generation:**
- Additional buffers: 1-2 GB
- **Peak**: 16-19 GB

**Safety Margin:**
- 16 GB machines: Tight, may OOM with long contexts
- 24 GB machines: Comfortable
- 32 GB+ machines: Plenty of headroom

### 6.2 Load/Unload Policy

**When to Load:**
- User explicitly selects GPTOSS mode
- Foundation Models invokes gptoss.generate tool
- User clicks "Preload Model" button (optional)
- App startup (if configured to preload)

**When to Unload:**
- Manual unload button clicked
- Idle timeout (default: 10 minutes, configurable)
- Memory pressure warning from system
- App moving to background (macOS)

**Implementation:**

```swift
public actor GPTOSSMemoryManager {
    private var lastUsedAt: Date?
    private var idleTimer: Task<Void, Never>?
    private var idleTimeout: TimeInterval = 600  // 10 minutes

    public func recordUsage() {
        lastUsedAt = Date()
        resetIdleTimer()
    }

    private func resetIdleTimer() {
        idleTimer?.cancel()
        idleTimer = Task {
            try? await Task.sleep(nanoseconds: UInt64(idleTimeout * 1_000_000_000))
            await handleIdleTimeout()
        }
    }

    private func handleIdleTimeout() async {
        guard let last = lastUsedAt else { return }
        let elapsed = Date().timeIntervalSince(last)
        if elapsed >= idleTimeout {
            print("[GPTOSS] Unloading model due to idle timeout")
            await modelManager.unloadModel()
        }
    }

    public func monitorMemoryPressure() {
        // Use ProcessInfo or os_signpost to monitor memory
        // Unload proactively if approaching limits
    }
}
```

### 6.3 User Warnings

**Display in Settings:**
- Current memory usage: "14.2 GB / 24 GB"
- Model status: "Loaded" / "Not Loaded" / "Loading..."
- Warning badge if <16 GB system memory
- Recommendation to close other apps

**Runtime Warnings:**
- "Memory pressure detected. Consider unloading GPTOSS."
- "Generation quality may degrade due to limited KV cache."
- "For best performance, upgrade to 24 GB+ memory."

---

## 7. User Interface

### 7.1 Settings Screen (macOS)

**Location:** Settings → Agents → GPTOSS

**Controls:**

1. **Model Status Card**
   - Status indicator: ⚫ Not Loaded / 🟡 Loading / 🟢 Ready / 🔴 Error
   - Model name: GPTOSS 20B (MXFP4-Q8)
   - Size: 12.1 GB
   - Memory usage: 14.2 GB / 24 GB
   - Last used: 5 minutes ago

2. **Download Section** (if not downloaded)
   - Download button
   - Progress bar (resumable)
   - Disk space check: 25 GB free required
   - Estimated time: 10-30 minutes (varies by network)

3. **Load/Unload Controls**
   - Load Model button (manual load)
   - Unload Model button (free memory)
   - Auto-unload timeout: [Dropdown: Never, 5m, 10m, 30m, 1h]
   - Preload on startup: [Toggle]

4. **Generation Settings**
   - Temperature: [Slider: 0.0 - 1.0, default 0.7]
   - Top-p: [Slider: 0.0 - 1.0, default 0.9]
   - Max tokens: [Field: blank = unlimited]
   - Reasoning level: [Low / Medium / High] (experimental)

5. **System Requirements Warning** (if applicable)
   - ⚠️ Warning: Only 16 GB memory detected
   - Recommendation: 24 GB+ for best performance
   - May experience slowdowns with long contexts

### 7.2 Agent Selection UI

**Chat Interface:**

When user types message, show agent selector:
- OpenAgents (Default) - Lightweight, fast
- **GPTOSS 20B** - Code generation, reasoning (badge: LOCAL)
- Codex - Specialized coding agent
- Claude Code - Advanced analysis

**Automatic Mode:**
- Toggle: "Let OpenAgents choose" (default: ON)
- When ON: Foundation Models decides which agent to use
- When OFF: User manually selects agent

### 7.3 Progress Indicators

**Download Progress:**
```
Downloading GPTOSS 20B...
[████████████░░░░░░░░] 67% (8.1 GB / 12.1 GB)
Remaining: ~3 minutes
[Pause] [Cancel]
```

**Loading Progress:**
```
Loading GPTOSS 20B into memory...
[████████████████████] 100%
Ready to generate!
```

**Generation Progress:**
```
GPTOSS 20B generating...
Tokens: 142 | Speed: 18 tok/sec
[Cancel Generation]
```

---

## 8. Testing Strategy

### 8.1 Unit Tests

**GPTOSSAgentProviderTests.swift:**

```swift
import XCTest
@testable import OpenAgentsCore

final class GPTOSSAgentProviderTests: XCTestCase {

    func testAvailabilityOnMacOS() async {
        #if os(macOS)
        let provider = GPTOSSAgentProvider()
        let available = await provider.isAvailable()
        // Should be true on Mac with sufficient memory
        XCTAssertTrue(available || ProcessInfo.processInfo.physicalMemory < 16_000_000_000)
        #endif
    }

    func testAvailabilityOniOS() async {
        #if os(iOS)
        let provider = GPTOSSAgentProvider()
        let available = await provider.isAvailable()
        XCTAssertFalse(available, "GPTOSS should not be available on iOS")
        #endif
    }

    func testCapabilities() {
        let provider = GPTOSSAgentProvider()
        XCTAssertEqual(provider.id, .gptoss_20b)
        XCTAssertEqual(provider.displayName, "GPTOSS 20B")
        XCTAssertEqual(provider.capabilities.executionMode, .native)
        XCTAssertEqual(provider.capabilities.streamingMode, .acp)
        XCTAssertTrue(provider.capabilities.supportsResume)
        XCTAssertFalse(provider.capabilities.requiresExternalBinary)
    }

    func testModelLoadingIdempotency() async throws {
        #if os(macOS)
        let manager = GPTOSSModelManager()

        try await manager.loadModel { _ in }
        let isLoadedFirst = await manager.isModelLoaded
        XCTAssertTrue(isLoadedFirst)

        // Second load should be no-op
        try await manager.loadModel { _ in }
        let isLoadedSecond = await manager.isModelLoaded
        XCTAssertTrue(isLoadedSecond)

        await manager.unloadModel()
        let isLoadedAfterUnload = await manager.isModelLoaded
        XCTAssertFalse(isLoadedAfterUnload)
        #endif
    }
}
```

### 8.2 Integration Tests

**GPTOSSIntegrationTests.swift:**

```swift
final class GPTOSSIntegrationTests: XCTestCase {

    func testEndToEndGeneration() async throws {
        #if os(macOS)
        let provider = GPTOSSAgentProvider()
        guard await provider.isAvailable() else {
            throw XCTSkip("GPTOSS not available on this system")
        }

        let updateHub = MockSessionUpdateHub()
        let sessionId = ACPSessionId(value: "test-session")
        let context = AgentContext(workingDirectory: nil, client: nil, server: nil, metadata: nil)

        let handle = try await provider.start(
            sessionId: sessionId,
            prompt: "Write a Swift function that reverses a string.",
            context: context,
            updateHub: updateHub
        )

        XCTAssertTrue(handle.isStarted)

        // Verify updates were sent
        let updates = await updateHub.getUpdates(for: sessionId)
        XCTAssertGreaterThan(updates.count, 0)

        // Verify at least one text chunk
        let hasTextChunk = updates.contains { update in
            if case .agentMessageChunk = update { return true }
            return false
        }
        XCTAssertTrue(hasTextChunk, "Should receive at least one text chunk")
        #endif
    }

    func testFoundationModelsDelega GPTOSStion() async throws {
        #if canImport(FoundationModels)
        if #available(macOS 26.0, *) {
            let server = DesktopWebSocketServer()
            let localProvider = OpenAgentsLocalProvider()
            let gptossProvider = GPTOSSAgentProvider()

            await server.agentRegistry.register(localProvider)
            await server.agentRegistry.register(gptossProvider)

            let sessionId = ACPSessionId(value: "test-delegation")
            let updateHub = MockSessionUpdateHub()

            // Start with FM provider
            let context = AgentContext(workingDirectory: nil, client: nil, server: server, metadata: nil)
            _ = try await localProvider.start(
                sessionId: sessionId,
                prompt: "Generate a comprehensive README for a Swift package",
                context: context,
                updateHub: updateHub
            )

            // Verify FM invoked gptoss.generate tool
            let updates = await updateHub.getUpdates(for: sessionId)
            let hasGPTOSSTool = updates.contains { update in
                if case .toolCall(let call) = update {
                    return call.name == "gptoss.generate"
                }
                return false
            }
            XCTAssertTrue(hasGPTOSSTool, "FM should invoke gptoss.generate for long-form generation")
        }
        #endif
    }
}
```

### 8.3 Memory Profiling Tests

**GPTOSSMemoryTests.swift:**

```swift
final class GPTOSSMemoryTests: XCTestCase {

    func testMemoryUsage() async throws {
        #if os(macOS)
        let provider = GPTOSSAgentProvider()

        let memoryBefore = getMemoryUsage()
        print("Memory before load: \(memoryBefore / 1_000_000) MB")

        try await provider.start(sessionId: ACPSessionId(value: "mem-test"),
                                   prompt: "Test",
                                   context: AgentContext(workingDirectory: nil, client: nil, server: nil, metadata: nil),
                                   updateHub: MockSessionUpdateHub())

        let memoryAfter = getMemoryUsage()
        print("Memory after load: \(memoryAfter / 1_000_000) MB")

        let memoryDelta = memoryAfter - memoryBefore
        print("Memory delta: \(memoryDelta / 1_000_000) MB")

        // Model should use 12-17 GB
        XCTAssertGreaterThan(memoryDelta, 10_000_000_000, "Should use >10 GB")
        XCTAssertLessThan(memoryDelta, 20_000_000_000, "Should use <20 GB")
        #endif
    }

    func testIdleUnload() async throws {
        #if os(macOS)
        let manager = GPTOSSMemoryManager(idleTimeout: 1.0)  // 1 second for testing

        try await manager.loadModel()
        XCTAssertTrue(await manager.isModelLoaded)

        // Wait for idle timeout
        try await Task.sleep(nanoseconds: 1_500_000_000)  // 1.5 seconds

        // Should be unloaded
        XCTAssertFalse(await manager.isModelLoaded)
        #endif
    }

    private func getMemoryUsage() -> UInt64 {
        var info = mach_task_basic_info()
        var count = mach_msg_type_number_t(MemoryLayout<mach_task_basic_info>.size) / 4
        let result = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: 1) {
                task_info(mach_task_self_, task_flavor_t(MACH_TASK_BASIC_INFO), $0, &count)
            }
        }
        return result == KERN_SUCCESS ? info.resident_size : 0
    }
}
```

### 8.4 Golden Tests

Create a corpus of test prompts with expected characteristics:

**goldens/simple-function.txt:**
```
Input: Write a Swift function that calculates factorial.

Expected Output Characteristics:
- Contains "func factorial"
- Uses recursion or iteration
- Handles edge cases (0, 1, negative)
- Valid Swift syntax
- Length: 10-30 lines
```

**goldens/refactor.txt:**
```
Input: Refactor this callback-based code to use async/await: [code]

Expected Output Characteristics:
- Replaces completion handlers with async
- Uses try await for error propagation
- Preserves original logic
- Valid Swift 5.5+ syntax
```

Run golden tests:

```swift
func testGoldenPrompts() async throws {
    let goldens = try loadGoldenPrompts()

    for golden in goldens {
        let output = try await runGPTOSS(prompt: golden.input)

        for characteristic in golden.expectedCharacteristics {
            XCTAssertTrue(
                characteristic.matches(output),
                "Output should satisfy: \(characteristic.description)"
            )
        }
    }
}
```

---

## 9. Implementation Phases

### Phase 1: Foundation (Weeks 1-2) - Issues #1-2

**Goal:** Working GPTOSS agent with basic generation

**Tasks:**
1. Add MLXLLM dependencies to Package.swift (#1)
2. Create GPTOSSAgentProvider.swift skeleton (#2)
3. Implement GPTOSSModelManager with loading/unloading (#2)
4. Add ACPSessionModeId.gptoss_20b enum case (#2)
5. Basic text generation test (no streaming yet) (#2)

**Deliverables:**
- ✅ GPTOSS provider compiles and loads
- ✅ Can generate single response
- ✅ Unit tests pass

**Acceptance Criteria:**
- `xcodebuild test` passes on macOS
- Model downloads and caches correctly
- Memory usage within expected range (14-17 GB)

### Phase 2: Integration (Weeks 2-3) - Issues #3-5

**Goal:** GPTOSS registered and invocable via session/set_mode

**Tasks:**
1. Register GPTOSSAgentProvider in DesktopWebSocketServer (#5)
2. Implement ACP streaming (token-by-token) (#4)
3. Add download progress UI in Settings (#3)
4. Implement cancellation support (#4)
5. Integration tests (start/resume/cancel) (#5)

**Deliverables:**
- ✅ GPTOSS appears in agent selector
- ✅ Can invoke via `session.set_mode({ "mode": "gptoss_20b" })`
- ✅ Streams tokens to UI in real-time
- ✅ Download progress bar works

**Acceptance Criteria:**
- User can select GPTOSS mode
- Generation streams smoothly (<1s first token on M2+)
- Cancel button stops generation immediately

### Phase 3: Routing (Weeks 3-4) - Issues #6-7

**Goal:** Automatic delegation from Foundation Models

**Tasks:**
1. Add FMTool_GPTOSSGenerate to OpenAgentsLocalProvider (#6)
2. Update FM instructions with delegation rubric (#7)
3. Implement complexity heuristics (#7)
4. Add fallback chain (GPTOSS → Codex → Claude) (#7)
5. Integration tests for delegation (#6)

**Deliverables:**
- ✅ FM decides when to use GPTOSS
- ✅ gptoss.generate tool works
- ✅ Fallback to Codex if GPTOSS unavailable

**Acceptance Criteria:**
- FM correctly routes code generation to GPTOSS
- FM still answers meta questions inline (no tool call)
- Delegation shows tool call in UI

### Phase 4: UI & Polish (Weeks 4-5) - Issues #8-9

**Goal:** Production-ready user experience

**Tasks:**
1. Full Settings UI (status, controls, warnings) (#8)
2. Memory management (idle timeout, pressure monitoring) (#9)
3. Temperature/top-p controls (#8)
4. System requirements warnings (#8)
5. Performance optimization (#9)

**Deliverables:**
- ✅ Polished Settings screen
- ✅ Auto-unload on idle
- ✅ Memory warnings for <16 GB systems
- ✅ Performance meets targets

**Acceptance Criteria:**
- Settings UI is intuitive and informative
- Model unloads after idle timeout
- Warnings appear for low-memory systems
- Generation latency <2s first token (M2+)

### Phase 5: Advanced Features (Weeks 5-6) - Issue #10

**Goal:** Full feature set

**Tasks:**
1. Implement resume support (conversation history) (#10)
2. Add tool calling to GPTOSS (file ops, search) (#10)
3. Integrate with orchestration (autonomous work) (#10)
4. Comprehensive testing (#10)
5. Documentation (user guides, ADR) (#10)

**Deliverables:**
- ✅ GPTOSS remembers conversation context
- ✅ GPTOSS can call tools (read/write files, search)
- ✅ Works with overnight orchestration
- ✅ All tests pass, documentation complete

**Acceptance Criteria:**
- Resume maintains conversation history
- Tool calling works (validated via integration tests)
- Can run autonomous multi-hour tasks
- Documentation covers all features

### Phase 6: Launch (Week 6+)

**Goal:** Production deployment

**Tasks:**
1. Beta testing with selected users
2. Collect feedback and metrics
3. Bug fixes and refinements
4. Announce to wider audience

**Success Metrics:**
- 80%+ user satisfaction
- <5% crash rate
- 50%+ adoption among active users
- Positive feedback on generation quality

---

## 10. Success Metrics

### 10.1 Performance

**Latency:**
- First token: <2 seconds p50, <5 seconds p95 (M2+)
- Throughput: >15 tokens/sec p50, >10 tokens/sec p95
- Model loading: <60 seconds (after download)

**Memory:**
- Baseline (loaded): 14-17 GB
- Peak (generating): <20 GB
- Idle (unloaded): <500 MB

**Disk:**
- Model size: ~12.1 GB
- Cache overhead: <1 GB

### 10.2 Quality

**Code Generation:**
- Syntactic validity: >95% (compiles without errors)
- Logical correctness: >80% (passes intended tests)
- Idiomatic Swift: >70% (follows conventions)

**Documentation:**
- Completeness: >85% (covers all key sections)
- Clarity: >80% (understandable by target audience)
- Accuracy: >90% (no factual errors)

**Routing Accuracy:**
- Correct agent selection: >90% (FM chooses GPTOSS for appropriate tasks)
- False negatives: <5% (tasks that should use GPTOSS but don't)
- False positives: <10% (tasks that shouldn't use GPTOSS but do)

### 10.3 Reliability

**Availability:**
- Model loading success rate: >99% (after successful download)
- Generation success rate: >98% (no crashes/hangs)
- Download resumption success rate: >95% (after interruption)

**Errors:**
- OOM crashes: <1% (on 16 GB systems)
- Timeout failures: <2% (long generations)
- Network failures: Handled gracefully with retry

### 10.4 Adoption

**Usage:**
- Active users: >50% of macOS users (within 3 months)
- Sessions per week: >100 (per active user)
- Average generation length: 50-500 tokens

**User Satisfaction:**
- Net Promoter Score: >7/10
- Positive feedback: >80%
- Feature requests addressed: >60% (within 6 months)

---

## 11. Risk Mitigation

### 11.1 Memory Pressure

**Risk:** Model + KV cache exceed available memory, causing OOM crashes

**Mitigation:**
- Pre-flight check: Warn users with <16 GB memory
- Runtime monitoring: Use `os_signpost` to track memory usage
- Proactive unloading: Unload model if usage exceeds 80% of available memory
- KV cache limits: Cap context window on low-memory systems
- Graceful degradation: Suggest closing other apps or using external agents

**Testing:**
- Profile on 16 GB, 24 GB, 32 GB Macs
- Simulate memory pressure scenarios
- Verify auto-unload triggers correctly

### 11.2 Download Failures

**Risk:** Model download fails due to network issues, disk space, or interruption

**Mitigation:**
- Use Hub.snapshot (built-in resume support)
- Retry logic with exponential backoff
- Disk space check before download (require 25 GB free)
- Clear error messages with action items ("Check network connection", "Free disk space")
- Manual retry button

**Testing:**
- Simulate network interruptions
- Test on slow connections
- Verify resume works after app restart

### 11.3 Performance Degradation

**Risk:** Generation is too slow, hurting user experience

**Mitigation:**
- Model caching: Keep model in memory between sessions
- Background loading: Load model during app startup (optional)
- Optimize generation params: Use shorter max_tokens for initial responses
- Profile and optimize: Identify bottlenecks, optimize critical paths
- Hardware recommendations: Encourage M2+ for best performance

**Testing:**
- Benchmark on M1, M2, M3, M4
- Profile first token latency and throughput
- Test with varying context lengths
- Compare against Codex/Claude performance

### 11.4 Quality Issues

**Risk:** Generated code has errors, doesn't meet user expectations

**Mitigation:**
- Conservative temperature: Default 0.7 (not too creative)
- Output validation: Check for syntax errors before sending
- User feedback mechanism: Thumbs up/down, report issues
- Golden tests: Maintain corpus of high-quality examples
- Prompt engineering: Provide clear instructions and context

**Testing:**
- Golden test suite (50+ prompts)
- Manual review of generated code
- User studies and feedback
- Compare against GPT-4, Claude, Codex

### 11.5 Security & Safety

**Risk:** Model generates harmful code (malware, exploits, PII leaks)

**Mitigation:**
- Model card review: GPTOSS 20B is trained on open-source code, lower risk
- Output scanning: Check for common patterns (hardcoded credentials, SQL injection)
- User education: Document limitations and risks
- Sandboxing: Generated code runs in isolated environment (future)
- Audit logging: Track generations for review

**Testing:**
- Security-focused test prompts
- Manual review of edge cases
- Collaborate with security researchers

---

## 12. Open Questions

### 12.1 Tool Calling Support

**Question:** Should GPTOSS have native tool calling (file ops, search, git)?

**Options:**
1. **Yes, via MLX Function Calling** - If GPTOSS 20B supports function calling
2. **Yes, via Prompt Engineering** - Extract tool calls from text output
3. **No, Delegate to Codex** - Use GPTOSS for generation, Codex for execution

**Recommendation:** Option 2 (prompt engineering) for Phase 5, Option 1 if model supports it

### 12.2 Context Window Optimization

**Question:** How do we handle large codebases that exceed context window?

**Options:**
1. **Embeddings + Retrieval** - Use semantic search to find relevant files
2. **Sliding Window** - Keep only recent messages and relevant context
3. **Summarization** - Use Foundation Models to summarize history

**Recommendation:** Option 1 (embeddings) - leverage existing embeddings infrastructure

### 12.3 Multi-Turn Conversations

**Question:** How do we maintain conversation state across multiple prompts?

**Options:**
1. **ChatSession History** - MLX ChatSession handles internally
2. **Explicit History Injection** - Include previous messages in each prompt
3. **Tinyvex Database** - Store conversation history, load on resume

**Recommendation:** Option 1 (ChatSession) for Phase 2, Option 3 (Tinyvex) for Phase 5

### 12.4 Model Versioning

**Question:** How do we handle model updates (20B → newer version)?

**Options:**
1. **Manual Update** - User clicks "Update Model" button
2. **Auto-Update** - Check for updates on app launch
3. **Version Pinning** - Pin to specific SHA, manual upgrade

**Recommendation:** Option 3 (pinning) for stability, with manual upgrade path

### 12.5 iOS Support

**Question:** Could we support iOS with a smaller model?

**Options:**
1. **7B Model** - `mlx-community/gpt-oss-7b-MXFP4` (~4 GB)
2. **Quantized Further** - 3-bit or 2-bit quantization
3. **Cloud Fallback** - iOS calls macOS via bridge

**Recommendation:** Option 3 (cloud fallback) for now, revisit Option 1 for iPad Pro

---

## 13. Future Extensions

### 13.1 Custom Model Support

Allow users to load their own fine-tuned models:

```swift
public struct CustomGPTOSSConfig {
    var modelPath: URL  // Local .safetensors files
    var configPath: URL  // config.json
    var tokenizerPath: URL  // tokenizer.json
}
```

**Use Cases:**
- Domain-specific models (medical, legal, finance)
- Company-internal fine-tuned models
- Experimental models from research

### 13.2 Multi-Model Support

Run multiple models simultaneously:

```swift
await registry.register(GPTOSSAgentProvider(modelID: "gpt-oss-20b"))
await registry.register(GPTOSSAgentProvider(modelID: "gpt-oss-7b", id: .gptoss_7b))
```

**Benefits:**
- 7B for quick tasks, 20B for complex tasks
- Specialized models for different domains
- A/B testing different models

### 13.3 Collaborative Generation

Multiple agents working together:

```swift
// GPTOSS generates draft
let draft = try await gptoss.generate("Write a server implementation")

// Claude Code reviews and refines
let refined = try await claude.review(draft)

// User approves and commits
await commit(refined)
```

### 13.4 Learning from Feedback

Improve routing and quality over time:

```swift
struct GenerationFeedback {
    var sessionId: String
    var prompt: String
    var output: String
    var rating: Int  // 1-5 stars
    var accepted: Bool  // User kept the output
}

// Collect feedback
await feedbackStore.record(feedback)

// Adjust routing heuristics
await routingEngine.updateFromFeedback()
```

### 13.5 Agentic Workflows

GPTOSS as part of autonomous workflows:

```
Overnight Task: Implement feature X

1. GPTOSS analyzes requirements
2. GPTOSS generates implementation plan
3. GPTOSS writes code (multiple files)
4. Codex runs tests
5. Claude Code reviews code
6. GPTOSS fixes issues based on review
7. Creates PR when tests pass
```

---

## 14. Documentation Plan

### 14.1 Architecture Decision Record

**ADR-0010: GPTOSS 20B Integration**

Topics:
- Why GPTOSS over other local LLMs
- Why native provider over CLI wrapper
- Why delegation pattern (FM → GPTOSS)
- Why macOS-only (not iOS)
- Why MLX over CoreML or other frameworks
- Trade-offs and alternatives considered

### 14.2 User Documentation

**Location:** `docs/gptoss/`

Files:
- `user-guide.md` - Getting started, when to use GPTOSS
- `settings.md` - Configuration options
- `troubleshooting.md` - Common issues and solutions
- `performance.md` - Optimization tips
- `comparison.md` - GPTOSS vs Codex vs Claude Code

### 14.3 Developer Documentation

**Location:** `docs/gptoss/`

Files:
- `architecture.md` - System design, component interactions
- `integration-patterns.md` - How to add new models
- `memory-management.md` - Load/unload policies, profiling
- `testing.md` - Test strategy, golden tests

### 14.4 API Documentation

**DocC Comments:**
- All public APIs documented with examples
- Parameter descriptions
- Throws documentation
- Availability annotations

---

## 15. Summary

GPTOSS 20B integration brings powerful on-device code generation to OpenAgents, filling the gap between lightweight Foundation Models and heavyweight external agents. By following proven patterns from embeddings (MLX integration) and OpenAgentsLocalProvider (delegation), we can deliver a native, performant, memory-efficient agent that enhances the OpenAgents experience.

**Key Benefits:**
- **No API costs**: Fully local, no network dependency
- **Privacy**: Code stays on device
- **Performance**: Native Swift, ~20 tok/sec on M2+
- **Flexibility**: Direct invocation or automatic routing
- **Extensibility**: Easy to swap models or add custom models

**Next Steps:**
1. Review and approve this specification
2. Create GitHub issues from issue templates (docs/gptoss/issues/)
3. Begin Phase 1 implementation (dependencies + core provider)
4. Iterate based on testing and feedback
