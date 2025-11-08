# Marketplace Payment Coordinator (Job ↔ Invoice Matching)

**Phase:** 2 - Payments
**Component:** OpenAgentsCore (Shared), macOS Worker
**Priority:** P0 (Critical - Links jobs to payments)
**Estimated Effort:** 1-2 weeks
**Dependencies:** Issues #010, #013 (Wallets), #011 (Job Creation), #014 (Bidding Engine)

## Summary

Implement the **Payment Coordinator**, the critical component that correlates NIP-90 job requests with Lightning invoice payments, tracks payment status, and triggers job execution when payment is confirmed. This is the "glue" that makes the decentralized marketplace function automatically.

## Motivation

The OpenAgents marketplace has two parallel communication channels:

1. **Nostr**: Job requests, bids, results (NIP-90 Data Vending Machine)
2. **Lightning**: Payments (BOLT11 invoices, Spark addresses)

These must be **synchronized**:

- **Buyer flow**: Submit job → Receive invoice → Pay invoice → Job executes → Receive result
- **Provider flow**: See job → Submit bid with invoice → Wait for payment → Execute job → Publish result

The **Payment Coordinator** ensures:
- ✅ Jobs are matched to invoices (via metadata/tags)
- ✅ Payment status is tracked (unpaid → pending → confirmed)
- ✅ Jobs execute only after payment confirmation
- ✅ Failed payments trigger refund flows
- ✅ Partial payments and overpayments are handled correctly

Without this coordinator, the marketplace is **manual** (users must match jobs to payments themselves).

## Acceptance Criteria

### Core Coordination Logic

- [ ] **Match jobs to invoices** using job ID embedded in invoice metadata
- [ ] **Track payment status** for each job (unpaid, pending, confirmed, failed)
- [ ] **Trigger job execution** when payment is confirmed
- [ ] **Publish NIP-90 feedback events** for payment status updates
- [ ] **Handle payment timeouts** (expire unpaid jobs after X minutes)
- [ ] **Support multiple payment methods** (BOLT11, Spark addresses, BTC on-chain)

### Provider (macOS Worker) Side

- [ ] **Generate invoices** for accepted jobs (via BiddingEngine)
- [ ] **Embed job ID** in invoice metadata/description
- [ ] **Listen for payment events** from SparkWalletManager
- [ ] **Match incoming payments** to pending jobs
- [ ] **Start job execution** when payment confirmed
- [ ] **Reject unpaid jobs** after timeout (configurable, default 15 minutes)

### Buyer (iOS) Side

- [ ] **Display payment status** in job details (awaiting payment, pending, confirmed)
- [ ] **Notify user** when payment is required
- [ ] **Handle payment failures** (insufficient balance, network errors)
- [ ] **Retry payment** if initial attempt fails

### Edge Cases

- [ ] **Overpayment**: If buyer pays more than invoice amount (accept and execute)
- [ ] **Partial payment**: If buyer pays less (reject and notify)
- [ ] **Late payment**: If payment arrives after timeout (refund or accept, configurable)
- [ ] **Duplicate payments**: If buyer pays twice (refund second payment)
- [ ] **Job cancellation**: If buyer cancels before payment (mark invoice as void)

### Data Persistence

- [ ] **Store job-invoice mappings** in local database (Core Data or SQLite)
- [ ] **Store payment status** per job
- [ ] **Persist across app restarts** (coordinator resumes tracking)

### Testing

- [ ] **Unit tests** for job-invoice matching logic
- [ ] **Integration tests** with mock wallet and Nostr client
- [ ] **End-to-end tests** with testnet (real Spark backend)
- [ ] **Test timeout handling** (simulate delayed/missing payments)
- [ ] **Test refund flows** (overpayment, cancellation)

### Monitoring

- [ ] **Log all payment events** (received, confirmed, failed)
- [ ] **Metrics**: Payment success rate, average time to confirmation
- [ ] **Alerts**: Failed payments, stuck jobs, refund requests

## Technical Design

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Marketplace Flow                         │
│                                                             │
│  Buyer (iOS)                        Provider (macOS)       │
│      │                                    │                 │
│      │  1. Submit NIP-90 job             │                 │
│      ├────────────────────────────────────>                │
│      │                                    │                 │
│      │  2. Bid + Invoice (NIP-90 feedback)                 │
│      <────────────────────────────────────┤                │
│      │                                    │                 │
│      │  3. Pay invoice (Lightning)       │                 │
│      │        via SparkWalletManager     │                 │
│      │                                    │                 │
│      │  4. Payment event ──────────────> │                 │
│      │     (Spark SDK EventListener)     │                 │
│      │                                    │                 │
│      │                     PaymentCoordinator matches      │
│      │                     payment → job ID                │
│      │                     Triggers job execution          │
│      │                                    │                 │
│      │  5. Result event (NIP-90)         │                 │
│      <────────────────────────────────────┤                │
│                                                             │
└─────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────┐
│         PaymentCoordinator (Actor)                   │
│                                                      │
│  - jobInvoiceMap: [JobID: InvoiceID]                │
│  - paymentStatus: [JobID: PaymentStatus]            │
│  - walletManager: SparkWalletManager                │
│  - nostrClient: NostrClient                         │
│                                                      │
│  + registerJobInvoice(jobId, invoiceId)             │
│  + handlePaymentReceived(payment)                   │
│  + checkJobPaymentStatus(jobId) -> PaymentStatus   │
│  + expireUnpaidJobs()                               │
│  + refundPayment(paymentId, reason)                 │
└──────────────────────────────────────────────────────┘
```

### Data Models

```swift
// OpenAgentsCore/Sources/OpenAgentsCore/Marketplace/PaymentStatus.swift

import Foundation

/// Payment status for a job
public enum PaymentStatus: String, Codable {
    case unpaid       // Invoice generated, awaiting payment
    case pending      // Payment initiated (in mempool/routing)
    case confirmed    // Payment confirmed, job can execute
    case failed       // Payment failed (insufficient balance, timeout)
    case refunded     // Payment refunded (overpayment, cancellation)
    case expired      // Job expired (payment not received in time)

    public var displayName: String {
        switch self {
        case .unpaid: return "Awaiting Payment"
        case .pending: return "Payment Pending"
        case .confirmed: return "Payment Confirmed"
        case .failed: return "Payment Failed"
        case .refunded: return "Refunded"
        case .expired: return "Expired"
        }
    }

    public var emoji: String {
        switch self {
        case .unpaid: return "⏳"
        case .pending: return "🔄"
        case .confirmed: return "✅"
        case .failed: return "❌"
        case .refunded: return "💸"
        case .expired: return "⏰"
        }
    }
}

/// Job-invoice correlation record
public struct JobInvoiceRecord: Codable, Identifiable {
    public let id: String           // Job ID (NIP-90 event ID)
    public let invoiceId: String    // BOLT11 invoice or Spark address
    public let amountSats: UInt64
    public let createdAt: Date
    public let expiresAt: Date
    public var paymentStatus: PaymentStatus

    public init(
        id: String,
        invoiceId: String,
        amountSats: UInt64,
        createdAt: Date = Date(),
        expiresAt: Date = Date().addingTimeInterval(15 * 60),  // 15 min default
        paymentStatus: PaymentStatus = .unpaid
    ) {
        self.id = id
        self.invoiceId = invoiceId
        self.amountSats = amountSats
        self.createdAt = createdAt
        self.expiresAt = expiresAt
        self.paymentStatus = paymentStatus
    }
}
```

### PaymentCoordinator (Actor)

```swift
// OpenAgentsCore/Sources/OpenAgentsCore/Marketplace/PaymentCoordinator.swift

import Foundation
import BreezSdkSpark

/// Coordinates job requests with Lightning payments
public actor PaymentCoordinator {
    // MARK: - Dependencies
    private let walletManager: SparkWalletManager
    private let nostrClient: NostrClient
    private let storage: PaymentStorage

    // MARK: - State
    private var jobRecords: [String: JobInvoiceRecord] = [:]
    private var expirationTimer: Task<Void, Never>?

    // MARK: - Callbacks
    public var onJobPaid: ((String) -> Void)?  // jobId → trigger execution

    public init(
        walletManager: SparkWalletManager,
        nostrClient: NostrClient,
        storage: PaymentStorage
    ) {
        self.walletManager = walletManager
        self.nostrClient = nostrClient
        self.storage = storage

        Task {
            await startExpirationTimer()
            await loadPersistedRecords()
        }
    }

    // MARK: - Provider API

    /// Register a job-invoice mapping (called by BiddingEngine)
    public func registerJobInvoice(
        jobId: String,
        invoiceId: String,
        amountSats: UInt64,
        expirationMinutes: Int = 15
    ) async throws {
        let expiresAt = Date().addingTimeInterval(TimeInterval(expirationMinutes * 60))

        let record = JobInvoiceRecord(
            id: jobId,
            invoiceId: invoiceId,
            amountSats: amountSats,
            createdAt: Date(),
            expiresAt: expiresAt,
            paymentStatus: .unpaid
        )

        jobRecords[jobId] = record
        try await storage.save(record)

        // Publish feedback event (job status + invoice)
        try await publishPaymentRequired(jobId: jobId, invoice: invoiceId, amountSats: amountSats)
    }

    /// Handle incoming payment from wallet
    public func handlePaymentReceived(payment: Payment) async throws {
        // Extract job ID from payment metadata
        guard let jobId = extractJobId(from: payment) else {
            print("⚠️ Payment received without job ID: \(payment.id)")
            return
        }

        // Find matching record
        guard var record = jobRecords[jobId] else {
            print("⚠️ Payment received for unknown job: \(jobId)")
            return
        }

        // Validate amount
        if payment.amountSats < record.amountSats {
            // Partial payment - reject
            record.paymentStatus = .failed
            jobRecords[jobId] = record
            try await storage.save(record)

            try await publishPaymentFailed(jobId: jobId, reason: "Partial payment not accepted")
            return
        }

        if payment.amountSats > record.amountSats {
            // Overpayment - accept but log
            print("⚠️ Overpayment for job \(jobId): expected \(record.amountSats), received \(payment.amountSats)")
        }

        // Update status
        record.paymentStatus = .confirmed
        jobRecords[jobId] = record
        try await storage.save(record)

        // Publish confirmation
        try await publishPaymentConfirmed(jobId: jobId)

        // Trigger job execution
        onJobPaid?(jobId)
    }

    /// Check payment status for a job
    public func checkJobPaymentStatus(jobId: String) async -> PaymentStatus? {
        return jobRecords[jobId]?.paymentStatus
    }

    // MARK: - Buyer API

    /// Get invoice for a job (buyer retrieves this from NIP-90 feedback)
    public func getInvoiceForJob(jobId: String) async -> String? {
        return jobRecords[jobId]?.invoiceId
    }

    // MARK: - Expiration Handling

    private func startExpirationTimer() async {
        expirationTimer = Task {
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 60_000_000_000)  // Every 60 seconds
                await expireUnpaidJobs()
            }
        }
    }

    private func expireUnpaidJobs() async {
        let now = Date()

        for (jobId, var record) in jobRecords where record.paymentStatus == .unpaid {
            if record.expiresAt < now {
                // Expire job
                record.paymentStatus = .expired
                jobRecords[jobId] = record
                try? await storage.save(record)

                // Publish expiration event
                try? await publishJobExpired(jobId: jobId)

                print("⏰ Job \(jobId) expired (payment not received)")
            }
        }
    }

    // MARK: - Nostr Publishing

    private func publishPaymentRequired(jobId: String, invoice: String, amountSats: UInt64) async throws {
        let feedbackEvent = NostrEvent(
            kind: 7000,  // Job feedback
            content: "Payment required: \(invoice)",
            tags: [
                ["e", jobId],
                ["status", "payment-required"],
                ["amount", "\(amountSats)"],
                ["invoice", invoice]
            ]
        )

        try await nostrClient.publish(event: feedbackEvent)
    }

    private func publishPaymentConfirmed(jobId: String) async throws {
        let feedbackEvent = NostrEvent(
            kind: 7000,
            content: "Payment confirmed, job starting",
            tags: [
                ["e", jobId],
                ["status", "payment-confirmed"]
            ]
        )

        try await nostrClient.publish(event: feedbackEvent)
    }

    private func publishPaymentFailed(jobId: String, reason: String) async throws {
        let feedbackEvent = NostrEvent(
            kind: 7000,
            content: "Payment failed: \(reason)",
            tags: [
                ["e", jobId],
                ["status", "payment-failed"]
            ]
        )

        try await nostrClient.publish(event: feedbackEvent)
    }

    private func publishJobExpired(jobId: String) async throws {
        let feedbackEvent = NostrEvent(
            kind: 7000,
            content: "Job expired (payment not received)",
            tags: [
                ["e", jobId],
                ["status", "expired"]
            ]
        )

        try await nostrClient.publish(event: feedbackEvent)
    }

    // MARK: - Utilities

    private func extractJobId(from payment: Payment) -> String? {
        // Extract from invoice description (format: "OpenAgents job <jobId>")
        if let description = payment.details?.description,
           description.contains("OpenAgents job ") {
            let components = description.split(separator: " ")
            return components.last.map(String.init)
        }

        return nil
    }

    private func loadPersistedRecords() async {
        do {
            let records = try await storage.loadAll()
            for record in records {
                jobRecords[record.id] = record
            }
        } catch {
            print("Failed to load persisted payment records: \(error)")
        }
    }
}
```

### PaymentStorage (Core Data)

```swift
// OpenAgentsCore/Sources/OpenAgentsCore/Marketplace/PaymentStorage.swift

import Foundation
import CoreData

/// Persists job-invoice records
public protocol PaymentStorage {
    func save(_ record: JobInvoiceRecord) async throws
    func loadAll() async throws -> [JobInvoiceRecord]
    func delete(jobId: String) async throws
}

// Core Data implementation
public actor CoreDataPaymentStorage: PaymentStorage {
    private let container: NSPersistentContainer

    public init(modelName: String = "OpenAgents") {
        self.container = NSPersistentContainer(name: modelName)
        container.loadPersistentStores { _, error in
            if let error = error {
                fatalError("Failed to load Core Data: \(error)")
            }
        }
    }

    public func save(_ record: JobInvoiceRecord) async throws {
        let context = container.viewContext

        let fetchRequest: NSFetchRequest<PaymentRecordEntity> = PaymentRecordEntity.fetchRequest()
        fetchRequest.predicate = NSPredicate(format: "jobId == %@", record.id)

        let existing = try context.fetch(fetchRequest).first

        let entity = existing ?? PaymentRecordEntity(context: context)
        entity.jobId = record.id
        entity.invoiceId = record.invoiceId
        entity.amountSats = Int64(record.amountSats)
        entity.createdAt = record.createdAt
        entity.expiresAt = record.expiresAt
        entity.paymentStatus = record.paymentStatus.rawValue

        try context.save()
    }

    public func loadAll() async throws -> [JobInvoiceRecord] {
        let context = container.viewContext

        let fetchRequest: NSFetchRequest<PaymentRecordEntity> = PaymentRecordEntity.fetchRequest()
        let entities = try context.fetch(fetchRequest)

        return entities.compactMap { entity in
            guard let jobId = entity.jobId,
                  let invoiceId = entity.invoiceId,
                  let createdAt = entity.createdAt,
                  let expiresAt = entity.expiresAt,
                  let statusRaw = entity.paymentStatus,
                  let status = PaymentStatus(rawValue: statusRaw) else {
                return nil
            }

            return JobInvoiceRecord(
                id: jobId,
                invoiceId: invoiceId,
                amountSats: UInt64(entity.amountSats),
                createdAt: createdAt,
                expiresAt: expiresAt,
                paymentStatus: status
            )
        }
    }

    public func delete(jobId: String) async throws {
        let context = container.viewContext

        let fetchRequest: NSFetchRequest<PaymentRecordEntity> = PaymentRecordEntity.fetchRequest()
        fetchRequest.predicate = NSPredicate(format: "jobId == %@", jobId)

        let entities = try context.fetch(fetchRequest)
        for entity in entities {
            context.delete(entity)
        }

        try context.save()
    }
}
```

### Integration with Worker

```swift
// OpenAgentsMac/Worker/WorkerService.swift

@MainActor
class WorkerService: ObservableObject {
    let coordinator: PaymentCoordinator
    let biddingEngine: BiddingEngine
    let jobExecutor: JobExecutor

    init(walletManager: SparkWalletManager, nostrClient: NostrClient) {
        let storage = CoreDataPaymentStorage()
        self.coordinator = PaymentCoordinator(
            walletManager: walletManager,
            nostrClient: nostrClient,
            storage: storage
        )

        self.biddingEngine = BiddingEngine()
        self.jobExecutor = JobExecutor()

        // Listen for paid jobs
        Task {
            await coordinator.setOnJobPaid { [weak self] jobId in
                await self?.executeJob(jobId: jobId)
            }
        }

        // Listen for wallet payment events
        Task {
            await listenForPayments()
        }
    }

    func handleJobRequest(_ job: NostrJob) async throws {
        // Evaluate bid
        let bid = try await biddingEngine.evaluateBid(for: job)

        guard let bid = bid else {
            // Reject job (too expensive, out of capacity, etc.)
            return
        }

        // Generate invoice
        let invoice = try await walletManager.generateInvoice(
            amountSats: bid.priceSats,
            description: "OpenAgents job \(job.id.prefix(8))"
        )

        // Register with coordinator
        try await coordinator.registerJobInvoice(
            jobId: job.id,
            invoiceId: invoice,
            amountSats: bid.priceSats
        )

        // Coordinator publishes NIP-90 feedback with invoice
    }

    private func listenForPayments() async {
        // Subscribe to wallet payment events
        // (This is simplified - actual implementation uses EventListener)
        while true {
            // Check for new payments
            // When payment received, notify coordinator
            // coordinator.handlePaymentReceived(payment)

            try? await Task.sleep(nanoseconds: 5_000_000_000)
        }
    }

    private func executeJob(jobId: String) async {
        // Job execution triggered by payment confirmation
        guard let job = getJob(jobId) else { return }

        let result = try? await jobExecutor.execute(job)

        // Publish result to Nostr
    }
}
```

## Testing

### Unit Tests

```swift
// OpenAgentsCoreTests/Marketplace/PaymentCoordinatorTests.swift

import XCTest
@testable import OpenAgentsCore

class PaymentCoordinatorTests: XCTestCase {
    func testRegisterJobInvoice() async throws {
        let coordinator = createTestCoordinator()

        try await coordinator.registerJobInvoice(
            jobId: "test-job-123",
            invoiceId: "lnbc1000...",
            amountSats: 1000
        )

        let status = await coordinator.checkJobPaymentStatus(jobId: "test-job-123")
        XCTAssertEqual(status, .unpaid)
    }

    func testPaymentReceived() async throws {
        let coordinator = createTestCoordinator()

        try await coordinator.registerJobInvoice(
            jobId: "test-job-123",
            invoiceId: "lnbc1000...",
            amountSats: 1000
        )

        var jobExecuted = false
        await coordinator.setOnJobPaid { _ in
            jobExecuted = true
        }

        // Simulate payment
        let payment = Payment(
            id: "pay-123",
            amountSats: 1000,
            details: PaymentDetails(description: "OpenAgents job test-job-123")
        )

        try await coordinator.handlePaymentReceived(payment: payment)

        let status = await coordinator.checkJobPaymentStatus(jobId: "test-job-123")
        XCTAssertEqual(status, .confirmed)
        XCTAssertTrue(jobExecuted)
    }

    func testPartialPaymentRejected() async throws {
        let coordinator = createTestCoordinator()

        try await coordinator.registerJobInvoice(
            jobId: "test-job-123",
            invoiceId: "lnbc1000...",
            amountSats: 1000
        )

        // Partial payment (500 sats instead of 1000)
        let payment = Payment(
            id: "pay-123",
            amountSats: 500,
            details: PaymentDetails(description: "OpenAgents job test-job-123")
        )

        try await coordinator.handlePaymentReceived(payment: payment)

        let status = await coordinator.checkJobPaymentStatus(jobId: "test-job-123")
        XCTAssertEqual(status, .failed)
    }

    func testJobExpiration() async throws {
        let coordinator = createTestCoordinator()

        try await coordinator.registerJobInvoice(
            jobId: "test-job-123",
            invoiceId: "lnbc1000...",
            amountSats: 1000,
            expirationMinutes: 0  // Expire immediately
        )

        // Wait for expiration timer
        try await Task.sleep(nanoseconds: 2_000_000_000)

        let status = await coordinator.checkJobPaymentStatus(jobId: "test-job-123")
        XCTAssertEqual(status, .expired)
    }
}
```

## Dependencies

### OpenAgents Issues

- **Issue #010**: iOS Wallet (buyer side payment)
- **Issue #013**: macOS Wallet (provider side payment receipt)
- **Issue #011**: iOS Job Creation (buyer creates jobs)
- **Issue #014**: macOS Bidding Engine (provider generates bids + invoices)
- **Issue #007**: macOS Worker (executes jobs after payment)

### External

- Breez Spark SDK (payment events)
- Core Data (persistence)
- Nostr client (NIP-90 feedback events)

## Success Metrics

- [ ] 95%+ of paid jobs execute automatically (no manual intervention)
- [ ] Payment-to-execution latency <10 seconds (after confirmation)
- [ ] Zero duplicate executions (payment matches exactly one job)
- [ ] Failed payments rejected within 5 seconds
- [ ] Expired jobs cleaned up within 1 minute of expiration

## Apple Compliance

✅ **No special compliance concerns** - standard data processing and networking

## Future Enhancements

- [ ] **Refund automation**: Auto-refund overpayments
- [ ] **Partial execution**: Accept partial payment for cheaper alternative (e.g., lower quality)
- [ ] **Escrow**: Hold payment until result delivered (requires time-locked contracts)
- [ ] **Multi-payment**: Split payment across multiple invoices
- [ ] **Subscription**: Recurring payments for ongoing services

## Notes

- **Critical path**: Without this, marketplace requires manual payment matching
- **Event-driven**: Uses Spark SDK EventListener for real-time payment detection
- **Persistent**: Survives app restarts (Core Data storage)
- **Nostr-native**: All status updates published via NIP-90 feedback events

## Reference

- **NIP-90**: https://github.com/nostr-protocol/nips/blob/master/90.md
- **Breez SDK Events**: https://sdk-doc-spark.breez.technology/guide/payment_events.html
