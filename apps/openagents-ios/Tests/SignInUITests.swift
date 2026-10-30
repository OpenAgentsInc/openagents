import CryptoKit
import Network
import XCTest

/// Sign in to openagents.com with the phone, against a fake oa-api on this
/// Mac's loopback (Debug builds take http://127.0.0.1 codes): the app opens a
/// code as if the system had opened its link, asks, and on Approve sends a
/// NIP-98 request this test checks byte for byte, including the BIP-340
/// signature (libsecp256k1, the shim the app signs with). The real server's
/// checks are coax/web/api/test_signin*.py.
final class SignInUITests: XCTestCase {
    private let secret = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

    /// One request the fake server took, and what it answers.
    final class Fake {
        let listener: NWListener
        var port: UInt16 { listener.port?.rawValue ?? 0 }
        var seen: [(path: String, auth: String, body: Data)] = []
        var status = 200
        var answer = "{\"ok\":true,\"kind\":\"signin\"}"
        let lock = NSLock()
        init() throws {
            let params = NWParameters.tcp
            params.requiredLocalEndpoint = NWEndpoint.hostPort(host: "127.0.0.1", port: 0)
            listener = try NWListener(using: params)
            listener.newConnectionHandler = { [weak self] c in self?.serve(c) }
            let ready = DispatchSemaphore(value: 0)
            listener.stateUpdateHandler = { if case .ready = $0 { ready.signal() } }
            listener.start(queue: .global())
            _ = ready.wait(timeout: .now() + 5)
        }
        private func serve(_ c: NWConnection) {
            c.start(queue: .global())
            var got = Data()
            func read() {
                c.receive(minimumIncompleteLength: 1, maximumLength: 65536) { data, _, done, _ in
                    if let data { got.append(data) }
                    if let end = got.range(of: Data("\r\n\r\n".utf8)) {
                        let head = String(decoding: got[..<end.lowerBound], as: UTF8.self)
                        let lines = head.components(separatedBy: "\r\n")
                        func header(_ name: String) -> String {
                            lines.first { $0.lowercased().hasPrefix(name.lowercased() + ":") }.map { String($0.dropFirst(name.count + 1)).trimmingCharacters(in: .whitespaces) } ?? ""
                        }
                        let length = Int(header("Content-Length")) ?? 0
                        let body = got[end.upperBound...]
                        if body.count >= length {
                            let path = lines.first?.split(separator: " ").dropFirst().first.map(String.init) ?? ""
                            self.lock.lock(); self.seen.append((path, header("Authorization"), Data(body.prefix(length)))); let s = self.status, a = self.answer; self.lock.unlock()
                            let out = "HTTP/1.1 \(s) X\r\nContent-Type: application/json\r\nContent-Length: \(a.utf8.count)\r\nConnection: close\r\n\r\n\(a)"
                            c.send(content: Data(out.utf8), completion: .contentProcessed { _ in c.cancel() })
                            return
                        }
                    }
                    if done { c.cancel() } else { read() }
                }
            }
            read()
        }
    }

    private func hex(_ d: Data) -> String { d.map { String(format: "%02x", $0) }.joined() }
    private func unhex(_ s: String) -> Data {
        var d = Data(); var i = s.startIndex
        while i < s.endIndex { let j = s.index(i, offsetBy: 2); d.append(UInt8(s[i..<j], radix: 16)!); i = j }
        return d
    }
    private func wait(_ timeout: TimeInterval, _ test: @escaping () -> Bool) -> Bool {
        XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in test() }, object: nil)], timeout: timeout) == .completed
    }
    private func shot(_ app: XCUIApplication, _ name: String) {
        let a = XCTAttachment(screenshot: app.screenshot()); a.name = name; a.lifetime = .keepAlways; add(a)
    }

    /// The request is NIP-98 for url and body, signed by a valid BIP-340 signature; the signer's pubkey.
    private func checkSigned(_ auth: String, url: String, body: Data, file: StaticString = #filePath, line: UInt = #line) -> String {
        XCTAssertTrue(auth.hasPrefix("Nostr "), file: file, line: line)
        guard let raw = Data(base64Encoded: String(auth.dropFirst(6))),
              let ev = try? JSONSerialization.jsonObject(with: raw) as? [String: Any],
              let id = ev["id"] as? String, let pubkey = ev["pubkey"] as? String, let sig = ev["sig"] as? String,
              let at = ev["created_at"] as? Int, let tags = ev["tags"] as? [[String]] else { XCTFail("not an event", file: file, line: line); return "" }
        XCTAssertEqual(ev["kind"] as? Int, 27235, file: file, line: line)
        XCTAssertEqual(ev["content"] as? String, "", file: file, line: line)
        XCTAssertLessThan(abs(Double(at) - Date().timeIntervalSince1970), 60, file: file, line: line)
        let payload = hex(Data(SHA256.hash(data: body)))
        XCTAssertEqual(tags, [["u", url], ["method", "POST"], ["payload", payload]], file: file, line: line)
        let serial = "[0,\"\(pubkey)\",\(at),27235,[[\"u\",\"\(url)\"],[\"method\",\"POST\"],[\"payload\",\"\(payload)\"]],\"\"]"
        let digest = Data(SHA256.hash(data: Data(serial.utf8)))
        XCTAssertEqual(id, hex(digest), file: file, line: line)
        let ok = unhex(pubkey).withUnsafeBytes { p in digest.withUnsafeBytes { m in unhex(sig).withUnsafeBytes { s in
            oa_nostr_verify(p.bindMemory(to: UInt8.self).baseAddress!, m.bindMemory(to: UInt8.self).baseAddress!, s.bindMemory(to: UInt8.self).baseAddress!) } } }
        XCTAssertEqual(ok, 1, "signature", file: file, line: line)
        return pubkey
    }

    private func launch(_ code: String) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments += ["-OAGridSeen", "NO", "-OASignInOpen", code]
        app.launch()
        return app
    }

    func testSignInLinkAndInvite() throws {
        let fake = try Fake()
        let origin = "http://127.0.0.1:\(fake.port)"

        // Sign in: the sheet names the host, Approve sends the request, the phone says what happened.
        var app = launch("\(origin)/signin#c=\(secret)")
        XCTAssertTrue(app.staticTexts["signin.question"].waitForExistence(timeout: 30))
        XCTAssertEqual(app.staticTexts["signin.question"].label, "Log in to 127.0.0.1:\(fake.port)?")
        shot(app, "Sign in?")
        app.buttons["signin.approve"].tap()
        XCTAssertTrue(wait(20) { app.staticTexts["signin.result"].label == "Logged in. Look at your computer." }, app.staticTexts["signin.result"].label)
        shot(app, "Signed in")
        guard fake.seen.count == 1 else { return XCTFail("requests: \(fake.seen.count)") }
        let first = fake.seen[0]
        XCTAssertEqual(first.path, "/api/auth/approve")
        XCTAssertEqual(String(decoding: first.body, as: UTF8.self), "{\"challenge\":\"\(secret)\",\"kind\":\"signin\"}")
        let pubkey = checkSigned(first.auth, url: "\(origin)/api/auth/approve", body: first.body)
        app.buttons["signin.done"].tap()

        // The key screen shows the same key as an npub, and it stays across launches.
        XCTAssertTrue(app.buttons["signin.open"].waitForExistence(timeout: 10))
        app.buttons["signin.open"].tap()
        let npub = app.staticTexts["signin.npub"]
        XCTAssertTrue(npub.waitForExistence(timeout: 10))
        XCTAssertTrue(npub.label.hasPrefix("npub1") && npub.label.count == 63, npub.label)
        shot(app, "Your key")
        app.terminate()

        // Link from a project page: the answer names the member.
        fake.answer = "{\"ok\":true,\"kind\":\"link\",\"name\":\"Ada\"}"
        app = launch("openagents://127.0.0.1:\(fake.port)/signin#link=\(secret)")
        XCTAssertTrue(app.staticTexts["signin.question"].waitForExistence(timeout: 30))
        XCTAssertEqual(app.staticTexts["signin.question"].label, "Link this phone to your project on 127.0.0.1:\(fake.port)?")
        app.buttons["signin.approve"].tap()
        XCTAssertTrue(wait(20) { app.staticTexts["signin.result"].label == "This phone is linked to your project as Ada." }, app.staticTexts["signin.result"].label)
        guard fake.seen.count == 2 else { return XCTFail("requests: \(fake.seen.count)") }
        XCTAssertEqual(String(decoding: fake.seen[1].body, as: UTF8.self), "{\"challenge\":\"\(secret)\",\"kind\":\"link\"}")
        XCTAssertEqual(checkSigned(fake.seen[1].auth, url: "\(origin)/api/auth/approve", body: fake.seen[1].body), pubkey)
        app.terminate()

        // An invite: the claim goes to the project's route; a used one says so.
        fake.answer = "{\"error\": \"not found\"}"; fake.status = 404
        app = launch("\(origin)/projects/fixture-proj#invite=\(secret)")
        XCTAssertTrue(app.staticTexts["signin.question"].waitForExistence(timeout: 30))
        XCTAssertEqual(app.staticTexts["signin.question"].label, "Link this phone to the project you were invited to?")
        app.buttons["signin.approve"].tap()
        XCTAssertTrue(wait(20) { app.staticTexts["signin.result"].label == "That code was used or has expired." }, app.staticTexts["signin.result"].label)
        guard fake.seen.count == 3 else { return XCTFail("requests: \(fake.seen.count)") }
        XCTAssertEqual(fake.seen[2].path, "/api/private/fixture-proj/claim")
        XCTAssertEqual(String(decoding: fake.seen[2].body, as: UTF8.self), "{\"token\":\"\(secret)\"}")
        XCTAssertEqual(checkSigned(fake.seen[2].auth, url: "\(origin)/api/private/fixture-proj/claim", body: fake.seen[2].body), pubkey)
        app.terminate()

        // Cancel sends nothing; a code for another site is not taken.
        app = launch("\(origin)/signin#c=\(secret)")
        XCTAssertTrue(app.buttons["signin.cancel"].waitForExistence(timeout: 30))
        app.buttons["signin.cancel"].tap()
        XCTAssertTrue(wait(10) { !app.staticTexts["signin.question"].exists })
        app.terminate()
        app = launch("https://example.com/signin#c=\(secret)")
        XCTAssertTrue(app.buttons["signin.open"].waitForExistence(timeout: 30))
        XCTAssertFalse(app.staticTexts["signin.question"].waitForExistence(timeout: 3))
        XCTAssertEqual(fake.seen.count, 3)
    }

    /// The key screen in the lander's HUD: its buttons keep their words for
    /// VoiceOver, Copy says so, and Done closes it.
    func testKeyScreen() {
        let app = XCUIApplication()
        app.launchArguments += ["-OAGridSeen", "NO"]
        app.launch()
        XCTAssertTrue(app.buttons["signin.open"].waitForExistence(timeout: 30))
        app.buttons["signin.open"].tap()
        XCTAssertTrue(app.staticTexts["signin.npub"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.buttons["signin.scan"].label, "Scan a code")
        XCTAssertEqual(app.buttons["signin.paste"].label, "Paste a link")
        shot(app, "Log in")
        app.buttons["signin.copy"].tap()
        XCTAssertTrue(app.staticTexts["signin.note"].waitForExistence(timeout: 5))
        XCTAssertEqual(app.staticTexts["signin.note"].label, "Copied.")
        app.buttons["signin.close"].tap()
        XCTAssertTrue(wait(10) { !app.staticTexts["signin.npub"].exists })
        XCTAssertTrue(app.buttons["signin.open"].waitForExistence(timeout: 10))
    }

    /// The shim against BIP-340's own test vectors 0 and 1.
    func testSchnorrVectors() {
        let vectors = [
            ("0000000000000000000000000000000000000000000000000000000000000003", "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
             "0000000000000000000000000000000000000000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000",
             "e907831f80848d1069a5371b402410364bdf1c5f8307b0084c55f1ce2dca821525f66a4a85ea8b71e482a74f382d2ce5ebeee8fdb2172f477df4900d310536c0"),
            ("b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef", "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
             "0000000000000000000000000000000000000000000000000000000000000001", "243f6a8885a308d313198a2e03707344a4093822299f31d0082efa98ec4e6c89",
             "6896bd60eeae296db48a229ff71dfe071bde413e6d43f917dc8dcf8c78de33418906d11ac976abccb20b091292bff4ea897efcb639ea871cfa95f6de339e4b0a"),
        ]
        for (sk, pk, aux, msg, want) in vectors {
            let k = unhex(sk), a = unhex(aux), m = unhex(msg)
            var pub = Data(count: 32), sig = Data(count: 64)
            let p = k.withUnsafeBytes { kb in pub.withUnsafeMutableBytes { oa_nostr_pubkey(kb.bindMemory(to: UInt8.self).baseAddress!, $0.bindMemory(to: UInt8.self).baseAddress!) } }
            XCTAssertEqual(p, 1); XCTAssertEqual(hex(pub), pk)
            let s = k.withUnsafeBytes { kb in m.withUnsafeBytes { mb in a.withUnsafeBytes { ab in sig.withUnsafeMutableBytes {
                oa_nostr_sign(kb.bindMemory(to: UInt8.self).baseAddress!, mb.bindMemory(to: UInt8.self).baseAddress!, ab.bindMemory(to: UInt8.self).baseAddress!, $0.bindMemory(to: UInt8.self).baseAddress!) } } } }
            XCTAssertEqual(s, 1); XCTAssertEqual(hex(sig), want)
            sig[0] ^= 1
            let v = pub.withUnsafeBytes { pb in m.withUnsafeBytes { mb in sig.withUnsafeBytes {
                oa_nostr_verify(pb.bindMemory(to: UInt8.self).baseAddress!, mb.bindMemory(to: UInt8.self).baseAddress!, $0.bindMemory(to: UInt8.self).baseAddress!) } } }
            XCTAssertEqual(v, 0)
        }
        var zero = Data(count: 32)
        XCTAssertEqual(zero.withUnsafeMutableBytes { oa_nostr_seckey_valid($0.bindMemory(to: UInt8.self).baseAddress!) }, 0)
    }
}
