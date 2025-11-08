import Foundation
import Combine
import NostrSDK
#if os(macOS)
import AppKit
#endif

/// Manages NIP-90 event feed subscription, filtering, and in-memory cache.
@MainActor
public final class NostrEventFeedManager: ObservableObject {
    // MARK: - Published State
    @Published public private(set) var events: [DVMEventItem] = []
    @Published public private(set) var isLoading = false
    @Published public private(set) var error: FeedError?
    @Published public var selectedKindFilter: KindFilter = .all {
        didSet { Task { await refreshFeed() } }
    }
    @Published public var timeFilter: TimeFilter = .lastMonth {
        didSet { Task { await refreshFeed() } }
    }

    // MARK: - Types
    public struct DVMEventItem: Identifiable, Equatable {
        public let id: String // event id
        public let event: NostrEvent
        public let type: EventType
        public let timestamp: Date
        public let authorNpub: String
        public let kindInfo: KindInfo?

        public enum EventType: Equatable {
            case jobRequest(kind: Int)
            case jobResult(kind: Int)
            case jobFeedback
        }

        public struct KindInfo: Equatable {
            public let name: String
            public let displayName: String
            public let isCustom: Bool
        }
    }

    public enum FeedError: Error, Equatable, CustomStringConvertible {
        case subscribeFailed
        case connectionFailed(String)

        public var description: String {
            switch self {
            case .subscribeFailed: return "Subscription failed"
            case .connectionFailed(let msg): return msg
            }
        }
    }

    public enum KindFilter: CaseIterable, Equatable {
        case all            // 5000-5999, 6000-6999, 7000
        case requests       // 5000-5999
        case results        // 6000-6999
        case feedback       // 7000
        case customOnly     // 6500-6599, 7500-7599

        public var kinds: [Int] {
            switch self {
            case .all:
                return Array(5000...5999) + Array(6000...6999) + Array(7500...7599) + [7000]
            case .requests:
                return Array(5000...5999)
            case .results:
                return Array(6000...6999)
            case .feedback:
                return [7000]
            case .customOnly:
                return Array(6500...6599) + Array(7500...7599)
            }
        }
    }

    public enum TimeFilter: CaseIterable, Equatable {
        case lastHour
        case last24Hours
        case last7Days
        case lastMonth
        case allTime

        public var since: Date? {
            let now = Date()
            switch self {
            case .lastHour: return now.addingTimeInterval(-3600)
            case .last24Hours: return now.addingTimeInterval(-86400)
            case .last7Days: return now.addingTimeInterval(-7 * 86400)
            case .lastMonth: return now.addingTimeInterval(-30 * 86400)
            case .allTime: return nil
            }
        }
    }

    // MARK: - Internals
    public protocol NostrRelayManaging {
        func connect() async throws
        func disconnect()
        func subscribe(kinds: [Int], limit: Int, since: Date?, until: Date?) -> AnyPublisher<NostrEvent, Never>
    }

    private let relayManager: NostrRelayManaging
    private let maxEvents = 1000
    private var cancellables = Set<AnyCancellable>()
    private var subscriptionCancellable: AnyCancellable?
    private var seenIds = Set<String>()

    public init(relayManager: NostrRelayManaging) {
        self.relayManager = relayManager
    }

    // MARK: - Public API
    public func startFeed() async {
        guard !isLoading else { return }
        isLoading = true
        error = nil
        do {
            try await relayManager.connect()
            resumeSubscription()
        } catch {
            self.error = .connectionFailed(error.localizedDescription)
        }
        isLoading = false
    }

    public func stopFeed() {
        subscriptionCancellable?.cancel()
        subscriptionCancellable = nil
        relayManager.disconnect()
        clearCache()
    }

    public func refreshFeed() async {
        subscriptionCancellable?.cancel()
        subscriptionCancellable = nil
        clearCache()
        resumeSubscription()
    }

    public func copyEventJSON(_ event: NostrEvent) {
        do {
            let data = try JSONEncoder().encode(event)
            if let s = String(data: data, encoding: .utf8) {
                #if os(macOS)
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(s, forType: .string)
                #endif
            }
        } catch {
            // ignore copy errors for now
        }
    }

    public func verifySignature(_ event: NostrEvent) -> Bool {
        // Create a lightweight verifier via protocol default implementation
        struct Verifier: EventVerifying {}
        do {
            try Verifier().verifyEvent(event)
            return true
        } catch {
            return false
        }
    }

    // MARK: - Subscription
    private func resumeSubscription() {
        let kinds = selectedKindFilter.kinds
        let since = timeFilter.since
        let pub = relayManager.subscribe(kinds: kinds, limit: 100, since: since, until: nil)

        subscriptionCancellable = pub
            .receive(on: DispatchQueue.global(qos: .userInitiated))
            .map { [weak self] in self?.transform(event: $0) }
            .compactMap { $0 }
            .collect(.byTime(DispatchQueue.main, .milliseconds(100))) // batch up to 10/sec
            .receive(on: DispatchQueue.main)
            .sink { [weak self] batch in
                guard let self else { return }
                for item in batch {
                    if self.seenIds.contains(item.id) { continue }
                    self.seenIds.insert(item.id)
                    self.events.insert(item, at: 0)
                    if self.events.count > self.maxEvents {
                        self.events.removeLast(self.events.count - self.maxEvents)
                    }
                }
            }
    }

    private func clearCache() {
        events.removeAll(keepingCapacity: false)
        seenIds.removeAll(keepingCapacity: false)
    }

    private func transform(event: NostrEvent) -> DVMEventItem? {
        let kindRaw = event.kind.rawValue
        let type: DVMEventItem.EventType
        switch kindRaw {
        case 5000...5999: type = .jobRequest(kind: kindRaw)
        case 6000...6999: type = .jobResult(kind: kindRaw)
        case 7000: type = .jobFeedback
        default: return nil
        }

        let authorNpub: String = (PublicKey(hex: event.pubkey)?.npub) ?? event.pubkey
        let info = kindInfo(for: kindRaw)
        return DVMEventItem(id: event.id,
                            event: event,
                            type: type,
                            timestamp: event.createdDate,
                            authorNpub: authorNpub,
                            kindInfo: info)
    }

    private func kindInfo(for kind: Int) -> DVMEventItem.KindInfo? {
        // Soft dependency on JobKind registry; fallback to generic
        let isCustom = (6500...6599).contains(kind) || (7500...7599).contains(kind)
        return .init(name: "Kind \(kind)", displayName: isCustom ? "Kind \(kind) (Custom)" : "Kind \(kind)", isCustom: isCustom)
    }
}
