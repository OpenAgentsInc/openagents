// Firm reason: 6 (iOS Network.framework Bonjour discovery, NWPathMonitor, and direct Tailscale TCP transport over port 47109; all wire framing & NIP-44 crypto execute in Coax).
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
        case .missingSecret: return "Device key unavailable in Keychain."
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
/// Zero public relay fallback, zero WebSockets, zero cloud brokers.
@MainActor
final class TailscaleClient {
    let book: AppTailnetBook
    var browsesLocalNetwork = true

    var onDiscoveredChange: (() -> Void)?
    var onReachabilityChange: ((String, ReachabilityState) -> Void)?

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
        dropAllDirect(with: .unreachable("App entered background"))
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
                        self.dropAllDirect(with: .unreachable("Network changed; reconnecting"))
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

    // MARK: - Sealed NIP-44 RPC over direct Tailscale TCP

    func sendRPC(
        paired: PairedComputer,
        endpoint: AppTailnetComputer,
        secret: Data,
        method: String,
        params: Data,
        timeout: TimeInterval = 18.0
    ) async -> Result<Data, TailscaleRPCError> {
        guard let work = MobileCoreWork() else { return .failure(.encodingFailed) }
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
                    guard let payload = openedPayload,
                          MobileCore.validateView(work: work, host: host, method: method, raw: payload) else {
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
        let boundedTimeout = min(max(timeout, 4.0), 20.0)
        DispatchQueue.main.asyncAfter(deadline: .now() + boundedTimeout) { [weak self] in
            MainActor.assumeIsolated {
                guard let self, !finished else { return }
                self.removeWaiter(host: host, id: waiterID)
                if self.directReady.contains(host) {
                    self.dropDirect(host, with: .timeout)
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
                            self.directBuffers[c.host] = restData
                            self.onReachabilityChange?(c.host, .reachable)
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
        if buf.count > 262_144 {
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

    private func dropDirect(_ host: String, with error: TailscaleRPCError) {
        directConns.removeValue(forKey: host)?.cancel()
        directReady.remove(host)
        directBuffers.removeValue(forKey: host)
        let waiters = directWaiters.removeValue(forKey: host) ?? []
        let queued = directQueued.removeValue(forKey: host) ?? []
        for w in waiters { w.finish(.failure(error)) }
        for (_, w) in queued { w.finish(.failure(error)) }
    }

    private func dropAllDirect(with error: TailscaleRPCError) {
        let hosts = Set(directConns.keys).union(directWaiters.keys).union(directQueued.keys)
        for h in hosts { dropDirect(h, with: error) }
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
