import Foundation
import Combine
import NostrSDK

/// Manages Nostr relay connections, status, and subscriptions.
@MainActor
public final class NostrRelayManager: ObservableObject {
    // MARK: - Types
    public struct RelayInfo: Identifiable, Equatable {
        public let id: String // URL string
        public let url: URL
        public var status: RelayStatus
        public var lastConnected: Date?
        public var errorMessage: String?

        public enum RelayStatus: Equatable {
            case connecting
            case connected
            case disconnected
            case error(String)
        }
    }

    public enum ConnectionStatus: Equatable {
        case disconnected
        case connecting
        case connected(count: Int)
        case error(String)
    }

    // MARK: - Published State
    @Published public private(set) var relays: [RelayInfo] = []
    @Published public private(set) var connectionStatus: ConnectionStatus = .disconnected

    // MARK: - Defaults
    public static let defaultRelays: [URL] = [
        URL(string: "wss://relay.damus.io")!,
        URL(string: "wss://relay.nostr.band")!,
        URL(string: "wss://nos.lol")!
    ]

    // MARK: - Internals
    private var relayPool: RelayPool?
    private var cancellables = Set<AnyCancellable>()
    private var backoffSeconds: [String: Int] = [:] // urlString -> seconds
    private var reconnectTasks: [String: Task<Void, Never>] = [:]

    // Indirection for testability
    public typealias RelayPoolFactory = (_ urls: Set<URL>, _ delegate: RelayDelegate?) throws -> RelayPool
    private let poolFactory: RelayPoolFactory

    // For mapping URL -> Relay reference
    private var relayByURL: [String: Relay] = [:]

    // Persisted relay URLs (DEBUG only)
    private static let relaysDefaultsKey = "dev.nostr.relays"

    public init(relayURLs: [URL],
                poolFactory: @escaping RelayPoolFactory = { try RelayPool(relayURLs: $0, delegate: $1) }) {
        self.poolFactory = poolFactory
        self.relays = relayURLs.map { RelayInfo(id: $0.absoluteString, url: $0, status: .disconnected) }
    }

    public convenience init() {
        self.init(relayURLs: Self.loadPersistedRelaysOrDefault())
    }

    // MARK: - Public API
    public func connect() async throws {
        connectionStatus = .connecting

        let urls = Set(relays.map { $0.url })
        let pool = try poolFactory(urls, self)
        self.relayPool = pool

        // Build URL map and track initial states
        relayByURL.removeAll()
        pool.relays.forEach { relay in
            relayByURL[relay.url.absoluteString] = relay
        }

        // Derive initial connection status
        updateAggregateStatus()

        // Pool connects individual relays in its setup
        pool.connect()

        // Observe events publisher just to keep it active (consumers subscribe via subscribe())
        pool.events
            .sink { _ in }
            .store(in: &cancellables)
    }

    public func disconnect() {
        reconnectTasks.values.forEach { $0.cancel() }
        reconnectTasks.removeAll()
        relayPool?.disconnect()
        connectionStatus = .disconnected
        relays = relays.map { var r = $0; r.status = .disconnected; return r }
    }

    /// Subscribe to events with a filter of kinds and time window.
    /// Returns a publisher of NostrEvent values for this subscription.
    public func subscribe(
        kinds: [Int],
        limit: Int = 100,
        since: Date? = nil,
        until: Date? = nil
    ) -> AnyPublisher<NostrEvent, Never> {
        guard let pool = relayPool else { return Empty().eraseToAnyPublisher() }

        let sinceUnix = since.map { Int($0.timeIntervalSince1970) }
        let untilUnix = until.map { Int($0.timeIntervalSince1970) }
        let filter = Filter(ids: nil,
                            authors: nil,
                            kinds: kinds,
                            events: nil,
                            pubkeys: nil,
                            tags: nil,
                            since: sinceUnix,
                            until: untilUnix,
                            limit: limit)

        let subId = pool.subscribe(with: filter ?? Filter(limit: limit)!)
        let subject = PassthroughSubject<NostrEvent, Never>()

        pool.events
            .filter { $0.subscriptionId == subId }
            .map { $0.event }
            .receive(on: DispatchQueue.main)
            .sink { subject.send($0) }
            .store(in: &cancellables)

        return subject.eraseToAnyPublisher()
    }

    public func addRelay(url: URL) throws {
        guard relays.contains(where: { $0.url == url }) == false else { return }
        relays.append(RelayInfo(id: url.absoluteString, url: url, status: .disconnected))
        Self.persistRelays(relays.map { $0.url })

        if let pool = relayPool {
            let relay = try Relay(url: url)
            pool.add(relay: relay)
            relayByURL[url.absoluteString] = relay
        }
    }

    public func removeRelay(url: URL) {
        relays.removeAll { $0.url == url }
        Self.persistRelays(relays.map { $0.url })
        relayPool?.removeRelay(withURL: url)
        relayByURL[url.absoluteString] = nil
    }

    // MARK: - Helpers
    private func updateAggregateStatus() {
        let connected = relays.filter {
            if case .connected = $0.status { return true } else { return false }
        }.count
        if connected == 0 {
            if relays.contains(where: { if case .error = $0.status { return true } else { return false } }) {
                connectionStatus = .error("One or more relays in error state")
            } else if relays.contains(where: { if case .connecting = $0.status { return true } else { return false } }) {
                connectionStatus = .connecting
            } else {
                connectionStatus = .disconnected
            }
        } else {
            connectionStatus = .connected(count: connected)
        }
    }

    private func setStatus(for urlString: String, to newStatus: RelayInfo.RelayStatus, error: String? = nil) {
        relays = relays.map { info in
            guard info.id == urlString else { return info }
            var r = info
            r.status = newStatus
            if case .connected = newStatus { r.lastConnected = Date() }
            r.errorMessage = error
            return r
        }
        updateAggregateStatus()
    }

    private func scheduleReconnect(for urlString: String) {
        reconnectTasks[urlString]?.cancel()
        let next = min(16, max(1, (backoffSeconds[urlString] ?? 1)))
        backoffSeconds[urlString] = next * 2

        reconnectTasks[urlString] = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(next) * 1_000_000_000)
            guard let self, let relay = self.relayByURL[urlString] else { return }
            relay.connect()
            self.setStatus(for: urlString, to: .connecting)
        }
    }

    private static func loadPersistedRelaysOrDefault() -> [URL] {
        #if DEBUG
        if let arr = UserDefaults.standard.array(forKey: relaysDefaultsKey) as? [String] {
            let urls = arr.compactMap { URL(string: $0) }
            if !urls.isEmpty { return urls }
        }
        #endif
        return defaultRelays
    }

    private static func persistRelays(_ urls: [URL]) {
        #if DEBUG
        let strings = urls.map { $0.absoluteString }
        UserDefaults.standard.set(strings, forKey: relaysDefaultsKey)
        #endif
    }
}

// MARK: - RelayDelegate
extension NostrRelayManager: RelayDelegate {
    public func relayStateDidChange(_ relay: Relay, state: Relay.State) {
        switch state {
        case .connecting:
            setStatus(for: relay.url.absoluteString, to: .connecting)
        case .connected:
            backoffSeconds[relay.url.absoluteString] = 1
            setStatus(for: relay.url.absoluteString, to: .connected)
        case .notConnected:
            setStatus(for: relay.url.absoluteString, to: .disconnected)
            scheduleReconnect(for: relay.url.absoluteString)
        case .error(let err):
            setStatus(for: relay.url.absoluteString, to: .error(err.localizedDescription), error: err.localizedDescription)
            scheduleReconnect(for: relay.url.absoluteString)
        }
    }

    public func relay(_ relay: Relay, didReceive response: RelayResponse) {
        // no-op for now
    }

    public func relay(_ relay: Relay, didReceive event: RelayEvent) {
        // no-op here; subscribers get events via subscribe()
    }
}

// MARK: - NostrEventFeedManager.NostrRelayManaging conformance
extension NostrRelayManager: NostrEventFeedManager.NostrRelayManaging {}
