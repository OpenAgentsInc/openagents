import XCTest
@testable import OpenAgents

/// The wallet's Lightning Address card (#13209) against a fake server: the card's states, the
/// limits line, the claim's proof request and signed claim, refusals in the server's words, and
/// the signature the wallet hands over (compact or DER).
@MainActor
final class LightningAddressTests: XCTestCase {
    final class Server: @unchecked Sendable {
        let lock = NSLock()
        var asked: [(method: String, path: String, auth: String)] = []
        var answers: [String: [(Int, String)]] = [:]
        func send(_ r: URLRequest) async throws -> (Data, Int) {
            let path = r.url.map { $0.path + ($0.query.map { "?" + $0 } ?? "") } ?? ""
            let key = r.url?.path ?? ""
            lock.lock(); defer { lock.unlock() }
            asked.append((r.httpMethod ?? "GET", path, r.value(forHTTPHeaderField: "Authorization") ?? ""))
            var q = answers[key] ?? []
            guard !q.isEmpty else { return (Data("{\"error\":\"Not found.\"}".utf8), 404) }
            let a = q.removeFirst(); answers[key] = q
            return (Data(a.1.utf8), a.0)
        }
        func on(_ path: String, _ status: Int, _ body: String) { lock.lock(); answers[path, default: []].append((status, body)); lock.unlock() }
        var paths: [String] { lock.lock(); defer { lock.unlock() }; return asked.map { $0.method + " " + $0.path } }
    }

    static let limits = "{\"min_msat\":1000,\"max_msat\":1000000000,\"comment_chars\":280,\"invoice_expiry_s\":600}"
    static let none = "{\"limits\":\(limits),\"address\":null,\"suggest\":\"alice\",\"spark_receiving\":false}"
    static func held(mode: String = "spark", state: String = "active", receiving: Bool = false) -> String {
        "{\"limits\":\(limits),\"address\":\"alice@openagents.com\",\"name\":\"alice\",\"mode\":\"\(mode)\",\"state\":\"\(state)\",\"forward_to\":\"alice@getalby.com\",\"spark_pubkey\":\"\",\"revision\":1,\"max_msat\":1000000000,\"comments_on\":true,\"spark_receiving\":\(receiving)}"
    }
    static let key = "02" + String(repeating: "ab", count: 32)
    static let sig = String(repeating: "11", count: 32) + String(repeating: "22", count: 32)

    func client(_ s: Server, loggedIn: Bool = true) -> LightningAddressClient {
        LightningAddressClient(origin: "https://example.test", send: { try await s.send($0) },
                               authorize: { r in var r = r; r.setValue("Bearer oa_at_T", forHTTPHeaderField: "Authorization"); return r },
                               loggedIn: { loggedIn })
    }

    override func setUp() { LightningAddressCardView.notNow = false }

    func testLimitsLine() {
        XCTAssertEqual(LightningAddress.limitsLine(.init()), "Receives 1 to 1,000,000 sats a payment. Notes up to 280 characters.")
        XCTAssertEqual(LightningAddress.limitsLine(.init(minMsat: 1000, maxMsat: 50_000_000, commentChars: 0)), "Receives 1 to 50,000 sats a payment. No notes.")
        let c = LightningAddress.card(Data(Self.none.utf8))!
        XCTAssertEqual(LightningAddress.limitsLine(c.limits), "Receives 1 to 1,000,000 sats a payment. Notes up to 280 characters.")
        XCTAssertNil(c.address); XCTAssertEqual(c.suggest, "alice")
    }

    func testStatusWords() {
        let spark = LightningAddress.card(Data(Self.held().utf8))!
        XCTAssertEqual(LightningAddress.status(spark), "This address isn't receiving yet. We'll turn it on when receiving into your wallet is ready; payments are refused until then, so nothing is lost.")
        XCTAssertEqual(LightningAddress.status(LightningAddress.card(Data(Self.held(receiving: true).utf8))!), "Payments go straight to this wallet, even while your phone is off.")
        XCTAssertEqual(LightningAddress.status(LightningAddress.card(Data(Self.held(mode: "forward").utf8))!), "Payments go to alice@getalby.com.")
        XCTAssertTrue(LightningAddress.status(LightningAddress.card(Data(Self.held(state: "off").utf8))!).hasPrefix("Turned off."))
    }

    func testTypedNameAndPaths() {
        XCTAssertEqual(LightningAddress.typed("  Alice@openagents.com "), "alice")
        XCTAssertEqual(LightningAddress.availablePath("a b&c"), "/api/lightning/address/available?name=a%20b%26c")
        XCTAssertEqual(LightningAddress.proofPath("alice"), "/api/lightning/address/proof?name=alice&op=claim")
        let p = LightningAddress.Proof(message: "m", nonce: "00ff00ff00ff00ff", expires: 1_800_000_300)
        XCTAssertEqual(LightningAddress.claimPath(name: "alice", signed: .init(pubkey: Self.key, signature: Self.sig), proof: p),
                       "/api/lightning/address?name=alice&spark_pubkey=\(Self.key)&sig=\(Self.sig)&nonce=00ff00ff00ff00ff&expires=1800000300")
    }

    func testSignatureForms() {
        XCTAssertEqual(LightningAddress.compact(Self.sig.uppercased()), Self.sig)
        // DER with a padded r (0x00 first) and a short s
        let r = "00" + "81" + String(repeating: "01", count: 31), s = String(repeating: "02", count: 31)
        let der = "30" + String(format: "%02x", 2 + 33 + 2 + 31) + "0221" + r + "021f" + s
        XCTAssertEqual(LightningAddress.compact(der), "81" + String(repeating: "01", count: 31) + "00" + s)
        XCTAssertNil(LightningAddress.compact("30"))
        XCTAssertNil(LightningAddress.compact("zz"))
        XCTAssertNil(LightningAddress.compact(String(repeating: "11", count: 63)))
        XCTAssertTrue(LightningAddress.isKey(Self.key))
        XCTAssertFalse(LightningAddress.isKey("04" + String(repeating: "ab", count: 32)))
        XCTAssertFalse(LightningAddress.isKey("02ab"))
    }

    func testClaimSignsTheServersMessageAndSendsTheProof() async throws {
        let s = Server()
        let message = "openagents.lightning-address.v1\naccount acct_7\nname alice\ndomain openagents.com\noperation claim\nrevision 1\nnonce 00ff00ff00ff00ff\nexpires 1800000300"
        let quoted = message.replacingOccurrences(of: "\n", with: "\\n")
        s.on("/api/lightning/address/proof", 200, "{\"message\":\"\(quoted)\",\"nonce\":\"00ff00ff00ff00ff\",\"expires\":1800000300,\"revision\":1}")
        s.on("/api/lightning/address", 200, Self.held())
        let signedFor = Box()
        let c = try await client(s).claim("alice", sign: { m in signedFor.text = m; return .init(pubkey: Self.key.uppercased(), signature: Self.sig) })
        XCTAssertEqual(signedFor.text, message)   // exactly the server's bytes
        XCTAssertEqual(c.address, "alice@openagents.com")
        XCTAssertEqual(s.paths, ["GET /api/lightning/address/proof?name=alice&op=claim",
                                 "POST /api/lightning/address?name=alice&spark_pubkey=\(Self.key)&sig=\(Self.sig)&nonce=00ff00ff00ff00ff&expires=1800000300"])
        XCTAssertTrue(s.asked.allSatisfy { $0.auth == "Bearer oa_at_T" })
    }

    func testClaimRefusalsAndBadSignatures() async {
        let s = Server()
        s.on("/api/lightning/address/proof", 200, "{\"message\":\"m\",\"nonce\":\"n\",\"expires\":5}")
        s.on("/api/lightning/address", 409, "{\"error\":\"That name is taken\",\"code\":5}")
        do { _ = try await client(s).claim("alice", sign: { _ in .init(pubkey: Self.key, signature: Self.sig) }); XCTFail() }
        catch { XCTAssertEqual(error.localizedDescription, "That name is taken") }
        s.on("/api/lightning/address/proof", 200, "{\"message\":\"m\",\"nonce\":\"n\",\"expires\":5}")
        do { _ = try await client(s).claim("alice", sign: { _ in .init(pubkey: "02ab", signature: Self.sig) }); XCTFail() }
        catch { XCTAssertEqual(error.localizedDescription, "Your wallet couldn't sign the claim. Try again.") }
        // a bad signature never reaches the claim
        XCTAssertEqual(s.paths.filter { $0.hasPrefix("POST") }.count, 1)
    }

    func testCardOffersTheSuggestedNameAndClaimsOnlyOnTheTap() async throws {
        let s = Server()
        s.on("/api/lightning/address", 200, Self.none)
        s.on("/api/lightning/address/available", 200, "{\"name\":\"alice\",\"available\":true,\"reason\":\"\"}")
        let card = LightningAddressCardView(client: client(s), sign: { _ in .init(pubkey: Self.key, signature: Self.sig) })
        await card.load()
        XCTAssertEqual(card.phase, .unclaimed)
        XCTAssertEqual(card.field.text, "alice")
        XCTAssertEqual(card.limits.text, "Receives 1 to 1,000,000 sats a payment. Notes up to 280 characters.")
        XCTAssertFalse(card.claim.isEnabled)
        await card.check("alice")
        XCTAssertEqual(card.note.text, "alice@openagents.com is free.")
        XCTAssertTrue(card.claim.isEnabled)
        // never auto-assigned: nothing was posted
        XCTAssertFalse(s.paths.contains { $0.hasPrefix("POST") })
    }

    func testCardSaysWhyANameIsntFree() async {
        let s = Server()
        s.on("/api/lightning/address", 200, Self.none)
        s.on("/api/lightning/address/available", 200, "{\"name\":\"alice\",\"available\":false,\"reason\":\"That name is taken\"}")
        let card = LightningAddressCardView(client: client(s), sign: { _ in .init(pubkey: Self.key, signature: Self.sig) })
        await card.load(); await card.check("alice")
        XCTAssertEqual(card.note.text, "That name is taken")
        XCTAssertFalse(card.claim.isEnabled)
    }

    func testCardShowsTheAddressAndItsHonestState() async {
        let s = Server()
        s.on("/api/lightning/address", 200, Self.held())
        let card = LightningAddressCardView(client: client(s), sign: { _ in .init(pubkey: Self.key, signature: Self.sig) })
        await card.load()
        XCTAssertEqual(card.phase, .claimed)
        XCTAssertEqual(card.status.text, "This address isn't receiving yet. We'll turn it on when receiving into your wallet is ready; payments are refused until then, so nothing is lost.")
        XCTAssertEqual(card.toggle.configuration?.title, "Turn off")
        s.on("/api/lightning/address", 200, Self.held(state: "off"))
        await card.load()
        XCTAssertEqual(card.toggle.configuration?.title, "Turn on")
    }

    func testLoggedOutOffersTheLoginAndNotNowHides() async {
        let card = LightningAddressCardView(client: client(Server(), loggedIn: false), sign: { _ in .init(pubkey: Self.key, signature: Self.sig) })
        await card.load()
        XCTAssertEqual(card.phase, .loggedOut)
        LightningAddressCardView.notNow = true
        await card.load()
        XCTAssertEqual(card.phase, .hidden)
        XCTAssertTrue(card.isHidden)
    }

    final class Box: @unchecked Sendable { var text = "" }
}
