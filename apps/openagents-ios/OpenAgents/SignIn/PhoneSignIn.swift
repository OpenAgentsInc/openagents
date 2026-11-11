import CryptoKit
import Foundation

/// Sign in to openagents.com with the phone: verse/phone-signin.coax decides
/// whether a scanned, pasted or opened text is one of our codes and writes
/// the NIP-98 request; this file hashes, signs with the phone's key
/// (NostrKey) and sends it. The code's secret is never logged.
struct PhoneSignIn {
    enum Kind: Int64 { case signin = 1, link = 2, invite = 3 }
    let kind: Kind
    /// What the confirmation sheet asks.
    let question: String
    private let text: [UInt8]
    private let code: coax_t_SigninCode

    /// Debug builds also take codes for 127.0.0.1 and localhost, for tests.
    static var acceptsLocal: Bool {
        #if DEBUG
        return true
        #else
        return false
        #endif
    }

    init?(_ string: String) {
        var bytes = Array(string.utf8)
        guard bytes.count <= 4096 else { return nil }
        let code = bytes.withUnsafeMutableBufferPointer { verse_signin_parse(coax_v_u8(data: $0.baseAddress, count: Int64($0.count)), PhoneSignIn.acceptsLocal) }
        guard let kind = Kind(rawValue: code.kind) else { return nil }
        self.kind = kind; self.code = code; self.text = bytes
        question = PhoneSignIn.write { out in var t = bytes; return t.withUnsafeMutableBufferPointer { verse_signin_question(coax_v_u8(data: $0.baseAddress, count: Int64($0.count)), code, out) } } ?? ""
    }

    /// A Coax writer's bytes as a string, or nil if it refused.
    private static func write(_ cap: Int = 2048, _ body: (coax_v_u8) -> Int64) -> String? {
        var buf = [UInt8](repeating: 0, count: cap)
        let n = buf.withUnsafeMutableBufferPointer { body(coax_v_u8(data: $0.baseAddress, count: Int64(cap))) }
        guard n >= 0 else { return nil }
        return String(decoding: buf[0..<Int(n)], as: UTF8.self)
    }
    private static func view<T>(_ s: String, _ body: (coax_v_u8) -> T) -> T {
        var b = Array(s.utf8)
        return b.withUnsafeMutableBufferPointer { body(coax_v_u8(data: $0.baseAddress, count: Int64($0.count))) }
    }
    private func withText<T>(_ body: (coax_v_u8) -> T) -> T {
        var t = text
        return t.withUnsafeMutableBufferPointer { body(coax_v_u8(data: $0.baseAddress, count: Int64($0.count))) }
    }
    private static func hex(_ d: Data) -> String { d.map { String(format: "%02x", $0) }.joined() }

    /// The npub of the phone's key, for showing.
    static func npub() throws -> String {
        var pub = [UInt8](try NostrKey.publicKey())
        return pub.withUnsafeMutableBufferPointer { p in
            write(120) { verse_signin_npub(coax_v_u8(data: p.baseAddress, count: 32), $0) }
        } ?? ""
    }

    /// A code from the new login page (openagents.com/login#c=..., coax/web/api/auth.c) is approved
    /// at /api/auth/phone/approve with {"challenge"}; the older /signin page's at /api/auth/approve
    /// (until that page is removed, #13207). The Coax parser has already chosen the kind; this
    /// reads only the path of the code it accepted.
    var newLogin: Bool {
        guard kind == .signin, let s = String(bytes: text, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
              let url = URL(string: s) else { return false }
        return url.path == "/login" || url.path == "/login/"
    }

    /// The signed request: Coax writes the URL, the body, the id's serialization and the event.
    func request(now: Date = Date()) throws -> URLRequest {
        let code = code
        guard var url = withText({ t in PhoneSignIn.write { verse_signin_url(t, code, $0) } }),
              var body = withText({ t in PhoneSignIn.write { verse_signin_body(t, code, $0) } }) else { throw NostrKey.Failure.signing }
        if newLogin, let old = url.range(of: "/api/auth/approve", options: .backwards),
           let challenge = (try? JSONSerialization.jsonObject(with: Data(body.utf8)) as? [String: Any])?["challenge"] as? String {
            url.replaceSubrange(old, with: "/api/auth/phone/approve")
            body = "{\"challenge\":\"\(challenge)\"}"
        }
        guard let target = URL(string: url) else { throw NostrKey.Failure.signing }
        var request = URLRequest(url: target, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20)
        request.httpMethod = "POST"
        request.httpBody = Data(body.utf8)
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue(try PhoneSignIn.nip98(url: url, body: body, now: now), forHTTPHeaderField: "Authorization")
        return request
    }

    /// The NIP-98 Authorization value ("Nostr " + base64 event) for a POST of body to url, signed with
    /// the phone's key. Coax writes the id's serialization and the event (verse/phone-signin.coax).
    static func nip98(url: String, body: String, now: Date = Date()) throws -> String {
        let payload = PhoneSignIn.hex(Data(SHA256.hash(data: Data(body.utf8))))
        let pubkey = PhoneSignIn.hex(try NostrKey.publicKey())
        let at = Int64(now.timeIntervalSince1970)
        guard let serial = PhoneSignIn.view(pubkey, { pk in PhoneSignIn.view(url, { u in PhoneSignIn.view(payload, { pl in
            PhoneSignIn.write(4096) { verse_signin_serial(pk, at, u, pl, $0) } }) }) }) else { throw NostrKey.Failure.signing }
        let id = Data(SHA256.hash(data: Data(serial.utf8)))
        let sig = PhoneSignIn.hex(try NostrKey.sign(id))
        let idHex = PhoneSignIn.hex(id)
        guard let event = PhoneSignIn.view(idHex, { i in PhoneSignIn.view(pubkey, { pk in PhoneSignIn.view(url, { u in PhoneSignIn.view(payload, { pl in PhoneSignIn.view(sig, { s in
            PhoneSignIn.write(4096) { verse_signin_event(i, pk, at, u, pl, s, $0) } }) }) }) }) }) else { throw NostrKey.Failure.signing }
        return "Nostr " + Data(event.utf8).base64EncodedString()
    }

    /// Signs and sends it; what to tell the person (verse_signin_result).
    func approve(session: URLSession = .shared) async -> String {
        var status: Int64 = 0
        var answer = Data()
        do {
            let (data, response) = try await session.data(for: try request())
            status = Int64((response as? HTTPURLResponse)?.statusCode ?? 0)
            answer = data.prefix(4096)
            if (200..<300).contains(status) { AppAnalytics.shared.loginApproved("\(kind)") }
        } catch let failure as NostrKey.Failure {
            return failure.localizedDescription
        } catch {
            status = 0
        }
        // The new login's refusals say what happened in their own words (auth.c): show them as given.
        if newLogin, status >= 400, let words = AppAuth.errorWords(answer) { return words }
        var bytes = [UInt8](answer)
        let kind = kind.rawValue
        return bytes.withUnsafeMutableBufferPointer { b in
            PhoneSignIn.write(400) { verse_signin_result(kind, status, coax_v_u8(data: b.baseAddress, count: Int64(b.count)), $0) }
        } ?? ""
    }
}
