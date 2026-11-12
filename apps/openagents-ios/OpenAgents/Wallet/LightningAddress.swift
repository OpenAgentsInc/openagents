import Foundation

/// The person's name@openagents.com Lightning Address, as the wallet screen's card shows it
/// (docs/auth/lightning-addresses.md, "The one-tap claim"; GitHub #13209). The server
/// (coax/web/api/lnaddr.c and lnaddr-policy.coax) makes every decision: whether a name is free,
/// whether a claim stands, whether the address receives. This file reads its answers, builds its
/// requests and words them; the wallet's own key signs the proof the server asks for.
///
/// The claim, in one tap: the server writes the exact message the wallet signs
/// (GET /api/lightning/address/proof), SparkHost signs it with the wallet's Spark identity key,
/// and the claim carries the key, the signature and the message's nonce and expiry.
enum LightningAddress {
    /// The limits the card shows before the address is used.
    struct Limits: Equatable {
        var minMsat: Int64 = 1_000
        var maxMsat: Int64 = 1_000_000_000
        var commentChars: Int = 280
    }

    /// What the server says about this account's address.
    struct Card: Equatable {
        var limits = Limits()
        /// "alice@openagents.com", or nil when the account has none.
        var address: String?
        var name = ""
        /// none, spark or forward.
        var mode = "none"
        /// active, off, suspended or retired.
        var state = "active"
        var forwardTo = ""
        var sparkReceiving = false
        /// A free name from the account's display name, when it has no address yet ("" when none).
        var suggest = ""
        /// A change of where the money goes is waiting.
        var waiting = false
    }

    /// The answer to "is this name free?".
    struct Availability: Equatable {
        var name: String
        var available: Bool
        var reason: String
    }

    /// The message the wallet signs for a claim, as the server wrote it.
    struct Proof: Equatable {
        var message: String
        var nonce: String
        var expires: Int64
    }

    /// The wallet's signature over a proof's message.
    struct Signed: Equatable {
        var pubkey: String
        var signature: String
    }

    // MARK: reading the server's answers

    static func object(_ data: Data) -> [String: Any]? {
        (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    }
    static func int(_ v: Any?) -> Int64? {
        if let n = v as? NSNumber { return n.int64Value }
        if let s = v as? String { return Int64(s) }
        return nil
    }

    static func card(_ data: Data) -> Card? {
        guard let o = object(data) else { return nil }
        var c = Card()
        if let l = o["limits"] as? [String: Any] {
            c.limits.minMsat = int(l["min_msat"]) ?? c.limits.minMsat
            c.limits.maxMsat = int(l["max_msat"]) ?? c.limits.maxMsat
            c.limits.commentChars = Int(int(l["comment_chars"]) ?? Int64(c.limits.commentChars))
        }
        if let a = o["address"] as? String, !a.isEmpty { c.address = a }
        c.name = o["name"] as? String ?? ""
        c.mode = o["mode"] as? String ?? "none"
        c.state = o["state"] as? String ?? "active"
        c.forwardTo = o["forward_to"] as? String ?? ""
        c.sparkReceiving = o["spark_receiving"] as? Bool ?? false
        c.suggest = o["suggest"] as? String ?? ""
        c.waiting = o["waiting"] is [String: Any]
        // The address's own lower ceiling, when the person set one.
        if c.address != nil, let m = int(o["max_msat"]), m > 0, m < c.limits.maxMsat { c.limits.maxMsat = m }
        return c
    }

    static func availability(_ data: Data) -> Availability? {
        guard let o = object(data), let available = o["available"] as? Bool else { return nil }
        return Availability(name: o["name"] as? String ?? "", available: available, reason: o["reason"] as? String ?? "")
    }

    static func proof(_ data: Data) -> Proof? {
        guard let o = object(data), let m = o["message"] as? String, let n = o["nonce"] as? String, let e = int(o["expires"]) else { return nil }
        return Proof(message: m, nonce: n, expires: e)
    }

    // MARK: words

    /// "1,000,000"
    static func grouped(_ n: Int64) -> String {
        let f = NumberFormatter(); f.numberStyle = .decimal; f.locale = Locale(identifier: "en_US")
        return f.string(from: NSNumber(value: n)) ?? String(n)
    }
    /// The one line of limits under the card: "Receives 1 to 1,000,000 sats a payment. Notes up to 280 characters."
    static func limitsLine(_ l: Limits) -> String {
        let low = l.minMsat / 1000, high = l.maxMsat / 1000
        let notes = l.commentChars > 0 ? "Notes up to \(grouped(Int64(l.commentChars))) characters." : "No notes."
        return "Receives \(grouped(low)) to \(grouped(high)) \(high == 1 ? "sat" : "sats") a payment. " + notes
    }

    /// What the card says about where payments go now.
    static func status(_ c: Card) -> String {
        switch c.state {
        case "off": return "Turned off. Payments to this address are refused until you turn it on. The name stays yours."
        case "suspended": return "This address is suspended. We emailed you why."
        case "retired": return "This address no longer receives."
        default: break
        }
        switch c.mode {
        case "spark":
            return c.sparkReceiving ? "Payments go straight to this wallet, even while your phone is off."
                : "This address isn't receiving yet. We'll turn it on when receiving into your wallet is ready; payments are refused until then, so nothing is lost."
        case "forward": return "Payments go to " + c.forwardTo + "."
        default: return "This address isn't receiving yet. Choose where payments go on openagents.com/settings/lightning."
        }
    }

    /// The name as typed, the way the server folds it: trimmed, lowercased, without "@openagents.com".
    static func typed(_ text: String) -> String {
        var t = text.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if let at = t.firstIndex(of: "@") { t = String(t[..<at]) }
        return t
    }

    // MARK: requests

    /// A query value: only unreserved characters are left as they are.
    static func escape(_ s: String) -> String {
        var allowed = CharacterSet.alphanumerics; allowed.insert(charactersIn: "-._~")
        allowed = allowed.intersection(CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~"))
        return s.addingPercentEncoding(withAllowedCharacters: allowed) ?? ""
    }
    static func availablePath(_ name: String) -> String { "/api/lightning/address/available?name=" + escape(name) }
    static func proofPath(_ name: String) -> String { "/api/lightning/address/proof?name=" + escape(name) + "&op=claim" }
    /// The claim, pointing the name at this wallet: the key, its signature and the proof's nonce and expiry.
    static func claimPath(name: String, signed: Signed, proof: Proof) -> String {
        "/api/lightning/address?name=" + escape(name) + "&spark_pubkey=" + escape(signed.pubkey) + "&sig=" + escape(signed.signature)
            + "&nonce=" + escape(proof.nonce) + "&expires=" + String(proof.expires)
    }

    // MARK: the signature

    /// The 64-byte r||s signature as 128 lowercase hex, from what the SDK returned: compact hex
    /// already, or DER hex (a 0x30 sequence of two integers). nil when it is neither.
    static func compact(_ hex: String) -> String? {
        let h = hex.lowercased()
        guard let bytes = unhex(h) else { return nil }
        if bytes.count == 64 { return h }
        // DER: 30 len 02 rlen r 02 slen s
        guard bytes.count >= 8, bytes[0] == 0x30, Int(bytes[1]) == bytes.count - 2 else { return nil }
        var i = 2
        func integer() -> [UInt8]? {
            guard i + 2 <= bytes.count, bytes[i] == 0x02 else { return nil }
            let n = Int(bytes[i + 1]); i += 2
            guard n >= 1, n <= 33, i + n <= bytes.count else { return nil }
            var v = Array(bytes[i..<i + n]); i += n
            while v.count > 32, v.first == 0 { v.removeFirst() }
            guard v.count <= 32 else { return nil }
            return [UInt8](repeating: 0, count: 32 - v.count) + v
        }
        guard let r = integer(), let s = integer(), i == bytes.count else { return nil }
        return (r + s).map { String(format: "%02x", $0) }.joined()
    }
    static func unhex(_ h: String) -> [UInt8]? {
        guard h.count % 2 == 0 else { return nil }
        var out: [UInt8] = []; out.reserveCapacity(h.count / 2)
        var it = h.makeIterator()
        while let a = it.next(), let b = it.next() {
            guard let x = a.hexDigitValue, let y = b.hexDigitValue else { return nil }
            out.append(UInt8(x * 16 + y))
        }
        return out
    }
    /// A compressed secp256k1 public key: 33 bytes, 02 or 03 first.
    static func isKey(_ hex: String) -> Bool {
        guard hex.count == 66, let b = unhex(hex.lowercased()) else { return false }
        return b[0] == 2 || b[0] == 3
    }
}

/// The card's requests to openagents.com, with the app's login. Tests give a fake `send` and token.
final class LightningAddressClient: @unchecked Sendable {
    typealias Send = AppAuth.Send
    /// The request with the login's Authorization header, or a throw when logged out.
    typealias Authorize = @Sendable (URLRequest) async throws -> URLRequest
    /// The wallet signs a message with its Spark identity key.
    typealias Sign = @Sendable (String) async throws -> LightningAddress.Signed

    let origin: String
    private let send: Send
    private let authorize: Authorize
    private let loggedIn: @Sendable () -> Bool

    init(origin: String = AppAuth.shared.origin, send: @escaping Send = AppAuth.network,
         authorize: @escaping Authorize = { try await AppAuth.shared.authorize($0) },
         loggedIn: @escaping @Sendable () -> Bool = { AppAuth.shared.loggedIn }) {
        self.origin = origin; self.send = send; self.authorize = authorize; self.loggedIn = loggedIn
    }

    var isLoggedIn: Bool { loggedIn() }

    /// The server's words for a refusal, or ours.
    private func call(_ method: String, _ path: String) async throws -> Data {
        guard let url = URL(string: origin + path) else { throw AppAuth.Failure.said("That didn't work. Try again.") }
        var r = URLRequest(url: url); r.httpMethod = method
        r.setValue("application/json", forHTTPHeaderField: "Accept")
        r = try await authorize(r)
        let data: Data, status: Int
        do { (data, status) = try await send(r) } catch { throw AppAuth.Failure.unreachable }
        guard (200..<300).contains(status) else {
            throw AppAuth.Failure.said(AppAuth.errorWords(data) ?? (status == 401 ? "Log in again." : "That didn't work. Try again."))
        }
        return data
    }

    func card() async throws -> LightningAddress.Card {
        guard let c = LightningAddress.card(try await call("GET", "/api/lightning/address")) else { throw AppAuth.Failure.said("That didn't work. Try again.") }
        return c
    }
    func available(_ name: String) async throws -> LightningAddress.Availability {
        guard let a = LightningAddress.availability(try await call("GET", LightningAddress.availablePath(name))) else { throw AppAuth.Failure.said("That didn't work. Try again.") }
        return a
    }
    /// Claims name for this wallet: asks for the proof's message, has the wallet sign it, sends the claim.
    func claim(_ name: String, sign: Sign) async throws -> LightningAddress.Card {
        guard let proof = LightningAddress.proof(try await call("GET", LightningAddress.proofPath(name))) else { throw AppAuth.Failure.said("That didn't work. Try again.") }
        let raw = try await sign(proof.message)
        guard LightningAddress.isKey(raw.pubkey), let sig = LightningAddress.compact(raw.signature) else {
            throw AppAuth.Failure.said("Your wallet couldn't sign the claim. Try again.")
        }
        let signed = LightningAddress.Signed(pubkey: raw.pubkey.lowercased(), signature: sig)
        guard let c = LightningAddress.card(try await call("POST", LightningAddress.claimPath(name: name, signed: signed, proof: proof))) else { throw AppAuth.Failure.said("That didn't work. Try again.") }
        return c
    }
    /// Turns the address on or off at once; the name stays the account's.
    func turn(on: Bool) async throws -> LightningAddress.Card {
        guard let c = LightningAddress.card(try await call("POST", on ? "/api/lightning/address/on" : "/api/lightning/address/off")) else { throw AppAuth.Failure.said("That didn't work. Try again.") }
        return c
    }
}
