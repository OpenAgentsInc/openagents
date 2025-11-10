# Issue #2: Implement GPTOSSAgentProvider Core

**Phase:** 1 (Foundation)
**Priority:** P0 (Blocking)
**Estimated Effort:** 1-2 days
**Dependencies:** #1 (MLXLLM dependencies must be added first)
**Related Issues:** #4 (Streaming), #5 (Registration)

---

## Summary

Implement the core `GPTOSSAgentProvider` following the `AgentProvider` protocol, with model loading/unloading via Hugging Face Hub and basic text generation capabilities. This establishes the foundation for GPTOSS 20B integration.

## Context

OpenAgents uses an `AgentProvider` protocol for pluggable agents. We need to create a new provider for GPTOSS 20B that:
- Loads the model from Hugging Face Hub with progress tracking
- Manages model lifecycle (load/unload)
- Generates text using MLX ChatSession
- Checks system requirements (macOS, memory)
- Is macOS-only (too large for iOS)

**Pattern to Follow:** `MLXEmbeddingProvider` (`ios/OpenAgentsCore/Sources/OpenAgentsCore/Embeddings/MLXEmbeddingProvider.swift`)

## Acceptance Criteria

- [ ] `GPTOSSAgentProvider.swift` created and implements `AgentProvider` protocol
- [ ] `GPTOSSModelManager.swift` created for model lifecycle management
- [ ] `GPTOSSTypes.swift` created for request/response types and errors
- [ ] `ACPSessionModeId` extended with `.gptoss_20b` case
- [ ] Model loads from Hugging Face Hub (`mlx-community/gpt-oss-20b-MXFP4-Q8`)
- [ ] Model caches in `~/.cache/huggingface/hub/`
- [ ] Basic text generation works (single prompt → single response)
- [ ] Harmony compliance: provider uses tokenizer chat template (via ChatSession) for all generations
- [ ] Channel handling: provider classifies and withholds `analysis` channel content from user‑visible UI
- [ ] Memory requirement check (16 GB minimum)
- [ ] macOS-only compilation (`#if os(macOS)`)
- [ ] Unit tests pass
- [ ] Code compiles without warnings

## Technical Details

### File Structure

Create new module:

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
└── GPTOSS/
    ├── GPTOSSTypes.swift              # Types and errors
    ├── GPTOSSModelManager.swift       # Model loading/unloading
    └── GPTOSSAgentProvider.swift      # Main provider implementation
```

Also modify:

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
└── Agents/
    └── ACPSessionModeId.swift         # Add .gptoss_20b case
```

### 1. Add Session Mode ID

**File:** `ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/ACPSessionModeId.swift`

```swift
public enum ACPSessionModeId: String, Codable, Sendable, Hashable {
    case default_mode
    case codex
    case claude_code
    case gptoss_20b      // NEW
    // ... existing cases
}
```

### 2. Types and Errors

**File:** `ios/OpenAgentsCore/Sources/OpenAgentsCore/GPTOSS/GPTOSSTypes.swift`

```swift
import Foundation

// MARK: - Configuration

public struct GPTOSSConfig: Codable, Sendable {
    public var modelID: String
    public var temperature: Double
    public var topP: Double
    public var maxTokens: Int?
    public var idleTimeoutSeconds: TimeInterval

    public static let `default` = GPTOSSConfig(
        modelID: "mlx-community/gpt-oss-20b-MXFP4-Q8",
        temperature: 0.7,
        topP: 0.9,
        maxTokens: nil,  // Unlimited
        idleTimeoutSeconds: 600  // 10 minutes
    )
}

// MARK: - Generation Options

public struct GPTOSSGenerationOptions: Sendable {
    public var temperature: Double
    public var topP: Double
    public var maxTokens: Int?

    public init(temperature: Double = 0.7, topP: Double = 0.9, maxTokens: Int? = nil) {
        self.temperature = temperature
        self.topP = topP
        self.maxTokens = maxTokens
    }
}

// MARK: - Model State

public enum GPTOSSModelState: Equatable, Sendable {
    case notLoaded
    case downloading(progress: Double)
    case loading
    case ready
    case error(String)
}

// MARK: - Errors

public enum GPTOSSError: Error, LocalizedError {
    case modelNotLoaded
    case modelUnavailable(reason: String)
    case downloadFailed(underlying: Error)
    case loadingFailed(underlying: Error)
    case generationFailed(underlying: Error)
    case insufficientMemory(available: UInt64, required: UInt64)
    case unsupportedPlatform
    case serverUnavailable
    case cancelled

    public var errorDescription: String? {
        switch self {
        case .modelNotLoaded:
            return "GPTOSS model is not loaded. Please load the model first."
        case .modelUnavailable(let reason):
            return "GPTOSS model unavailable: \(reason)"
        case .downloadFailed(let error):
            return "Model download failed: \(error.localizedDescription)"
        case .loadingFailed(let error):
            return "Model loading failed: \(error.localizedDescription)"
        case .generationFailed(let error):
            return "Text generation failed: \(error.localizedDescription)"
        case .insufficientMemory(let available, let required):
            let availGB = Double(available) / 1_000_000_000
            let reqGB = Double(required) / 1_000_000_000
            return "Insufficient memory: \(String(format: "%.1f", availGB)) GB available, \(String(format: "%.1f", reqGB)) GB required"
        case .unsupportedPlatform:
            return "GPTOSS 20B is only supported on macOS with Apple Silicon"
        case .serverUnavailable:
            return "Server reference unavailable for delegation"
        case .cancelled:
            return "Generation cancelled by user"
        }
    }
}
```

### 3. Model Manager

**File:** `ios/OpenAgentsCore/Sources/OpenAgentsCore/GPTOSS/GPTOSSModelManager.swift`

```swift
#if os(macOS)
import Foundation
import Hub
import MLXLLM
import MLXLMCommon
import Tokenizers

/// Manages the lifecycle of the GPTOSS 20B model
///
/// Handles downloading from Hugging Face Hub, loading into memory,
/// and unloading to free resources. Thread-safe via actor isolation.
public actor GPTOSSModelManager {
    // MARK: - Properties

    private let config: GPTOSSConfig
    private var modelContainer: ModelContainer?
    private var state: GPTOSSModelState = .notLoaded

    // MARK: - Constants

    private static let minimumMemoryBytes: UInt64 = 16_000_000_000  // 16 GB

    // MARK: - Initialization

    public init(config: GPTOSSConfig = .default) {
        self.config = config
    }

    // MARK: - State

    public var currentState: GPTOSSModelState {
        state
    }

    public var isModelLoaded: Bool {
        if case .ready = state {
            return true
        }
        return false
    }

    // MARK: - Loading

    /// Load the model from Hugging Face Hub
    ///
    /// Downloads if not cached, then loads into memory. Idempotent - safe to call multiple times.
    ///
    /// - Parameter progressHandler: Optional callback for download progress (0.0-1.0)
    /// - Throws: GPTOSSError if loading fails
    public func loadModel(progressHandler: ((Double) -> Void)? = nil) async throws {
        // Check if already loaded
        guard !isModelLoaded else {
            print("[GPTOSS] Model already loaded")
            return
        }

        // Check system requirements
        try checkSystemRequirements()

        // Update state
        state = .loading
        print("[GPTOSS] Loading model: \(config.modelID)")

        do {
            // Create model configuration
            let configuration = ModelConfiguration(id: config.modelID)

            // Load model container (MLX handles download/caching automatically)
            // Note: For Phase 1, we use the simple approach. Phase 3 will add Hub.snapshot for progress.
            modelContainer = try await loadModelContainer(configuration: configuration)

            state = .ready
            print("[GPTOSS] Model loaded successfully")
        } catch {
            state = .error(error.localizedDescription)
            print("[GPTOSS] Failed to load model: \(error)")
            throw GPTOSSError.loadingFailed(underlying: error)
        }
    }

    /// Unload the model from memory
    ///
    /// Frees ~12-17 GB of memory. After calling this, loadModel() must be called again.
    public func unloadModel() async {
        guard isModelLoaded else {
            print("[GPTOSS] Model not loaded, nothing to unload")
            return
        }

        modelContainer = nil
        state = .notLoaded
        print("[GPTOSS] Model unloaded, memory freed")
    }

    // MARK: - Generation

    /// Generate text response for a prompt
    ///
    /// - Parameters:
    ///   - prompt: Input text
    ///   - options: Generation parameters (temperature, top-p, max tokens)
    /// - Returns: Generated text
    /// - Throws: GPTOSSError if generation fails
    public func generate(prompt: String, options: GPTOSSGenerationOptions = GPTOSSGenerationOptions()) async throws -> String {
        guard let container = modelContainer else {
            throw GPTOSSError.modelNotLoaded
        }

        do {
            // Perform generation within the model container for thread safety
            let output = try await container.perform { model, tokenizer in
                // Harmony compliance: always use ChatSession to apply chat template
                let chat = ChatSession(model)
                // Minimal roles: developer instructions (optional) + user
                let messages: [ChatMessage] = [
                    .user(prompt)
                ]
                // For Phase 1, return a single response (non‑streaming)
                let text = try await chat.respond(messages: messages)
                return text
            }

            return output
        } catch {
            throw GPTOSSError.generationFailed(underlying: error)
        }
    }

    // MARK: - System Requirements

    private func checkSystemRequirements() throws {
        // Check platform
        #if !os(macOS)
        throw GPTOSSError.unsupportedPlatform
        #endif

        // Check memory
        let availableMemory = ProcessInfo.processInfo.physicalMemory
        guard availableMemory >= Self.minimumMemoryBytes else {
            throw GPTOSSError.insufficientMemory(
                available: availableMemory,
                required: Self.minimumMemoryBytes
            )
        }
    }
}
#endif // os(macOS)
```

### 4. Agent Provider

**File:** `ios/OpenAgentsCore/Sources/OpenAgentsCore/GPTOSS/GPTOSSAgentProvider.swift`

```swift
#if os(macOS)
import Foundation

/// GPTOSS 20B agent provider for local code generation and reasoning
///
/// Uses MLX Swift to run GPT-OSS 20B locally on Apple Silicon Macs.
/// Requires macOS 13.0+, Apple Silicon, and 16 GB+ RAM.
///
/// Example usage:
/// ```swift
/// let provider = GPTOSSAgentProvider()
/// let available = await provider.isAvailable()
/// if available {
///     let handle = try await provider.start(
///         sessionId: sessionId,
///         prompt: "Generate a Swift function...",
///         context: context,
///         updateHub: updateHub
///     )
/// }
/// ```
public final class GPTOSSAgentProvider: AgentProvider, @unchecked Sendable {
    // MARK: - AgentProvider Properties

    public let id: ACPSessionModeId = .gptoss_20b
    public let displayName: String = "GPTOSS 20B"
    public let capabilities: AgentCapabilities = .init(
        executionMode: .native,          // In-process Swift
        streamingMode: .acp,             // ACP format
        supportsResume: true,            // Conversation history
        supportsWorkingDirectory: true,  // Workspace awareness
        requiresExternalBinary: false,   // No CLI needed
        supportsMCP: false               // Not yet
    )

    // MARK: - Private Properties

    private let modelManager: GPTOSSModelManager
    private var cancelled: Set<String> = []
    private let config: GPTOSSConfig

    // MARK: - Initialization

    public init(config: GPTOSSConfig = .default) {
        self.config = config
        self.modelManager = GPTOSSModelManager(config: config)
    }

    // MARK: - Availability

    public func isAvailable() async -> Bool {
        #if os(macOS)
        // Check memory
        let memory = ProcessInfo.processInfo.physicalMemory
        guard memory >= 16_000_000_000 else {
            print("[GPTOSS] Insufficient memory: \(memory / 1_000_000_000) GB (need 16 GB)")
            return false
        }

        // Check Apple Silicon (heuristic: look for "arm" in machine type)
        var size = 0
        sysctlbyname("hw.machine", nil, &size, nil, 0)
        var machine = [CChar](repeating: 0, count: size)
        sysctlbyname("hw.machine", &machine, &size, nil, 0)
        let machineStr = String(cString: machine)
        let isAppleSilicon = machineStr.contains("arm") || machineStr.contains("ARM")

        if !isAppleSilicon {
            print("[GPTOSS] Not Apple Silicon: \(machineStr)")
        }

        return isAppleSilicon
        #else
        return false
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
        try await modelManager.loadModel()

        // Generate response (Phase 1: non‑streaming)
        // Phase 2 will implement streaming via updateHub
        let response = try await modelManager.generate(
            prompt: prompt,
            options: GPTOSSGenerationOptions(
                temperature: config.temperature,
                topP: config.topP,
                maxTokens: config.maxTokens
            )
        )

        // Send as single chunk for Phase 1 (final channel only)
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: response)))
        await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(chunk))

        return AgentHandle(sessionId: sessionId, mode: id, isStarted: true)
    }

    public func resume(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws {
        // For Phase 1, resume is the same as start (no conversation history yet)
        _ = try await start(sessionId: sessionId, prompt: prompt, context: context, updateHub: updateHub)
    }

    public func cancel(sessionId: ACPSessionId, handle: AgentHandle) async {
        cancelled.insert(sessionId.value)
        // Phase 2 will implement actual cancellation of streaming generation
        print("[GPTOSS] Cancel requested for session: \(sessionId.value)")
    }

    // MARK: - Helpers

    private func isCancelled(_ sessionId: ACPSessionId) -> Bool {
        cancelled.contains(sessionId.value)
    }
}
#endif // os(macOS)
```

## Testing Steps

### Unit Tests

**File:** `ios/OpenAgentsCoreTests/GPTOSS/GPTOSSAgentProviderTests.swift`

```swift
import XCTest
@testable import OpenAgentsCore

final class GPTOSSAgentProviderTests: XCTestCase {

    func testProviderProperties() {
        let provider = GPTOSSAgentProvider()
        XCTAssertEqual(provider.id, .gptoss_20b)
        XCTAssertEqual(provider.displayName, "GPTOSS 20B")
        XCTAssertEqual(provider.capabilities.executionMode, .native)
        XCTAssertEqual(provider.capabilities.streamingMode, .acp)
    }

    func testAvailabilityOnMacOS() async {
        #if os(macOS)
        let provider = GPTOSSAgentProvider()
        let available = await provider.isAvailable()
        // Should be true on Mac with sufficient memory and Apple Silicon
        // May be false on Intel or low-memory systems
        print("[Test] GPTOSS available: \(available)")
        #endif
    }

    func testModelManagerLoadUnload() async throws {
        #if os(macOS)
        let manager = GPTOSSModelManager()

        XCTAssertFalse(await manager.isModelLoaded)

        // Load model (may take 30-60 seconds on first run)
        try await manager.loadModel()
        XCTAssertTrue(await manager.isModelLoaded)

        // Unload
        await manager.unloadModel()
        XCTAssertFalse(await manager.isModelLoaded)
        #endif
    }

    func testGeneration() async throws {
        #if os(macOS)
        let manager = GPTOSSModelManager()
        try await manager.loadModel()

        let output = try await manager.generate(
            prompt: "Say hello",
            options: GPTOSSGenerationOptions()
        )

        XCTAssertFalse(output.isEmpty)
        print("[Test] Generated output: \(output)")
        #endif
    }
}
```

### Manual Testing

1. **Build and run tests:**
   ```bash
   cd ios
   xcodebuild test -workspace OpenAgents.xcworkspace -scheme OpenAgents -sdk macosx -destination 'platform=macOS'
   ```

2. **Check model download** (first run):
   ```bash
   ls -lh ~/.cache/huggingface/hub/models--mlx-community--gpt-oss-20b-MXFP4-Q8/
   # Should show ~12 GB of files
   ```

3. **Monitor memory usage:**
   - Open Activity Monitor
   - Run tests
   - Observe memory spike (~14-17 GB) when model loads
   - Verify memory drops after unload

## Performance Expectations

**Phase 1 (Non-streaming):**
- Model loading: 30-60 seconds (first run), 5-10 seconds (cached)
- Memory usage: 14-17 GB (loaded), <500 MB (unloaded)
- Generation: Placeholder only (real generation in Phase 2)

**Phase 2 (Streaming):**
- First token: 1-3 seconds
- Throughput: 10-30 tokens/sec (varies by chip)

## Rollback Plan

If implementation fails:
1. Revert all GPTOSS files
2. Revert ACPSessionModeId changes
3. Keep MLX dependencies (safe to have unused)
4. Review error messages and adjust approach

## References

- **AgentProvider Protocol:** `ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/AgentProvider.swift`
- **MLXEmbeddingProvider (pattern to follow):** `ios/OpenAgentsCore/Sources/OpenAgentsCore/Embeddings/MLXEmbeddingProvider.swift`
- **OpenAgentsLocalProvider (another native provider):** `ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/OpenAgentsLocalProvider.swift`
- **MLX Swift Examples:** https://github.com/ml-explore/mlx-swift-examples
- **GPTOSS Research:** `docs/gptoss/research.md`
- **Integration Spec:** `docs/gptoss/gptoss-integration-spec.md` (Sections 5.2, 5.3)

## Notes

- **Phase 1 Scope:** Model loading + basic generation (non-streaming)
- **Phase 2 Scope:** Token-by-token streaming (#4)
- **Phase 3 Scope:** Download progress UI (#3)
- Generation is currently a placeholder - real implementation in Phase 2
- No conversation history yet - will be added in Phase 5

## Definition of Done

- [ ] All 4 files created (Types, ModelManager, AgentProvider, + ACPSessionModeId update)
- [ ] Code compiles without errors or warnings on macOS
- [ ] Unit tests added and passing
- [ ] Model downloads and caches on first run
- [ ] Model loads into memory successfully
- [ ] Basic generation returns text (even if placeholder)
- [ ] Memory usage within expected range (14-17 GB)
- [ ] Model unloads and frees memory
- [ ] Changes committed with message: "Implement GPTOSSAgentProvider core (Phase 1)"
- [ ] Ready for Issue #4 (ACP streaming integration)
