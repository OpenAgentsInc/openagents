// Firm reason: 6 (cross-Apple Network.framework Bonjour discovery, NWPathMonitor, relay-free direct grants, multiplexed NIP-44 Tailscale TCP, and real-time push frames; all wire framing & crypto execute in Coax).
import Foundation
import Network

enum TailscaleRPCError: Error, LocalizedError, Equatable {
    case missingSecret
    case missingEndpoint
    case encodingFailed
    case refused(String)
    case unreachable(String)
    case timeout
    case invalidResponse

    var errorDescription: String? {
        switch self {
        case .missingSecret: return "Device key unavailable."
        case .missingEndpoint: return "No Tailscale address saved for this computer."
        case .encodingFailed: return "Could not seal request with Coax core."
        case .refused(let code): return ReachabilityState.humanRefusal(code)
        case .unreachable(let msg): return msg.isEmpty ? "Could not reach computer over Tailscale." : msg
        case .timeout: return "Computer did not respond in time over Tailscale."
        case .invalidResponse: return "Computer returned an invalid or mismatched response."
        }
    }
}

/// Direct Tailscale-only client for OpenAgents hosts (`_openagents-coder._tcp` + port 47109).
/// Supports relay-free direct grants, multiplexed NIP-44 RPCs (Environment & Workspace), and push frames.
/// Zero public relay fallback, zero WebSockets, zero cloud brokers.
@MainActor
final class TailscaleClient {
    let book: AppTailnetBook
    var browsesLocalNetwork = true

    var onDiscoveredChange: (() -> Void)?
    var onReachabilityChange: ((String, ReachabilityState) -> Void)?
    var onPushEvent: ((String, AppTailnet.PushEvent) -> Void)?
    /// Fires accurately when the direct TCP push channel transitions connected/disconnected (#14007).
    var onLivePushStateChange: ((String, Bool) -> Void)?

    private struct DirectWaiter {
        let id: UInt64
        let accept: (Data) -> Bool
        let finish: (Result<Data, TailscaleRPCError>) -> Void
    }

    private var active = false
    private var browser: NWBrowser?
    private var pathMonitor: NWPathMonitor?
    private var lastPathSignature: String?

    private var directConns: [String: NWConnection] = [:]
    private var directReady: Set<String> = []
    private var directBuffers: [String: Data] = [:]
    private var directWaiters: [String: [DirectWaiter]] = [:]
    private var directQueued: [String: [(Data, DirectWaiter)]] = [:]
    private var subscribedSessions: [String: String] = [:]
    private var subscribedEndpoints: [String: AppTailnetComputer] = [:]
    private var reconnectAttempts: [String: Int] = [:]
    private var reconnectTasks: [String: Task<Void, Never>] = [:]
    private var consecutiveTimeouts: [String: Int] = [:]
    private var directSerial: UInt64 = 0

    init(book: AppTailnetBook) {
        self.book = book
    }

    private static var nowSeconds: Int64 { Int64(Date().timeIntervalSince1970) }

    func resume() {
        active = true
        startPathMonitorIfNeeded()
        if browsesLocalNetwork {
            startBrowser()
        }
    }

    func suspend() {
        active = false
        browser?.cancel()
        browser = nil
        for (_, t) in reconnectTasks { t.cancel() }
        reconnectTasks.removeAll()
        reconnectAttempts.removeAll()
        dropAllDirect(with: .unreachable("Transport suspended"), scheduleReconnect: false)
    }

    // MARK: - Path monitoring

    private func startPathMonitorIfNeeded() {
        guard pathMonitor == nil else { return }
        let monitor = NWPathMonitor()
        monitor.pathUpdateHandler = { [weak self] path in
            let interfaces = path.availableInterfaces.map(\.name).sorted().joined(separator: ",")
            let sig = "\(path.status)|\(interfaces)|\(path.isExpensive)|\(path.isConstrained)"
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    guard let self else { return }
                    let prev = self.lastPathSignature
                    self.lastPathSignature = sig
                    if let prev, prev != sig, self.active {
                        self.reconnectAttempts.removeAll()
                        self.dropAllDirect(with: .unreachable("Network changed; reconnecting"), scheduleReconnect: true)
                    }
                }
            }
        }
        monitor.start(queue: DispatchQueue(label: "com.openagents.tailscale.netpath", qos: .utility))
        pathMonitor = monitor
    }

    // MARK: - Bonjour discovery (_openagents-coder._tcp)

    private func startBrowser() {
        guard browser == nil else { return }
        let b = NWBrowser(for: .bonjourWithTXTRecord(type: AppTailnet.service, domain: nil), using: .tcp)
        b.browseResultsChangedHandler = { [weak self] results, _ in
            let records: [[String: String]] = results.compactMap {
                if case .bonjour(let txt) = $0.metadata { return txt.dictionary }
                return nil
            }
            DispatchQueue.main.async {
                MainActor.assumeIsolated { self?.ingestBonjourRecords(records) }
            }
        }
        b.stateUpdateHandler = { [weak self] state in
            if case .failed = state {
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        self?.browser?.cancel()
                        self?.browser = nil
                    }
                }
            }
        }
        b.start(queue: .main)
        browser = b
    }

    private func ingestBonjourRecords(_ records: [[String: String]]) {
        guard active else { return }
        var changedAny = false
        for txt in records {
            guard let comp = AppTailnetComputer(txt: txt, seen: Self.nowSeconds) else { continue }
            let previous = book.computers.first(where: { $0.host == comp.host })
            book.remember(comp)
            if let previous, previous.ip != comp.ip || previous.port != comp.port || previous.dns != comp.dns {
                dropDirect(comp.host, with: .unreachable("Host Tailscale endpoint updated"))
                changedAny = true
            } else if previous == nil {
                changedAny = true
            }
        }
        if changedAny {
            onDiscoveredChange?()
        }
    }

    // MARK: - Direct Tailscale reachability probe

    /// Probes a Tailscale IP or MagicDNS address on port `port` using `openagents.history-observer-direct.v1`.
    func probe(address: String, port: UInt16) async -> ReachabilityState {
        let trimmed = address.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let nwPort = NWEndpoint.Port(rawValue: port) else {
            return .unreachable("Invalid Tailscale address or port")
        }
        return await withCheckedContinuation { continuation in
            let conn = NWConnection(host: NWEndpoint.Host(trimmed), port: nwPort, using: .tcp)
            var settled = false
            let finish: (ReachabilityState) -> Void = { state in
                guard !settled else { return }
                settled = true
                conn.cancel()
                continuation.resume(returning: state)
            }
            conn.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    conn.send(content: AppTailnet.directHelloLine(), completion: .contentProcessed { err in
                        if let err {
                            DispatchQueue.main.async { MainActor.assumeIsolated { finish(.unreachable(err.localizedDescription)) } }
                        }
                    })
                    Self.readFrame(conn, Data()) { line, _ in
                        DispatchQueue.main.async {
                            MainActor.assumeIsolated {
                                guard let line else {
                                    finish(.unreachable("Closed before Tailscale handshake"))
                                    return
                                }
                                let ack = AppTailnet.directAck(line)
                                if ack.ok {
                                    finish(.reachable)
                                } else if let refused = ack.refused, !refused.isEmpty {
                                    finish(.refused(refused))
                                } else {
                                    finish(.refused("malformed"))
                                }
                            }
                        }
                    }
                case .failed(let err):
                    DispatchQueue.main.async { MainActor.assumeIsolated { finish(.unreachable(err.localizedDescription)) } }
                case .waiting:
                    DispatchQueue.main.async { MainActor.assumeIsolated { finish(.unreachable("No route over Tailscale")) } }
                case .cancelled:
                    DispatchQueue.main.async { MainActor.assumeIsolated { finish(.unreachable("Connection cancelled")) } }
                default:
                    break
                }
            }
            conn.start(queue: .main)
            DispatchQueue.main.asyncAfter(deadline: .now() + 3.5) {
                MainActor.assumeIsolated { finish(.unreachable("Timed out reaching \(trimmed):\(port) over Tailscale")) }
            }
        }
    }

    // MARK: - Relay-free Direct Tailscale Grant (#13993 Phase 1.1)

    /// Requests a signed grant directly over Tailscale TCP port `port` with zero public relays.
    /// Host validates caller identity via `tailscale whois` (same untagged Tailscale user).
    func requestDirectGrant(
        address: String,
        port: UInt16,
        devicePubkey: String,
        clientLabel: String
    ) async -> Result<AppTailnet.DirectGrantAnswer, TailscaleRPCError> {
        let trimmed = address.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let nwPort = NWEndpoint.Port(rawValue: port) else {
            return .failure(.missingEndpoint)
        }
        let line = AppTailnet.directGrantRequestLine(device: devicePubkey, label: clientLabel)
        guard !line.isEmpty else { return .failure(.encodingFailed) }

        return await withCheckedContinuation { continuation in
            let conn = NWConnection(host: NWEndpoint.Host(trimmed), port: nwPort, using: .tcp)
            var settled = false
            let finish: (Result<AppTailnet.DirectGrantAnswer, TailscaleRPCError>) -> Void = { result in
                guard !settled else { return }
                settled = true
                conn.cancel()
                continuation.resume(returning: result)
            }
            conn.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    conn.send(content: line, completion: .contentProcessed { err in
                        if let err {
                            DispatchQueue.main.async { MainActor.assumeIsolated { finish(.failure(.unreachable(err.localizedDescription))) } }
                        }
                    })
                    Self.readFrame(conn, Data()) { reply, _ in
                        DispatchQueue.main.async {
                            MainActor.assumeIsolated {
                                guard let reply, let parsed = AppTailnet.directGrantAnswer(reply) else {
                                    finish(.failure(.invalidResponse))
                                    return
                                }
                                if let refused = parsed.refused, !refused.isEmpty {
                                    finish(.failure(.refused(refused)))
                                    return
                                }
                                guard let grant = parsed.grant, !grant.isEmpty, MobileCore.isValidHostKey(parsed.host) else {
                                    finish(.failure(.invalidResponse))
                                    return
                                }
                                finish(.success(parsed))
                            }
                        }
                    }
                case .failed(let err):
                    DispatchQueue.main.async { MainActor.assumeIsolated { finish(.failure(.unreachable(err.localizedDescription))) } }
                case .waiting:
                    DispatchQueue.main.async { MainActor.assumeIsolated { finish(.failure(.unreachable("No route over Tailscale"))) } }
                case .cancelled:
                    DispatchQueue.main.async { MainActor.assumeIsolated { finish(.failure(.unreachable("Connection cancelled"))) } }
                default:
                    break
                }
            }
            conn.start(queue: .main)
            DispatchQueue.main.asyncAfter(deadline: .now() + 8.0) {
                MainActor.assumeIsolated { finish(.failure(.timeout)) }
            }
        }
    }

    // MARK: - Real-time push subscription (#13993 Phase 1.2)

    /// Subscribes to live change notifications (`openagents.host-tailnet-push.v1`) on the persistent
    /// direct Tailscale TCP stream for `endpoint`. Pass `session == ""` to watch all sessions.
    func subscribe(endpoint: AppTailnetComputer, session: String = "") {
        let host = endpoint.host
        subscribedSessions[host] = session
        subscribedEndpoints[host] = endpoint
        guard active else { return }
        if directReady.contains(host), let conn = directConns[host] {
            let line = AppTailnet.subscribeLine(session: session)
            if !line.isEmpty {
                conn.send(content: line, completion: .contentProcessed { _ in })
            }
            onLivePushStateChange?(host, true)
        } else if directConns[host] == nil {
            dialDirect(endpoint, order: endpoint.dialOrder)
        }
    }

    func unsubscribe(host: String) {
        subscribedSessions.removeValue(forKey: host)
        subscribedEndpoints.removeValue(forKey: host)
        reconnectTasks.removeValue(forKey: host)?.cancel()
        reconnectAttempts.removeValue(forKey: host)
        onLivePushStateChange?(host, false)
    }

    /// Computes capped jittered exponential backoff delay (`1s -> 2s -> 4s -> 8s -> 15s`; #14007).
    nonisolated static func backoffDelaySeconds(attempt: Int, jitterUnit: Double = Double.random(in: 0...1)) -> Double {
        let exp = min(15.0, pow(2.0, Double(max(0, min(attempt, 4)))))
        let jitter = exp * 0.25 * max(0.0, min(1.0, jitterUnit))
        return min(15.0, exp + jitter)
    }

    private func scheduleReconnect(for host: String) {
        guard active, let endpoint = subscribedEndpoints[host], subscribedSessions[host] != nil else { return }
        guard directConns[host] == nil, reconnectTasks[host] == nil else { return }
        let attempt = reconnectAttempts[host] ?? 0
        reconnectAttempts[host] = attempt + 1
        let delay = Self.backoffDelaySeconds(attempt: attempt)
        reconnectTasks[host] = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            await MainActor.run {
                guard let self, self.active else { return }
                self.reconnectTasks.removeValue(forKey: host)
                guard self.directConns[host] == nil, let ep = self.subscribedEndpoints[host] else { return }
                self.dialDirect(ep, order: ep.dialOrder)
            }
        }
    }

    // MARK: - Sealed NIP-44 RPC over direct Tailscale TCP (Environment & Workspace)

    func sendRPC(
        paired: PairedComputer,
        endpoint: AppTailnetComputer,
        secret: Data,
        method: String,
        params: Data,
        isWorkspace: Bool = false,
        timeout: TimeInterval = 18.0
    ) async -> Result<Data, TailscaleRPCError> {
        guard let work = MobileCorePool.shared.checkout() else { return .failure(.encodingFailed) }
        defer { MobileCorePool.shared.checkin(work) }
        guard let sealed = MobileCore.sealRPC(
            work: work,
            secret: secret,
            host: paired.host,
            relay: paired.relay,
            grant: paired.grant,
            epoch: paired.epoch,
            params: params
        ) else {
            return .failure(.encodingFailed)
        }

        let host = paired.host
        return await withCheckedContinuation { continuation in
            var openedPayload: Data?
            let accept: (Data) -> Bool = { line in
                guard let plain = MobileCore.openReply(work: work, secret: secret, host: host, event: line) else {
                    return false
                }
                openedPayload = plain
                return true
            }
            sendDirectFrame(endpoint: endpoint, event: sealed.event, timeout: timeout, accept: accept) { result in
                switch result {
                case .failure(let err):
                    continuation.resume(returning: .failure(err))
                case .success:
                    guard let payload = openedPayload else {
                        continuation.resume(returning: .failure(.invalidResponse))
                        return
                    }
                    let valid = isWorkspace
                        ? MobileCore.validateWorkspaceView(work: work, host: host, method: method, raw: payload)
                        : MobileCore.validateView(work: work, host: host, method: method, raw: payload)
                    guard valid else {
                        continuation.resume(returning: .failure(.invalidResponse))
                        return
                    }
                    continuation.resume(returning: .success(payload))
                }
            }
        }
    }

    private func sendDirectFrame(
        endpoint: AppTailnetComputer,
        event: Data,
        timeout: TimeInterval,
        accept: @escaping (Data) -> Bool,
        completion: @escaping (Result<Data, TailscaleRPCError>) -> Void
    ) {
        let host = endpoint.host
        guard active else {
            completion(.failure(.unreachable("Transport inactive")))
            return
        }
        directSerial += 1
        let waiterID = directSerial
        var finished = false
        let finish: (Result<Data, TailscaleRPCError>) -> Void = { result in
            guard !finished else { return }
            finished = true
            completion(result)
        }
        let waiter = DirectWaiter(id: waiterID, accept: accept, finish: finish)
        let boundedTimeout = min(max(timeout, 4.0), 25.0)
        DispatchQueue.main.asyncAfter(deadline: .now() + boundedTimeout) { [weak self] in
            MainActor.assumeIsolated {
                guard let self, !finished else { return }
                self.removeWaiter(host: host, id: waiterID)
                // Isolate single-RPC timeout from tearing down concurrent RPCs & push stream;
                // only tear down after 2+ consecutive timeouts on the same socket (#14007).
                let count = (self.consecutiveTimeouts[host] ?? 0) + 1
                self.consecutiveTimeouts[host] = count
                if count >= 2, self.directReady.contains(host) {
                    self.dropDirect(host, with: .timeout, scheduleReconnect: true)
                }
                finish(.failure(.timeout))
            }
        }

        if directReady.contains(host), let conn = directConns[host] {
            writeFrame(host: host, conn: conn, event: event, waiter: waiter)
            return
        }
        directQueued[host, default: []].append((event, waiter))
        if directConns[host] == nil {
            dialDirect(endpoint, order: endpoint.dialOrder)
        }
    }

    private func removeWaiter(host: String, id: UInt64) {
        if var list = directWaiters[host] {
            list.removeAll { $0.id == id }
            directWaiters[host] = list.isEmpty ? nil : list
        }
        if var list = directQueued[host] {
            list.removeAll { $0.1.id == id }
            directQueued[host] = list.isEmpty ? nil : list
        }
    }

    private func writeFrame(host: String, conn: NWConnection, event: Data, waiter: DirectWaiter) {
        var line = event
        line.append(0x0A)
        directWaiters[host, default: []].append(waiter)
        conn.send(content: line, completion: .contentProcessed { [weak self] error in
            if let error {
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        self?.dropDirect(host, with: .unreachable(error.localizedDescription))
                    }
                }
            }
        })
    }

    private func dialDirect(_ c: AppTailnetComputer, order: [String]) {
        guard active, let first = order.first, let port = NWEndpoint.Port(rawValue: c.port) else {
            dropDirect(c.host, with: .missingEndpoint)
            return
        }
        let rest = Array(order.dropFirst())
        let conn = NWConnection(host: NWEndpoint.Host(first), port: port, using: .tcp)
        directConns[c.host] = conn
        var settled = false
        let fail: (TailscaleRPCError) -> Void = { [weak self] err in
            guard let self, !settled else { return }
            settled = true
            conn.cancel()
            guard self.directConns[c.host] === conn else { return }
            self.directConns[c.host] = nil
            if case .refused(let code) = err {
                self.onReachabilityChange?(c.host, .refused(code))
                self.dropDirect(c.host, with: err)
                return
            }
            if !rest.isEmpty && self.active {
                self.dialDirect(c, order: rest)
            } else {
                self.onReachabilityChange?(c.host, .unreachable(err.localizedDescription))
                self.dropDirect(c.host, with: err)
            }
        }

        conn.stateUpdateHandler = { [weak self] state in
            switch state {
            case .ready:
                conn.send(content: AppTailnet.directHelloLine(), completion: .contentProcessed { _ in })
                Self.readFrame(conn, Data()) { line, restData in
                    DispatchQueue.main.async {
                        MainActor.assumeIsolated {
                            guard let self, !settled, self.directConns[c.host] === conn else { return }
                            guard let line else {
                                fail(.unreachable("Host closed before Tailscale handshake"))
                                return
                            }
                            let ack = AppTailnet.directAck(line)
                            guard ack.ok else {
                                if let code = ack.refused, !code.isEmpty {
                                    fail(.refused(code))
                                } else {
                                    fail(.refused("malformed"))
                                }
                                return
                            }
                            settled = true
                            self.directReady.insert(c.host)
                            self.reconnectAttempts[c.host] = 0
                            self.consecutiveTimeouts[c.host] = 0
                            self.reconnectTasks.removeValue(forKey: c.host)?.cancel()
                            self.directBuffers[c.host] = restData
                            self.onReachabilityChange?(c.host, .reachable)
                            if let subSession = self.subscribedSessions[c.host] {
                                let subLine = AppTailnet.subscribeLine(session: subSession)
                                if !subLine.isEmpty {
                                    conn.send(content: subLine, completion: .contentProcessed { _ in })
                                }
                                self.onLivePushStateChange?(c.host, true)
                            }
                            let queued = self.directQueued.removeValue(forKey: c.host) ?? []
                            for (ev, waiter) in queued {
                                self.writeFrame(host: c.host, conn: conn, event: ev, waiter: waiter)
                            }
                            self.drainDirectBuffer(host: c.host)
                            self.receiveDirect(host: c.host, conn: conn)
                        }
                    }
                }
            case .failed(let err):
                DispatchQueue.main.async { MainActor.assumeIsolated { fail(.unreachable(err.localizedDescription)) } }
            case .waiting:
                DispatchQueue.main.async { MainActor.assumeIsolated { fail(.unreachable("No route over Tailscale")) } }
            case .cancelled:
                DispatchQueue.main.async { MainActor.assumeIsolated { fail(.unreachable("Connection cancelled")) } }
            default:
                break
            }
        }
        conn.start(queue: .main)
        DispatchQueue.main.asyncAfter(deadline: .now() + 3.5) {
            MainActor.assumeIsolated {
                if !settled { fail(.unreachable("Timed out reaching \(first):\(c.port) over Tailscale")) }
            }
        }
    }

    private func deliverDirectLine(host: String, line: Data) {
        consecutiveTimeouts[host] = 0
        if let push = AppTailnet.parsePush(line) {
            onPushEvent?(host, push)
            return
        }
        guard var waiters = directWaiters[host], !waiters.isEmpty else { return }
        if let idx = waiters.firstIndex(where: { $0.accept(line) }) {
            let matched = waiters.remove(at: idx)
            directWaiters[host] = waiters.isEmpty ? nil : waiters
            matched.finish(.success(line))
        }
    }

    private func drainDirectBuffer(host: String) {
        var buf = directBuffers[host] ?? Data()
        while let nl = buf.firstIndex(of: 0x0A) {
            let line = Data(buf[buf.startIndex..<nl])
            buf.removeSubrange(buf.startIndex...nl)
            deliverDirectLine(host: host, line: line)
        }
        if buf.count > 524_288 {
            dropDirect(host, with: .invalidResponse)
            return
        }
        directBuffers[host] = buf
    }

    private func receiveDirect(host: String, conn: NWConnection) {
        conn.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { [weak self] data, _, complete, error in
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    guard let self, self.directConns[host] === conn else { return }
                    if let data, !data.isEmpty {
                        var buf = self.directBuffers[host] ?? Data()
                        buf.append(data)
                        self.directBuffers[host] = buf
                        self.drainDirectBuffer(host: host)
                        guard self.directConns[host] === conn else { return }
                    }
                    if error != nil || complete {
                        self.dropDirect(host, with: .unreachable("Tailscale connection closed"))
                        return
                    }
                    self.receiveDirect(host: host, conn: conn)
                }
            }
        }
    }

    private func dropDirect(_ host: String, with error: TailscaleRPCError, scheduleReconnect shouldReconnect: Bool = true) {
        let wasReady = directReady.contains(host)
        directConns.removeValue(forKey: host)?.cancel()
        directReady.remove(host)
        directBuffers.removeValue(forKey: host)
        consecutiveTimeouts.removeValue(forKey: host)
        if wasReady || subscribedSessions[host] != nil {
            onLivePushStateChange?(host, false)
        }
        let waiters = directWaiters.removeValue(forKey: host) ?? []
        let queued = directQueued.removeValue(forKey: host) ?? []
        for w in waiters { w.finish(.failure(error)) }
        for (_, w) in queued { w.finish(.failure(error)) }
        if shouldReconnect, active, subscribedSessions[host] != nil {
            if case .refused = error { return }
            scheduleReconnect(for: host)
        }
    }

    private func dropAllDirect(with error: TailscaleRPCError, scheduleReconnect shouldReconnect: Bool = true) {
        let hosts = Set(directConns.keys)
            .union(directWaiters.keys)
            .union(directQueued.keys)
            .union(subscribedSessions.keys)
        for h in hosts { dropDirect(h, with: error, scheduleReconnect: shouldReconnect) }
    }

    nonisolated private static func readFrame(_ conn: NWConnection, _ have: Data, _ done: @escaping @Sendable (Data?, Data) -> Void) {
        if let nl = have.firstIndex(of: 0x0A) {
            done(Data(have[have.startIndex..<nl]), Data(have[have.index(after: nl)...]))
            return
        }
        conn.receive(minimumIncompleteLength: 1, maximumLength: AppTailnet.answerMax) { data, _, complete, error in
            var got = have
            if let data { got.append(data) }
            if let nl = got.firstIndex(of: 0x0A) {
                done(Data(got[got.startIndex..<nl]), Data(got[got.index(after: nl)...]))
                return
            }
            if got.count > AppTailnet.answerMax || error != nil || complete {
                done(nil, Data())
                return
            }
            readFrame(conn, got, done)
        }
    }
}
