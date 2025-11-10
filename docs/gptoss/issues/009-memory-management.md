# Issue #9: Memory Management and Performance Optimization

**Phase:** 4 (UI & Polish)
**Priority:** P1
**Estimated Effort:** 1 day
**Dependencies:** #2 (Provider Core), #5 (Registration)
**Related Issues:** #8 (Settings UI)

---

## Summary

Implement comprehensive memory management for GPTOSS 20B including preflight checks, idle unloading, memory pressure monitoring, and performance profiling. Ensures stable operation on 16 GB Macs and optimal performance on higher-memory systems.

## Context

From `docs/gptoss/next-steps-20251110.md`:
- Need preflight checks (total and available memory) with actionable guidance
- Auto-unload on idle; show memory watermark while loaded
- Cancellation must propagate cleanly to MLX session (no dangling tasks)
- Backpressure on UI stream to avoid buffer growth
- Dev builds should collect anonymous metrics (latency, throughput, memory)

## Acceptance Criteria

- [ ] Preflight memory checks before load (16 GB minimum, 24 GB recommended)
- [ ] Available memory check (not just total) - warn if <4 GB free
- [ ] Idle timeout auto-unload (configurable: never, 5m, 10m, 30m, 1h)
- [ ] Memory watermark displayed in UI during generation
- [ ] Memory pressure monitoring with proactive unload
- [ ] Clean cancellation (no dangling tasks, memory freed)
- [ ] Backpressure on streaming (pause if UI buffer exceeds threshold)
- [ ] Performance metrics collected (dev builds only)
- [ ] Memory profiling tests pass
- [ ] Works reliably on 16 GB Macs

## Technical Implementation

### 1. Memory Manager Actor

**File:** `ios/OpenAgentsCore/Sources/OpenAgentsCore/GPTOSS/GPTOSSMemoryManager.swift`

```swift
#if os(macOS)
import Foundation

/// Manages memory lifecycle and monitoring for GPTOSS 20B
///
/// Handles idle timeouts, memory pressure monitoring, and automatic unloading
/// to prevent OOM crashes on low-memory systems.
public actor GPTOSSMemoryManager {
    // MARK: - Properties

    private weak var modelManager: GPTOSSModelManager?
    private var config: MemoryConfig
    private var lastUsedAt: Date?
    private var idleTimer: Task<Void, Never>?
    private var memoryMonitorTimer: Task<Void, Never>?

    // MARK: - Configuration

    public struct MemoryConfig: Codable {
        public var idleTimeoutSeconds: TimeInterval = 600  // 10 minutes
        public var minimumTotalBytes: UInt64 = 16_000_000_000  // 16 GB
        public var recommendedTotalBytes: UInt64 = 24_000_000_000  // 24 GB
        public var minimumFreeBytes: UInt64 = 4_000_000_000  // 4 GB
        public var memoryPressureThresholdPercent: Double = 0.85  // Unload at 85% usage
        public var enableIdleUnload: Bool = true
        public var enableMemoryPressureMonitoring: Bool = true
    }

    // MARK: - Initialization

    public init(modelManager: GPTOSSModelManager, config: MemoryConfig = MemoryConfig()) {
        self.modelManager = modelManager
        self.config = config
    }

    // MARK: - Preflight Checks

    /// Check if system meets memory requirements
    ///
    /// - Returns: Result indicating if system is suitable, with reasons if not
    public func checkMemoryRequirements() -> MemoryCheckResult {
        let totalMemory = ProcessInfo.processInfo.physicalMemory
        let freeMemory = getFreeMemory()

        var warnings: [String] = []
        var canProceed = true

        // Check total memory
        if totalMemory < config.minimumTotalBytes {
            let totalGB = Double(totalMemory) / 1_000_000_000
            warnings.append("Only \(String(format: "%.1f", totalGB)) GB total memory (need 16 GB)")
            canProceed = false
        } else if totalMemory < config.recommendedTotalBytes {
            let totalGB = Double(totalMemory) / 1_000_000_000
            warnings.append("\(String(format: "%.1f", totalGB)) GB detected. 24 GB+ recommended for best performance.")
        }

        // Check free memory
        if freeMemory < config.minimumFreeBytes {
            let freeGB = Double(freeMemory) / 1_000_000_000
            warnings.append("Only \(String(format: "%.1f", freeGB)) GB free memory. Close other apps to free memory.")
            canProceed = false
        }

        return MemoryCheckResult(
            canProceed: canProceed,
            totalMemory: totalMemory,
            freeMemory: freeMemory,
            warnings: warnings
        )
    }

    public struct MemoryCheckResult {
        public var canProceed: Bool
        public var totalMemory: UInt64
        public var freeMemory: UInt64
        public var warnings: [String]

        public var actionableGuidance: String? {
            if !canProceed {
                if totalMemory < 16_000_000_000 {
                    return "GPTOSS 20B requires a Mac with 16 GB+ RAM. Consider using external agents (Codex/Claude Code)."
                } else {
                    return "Close memory-intensive apps (browsers, dev tools) and try again."
                }
            } else if !warnings.isEmpty {
                return warnings.first
            }
            return nil
        }
    }

    // MARK: - Usage Tracking

    /// Record that the model was just used
    ///
    /// Resets idle timer to delay auto-unload.
    public func recordUsage() {
        lastUsedAt = Date()
        resetIdleTimer()
    }

    /// Get last usage timestamp
    public var lastUsed: Date? {
        lastUsedAt
    }

    // MARK: - Idle Timer

    private func resetIdleTimer() {
        guard config.enableIdleUnload && config.idleTimeoutSeconds > 0 else {
            return
        }

        idleTimer?.cancel()
        idleTimer = Task { [weak self] in
            guard let self = self else { return }
            try? await Task.sleep(nanoseconds: UInt64(config.idleTimeoutSeconds * 1_000_000_000))
            await self.handleIdleTimeout()
        }
    }

    private func handleIdleTimeout() async {
        guard let last = lastUsedAt else { return }
        let elapsed = Date().timeIntervalSince(last)

        if elapsed >= config.idleTimeoutSeconds {
            print("[GPTOSS Memory] Unloading model due to idle timeout (\(Int(elapsed))s)")
            await modelManager?.unloadModel()
        }
    }

    /// Cancel idle timer (call when model is unloaded manually)
    public func cancelIdleTimer() {
        idleTimer?.cancel()
        idleTimer = nil
    }

    // MARK: - Memory Pressure Monitoring

    /// Start monitoring memory pressure
    ///
    /// Automatically unloads model if memory usage exceeds threshold.
    public func startMemoryPressureMonitoring() {
        guard config.enableMemoryPressureMonitoring else { return }

        memoryMonitorTimer = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 5_000_000_000)  // Check every 5 seconds
                await self?.checkMemoryPressure()
            }
        }
    }

    /// Stop monitoring memory pressure
    public func stopMemoryPressureMonitoring() {
        memoryMonitorTimer?.cancel()
        memoryMonitorTimer = nil
    }

    private func checkMemoryPressure() async {
        guard let modelManager = modelManager else { return }
        guard await modelManager.isModelLoaded else { return }

        let totalMemory = ProcessInfo.processInfo.physicalMemory
        let usedMemory = totalMemory - getFreeMemory()
        let usagePercent = Double(usedMemory) / Double(totalMemory)

        if usagePercent > config.memoryPressureThresholdPercent {
            print("[GPTOSS Memory] Memory pressure detected: \(Int(usagePercent * 100))% usage")
            print("[GPTOSS Memory] Unloading model to free memory")
            await modelManager.unloadModel()
        }
    }

    // MARK: - Memory Usage

    /// Get current memory usage information
    public func getMemoryUsage() -> MemoryUsage {
        let totalMemory = ProcessInfo.processInfo.physicalMemory
        let freeMemory = getFreeMemory()
        let usedMemory = totalMemory - freeMemory

        return MemoryUsage(
            totalBytes: totalMemory,
            usedBytes: usedMemory,
            freeBytes: freeMemory,
            usagePercent: Double(usedMemory) / Double(totalMemory)
        )
    }

    public struct MemoryUsage {
        public var totalBytes: UInt64
        public var usedBytes: UInt64
        public var freeBytes: UInt64
        public var usagePercent: Double

        public var totalGB: Double {
            Double(totalBytes) / 1_000_000_000
        }

        public var usedGB: Double {
            Double(usedBytes) / 1_000_000_000
        }

        public var freeGB: Double {
            Double(freeBytes) / 1_000_000_000
        }

        public var displayText: String {
            "\(String(format: "%.1f", usedGB)) GB / \(String(format: "%.1f", totalGB)) GB (\(Int(usagePercent * 100))%)"
        }

        public var isWarning: Bool {
            usagePercent > 0.8
        }
    }

    // MARK: - Helpers

    private func getFreeMemory() -> UInt64 {
        var stats = vm_statistics64()
        var count = mach_msg_type_number_t(MemoryLayout<vm_statistics64>.size / MemoryLayout<integer_t>.size)

        let result = withUnsafeMutablePointer(to: &stats) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                host_statistics64(mach_host_self(), HOST_VM_INFO64, $0, &count)
            }
        }

        guard result == KERN_SUCCESS else {
            return 0
        }

        let pageSize = UInt64(vm_kernel_page_size)
        let freePages = UInt64(stats.free_count)
        let inactivePages = UInt64(stats.inactive_count)

        return (freePages + inactivePages) * pageSize
    }
}
#endif // os(macOS)
```

### 2. Performance Metrics (Dev Builds)

**File:** `ios/OpenAgentsCore/Sources/OpenAgentsCore/GPTOSS/GPTOSSMetrics.swift`

```swift
#if os(macOS) && DEBUG
import Foundation

/// Performance metrics collection for development builds
///
/// Collects anonymous metrics: latency, throughput, memory usage.
/// Data stays local - no network transmission.
public actor GPTOSSMetrics {
    private var generations: [GenerationMetric] = []

    public struct GenerationMetric: Codable {
        public var timestamp: Date
        public var promptTokens: Int
        public var generatedTokens: Int
        public var firstTokenLatencyMs: Double
        public var totalTimeMs: Double
        public var tokensPerSecond: Double
        public var memoryUsedGB: Double
        public var cancelled: Bool

        public var displayText: String {
            """
            First token: \(Int(firstTokenLatencyMs))ms
            Throughput: \(String(format: "%.1f", tokensPerSecond)) tok/sec
            Memory: \(String(format: "%.1f", memoryUsedGB)) GB
            """
        }
    }

    public func recordGeneration(_ metric: GenerationMetric) {
        generations.append(metric)

        // Keep only last 100 generations
        if generations.count > 100 {
            generations.removeFirst(generations.count - 100)
        }

        // Log to console
        print("[GPTOSS Metrics] \(metric.displayText)")
    }

    public func getMetrics() -> [GenerationMetric] {
        generations
    }

    public func exportMetrics() throws -> Data {
        try JSONEncoder().encode(generations)
    }

    public func clearMetrics() {
        generations.removeAll()
    }
}
#endif
```

### 3. Clean Cancellation

Update `GPTOSSAgentProvider.cancel()`:

```swift
public func cancel(sessionId: ACPSessionId, handle: AgentHandle) async {
    cancelled.insert(sessionId.value)

    // Cancel any in-flight generation tasks
    // (Implementation depends on MLX ChatSession API)

    // Free memory if idle
    await memoryManager.recordUsage()  // Update last-used timestamp
    await memoryManager.cancelIdleTimer()  // Cancel auto-unload timer

    print("[GPTOSS] Cancelled session: \(sessionId.value)")
}
```

### 4. Stream Backpressure

**In GPTOSSAgentProvider.start()**, add buffer size monitoring:

```swift
private let maxBufferSize = 100  // Max pending chunks

await modelManager.streamGenerate(
    // ... existing params ...
    onToken: { [weak updateHub, weak self] token in
        Task {
            // Check buffer size (pseudocode - depends on SessionUpdateHub implementation)
            if await updateHub?.pendingUpdateCount(for: sessionId) ?? 0 > maxBufferSize {
                // Pause generation until buffer drains
                try? await Task.sleep(nanoseconds: 100_000_000)  // 100ms
            }

            let chunk = ACP.Client.ContentChunk(content: .text(.init(text: token)))
            await updateHub?.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(chunk))
        }
    }
)
```

## Testing

**Memory Profiling Test**:

```swift
func testMemoryManagement() async throws {
    #if os(macOS)
    let modelManager = GPTOSSModelManager()
    let memoryManager = GPTOSSMemoryManager(modelManager: modelManager)

    // Check requirements
    let check = await memoryManager.checkMemoryRequirements()
    XCTAssertTrue(check.canProceed, "Test Mac should meet requirements")

    // Record usage before load
    let beforeLoad = await memoryManager.getMemoryUsage()

    // Load model
    try await modelManager.loadModel()
    await memoryManager.recordUsage()

    // Check memory increased
    let afterLoad = await memoryManager.getMemoryUsage()
    let memoryDelta = afterLoad.usedBytes - beforeLoad.usedBytes
    print("Memory delta: \(Double(memoryDelta) / 1_000_000_000) GB")
    XCTAssertGreaterThan(memoryDelta, 10_000_000_000, "Should use >10 GB")

    // Test idle unload
    let fastConfig = GPTOSSMemoryManager.MemoryConfig(idleTimeoutSeconds: 1.0)
    let fastManager = GPTOSSMemoryManager(modelManager: modelManager, config: fastConfig)
    await fastManager.recordUsage()

    try await Task.sleep(nanoseconds: 1_500_000_000)  // 1.5 seconds

    let afterTimeout = await memoryManager.getMemoryUsage()
    let freedMemory = afterLoad.usedBytes - afterTimeout.usedBytes
    XCTAssertGreaterThan(freedMemory, 10_000_000_000, "Should free >10 GB after unload")
    #endif
}
```

**Cancellation Test**:

```swift
func testCleanCancellation() async throws {
    let provider = GPTOSSAgentProvider()
    let updateHub = MockSessionUpdateHub()
    let sessionId = ACPSessionId(value: "cancel-test")

    // Start long generation
    let handle = try await provider.start(
        sessionId: sessionId,
        prompt: "Generate 10,000 words about Swift concurrency",
        context: AgentContext(...),
        updateHub: updateHub
    )

    // Wait for first few tokens
    try await Task.sleep(nanoseconds: 500_000_000)  // 500ms

    // Cancel
    await provider.cancel(sessionId: sessionId, handle: handle)

    // Verify no more updates after cancel
    let updateCountBefore = await updateHub.getUpdates(for: sessionId).count
    try await Task.sleep(nanoseconds: 1_000_000_000)  // 1 second
    let updateCountAfter = await updateHub.getUpdates(for: sessionId).count

    XCTAssertEqual(updateCountBefore, updateCountAfter, "No new updates after cancel")
}
```

## References

- docs/gptoss/next-steps-20251110.md (Gaps section)
- Integration Spec Section 6 (Memory Management)

## Definition of Done

- [ ] Preflight checks implemented and working
- [ ] Idle auto-unload functional
- [ ] Memory pressure monitoring prevents OOM
- [ ] Clean cancellation (no dangling tasks)
- [ ] Backpressure prevents buffer overflow
- [ ] Performance metrics collected (dev builds)
- [ ] Memory profiling tests pass
- [ ] Committed with message: "Implement memory management and performance optimization for GPTOSS"
