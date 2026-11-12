import CryptoKit
import XCTest
@testable import OpenAgents

/// The wallet's encrypted backup, end to end against a fake server and a software passkey
/// (#13217): back up with one prompt, lose the phone, bring the wallet back with one prompt or
/// with the recovery code; and what the server was sent on the way (ciphertext only). TEST
/// SECRETS ONLY: the seed is a fixed pattern and holds no money.
@MainActor
final class WalletBackupTests: XCTestCase {
    final class Tokens: TokenStore, @unchecked Sendable {
        var tokens: AppAuth.Tokens?
        func load() -> AppAuth.Tokens? { tokens }
        func save(_ t: AppAuth.Tokens) { tokens = t }
        func clear() { tokens = nil }
    }
    final class Seeds: SeedStore, @unchecked Sendable {
        var seed: Data?
        func read() -> Data? { seed }
        func add(_ s: Data) -> Bool { guard seed == nil else { return false }; seed = s; return true }
    }
    /// A software passkey: its PRF is HMAC-SHA256 under a key only this "authenticator" holds.
    final class Passkey: PasskeyAsking {
        let credential = Data((0..<16).map { UInt8($0 * 7 + 3) })
        let key = SymmetricKey(data: Data(repeating: 0x5a, count: 32))
        var canPRF = true
        var prompts = 0
        private(set) var prfOutput: Data?
        func assert(_ c: AppAuth.PasskeyChallenge, prf: Data?) async throws -> AppAuth.PasskeyAssertion {
            prompts += 1
            prfOutput = canPRF ? prf.map { Data(HMAC<SHA256>.authenticationCode(for: $0, using: key)) } : nil
            return AppAuth.PasskeyAssertion(credentialID: credential, authenticatorData: Data([1]), clientData: Data([2]), signature: Data([3]), userHandle: nil)
        }
        func register(_ c: AppAuth.PasskeyNew) async throws -> PasskeyAsk.Registration {
            prompts += 1
            return PasskeyAsk.Registration(attestation: Data([9]), clientData: Data([8]), credentialID: credential, prfSupported: canPRF)
        }
    }
    /// The API as the app sees it: it keeps what it was sent and answers as wallet.c does.
    final class Server: @unchecked Sendable {
        let lock = NSLock()
        var bodies: [String] = []
        var paths: [String] = []
        var passkeys = 1
        var fresh = false                     // the session proved it is you in the last five minutes
        var backup: (vault: String, epoch: Int64, slots: String, object: String)?
        var account = "acct_aaaaaaaaaaaaaaaaaaaaaaaaaa"
        var spendAnswers: [(Int, String)] = []
        var down = false
        func send(_ r: URLRequest) async throws -> (Data, Int) {
            let path = r.url?.path ?? "", body = String(decoding: r.httpBody ?? Data(), as: UTF8.self)
            lock.lock(); defer { lock.unlock() }
            if down { throw URLError(.notConnectedToInternet) }
            paths.append(path); bodies.append(body)
            func json(_ s: String, _ status: Int = 200) -> (Data, Int) { (Data(s.utf8), status) }
            let stepUp = json("{\"error\":\"Confirm it's you with your passkey or key to do this.\",\"step_up\":\"L2-fresh\",\"methods\":[\"passkey\"]}", 401)
            switch path {
            case "/api/auth/passkeys":
                return json("{\"passkeys\":[" + Array(repeating: "{\"id\":1}", count: passkeys).joined(separator: ",") + "],\"offer\":false}")
            case "/api/auth/passkey/register/options":
                return json("{\"challenge\":\"\(AppAuth.b64url(Data(repeating: 7, count: 32)))\",\"rp_id\":\"openagents.com\",\"user_id\":\"\(AppAuth.b64url(Data(repeating: 4, count: 32)))\",\"user_name\":\"ada@example.com\"}")
            case "/api/auth/passkey/register": passkeys += 1; return json("{\"state\":\"added\"}")
            case "/api/auth/passkey/step-up/options":
                return json("{\"challenge\":\"\(AppAuth.b64url(Data(repeating: 6, count: 32)))\",\"rp_id\":\"openagents.com\"}")
            case "/api/auth/passkey/step-up": fresh = true; return json("{\"state\":\"stepped-up\",\"level\":\"L2\"}")
            case "/api/wallet":
                let b = backup.map { "{\"vault\":\"\($0.vault)\",\"epoch\":\($0.epoch),\"slots\":[{\"method\":\"passkey\",\"label\":\"Test\",\"live\":true},{\"method\":\"recovery-code\",\"label\":\"Recovery code\",\"live\":true}],\"counts\":true}" } ?? "null"
                return json("{\"limit\":{\"daily_sat\":20000,\"default_sat\":20000,\"spent_sat\":0,\"new_destination_sat\":100000,\"raise\":null},\"backup\":\(b)}")
            case "/api/wallet/backup":
                guard fresh else { return stepUp }
                guard let o = try? JSONSerialization.jsonObject(with: Data(body.utf8)) as? [String: String], let after = Int64(o["after"] ?? ""),
                      let slots = o["slots"], let object = o["object"], let text = Data(base64Encoded: slots).map({ String(decoding: $0, as: UTF8.self) }) else { return json("{\"error\":\"bad\"}", 400) }
                guard after == (backup?.epoch ?? 0) else { return json("{\"error\":\"Your backup changed somewhere else. Load it again.\"}", 409) }
                let vault = (try? JSONSerialization.jsonObject(with: Data(text.split(separator: "\n")[0].utf8)) as? [String: Any])?["vault"] as? String ?? ""
                backup = (vault, after + 1, slots, object)
                return json("{\"state\":\"stored\",\"epoch\":\(after + 1),\"vault\":\"\(vault)\"}")
            case "/api/wallet/backup/export":
                guard fresh else { return stepUp }
                guard let b = backup else { return json("{\"backup\":null}") }
                return json("{\"backup\":{\"account\":\"\(account)\",\"vault\":\"\(b.vault)\",\"epoch\":\(b.epoch),\"slots\":\"\(b.slots)\",\"object\":\"\(b.object)\"}}")
            case "/api/wallet/spend":
                guard !spendAnswers.isEmpty else { return json("{\"state\":\"ok\",\"daily_sat\":20000,\"spent_sat\":0}") }
                let a = spendAnswers.removeFirst()
                return json(a.1, a.0)
            case "/api/wallet/spend/void": return json("{\"state\":\"voided\"}")
            default: return json("{\"error\":\"Not found.\"}", 404)
            }
        }
    }

    let seed = Data((0..<16).map { UInt8($0 * 11 + 5) })
    func phone(_ server: Server, _ seeds: SeedStore, _ passkey: Passkey, account: String = "acct_aaaaaaaaaaaaaaaaaaaaaaaaaa") -> WalletBackupFlow {
        let tokens = Tokens()
        tokens.tokens = AppAuth.Tokens(access: "oa_at_A", refresh: "oa_rt_R", accessUntil: Date().addingTimeInterval(3000), account: account)
        let auth = AppAuth(origin: "https://example.test", send: { try await server.send($0) }, store: tokens, device: nil)
        let client = WalletBackupClient(origin: "https://example.test", send: { try await server.send($0) }, authorize: { try await auth.authorize($0) },
                                        loggedIn: { auth.loggedIn }, account: { auth.tokens?.account ?? "" })
        let defaults = UserDefaults(suiteName: "wallet-backup-tests-" + UUID().uuidString)!
        var ledger = SpendLedger(defaults: defaults); ledger.keep = MemoryLimitStore()
        return WalletBackupFlow(auth: auth, client: client, seeds: seeds, memory: WalletBackup.Memory(defaults: defaults), ledger: ledger, asker: { passkey })
    }

    func testTheNumbersSwiftAndCoaxShare() {
        XCTAssertEqual(WalletVault.number(0), 235); XCTAssertEqual(WalletVault.number(3), 17)
        XCTAssertEqual(WalletVault.prfInput, Data(SHA256.hash(data: Data("openagents.wallet-backup.v1/prf".utf8))))
    }

    func testBackUpWithOnePromptThenLoseThePhoneAndRestoreWithOnePrompt() async throws {
        let server = Server(), passkey = Passkey(), seeds = Seeds()
        seeds.seed = seed
        let old = phone(server, seeds, passkey)
        let words = try await old.backUp(label: "Test \"phone\"")
        XCTAssertEqual(words.split(separator: " ").count, 24)
        XCTAssertEqual(passkey.prompts, 1, "one passkey prompt backs the wallet up")
        XCTAssertEqual(server.backup?.epoch, 1)
        XCTAssertEqual(old.memory.vault, server.backup?.vault); XCTAssertEqual(old.memory.seen, 1)
        // What the server was sent: nothing that opens the backup.
        let prf = Data(HMAC<SHA256>.authenticationCode(for: WalletVault.prfInput, using: passkey.key))
        let sent = server.bodies.joined(separator: "\n")
        for secret in [seed.base64EncodedString(), seed.map { String(format: "%02x", $0) }.joined(), prf.base64EncodedString(), prf.map { String(format: "%02x", $0) }.joined()] {
            XCTAssertFalse(sent.contains(secret))
        }
        for word in Set(words.split(separator: " ")) where word.count >= 5 { XCTAssertFalse(sent.contains(String(word)), "a recovery word reached the server") }
        let object = Data(base64Encoded: server.backup!.object)!
        XCTAssertNil(object.range(of: seed), "the seed is in the stored object in the clear")

        // A new phone: no wallet, no memory. The passkey prompt that logs in opens the backup.
        let lost = Seeds(), prompt = Passkey()
        let new = phone(server, lost, prompt)
        server.fresh = true   // a passkey login is a fresh proof
        let answer = try await prompt.assert(AppAuth.PasskeyChallenge(challenge: Data(repeating: 1, count: 32), text: "x", rpID: "openagents.com"), prf: WalletVault.prfInput)
        let back = await new.restoreAfterLogin(credential: answer.credentialID, prf: prompt.prfOutput)
        XCTAssertTrue(back); XCTAssertEqual(prompt.prompts, 1, "one passkey prompt brings the wallet back")
        XCTAssertEqual(lost.seed, seed)
        XCTAssertEqual(new.memory.vault, server.backup?.vault); XCTAssertEqual(new.memory.seen, 1)
        // A phone that has a wallet keeps it: nothing is restored over it.
        let again = await new.restoreAfterLogin(credential: answer.credentialID, prf: prompt.prfOutput)
        XCTAssertFalse(again); XCTAssertEqual(lost.seed, seed)
    }

    func testTheRecoveryCodeBringsTheWalletBackAndNothingElseDoes() async throws {
        let server = Server(), passkey = Passkey(), seeds = Seeds()
        seeds.seed = seed
        let words = try await phone(server, seeds, passkey).backUp(label: "Test")
        // typed back in capitals with stray spaces
        let lost = Seeds(), new = phone(server, lost, Passkey())
        let done = try await new.restore(.words("  " + words.uppercased().replacingOccurrences(of: " ", with: "  ") + "\n"))
        XCTAssertTrue(done); XCTAssertEqual(lost.seed, seed)
        // a wrong code, a code that isn't one, another passkey's PRF output
        var other = words.split(separator: " ").map(String.init); other.swapAt(0, 1)
        let wrong = Seeds(), third = phone(server, wrong, Passkey())
        for typed in [other.joined(separator: " "), "abandon ability able", words + " zoo"] {
            do { _ = try await third.restore(.words(typed)); XCTFail("opened with \(typed.prefix(20))") } catch {}
        }
        do { _ = try await third.restore(.passkey(credential: passkey.credential, prf: Data(repeating: 1, count: 32))); XCTFail("another PRF output opened it") } catch {}
        do { _ = try await third.restore(.passkey(credential: Data([1, 2, 3]), prf: Data(repeating: 1, count: 32))); XCTFail("another passkey's slot was found") } catch {}
        XCTAssertNil(wrong.seed)
    }

    func testASwappedOrOlderBackupIsRefused() async throws {
        let server = Server(), passkey = Passkey(), seeds = Seeds()
        seeds.seed = seed
        let flow = phone(server, seeds, passkey)
        _ = try await flow.backUp(label: "Test")
        let first = server.backup!
        let prf = Data(HMAC<SHA256>.authenticationCode(for: WalletVault.prfInput, using: passkey.key))
        // another account is served this backup: its record names the account it was made for
        let other = Seeds(), stranger = phone(server, other, Passkey(), account: "acct_bbbbbbbbbbbbbbbbbbbbbbbbbb")
        do { _ = try await stranger.restore(.passkey(credential: passkey.credential, prf: prf)); XCTFail("another account's backup opened") }
        catch { XCTAssertTrue(error.localizedDescription.contains("another account")) }
        XCTAssertNil(other.seed)
        // the server names a newer epoch for the old bytes: the record's own epoch says otherwise
        server.backup = (first.vault, 2, first.slots, first.object)
        let lost = Seeds(), new = phone(server, lost, Passkey())
        do { _ = try await new.restore(.passkey(credential: passkey.credential, prf: prf)); XCTFail("an old backup opened as a newer one") } catch {}
        // a phone that has seen epoch 3 of this vault is handed epoch 1 again
        server.backup = first
        new.memory.remember(vault: first.vault, epoch: 3)
        do { _ = try await new.restore(.passkey(credential: passkey.credential, prf: prf)); XCTFail("a rollback opened") }
        catch { XCTAssertTrue(error.localizedDescription.contains("older backup")) }
        XCTAssertNil(lost.seed)
    }

    func testNothingIsStoredWithoutThePasskeysPRFAndANewAccountMakesAPasskeyFirst() async throws {
        let server = Server(), passkey = Passkey(), seeds = Seeds()
        seeds.seed = seed
        passkey.canPRF = false
        do { _ = try await phone(server, seeds, passkey).backUp(label: "Test"); XCTFail("backed up without a PRF output") } catch {}
        XCTAssertNil(server.backup)
        XCTAssertFalse(server.paths.contains("/api/wallet/backup"))
        // an account with no passkey: one prompt makes it, one proves it
        let fresh = Server(), key = Passkey()
        fresh.passkeys = 0
        _ = try await phone(fresh, seeds, key).backUp(label: "Test")
        XCTAssertEqual(key.prompts, 2); XCTAssertEqual(fresh.passkeys, 1); XCTAssertEqual(fresh.backup?.epoch, 1)
        // a phone with no wallet has nothing to back up
        do { _ = try await phone(fresh, Seeds(), key).backUp(label: "Test"); XCTFail() } catch {}
    }

    func testAPaymentAsksTheAccountAndPastTheLimitAsksForThePasskey() async {
        let server = Server(), passkey = Passkey(), flow = phone(server, Seeds(), passkey)
        // within the limit: no prompt
        guard case .pay(let id) = await flow.maySpend(sats: 1000, destination: "lnbc1a") else { return XCTFail() }
        XCTAssertEqual(passkey.prompts, 0); XCTAssertEqual(flow.ledger.limit, 20000)
        flow.paid(id: id, sats: 1000, destination: "lnbc1a")
        XCTAssertEqual(flow.ledger.spent, 1000); XCTAssertFalse(flow.ledger.isNew("LNBC1A")); XCTAssertTrue(flow.ledger.isNew("lnbc1b"))
        // past it: the server asks for a fresh proof, the passkey gives it, the payment goes
        server.spendAnswers = [(401, "{\"error\":\"Confirm it's you with your passkey or key to do this.\",\"step_up\":\"L2-fresh\"}"), (200, "{\"state\":\"ok\",\"daily_sat\":20000}")]
        guard case .pay = await flow.maySpend(sats: 30000, destination: "lnbc1a") else { return XCTFail() }
        XCTAssertEqual(passkey.prompts, 1)
        // still refused after the proof: not paid, and not asked for the passkey again
        server.spendAnswers = [(401, "{\"error\":\"x\",\"step_up\":\"L2-fresh\"}"), (401, "{\"error\":\"x\",\"step_up\":\"L2-fresh\"}")]
        guard case .refuse(let why) = await flow.maySpend(sats: 30000, destination: "lnbc1a") else { return XCTFail() }
        XCTAssertTrue(why.contains("daily limit of 20,000 sats")); XCTAssertEqual(passkey.prompts, 2)
        // the server refuses the payment itself
        server.spendAnswers = [(409, "{\"error\":\"That payment was already asked about with another amount.\"}")]
        guard case .refuse(let said) = await flow.maySpend(sats: 5, destination: "lnbc1a") else { return XCTFail() }
        XCTAssertEqual(said, "That payment was already asked about with another amount.")
        // the server can't be reached: this phone's own count of the last 24 hours stands in
        server.down = true
        guard case .pay = await flow.maySpend(sats: 19000, destination: "lnbc1a") else { return XCTFail("within the last known limit") }
        guard case .refuse = await flow.maySpend(sats: 19001, destination: "lnbc1a") else { return XCTFail("past the last known limit with nobody to ask") }
        // a phone that never heard a limit (no account ever) pays as before
        let alone = phone(server, Seeds(), Passkey())
        guard case .pay = await alone.maySpend(sats: 5_000_000, destination: "lnbc1a") else { return XCTFail() }
        // the account was frozen (every login ended): the server answers 401 with no proof to give,
        // and a phone that has heard a limit pays nothing until someone logs in
        server.down = false
        server.spendAnswers = [(401, "{\"error\":\"Log in first.\"}")]
        guard case .refuse(let frozen) = await flow.maySpend(sats: 1, destination: "lnbc1a") else { return XCTFail("a logged-out phone paid") }
        XCTAssertTrue(frozen.hasPrefix("Log in to pay"))
    }

    /// On a new Simulator device only (OA_TEST_NEW_PHONE=1, passed as TEST_RUNNER_OA_TEST_NEW_PHONE=1):
    /// this writes a test seed into the device's real Keychain, as a restore does, and nothing
    /// removes it, so it never runs on a device someone uses. A phone with no wallet logs in with
    /// its passkey; that one prompt brings the wallet back into the Keychain, where SparkHost
    /// reads it. A second restore, and a second seed, change nothing.
    func testANewPhonesKeychainGetsTheWalletBackWithOnePasskeyPrompt() async throws {
        guard ProcessInfo.processInfo.environment["OA_TEST_NEW_PHONE"] == "1" else { throw XCTSkip("set TEST_RUNNER_OA_TEST_NEW_PHONE=1 on a new Simulator device") }
        guard case .notFound = WalletKeychain.readSeed() else { throw XCTSkip("this device already has a wallet: it is left alone") }
        // the old phone backs up
        let server = Server(), passkey = Passkey(), old = Seeds()
        old.seed = seed
        _ = try await phone(server, old, passkey).backUp(label: "Old phone")
        // the new phone: the real Keychain, one passkey prompt
        let prompt = Passkey()
        let new = phone(server, KeychainSeedStore(), prompt)
        server.fresh = true
        let answer = try await prompt.assert(AppAuth.PasskeyChallenge(challenge: Data(repeating: 1, count: 32), text: "x", rpID: "openagents.com"), prf: WalletVault.prfInput)
        let back = await new.restoreAfterLogin(credential: answer.credentialID, prf: prompt.prfOutput)
        XCTAssertTrue(back); XCTAssertEqual(prompt.prompts, 1)
        guard case .found(let kept) = WalletKeychain.readSeed() else { return XCTFail("the Keychain has no wallet after the restore") }
        XCTAssertEqual(kept, seed)
        // SparkHost's own path from the Keychain: the entropy's 12 English words
        let words = try XCTUnwrap(BIP39.englishWords())
        XCTAssertEqual(BIP39.mnemonic(entropy: kept, words: words)?.split(separator: " ").count, 12)
        // a wallet on the phone is never replaced
        XCTAssertFalse(WalletKeychain.addSeed(Data(repeating: 9, count: 16)))
        let again = await new.restoreAfterLogin(credential: answer.credentialID, prf: prompt.prfOutput)
        XCTAssertFalse(again)
        guard case .found(let still) = WalletKeychain.readSeed() else { return XCTFail() }
        XCTAssertEqual(still, seed)
        // the last limit this phone heard stays in the Keychain too
        let keep = KeychainLimitStore()
        keep.write(20_000); XCTAssertEqual(keep.read(), 20_000)
        keep.write(5_000); XCTAssertEqual(keep.read(), 5_000)
    }

    func testTheWalletWordsNostrProofIsANIP98EventByTheWordsKeyAndCarriesNoSecret() async throws {
        let url = "https://example.test/api/auth/link/nostr?c=" + String(repeating: "a", count: 64)
        let seed = self.seed
        let got = try await WalletVault.onBigStack { try WalletVault.nostrProof(seed: seed, url: url) }
        let made = try XCTUnwrap(got)
        XCTAssertEqual(made.pubkey.count, 64)
        let event = String(decoding: Data(base64Encoded: made.proof)!, as: UTF8.self)
        XCTAssertTrue(event.contains("\"kind\":27235")); XCTAssertTrue(event.contains("[\"method\",\"POST\"]")); XCTAssertFalse(event.contains("payload"))
        XCTAssertTrue(event.contains(url)); XCTAssertTrue(event.contains(made.pubkey))
        XCTAssertFalse(event.contains(seed.map { String(format: "%02x", $0) }.joined()))
        // the same words give the same key each time; a seed that isn't one gives nothing
        let again = try await WalletVault.onBigStack { try WalletVault.nostrProof(seed: seed, url: url) }
        XCTAssertEqual(again?.pubkey, made.pubkey)
        let none = try await WalletVault.onBigStack { try WalletVault.nostrProof(seed: Data([1, 2, 3]), url: url) }
        XCTAssertNil(none.map { $0.pubkey })
    }
}
