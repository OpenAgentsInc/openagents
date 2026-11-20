// Firm reason: 6 (Network.framework: Bonjour browsing and a TCP connection over the tailnet).
// The phone's half of zero-config pairing with Coder Desktop over Tailscale (GitHub #13778;
// AppTailnet.swift has the rules and the steps). While the app is in front it browses the local
// network for `_openagents-coder._tcp`, keeps each Mac's record (AppTailnetBook), and asks a Mac
// it holds no grant for, at the Mac's tailnet address, for a pairing link. The link goes to
// AppDeviceLink.opened, the same path a scanned link takes, so pairing, the grant and every later
// request are the existing relay ones.
//
// The saved address is used again whenever the app comes to the front or its network changes
// (Wi-Fi to cellular and back), with no discovery: a phone away from home re-pairs over its
// tailnet when its grant nears its end. Without Tailscale the request simply finds no one, and
// nothing is shown.
import Foundation
import Network

@MainActor final class AppTailnetLink {
    let book: AppTailnetBook
    private var browser: NWBrowser?
    private var connections: [String: NWConnection] = [:]
    private var lastAsked: [String: Int64] = [:]
    private var active = false
    /// The grant's end for a paired host, if the phone holds one (AppDeviceLink's store).
    private let expires: (String) -> Int64?
    /// Opens a pairing link the way a scanned one opens (AppDeviceLink.opened).
    private let open: (String) -> Void
    /// Whether it browses the local network (tailnet-check.swift turns it off to test the saved Macs alone).
    var browses = true

    init(book: AppTailnetBook? = nil, expires: @escaping (String) -> Int64?, open: @escaping (String) -> Void) {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        self.book = book ?? AppTailnetBook(url: support.appendingPathComponent("OpenAgents/tailnet-computers-v1.json"))
        self.expires = expires; self.open = open
    }
    private static var now: Int64 { Int64(Date().timeIntervalSince1970) }

    /// The app is in front: browse, and ask every saved Mac that needs it.
    func resume() {
        active = true
        if browses { startBrowser() }
        askSaved()
    }
    /// The app leaves the front: stop browsing and drop open requests (iOS would end them anyway).
    func suspend() {
        active = false
        browser?.cancel(); browser = nil
        for (_, c) in connections { c.cancel() }
        connections = [:]
    }
    /// Wi-Fi to cellular or back: the saved tailnet addresses are tried again, no discovery needed.
    func networkChanged() {
        guard active else { return }
        for host in Array(lastAsked.keys) where connections[host] == nil { lastAsked[host] = nil }
        askSaved()
    }

    private func askSaved() { for c in book.computers { ask(c) } }

    private func startBrowser() {
        guard browser == nil else { return }
        let b = NWBrowser(for: .bonjourWithTXTRecord(type: AppTailnet.service, domain: nil), using: .tcp)
        b.browseResultsChangedHandler = { [weak self] results, _ in
            let records: [[String: String]] = results.compactMap {
                if case .bonjour(let txt) = $0.metadata { return txt.dictionary }
                return nil
            }
            DispatchQueue.main.async { MainActor.assumeIsolated { self?.found(records) } }
        }
        // a refused local-network permission or a failed browse leaves the saved Macs working
        b.stateUpdateHandler = { [weak self] state in
            if case .failed = state { DispatchQueue.main.async { MainActor.assumeIsolated { self?.browser?.cancel(); self?.browser = nil } } }
        }
        b.start(queue: .main)
        browser = b
    }

    private func found(_ records: [[String: String]]) {
        guard active else { return }
        for txt in records {
            guard let c = AppTailnetComputer(txt: txt, seen: AppTailnetLink.now) else { continue }
            let old = book.computers.first(where: { $0.host == c.host })
            book.remember(c)
            // a Mac whose address changed is asked again at once
            if let old, old.ip != c.ip || old.port != c.port { lastAsked[c.host] = nil }
            ask(c)
        }
    }

    /// Asks Mac `c` for a pairing link when the phone needs one, trying its address, then its name.
    private func ask(_ c: AppTailnetComputer) {
        let now = AppTailnetLink.now
        guard active, connections[c.host] == nil,
              AppTailnetBook.wantsAdmission(expires: expires(c.host), lastAsked: lastAsked[c.host], now: now) else { return }
        lastAsked[c.host] = now
        dial(c, order: c.dialOrder)
    }
    private func dial(_ c: AppTailnetComputer, order: [String]) {
        guard active, let first = order.first, let port = NWEndpoint.Port(rawValue: c.port) else { connections[c.host] = nil; return }
        let rest = Array(order.dropFirst())
        let conn = NWConnection(host: NWEndpoint.Host(first), port: port, using: .tcp)
        connections[c.host] = conn
        var done = false
        let finish: (AppTailnet.Answer?) -> Void = { [weak self] answer in
            guard let self, !done else { return }
            done = true; conn.cancel()
            if self.connections[c.host] === conn { self.connections[c.host] = nil }
            if let link = answer?.invitation {
                var seen = c; seen.seen = AppTailnetLink.now; self.book.remember(seen)
                self.open(link)
            } else if answer == nil, !rest.isEmpty {
                self.dial(c, order: rest)
            }
        }
        conn.stateUpdateHandler = { state in
            switch state {
            case .ready:
                conn.send(content: AppTailnet.requestLine(), completion: .contentProcessed { _ in })
                AppTailnetLink.readLine(conn, Data()) { line in
                    DispatchQueue.main.async { MainActor.assumeIsolated { finish(line.flatMap { AppTailnet.answer($0, host: c.host) }) } }
                }
            case .failed, .cancelled:
                DispatchQueue.main.async { MainActor.assumeIsolated { finish(nil) } }
            case .waiting:
                // no route (Tailscale off): give up on this address now rather than wait for one
                DispatchQueue.main.async { MainActor.assumeIsolated { finish(nil) } }
            default: break
            }
        }
        conn.start(queue: .main)
        DispatchQueue.main.asyncAfter(deadline: .now() + 10) { MainActor.assumeIsolated { finish(nil) } }
    }

    /// Reads up to one newline (the answer), at most AppTailnet.answerMax bytes.
    nonisolated private static func readLine(_ conn: NWConnection, _ have: Data, _ done: @escaping @Sendable (Data?) -> Void) {
        conn.receive(minimumIncompleteLength: 1, maximumLength: AppTailnet.answerMax) { data, _, complete, error in
            var got = have
            if let data { got.append(data) }
            if let nl = got.firstIndex(of: 0x0A) { done(got[got.startIndex..<nl]); return }
            if got.count > AppTailnet.answerMax || error != nil || complete { done(nil); return }
            readLine(conn, got, done)
        }
    }
}
