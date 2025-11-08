# macOS: Foundation Models Worker

**Phase:** 1 - MVP
**Component:** macOS App
**Priority:** P0 (Critical - Core marketplace functionality)
**Estimated Effort:** 3-4 weeks

## Summary

Implement a complete Foundation Models-based compute worker on macOS that accepts marketplace jobs (NIP-90), executes them using Apple's on-device Foundation Models, and publishes results back to Nostr. This is the core compute provider for the OpenAgents marketplace MVP.

## Motivation

The marketplace needs compute providers. For MVP, macOS devices running OpenAgents will:
- **Accept jobs** from Nostr marketplace (NIP-90 requests)
- **Execute using Foundation Models** (on-device, privacy-first)
- **Enforce AUP** (using Policy Module from issue #009)
- **Publish results** (NIP-90 results + feedback)
- **Handle payments** (BOLT11 invoices in Phase 2)

This transforms a Mac into a marketplace compute node, providing:
- ✅ On-device intelligence (Foundation Models)
- ✅ Privacy (no cloud API calls)
- ✅ Low latency (local processing)
- ✅ Apple compliance (AUP enforcement)

## Acceptance Criteria

### Job Queue & Scheduler
- [ ] Job queue (FIFO with priority support)
- [ ] Configurable concurrency (max in-flight jobs: default 2)
- [ ] Job acceptance criteria (kind support, resource availability)
- [ ] Job rejection with feedback (unsupported kind, overloaded, AUP violation)
- [ ] Timeout handling (cancel long-running jobs)
- [ ] Graceful cancellation (in-progress jobs)

### Nostr Integration
- [ ] Subscribe to job requests on configured relays (kind:5000-5999)
- [ ] Filter jobs by supported kinds (from config)
- [ ] Publish feedback events (kind:7000):
  - `payment-required` (if bid < min price)
  - `processing` (job accepted, work started)
  - `error` (job failed)
  - `success` (job completed)
- [ ] Publish result events (kind:6000-6999)
- [ ] Handle encrypted job params (NIP-04 decrypt)
- [ ] Encrypt results if requested (NIP-04)

### Foundation Models Execution
- [ ] Session management (create/reuse sessions per job kind)
- [ ] System instructions per job kind (from Job Schema Registry)
- [ ] Generate response using Foundation Models
- [ ] Streaming support (optional, for real-time feedback)
- [ ] Error handling (model unavailable, generation failed)
- [ ] Token accounting (track usage per job)

### Policy Enforcement
- [ ] Check all jobs against Policy Module (issue #009)
- [ ] Reject prohibited jobs with reason (AUP violation)
- [ ] Log policy checks for audit
- [ ] Configurable allowlist (job kinds to accept)

### Job Kind Support (MVP)
- [ ] **Text Summarization** (kind:5100)
- [ ] **Q&A / RAG** (kind:5104)
- [ ] **Code Review** (kind:5103)
- [ ] **Data Extraction** (kind:5108)
- [ ] **Sentiment Analysis** (kind:5106)

### Configuration
- [ ] Enable/disable worker
- [ ] Supported job kinds (allowlist)
- [ ] Min bid price per job kind (msats)
- [ ] Max concurrent jobs
- [ ] Max job timeout (seconds)
- [ ] Relay list for marketplace
- [ ] Nostr identity (provider pubkey)

### Observability
- [ ] Job ledger (all jobs: accepted, rejected, completed, failed)
- [ ] Stats: jobs completed, success rate, avg duration
- [ ] Foundation Models availability status
- [ ] Current queue depth
- [ ] Logs (OSLog with configurable level)

## Technical Design

### Module Structure

```swift
// ios/OpenAgentsCore/Sources/OpenAgentsCore/Worker/

WorkerService.swift          // Main worker service
JobQueue.swift               // Job queue + scheduler
JobExecutor.swift            // Execute jobs with Foundation Models
WorkerConfig.swift           // Worker configuration
WorkerStats.swift            // Stats tracking
JobLedger.swift              // Job history/audit
```

### Core Types

```swift
// WorkerService.swift

import Foundation
import OpenAgentsCore

/// Foundation Models worker service
@MainActor
public class WorkerService: ObservableObject {
    @Published public var isRunning = false
    @Published public var stats: WorkerStats
    @Published public var queueDepth = 0

    private let config: WorkerConfig
    private let nostrClient: NostrRelayManager
    private let policyEnforcer: PolicyEnforcer
    private let jobQueue: JobQueue
    private let executor: JobExecutor
    private let ledger: JobLedger

    public init(
        config: WorkerConfig,
        nostrClient: NostrRelayManager,
        policyEnforcer: PolicyEnforcer = PolicyEnforcer()
    ) {
        self.config = config
        self.nostrClient = nostrClient
        self.policyEnforcer = policyEnforcer
        self.jobQueue = JobQueue(maxConcurrent: config.maxConcurrentJobs)
        self.executor = JobExecutor()
        self.ledger = JobLedger()
        self.stats = WorkerStats()
    }

    // MARK: - Lifecycle

    /// Start worker (subscribe to job requests)
    public func start() async throws {
        guard !isRunning else { return }

        // Subscribe to job requests (kind:5000-5999 for supported kinds)
        let supportedKinds = config.supportedJobKinds.map { $0.rawValue }
        try await nostrClient.subscribe(
            id: "worker-jobs",
            filters: [NostrFilter(kinds: supportedKinds)],
            relays: Set(config.relays)
        ) { [weak self] event, relayURL in
            await self?.handleJobRequest(event, from: relayURL)
        }

        isRunning = true
        logger.info("Worker started, listening for jobs: \(supportedKinds)")

        // Start job processing loop
        Task {
            await processJobs()
        }
    }

    /// Stop worker
    public func stop() {
        guard isRunning else { return }
        nostrClient.unsubscribe(id: "worker-jobs")
        jobQueue.cancelAll()
        isRunning = false
        logger.info("Worker stopped")
    }

    // MARK: - Job Handling

    private func handleJobRequest(_ event: NostrEvent, from relayURL: String) async {
        logger.debug("Received job request: \(event.id) from \(relayURL)")

        // Parse job request (NIP-90)
        guard let jobRequest = try? DVMJobRequest.from(event: event) else {
            logger.error("Failed to parse job request: \(event.id)")
            return
        }

        // Check policy (AUP enforcement)
        let policyResult = await policyEnforcer.check(job: jobRequest)
        switch policyResult {
        case .denied(let reason, let category):
            logger.warning("Job denied: \(reason) (\(category))")
            await publishFeedback(
                jobId: event.id,
                status: "error",
                message: "Job rejected: \(reason)"
            )
            ledger.record(job: jobRequest, result: .rejected(reason: reason))
            return

        case .flagged(let reason):
            logger.info("Job flagged: \(reason) (allowing)")
            // Continue but log

        case .allowed:
            break
        }

        // Check bid (if configured)
        if let minBid = config.minBidPerKind[jobRequest.kind],
           let jobBid = jobRequest.bid,
           jobBid < minBid {
            logger.info("Job bid too low: \(jobBid) < \(minBid)")
            await publishFeedback(
                jobId: event.id,
                status: "payment-required",
                amount: minBid
            )
            ledger.record(job: jobRequest, result: .rejected(reason: "Bid too low"))
            return
        }

        // Accept job
        await publishFeedback(jobId: event.id, status: "processing")

        // Enqueue
        let job = Job(
            id: event.id,
            request: jobRequest,
            customerPubkey: event.pubkey,
            receivedAt: Date()
        )
        jobQueue.enqueue(job)
        queueDepth = jobQueue.depth
        ledger.record(job: jobRequest, result: .accepted)
    }

    private func processJobs() async {
        while isRunning {
            // Get next job from queue
            guard let job = await jobQueue.dequeue() else {
                try? await Task.sleep(nanoseconds: 100_000_000) // 100ms
                continue
            }

            queueDepth = jobQueue.depth

            // Execute job
            do {
                let result = try await executor.execute(job: job, config: config)
                await publishResult(job: job, result: result)
                await publishFeedback(jobId: job.id, status: "success")

                stats.recordCompletion(duration: result.duration)
                ledger.record(job: job.request, result: .completed(duration: result.duration))

            } catch {
                logger.error("Job execution failed: \(error)")
                await publishFeedback(
                    jobId: job.id,
                    status: "error",
                    message: error.localizedDescription
                )

                stats.recordFailure()
                ledger.record(job: job.request, result: .failed(error: error))
            }

            queueDepth = jobQueue.depth
        }
    }

    // MARK: - Nostr Publishing

    private func publishFeedback(
        jobId: String,
        status: String,
        message: String? = nil,
        amount: Int64? = nil
    ) async {
        var tags: [[String]] = [
            ["e", jobId],
            ["status", status]
        ]

        if let message = message {
            tags.append(["message", message])
        }

        if let amount = amount {
            tags.append(["amount", "\(amount)"])
            // TODO: Add BOLT11 invoice (Phase 2)
        }

        let event = try! NostrEvent.sign(
            privateKey: config.workerPrivateKey,
            created_at: Int64(Date().timeIntervalSince1970),
            kind: 7000,  // Feedback
            tags: tags,
            content: message ?? ""
        )

        let _ = await nostrClient.broadcast(event)
    }

    private func publishResult(job: Job, result: JobResult) async {
        let resultKind = job.request.kind + 1000  // Result kind = request kind + 1000

        var tags: [[String]] = [
            ["request", job.id],
            ["p", job.customerPubkey]
        ]

        // Add input tags (echo)
        for input in job.request.inputs {
            switch input {
            case .text(let content):
                tags.append(["i", content, "text"])
            case .url(let url):
                tags.append(["i", url, "url"])
            case .event(let id):
                tags.append(["i", id, "event"])
            case .job(let id):
                tags.append(["i", id, "job"])
            }
        }

        // Add amount (if bid was provided)
        if let bid = job.request.bid {
            tags.append(["amount", "\(bid)"])
            // TODO: Add BOLT11 invoice (Phase 2)
        }

        // Content (result)
        let content: String
        if job.request.encrypted {
            // Encrypt result for customer (NIP-04)
            content = try! NostrEncryption.encrypt(
                content: result.output,
                privateKey: config.workerPrivateKey,
                recipientPubkey: job.customerPubkey
            )
        } else {
            content = result.output
        }

        let event = try! NostrEvent.sign(
            privateKey: config.workerPrivateKey,
            created_at: Int64(Date().timeIntervalSince1970),
            kind: resultKind,
            tags: tags,
            content: content
        )

        let _ = await nostrClient.broadcast(event)
        logger.info("Published result for job \(job.id)")
    }
}

// MARK: - Job

struct Job {
    let id: String
    let request: DVMJobRequest
    let customerPubkey: String
    let receivedAt: Date
}

struct JobResult {
    let output: String
    let metadata: [String: String]
    let duration: TimeInterval
}
```

```swift
// JobQueue.swift

actor JobQueue {
    private var queue: [Job] = []
    private var inFlight: Set<String> = []
    private let maxConcurrent: Int

    var depth: Int {
        queue.count
    }

    init(maxConcurrent: Int) {
        self.maxConcurrent = maxConcurrent
    }

    func enqueue(_ job: Job) {
        queue.append(job)
    }

    func dequeue() -> Job? {
        guard inFlight.count < maxConcurrent, !queue.isEmpty else {
            return nil
        }

        let job = queue.removeFirst()
        inFlight.insert(job.id)
        return job
    }

    func complete(_ jobId: String) {
        inFlight.remove(jobId)
    }

    func cancelAll() {
        queue.removeAll()
        inFlight.removeAll()
    }
}
```

```swift
// JobExecutor.swift

import Foundation

struct JobExecutor {
    /// Execute job using Foundation Models
    func execute(job: Job, config: WorkerConfig) async throws -> JobResult {
        let startTime = Date()

        // Get job schema
        guard let schema = JobSchemaRegistry.shared.schema(for: job.request.kind) else {
            throw ExecutorError.unsupportedJobKind(job.request.kind)
        }

        // Check Foundation Models availability
        guard SystemLanguageModel.default.availability == .available else {
            throw ExecutorError.foundationModelsUnavailable
        }

        // Create session with instructions
        let instructions = systemInstructionsFor(jobKind: job.request.kind)
        let session = try SystemLanguageModel.default.createSession(instructions: instructions)

        // Build prompt from job inputs
        let prompt = buildPrompt(from: job.request, schema: schema)

        // Generate response
        let response = try await session.generateResponse(prompt: prompt)

        let duration = Date().timeIntervalSince(startTime)

        return JobResult(
            output: response,
            metadata: [
                "model": "foundation-models",
                "job_kind": "\(job.request.kind)",
                "duration_ms": "\(Int(duration * 1000))"
            ],
            duration: duration
        )
    }

    private func systemInstructionsFor(jobKind: JobKind) -> String {
        switch jobKind {
        case .textSummarization:
            return "You are a text summarization assistant. Provide concise, accurate summaries of the given text."
        case .qaRag:
            return "You are a helpful Q&A assistant. Answer questions accurately and concisely based on the provided context."
        case .codeReview:
            return "You are a code review assistant. Review code for bugs, style, performance, and best practices."
        case .dataExtraction:
            return "You are a data extraction assistant. Extract structured data from unstructured text."
        case .sentimentAnalysis:
            return "You are a sentiment analysis assistant. Classify the sentiment of the given text as positive, negative, or neutral."
        default:
            return "You are a helpful assistant."
        }
    }

    private func buildPrompt(from request: DVMJobRequest, schema: JobSchema) -> String {
        var prompt = ""

        // Add inputs
        for input in request.inputs {
            switch input {
            case .text(let content):
                prompt += content + "\n\n"
            case .url(let url):
                prompt += "URL: \(url)\n\n"
            case .event(let id):
                prompt += "Nostr event: \(id)\n\n"
            case .job(let id):
                prompt += "Previous job: \(id)\n\n"
            }
        }

        // Add params as instructions
        if let params = request.params {
            prompt += "Parameters:\n"
            for (key, value) in params {
                prompt += "- \(key): \(value)\n"
            }
            prompt += "\n"
        }

        return prompt
    }

    enum ExecutorError: Error, LocalizedError {
        case unsupportedJobKind(JobKind)
        case foundationModelsUnavailable
        case generationFailed(Error)
        case timeoutExceeded

        var errorDescription: String? {
            switch self {
            case .unsupportedJobKind(let kind):
                return "Unsupported job kind: \(kind.name)"
            case .foundationModelsUnavailable:
                return "Foundation Models not available on this device"
            case .generationFailed(let error):
                return "Generation failed: \(error.localizedDescription)"
            case .timeoutExceeded:
                return "Job execution exceeded timeout"
            }
        }
    }
}
```

```swift
// WorkerConfig.swift

public struct WorkerConfig: Codable {
    public var enabled: Bool = false
    public var supportedJobKinds: [JobKind] = [
        .textSummarization,
        .qaRag,
        .codeReview,
        .dataExtraction,
        .sentimentAnalysis
    ]
    public var minBidPerKind: [JobKind: Int64] = [:]  // msats
    public var maxConcurrentJobs: Int = 2
    public var maxJobTimeout: TimeInterval = 300  // 5 minutes
    public var relays: [String] = [
        "wss://relay.damus.io",
        "wss://nos.lol"
    ]
    public var workerPrivateKey: String  // Nostr private key (hex)

    public static let `default` = WorkerConfig(
        workerPrivateKey: ""  // Must be set by user
    )
}
```

```swift
// WorkerStats.swift

public struct WorkerStats: Codable {
    public var jobsCompleted: Int = 0
    public var jobsFailed: Int = 0
    public var totalDuration: TimeInterval = 0

    public var successRate: Double {
        let total = jobsCompleted + jobsFailed
        guard total > 0 else { return 0 }
        return Double(jobsCompleted) / Double(total)
    }

    public var avgDuration: TimeInterval? {
        guard jobsCompleted > 0 else { return nil }
        return totalDuration / Double(jobsCompleted)
    }

    mutating func recordCompletion(duration: TimeInterval) {
        jobsCompleted += 1
        totalDuration += duration
    }

    mutating func recordFailure() {
        jobsFailed += 1
    }
}
```

## Dependencies

### OpenAgents Dependencies
- **Issue #001**: Nostr Client Library (relay subscriptions, event publishing)
- **Issue #004**: Job Schema Registry (job kind definitions, schemas)
- **Issue #009**: Policy & Safety Module (AUP enforcement)
- **ADR-0006**: Foundation Models integration

### System Frameworks
- **Foundation**: Core types
- **LanguageModel**: Foundation Models framework (iOS 26+, macOS 15+)

## Testing Requirements

### Unit Tests
- [ ] Job queue FIFO ordering
- [ ] Concurrency limits enforced
- [ ] Policy checks reject prohibited jobs
- [ ] Prompt building from job inputs
- [ ] System instructions per job kind
- [ ] Stats calculation (success rate, avg duration)

### Integration Tests
- [ ] Subscribe to Nostr job requests
- [ ] Execute job with Foundation Models
- [ ] Publish result to Nostr
- [ ] Publish feedback events
- [ ] Handle encrypted params/results (NIP-04)

### E2E Tests
- [ ] Full job flow:
  1. Receive job request from relay
  2. Check policy (allowed)
  3. Execute with Foundation Models
  4. Publish result
  5. Verify result on relay

## Apple Compliance Considerations

### DPLA & Foundation Models AUP

**DPLA §3.3.8(I)**
- ✅ **Compliant**: Policy Module (issue #009) enforces AUP
- ✅ Prohibited jobs rejected before reaching Foundation Models
- ✅ Audit trail (JobLedger) for compliance review

**Acceptable Use Requirements**
- ✅ No regulated healthcare/legal/financial (filtered by Policy Module)
- ✅ No academic textbooks
- ✅ No guardrail circumvention
- ✅ Content safety (violence, explicit, fraud filtered)

### App Store Review Guidelines

**ASRG 2.4.2 (Background Processes)**
- ✅ **Compliant**: Worker runs in foreground (user-initiated)
- ✅ User can stop worker at any time
- ⚠️ **Note**: Thermal/power monitoring (issue #021) will throttle/pause

**ASRG 2.5.2 (No Downloaded Code)**
- ✅ **Compliant**: Job params are **data**, not code
- ✅ Foundation Models executes prompts, not scripts

## Reference Links

### Apple Documentation
- **Foundation Models**: https://developer.apple.com/documentation/languagemodel
- **AUP**: https://developer.apple.com/apple-intelligence/acceptable-use-requirements-for-the-foundation-models-framework/
- **WWDC 2025 - Meet Foundation Models**: https://developer.apple.com/videos/play/wwdc2025/286/

### Nostr
- **NIP-90**: https://github.com/nostr-protocol/nips/blob/master/90.md

### OpenAgents
- **Issue #001**: Nostr Client Library
- **Issue #004**: Job Schema Registry
- **Issue #009**: Policy & Safety Module
- **ADR-0006**: Foundation Models

## Success Metrics

- [ ] Execute 10+ jobs successfully
- [ ] AUP policy checks prevent prohibited jobs
- [ ] Avg job latency <10s for summarization
- [ ] Success rate >95%
- [ ] Foundation Models availability checked
- [ ] Published results verified on relay

## Notes

- **Foreground Only**: Worker should be user-visible (status in menu bar)
- **Thermal Awareness**: Monitor heat; pause if overheating (Phase 3)
- **Session Reuse**: Cache FM sessions per job kind for performance
- **Logging**: Use OSLog for production debugging

## Future Enhancements (Post-MVP)

- Multi-model routing (Foundation Models → MLX → Ollama) - Phase 3
- Job chaining (NIP-90 Appendix 1)
- Streaming results (real-time updates)
- Tool calling within jobs (NIP-90 + Foundation Models tools)
- Reputation system integration (issue #022)
- Resource management (thermal, power) - issue #021
