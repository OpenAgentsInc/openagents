// Firm reason: 6 (the Keychain and the relay's WebSocket: platform effects; every protocol rule is Coax).
// The phone's own computers, with no OpenAgents account (GitHub #13645, for #13620): the device
// service behind the core's FX_DEVICE_LIST, FX_DEVICE_PAIR and FX_DEVICE_CALL
// (apps/shared/app/device/lane.coax).
//
// The phone has its own secp256k1 device key, made once and kept in the Keychain (this device
// only, after first unlock, never synced), with the computers it is paired with beside it. A
// computer runs `openagents-environment pair --workspace DIR`, which prints an
// openagents://pair? link with its key, its relay and a one-time code. Opening the link asks that
// computer to pair; the code, inside the encrypted request, is what it allows.
//
// Every request and reply is sealed, opened and checked by seal.coax (dv_*, through
// core-host.c); this file holds the key, moves bytes over the relay's WebSocket and answers the
// relay's AUTH challenge with the event dv_auth signs. The key is passed to each call and never
// enters the core, its outbox or a log.
import Foundation
import Security

/// One computer this phone is paired with. The grant works only with this phone's key.
struct AppPaired: Codable, Equatable {
    var host: String; var relay: String; var grant: String; var epoch: Int64; var expires: Int64; var name: String
}

@MainActor final class AppDeviceLink: AppDeviceService {
    static let shared = AppDeviceLink()

    private let keyStore = KeychainDeviceKeyStore(account: "own-computer-key-v1")
    private let pairStore = KeychainDeviceKeyStore(account: "own-computer-paired-v1")
    private var sockets: [String: AppRelaySocket] = [:]
    /// Pairing links opened before the core could take them.
    private var waiting: [String] = []
    /// The core's entry for a pairing link (AppServices sets it once the core is up).
    var onLink: ((String) -> Void)? { didSet { let links = waiting; waiting = []; for l in links { onLink?(l) } } }

    /// A link the system opened in the app (OpenAgentsApp's URL handler).
    func opened(_ link: String) {
        if let onLink { onLink(link) } else if waiting.count < 4 { waiting.append(link) }
    }

    // ---------------------------------------------------------------- the key and the pairings

    private func secret() -> [UInt8]? {
        var pub = [UInt8](repeating: 0, count: 64)
        if let kept = keyStore.load(), kept.count == 32 {
            let k = [UInt8](kept)
            if oa_device_public(k, &pub) == 64 { return k }
        }
        for _ in 0..<4 {
            guard let k = AppDeviceLink.random(32) else { return nil }
            if oa_device_public(k, &pub) == 64 { keyStore.save(Data(k)); return k }
        }
        return nil
    }
    var paired: [AppPaired] {
        get { pairStore.load().flatMap { try? JSONDecoder().decode([AppPaired].self, from: $0) } ?? [] }
        set { if let data = try? JSONEncoder().encode(newValue) { pairStore.save(data) } }
    }
    static func random(_ n: Int) -> [UInt8]? {
        var b = [UInt8](repeating: 0, count: n)
        return SecRandomCopyBytes(kSecRandomDefault, n, &b) == errSecSuccess ? b : nil
    }
    private static var now: Int64 { Int64(Date().timeIntervalSince1970) }

    // ---------------------------------------------------------------- the core's services

    /// The paired computers: one a line, its key, TAB, its name.
    func list() -> String {
        paired.filter { $0.expires > AppDeviceLink.now }.map { "\($0.host)\t\($0.name)\n" }.joined()
    }

    /// Asks the computer in `link` to pair, then waits for it to allow this phone (at most ten minutes).
    func pair(_ link: String, done: @escaping (_ paired: Bool, _ words: String) -> Void) {
        guard let parts = URLComponents(string: link), parts.scheme == "openagents", parts.host == "pair" else { done(false, "That is not a pairing link."); return }
        let q = Dictionary((parts.queryItems ?? []).map { ($0.name, $0.value ?? "") }, uniquingKeysWith: { a, _ in a })
        let host = q["host"] ?? "", relay = q["relay"] ?? "", code = q["code"] ?? ""
        let name = String((q["name"] ?? "").filter { !$0.isNewline }.prefix(80))
        guard host.count == 64, host.allSatisfy({ $0.isHexDigit && !$0.isUppercase }), relay.hasPrefix("wss://"), (4...16).contains(code.count), code.allSatisfy({ $0.isLetter || $0.isNumber }) else {
            done(false, "That pairing link is not complete. Run the pairing again on your computer."); return
        }
        let shown = name.isEmpty ? "your computer" : name
        exchange(op: 1, host: host, relay: relay, arg: "OpenAgents iPhone \(code)", epoch: -1, params: Data(), timeout: 20) { [weak self] result, _ in
            guard let self, let answer = result.flatMap({ try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }), let pairing = answer["pairing"] as? String else {
                done(false, "We could not reach \(shown). Check that it is on and still waiting to pair."); return
            }
            self.wait(host: host, relay: relay, pairing: pairing, name: name.isEmpty ? "My computer" : name, until: Date().addingTimeInterval(600), answer: answer, done: done)
        }
    }
    private func wait(host: String, relay: String, pairing: String, name: String, until: Date, answer: [String: Any], done: @escaping (Bool, String) -> Void) {
        let state = answer["state"] as? String ?? ""
        if state == "allowed", let grant = answer["grant"] as? String {
            var list = paired.filter { $0.host != host }
            let epoch = (answer["epoch"] as? NSNumber)?.int64Value ?? 0
            let expires = (answer["expires"] as? NSNumber)?.int64Value ?? AppDeviceLink.now + 7 * 86400
            list.append(AppPaired(host: host, relay: relay, grant: grant, epoch: epoch, expires: expires, name: name))
            paired = list
            done(true, name); return
        }
        if state == "denied" { done(false, "\(name) did not allow this phone."); return }
        if state == "expired" || Date() > until { done(false, "The pairing ran out of time. Run it again on your computer."); return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.exchange(op: 2, host: host, relay: relay, arg: pairing, epoch: -1, params: Data(), timeout: 20) { result, _ in
                    let next = result.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] } ?? ["state": "asked"]
                    self.wait(host: host, relay: relay, pairing: pairing, name: name, until: until, answer: next, done: done)
                }
            }
        }
    }

    /// One environment request to paired computer `host`: its view, or no answer
    /// (.unknown once the relay took the request, since the computer may have run it).
    func call(host: String, body: Data, done: @escaping (AppCompletion.State, Data) -> Void) {
        guard let p = paired.first(where: { $0.host == host }) else { done(.failed, Data()); return }
        exchange(op: 3, host: host, relay: p.relay, arg: p.grant, epoch: p.epoch, params: body, timeout: 25) { result, sent in
            if let result { done(.done, result) } else { done(sent ? .unknown : .failed, Data()) }
        }
    }

    // ---------------------------------------------------------------- one sealed exchange

    /// Seals the request, sends it, and answers with its opened reply (nil: none in time) and whether the relay took it.
    private func exchange(op: Int64, host: String, relay: String, arg: String, epoch: Int64, params: Data, timeout: TimeInterval, done: @escaping (Data?, Bool) -> Void) {
        guard let sk = secret(), let random = AppDeviceLink.random(96), let work = oa_device_work_open() else { done(nil, false); return }
        var event = [UInt8](repeating: 0, count: 131072)
        let p = [UInt8](params)
        let n = oa_device_seal(work, op, sk, host, relay, arg, epoch, p, Int64(p.count), AppDeviceLink.now, random, &event, Int64(event.count))
        var filter = [UInt8](repeating: 0, count: 512), id = [UInt8](repeating: 0, count: 64)
        let fn = oa_device_filter(work, &filter, Int64(filter.count))
        guard n > 0, fn > 0, oa_device_event_id(work, &id) == 64 else { oa_device_work_close(work); done(nil, false); return }
        let socket = sockets[relay] ?? AppRelaySocket(url: relay, sign: { challenge in
            guard let sk = self.secret(), let aux = AppDeviceLink.random(32) else { return nil }
            var out = [UInt8](repeating: 0, count: 4096); let c = [UInt8](challenge.utf8)
            let m = oa_device_auth(sk, relay, c, Int64(c.count), AppDeviceLink.now, aux, &out, Int64(out.count))
            return m > 0 ? String(decoding: out[0..<Int(m)], as: UTF8.self) : nil
        })
        sockets[relay] = socket
        var finished = false
        let finish = { (result: Data?, sent: Bool) in
            if finished { return }
            finished = true; oa_device_work_close(work); done(result, sent)
        }
        socket.ask(filter: String(decoding: filter[0..<Int(fn)], as: UTF8.self), event: String(decoding: event[0..<Int(n)], as: UTF8.self),
                   eventID: String(decoding: id, as: UTF8.self), timeout: timeout,
                   reply: { [weak self] raw in
                       guard self != nil, !finished, let sk = self?.secret() else { return false }
                       var out = [UInt8](repeating: 0, count: 262144); let e = [UInt8](raw.utf8)
                       let m = oa_device_open(work, sk, host, e, Int64(e.count), AppDeviceLink.now, &out, Int64(out.count))
                       if m > 0 { finish(Data(out[0..<Int(m)]), true); return true }
                       return false
                   }, ended: { sent in finish(nil, sent) })
    }
}

/// One relay connection, kept open and reused: NIP-42 AUTH, then each request's REQ and EVENT,
/// and every EVENT delivered to the request whose subscription it answers.
@MainActor final class AppRelaySocket: NSObject {
    private let url: String
    private let sign: (String) -> String?
    private var task: URLSessionWebSocketTask?
    private var ready = false
    private var queued: [() -> Void] = []
    private var authID = ""
    private struct Ask { var reply: (String) -> Bool; var ended: (Bool) -> Void; var sent: Bool; var eventID: String }
    private var asks: [String: Ask] = [:]
    private var serial = 0

    init(url: String, sign: @escaping (String) -> String?) { self.url = url; self.sign = sign }

    func ask(filter: String, event: String, eventID: String, timeout: TimeInterval, reply: @escaping (String) -> Bool, ended: @escaping (Bool) -> Void) {
        serial += 1
        let sub = "oa\(serial)"
        asks[sub] = Ask(reply: reply, ended: ended, sent: false, eventID: eventID)
        whenReady { [weak self] in
            guard let self, self.asks[sub] != nil else { return }
            self.send("[\"REQ\",\"\(sub)\",\(filter)]")
            self.send("[\"EVENT\",\(event)]")
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + timeout) { [weak self] in MainActor.assumeIsolated { self?.end(sub) } }
    }
    private func end(_ sub: String) {
        guard let a = asks.removeValue(forKey: sub) else { return }
        if ready { send("[\"CLOSE\",\"\(sub)\"]") }
        a.ended(a.sent)
    }

    private func whenReady(_ go: @escaping () -> Void) {
        if ready { go(); return }
        queued.append(go)
        guard task == nil, let u = URL(string: url) else { return }
        let t = URLSession.shared.webSocketTask(with: u)
        task = t; t.resume(); receive(t)
        // a relay that asks for no AUTH is ready once it has had a moment to ask
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak self] in MainActor.assumeIsolated { if let self, self.task === t, self.authID.isEmpty { self.open() } } }
    }
    private func open() {
        guard !ready else { return }
        ready = true
        let go = queued; queued = []
        for g in go { g() }
    }
    private func send(_ text: String) { task?.send(.string(text)) { _ in } }
    private func receive(_ t: URLSessionWebSocketTask) {
        t.receive { [weak self] result in
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    guard let self, self.task === t else { return }
                    switch result {
                    case .failure: self.drop()
                    case .success(let message):
                        if case .string(let text) = message { self.heard(text) }
                        self.receive(t)
                    }
                }
            }
        }
    }
    private func heard(_ text: String) {
        guard let m = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [Any], let verb = m.first as? String else { return }
        if verb == "AUTH", m.count > 1, let challenge = m[1] as? String {
            guard let event = sign(challenge), let e = try? JSONSerialization.jsonObject(with: Data(event.utf8)) as? [String: Any] else { return }
            authID = e["id"] as? String ?? ""
            send("[\"AUTH\",\(event)]")
        } else if verb == "OK", m.count > 2, let id = m[1] as? String {
            let accepted = (m[2] as? Bool) ?? false
            if id == authID { if accepted { open() } else { drop() }; return }
            for (sub, a) in asks where a.eventID == id {
                if accepted { asks[sub]?.sent = true } else { end(sub) }
            }
        } else if verb == "EVENT", m.count > 2, let sub = m[1] as? String, let a = asks[sub],
                  let raw = try? JSONSerialization.data(withJSONObject: m[2], options: [.withoutEscapingSlashes]) {
            // the relay has the request once it answers for it
            asks[sub]?.sent = true
            if a.reply(String(decoding: raw, as: UTF8.self)) { asks.removeValue(forKey: sub); send("[\"CLOSE\",\"\(sub)\"]") }
        } else if verb == "CLOSED", m.count > 1, let sub = m[1] as? String {
            end(sub)
        }
    }
    /// The connection ended: every request on it ends (unknown once the relay took it), and the next one connects again.
    private func drop() {
        task?.cancel(with: .goingAway, reason: nil); task = nil; ready = false; authID = ""
        let waiting = asks; asks = [:]; queued = []
        for (_, a) in waiting { a.ended(a.sent) }
    }
}
