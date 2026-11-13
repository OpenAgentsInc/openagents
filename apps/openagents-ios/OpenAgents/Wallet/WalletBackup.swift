import CryptoKit
import Foundation
import Security

/// The wallet's encrypted backup and daily limit (docs/auth/wallets-and-custody.md, decisions D4
/// and D5; GitHub #13217). The seed is sealed on this phone and we store only the ciphertext: a
/// NIP-VAULT object and key slots that open with the passkey's PRF output or with a recovery code
/// made here and shown once. Neither ever leaves the phone, so nobody but the person opens the
/// backup, us included.
///
/// Coax does the sealing, the opening and every decision (coax/lib/nostr/vault-wallet.coax and
/// pay/wallet-vault.coax, through WalletVault.h; the app's rules are pay/wallet-backup.coax's). This file does the effects: the system's
/// randomness, the memory scrypt needs, the Keychain, and the requests to openagents.com
/// (coax/web/api/wallet.c), whose answers it reads and words.
enum WalletBackup {
    /// A slot of the account's backup, as the server lists it: public facts only.
    struct Slot: Equatable {
        /// passkey, recovery-code or nostr.
        var method: String
        var label: String
        /// For a passkey's slot: the passkey is still on the account.
        var live: Bool
    }
    /// What the server says of the account's backup: no ciphertext.
    struct Facts: Equatable {
        var vault: String
        var epoch: Int64
        var slots: [Slot]
        /// A passkey of the account still opens it.
        var counts: Bool
    }
    /// The daily limit and what the last 24 hours counted.
    struct Limit: Equatable {
        var daily: Int64 = 20_000
        var spent: Int64 = 0
        var newDestination: Int64 = 100_000
        /// A raise that is waiting, and when it counts.
        var raiseTo: Int64?
        var raiseAt: Date?
    }
    struct Summary: Equatable {
        var limit = Limit()
        var backup: Facts?
    }
    /// The backup itself, as the server holds it.
    struct Stored: Equatable {
        var account: String
        var vault: String
        var epoch: Int64
        /// Every slot's text, a newline between them.
        var slots: Data
        var object: Data
    }
    /// A backup made on this phone, before it is stored: the two slots, the object and the
    /// recovery code's words (shown once, never stored or sent).
    struct Made {
        var slots: Data
        var object: Data
        var words: String
        var vault: String
    }

    enum Failure: Error, LocalizedError, Equatable {
        /// The server wants a fresh proof that it is you (a passkey, or a key).
        case stepUp(String)
        case said(String)
        var errorDescription: String? {
            switch self { case .stepUp(let w), .said(let w): return w }
        }
    }

    // MARK: reading the server's answers

    static func summary(_ data: Data) -> Summary? {
        guard let o = LightningAddress.object(data), let l = o["limit"] as? [String: Any] else { return nil }
        var s = Summary()
        s.limit.daily = LightningAddress.int(l["daily_sat"]) ?? s.limit.daily
        s.limit.spent = LightningAddress.int(l["spent_sat"]) ?? 0
        s.limit.newDestination = LightningAddress.int(l["new_destination_sat"]) ?? s.limit.newDestination
        if let r = l["raise"] as? [String: Any], let to = LightningAddress.int(r["sat"]), let at = LightningAddress.int(r["at_ms"]) {
            s.limit.raiseTo = to; s.limit.raiseAt = Date(timeIntervalSince1970: Double(at) / 1000)
        }
        if let b = o["backup"] as? [String: Any], let vault = b["vault"] as? String, let epoch = LightningAddress.int(b["epoch"]) {
            let slots = (b["slots"] as? [[String: Any]] ?? []).map {
                Slot(method: $0["method"] as? String ?? "", label: $0["label"] as? String ?? "", live: $0["live"] as? Bool ?? false)
            }
            s.backup = Facts(vault: vault, epoch: epoch, slots: slots, counts: b["counts"] as? Bool ?? false)
        }
        return s
    }
    /// nil when the account has no backup.
    static func stored(_ data: Data) -> Stored? {
        guard let o = LightningAddress.object(data), let b = o["backup"] as? [String: Any],
              let account = b["account"] as? String, let vault = b["vault"] as? String, let epoch = LightningAddress.int(b["epoch"]),
              let slots = (b["slots"] as? String).flatMap({ Data(base64Encoded: $0) }),
              let object = (b["object"] as? String).flatMap({ Data(base64Encoded: $0) }) else { return nil }
        return Stored(account: account, vault: vault, epoch: epoch, slots: slots, object: object)
    }
    /// The body of POST /api/wallet/backup.
    static func storeBody(after: Int64, slots: Data, object: Data) -> String {
        "{\"after\":\"\(after)\",\"object\":\"\(object.base64EncodedString())\",\"slots\":\"\(slots.base64EncodedString())\"}"
    }

    /// 20,000
    static func sats(_ n: Int64) -> String { LightningAddress.grouped(n) }
    /// How a slot reads in the list.
    static func slotLine(_ s: Slot) -> String {
        switch s.method {
        case "passkey": return "Passkey · " + s.label + (s.live ? "" : " · removed from your account")
        case "recovery-code": return "Recovery code"
        default: return "Nostr key · " + s.label
        }
    }

    // MARK: what this phone remembers (nothing secret)

    /// The vault this phone made or restored from, and the highest epoch it has made or opened
    /// (the rollback floor): a backup older than that isn't opened.
    struct Memory {
        var defaults: UserDefaults = .standard
        var vault: String? {
            get { defaults.string(forKey: "wallet.backup.vault") }
            nonmutating set { defaults.set(newValue, forKey: "wallet.backup.vault") }
        }
        var seen: Int64 {
            get { Int64(defaults.integer(forKey: "wallet.backup.seen")) }
            nonmutating set { if metaverse_backup_newer(seen, newValue) { defaults.set(Int(newValue), forKey: "wallet.backup.seen") } }
        }
        /// Remembers a backup this phone made or opened. A new vault starts its epochs again.
        func remember(vault v: String, epoch: Int64) {
            if vault != v { defaults.set(0, forKey: "wallet.backup.seen"); vault = v }
            seen = epoch
        }
        /// The one-time warning before the wallet's words become a Nostr identity was shown.
        var warnedOneSeed: Bool {
            get { defaults.bool(forKey: "wallet.oneseed.warned") }
            nonmutating set { defaults.set(newValue, forKey: "wallet.oneseed.warned") }
        }
        /// The Nostr key of the wallet's words that this phone linked to the account, as hex.
        var oneSeedKey: String? {
            get { defaults.string(forKey: "wallet.oneseed.pubkey") }
            nonmutating set { defaults.set(newValue, forKey: "wallet.oneseed.pubkey") }
        }
    }
}

/// Sealing and opening, through Coax. Every value that must be random comes from the system here.
enum WalletVault {
    /// The passkey's PRF input for a wallet backup: the same for every backup, so the prompt that
    /// logs a person in on a new phone can open their backup too.
    static var prfInput: Data {
        var out = [UInt8](repeating: 0, count: 32)
        _ = out.withUnsafeMutableBufferPointer { wb_prf_input(coax_v_u8(data: $0.baseAddress, count: 32)) }
        return Data(out)
    }

    /// Runs body on a thread of its own with a 16 MiB stack, and waits for it. Coax keeps its
    /// working memory on the stack (a JSON tree, a 64 KiB header buffer): more than the 512 KiB
    /// a queue's thread has. scrypt's 128 MiB is not on the stack; recoverySecret allocates it.
    static func onBigStack<T>(_ body: @escaping () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { k in
            let thread = Thread { k.resume(with: Result { try body() }) }
            thread.stackSize = 16 << 20
            thread.qualityOfService = .userInitiated
            thread.start()
        }
    }

    /// A size or code Swift can't import from Coax (pay/wallet-vault.coax's metaverse_backup_number).
    static func number(_ which: Int64) -> Int { Int(metaverse_backup_number(which)) }

    static func random(_ n: Int) throws -> [UInt8] {
        var b = [UInt8](repeating: 0, count: n)
        guard SecRandomCopyBytes(kSecRandomDefault, n, &b) == errSecSuccess else { throw WalletBackup.Failure.said("This phone couldn't make random numbers. Try again.") }
        return b
    }
    private static func bytes<T>(_ a: inout [UInt8], _ body: (coax_v_u8) -> T) -> T {
        a.withUnsafeMutableBufferPointer { body(coax_v_u8(data: $0.baseAddress, count: Int64($0.count))) }
    }
    private static func text<T>(_ s: String, _ body: (coax_string) -> T) -> T {
        var b = Array(s.utf8)
        return b.withUnsafeMutableBufferPointer { p in
            p.withMemoryRebound(to: CChar.self) { body(coax_string(data: $0.baseAddress, count: Int64($0.count))) }
        }
    }
    private static func hex(_ b: ArraySlice<UInt8>) -> String { b.map { String(format: "%02x", $0) }.joined() }
    private static func wipe(_ a: inout [UInt8]) { for i in a.indices { a[i] = 0 } }

    /// scrypt of a recovery code's normal words with a slot's salt and cost: 128 MiB for a moment.
    static func recoverySecret(words: String, salt: [UInt8], logN: Int) -> [UInt8]? {
        guard logN >= 16, logN <= 18 else { return nil }
        let size = 1024 << logN
        guard let memory = malloc(size) else { return nil }
        defer { memset(memory, 0, size); free(memory) }
        var out = [UInt8](repeating: 0, count: 32), s = salt, w = Array(words.utf8)
        let ok = bytes(&w) { wv in bytes(&s) { sv in bytes(&out) { ov in
            wb_recovery_secret(wv, sv, coax_v_u8(data: memory.assumingMemoryBound(to: UInt8.self), count: Int64(size)), Int64(logN), ov)
        } } }
        wipe(&w)
        return ok ? out : nil
    }
    /// What a person typed as their recovery code, made normal; nil unless it is a whole 24-word code.
    static func normalCode(_ typed: String) -> String? {
        var t = Array(typed.utf8), work = [UInt8](repeating: 0, count: WalletVault.number(6)), out = [UInt8](repeating: 0, count: WalletVault.number(7))
        let n = bytes(&t) { tv in bytes(&work) { wv in bytes(&out) { ov in wb_code_normal(tv, wv, ov) } } }
        wipe(&t)
        guard n > 0 else { return nil }
        defer { wipe(&out) }
        return String(decoding: out[0..<Int(n)], as: UTF8.self)
    }

    /// A new backup of seed for account at epoch: a new vault with a slot for the passkey (its PRF
    /// output and credential id) and a slot for a new recovery code. Before it answers it opens
    /// what it made through the passkey's slot and through the code's words, made normal and
    /// stretched again as a restore would, and compares the seed.
    static func make(account: String, epoch: Int64, seed: Data, prf: Data, credential: Data, rp: String, label: String, now: Date = Date()) throws -> WalletBackup.Made {
        let failed = WalletBackup.Failure.said("This phone couldn't make the backup. Nothing was saved. Try again.")
        var fresh = try random(WalletVault.number(0)), codeEntropy = try random(32), salt = try random(16)
        var work = [UInt8](repeating: 0, count: WalletVault.number(6)), wordsOut = [UInt8](repeating: 0, count: WalletVault.number(7))
        defer { wipe(&fresh); wipe(&codeEntropy); wipe(&wordsOut) }
        let wn = bytes(&codeEntropy) { e in bytes(&work) { w in bytes(&wordsOut) { o in wb_code_words(e, w, o) } } }
        guard wn > 0 else { throw failed }
        let words = String(decoding: wordsOut[0..<Int(wn)], as: UTF8.self)
        guard var secret = recoverySecret(words: words, salt: salt, logN: WalletVault.number(3)) else { throw failed }
        defer { wipe(&secret) }
        var slots = [UInt8](repeating: 0, count: WalletVault.number(1)), object = [UInt8](repeating: 0, count: WalletVault.number(2))
        var s = [UInt8](seed), p = [UInt8](prf), c = [UInt8](credential)
        defer { wipe(&s); wipe(&p) }
        let created = Int64(now.timeIntervalSince1970)
        let made = text(account) { acct in text(rp) { rpv in text(label) { lv in
            bytes(&slots) { sv in bytes(&object) { ov in bytes(&s) { seedv in bytes(&p) { pv in bytes(&c) { cv in
                bytes(&secret) { rv in bytes(&salt) { saltv in bytes(&fresh) { fv in
                    wb_make(sv, ov, acct, epoch, created, seedv, pv, cv, rpv, lv, rv, saltv, Int64(WalletVault.number(3)), fv)
                } } }
            } } } } }
        } } }
        guard made.code == 0 else { throw failed }
        let slotsData = Data(slots[0..<Int(made.slots)]), objectData = Data(object[0..<Int(made.object)])
        // Open it as a restore will: the words as shown, typed back, with the slot's own salt and cost.
        guard let typed = normalCode(words), try verify(object: objectData, slots: slotsData, account: account, epoch: epoch, prf: prf, credential: credential, words: typed, seed: seed) else { throw failed }
        return WalletBackup.Made(slots: slotsData, object: objectData, words: words, vault: hex(fresh[0..<32]))
    }

    /// True when the backup opens to seed through the passkey's slot and through the words.
    static func verify(object: Data, slots: Data, account: String, epoch: Int64, prf: Data, credential: Data, words: String, seed: Data) throws -> Bool {
        let slotsText = String(decoding: slots, as: UTF8.self)
        guard var secret = recoverySecret(inSlots: slotsText, words: words) else { return false }
        defer { wipe(&secret) }
        var o = [UInt8](object), p = [UInt8](prf), c = [UInt8](credential), s = [UInt8](seed)
        defer { wipe(&p); wipe(&s) }
        let code = text(slotsText) { sl in text(account) { acct in
            bytes(&o) { ov in bytes(&p) { pv in bytes(&c) { cv in bytes(&secret) { rv in bytes(&s) { sv in
                wb_verify(ov, sl, acct, epoch, pv, cv, rv, sv)
            } } } } }
        } }
        return code == 0
    }

    /// The recovery slot of a set, and scrypt of the words with its salt and cost.
    private static func recoverySecret(inSlots slots: String, words: String) -> [UInt8]? {
        var none = [UInt8]()
        let span = text(slots) { sl in bytes(&none) { wb_pick(sl, Int64(WalletVault.number(5)), $0) } }
        guard span.from >= 0 else { return nil }
        let slot = String(decoding: Array(slots.utf8)[Int(span.from)..<Int(span.to)], as: UTF8.self)
        var salt = [UInt8](repeating: 0, count: 16)
        let logN = text(slot) { sl in bytes(&salt) { wb_recovery_params(sl, $0) } }
        guard logN > 0 else { return nil }
        return recoverySecret(words: words, salt: salt, logN: Int(logN))
    }

    enum Key {
        /// The passkey that answered: its credential id and PRF output.
        case passkey(credential: Data, prf: Data)
        /// The recovery code, as typed.
        case words(String)
    }
    /// Why a backup didn't open, in words a person can act on.
    static func words(_ code: Int64) -> String {
        // coax/lib/nostr/vault-wallet.coax's WB_ codes
        switch code {
        case 8: return "That didn't open your backup. Check the recovery code, or use the passkey you backed up with."
        case 10: return "This backup was made for another account. Log in to the account you backed up with."
        case 11, 12: return "We were handed an older backup than this phone has seen, so it wasn't opened. Try again; if it keeps happening, tell us."
        case 13: return "That way of opening the backup was removed from it. Use your passkey or your recovery code."
        default: return "This backup can't be read. Nothing was changed."
        }
    }
    /// Opens a stored backup for the logged-in account: the seed's entropy (16 or 32 bytes).
    /// seen: the highest epoch this phone has made or opened (0 on a new phone).
    static func open(_ stored: WalletBackup.Stored, account: String, key: Key, seen: Int64) throws -> Data {
        let slots = String(decoding: stored.slots, as: UTF8.self)
        var secret: [UInt8]
        var credential = [UInt8]()
        let method: Int64
        switch key {
        case .passkey(let c, let prf): secret = [UInt8](prf); credential = [UInt8](c); method = Int64(WalletVault.number(4))
        case .words(let typed):
            guard let normal = normalCode(typed) else { throw WalletBackup.Failure.said("A recovery code is 24 words. Check each word and try again.") }
            guard let s = recoverySecret(inSlots: slots, words: normal) else { throw WalletBackup.Failure.said("This backup has no recovery code.") }
            secret = s; method = Int64(WalletVault.number(5))
        }
        defer { wipe(&secret) }
        let span = text(slots) { sl in bytes(&credential) { wb_pick(sl, method, $0) } }
        guard span.from >= 0 else { throw WalletBackup.Failure.said("This backup wasn't made with this passkey. Use your recovery code.") }
        let slot = String(decoding: Array(slots.utf8)[Int(span.from)..<Int(span.to)], as: UTF8.self)
        var o = [UInt8](stored.object), out = [UInt8](repeating: 0, count: 32)
        defer { wipe(&out) }
        let opened = text(slot) { sl in text(account) { acct in
            bytes(&o) { ov in bytes(&secret) { sv in bytes(&out) { outv in wb_open(ov, sl, sv, acct, stored.epoch, seen, outv) } } }
        } }
        guard opened.code == 0, opened.n == 16 || opened.n == 32 else { throw WalletBackup.Failure.said(words(opened.code)) }
        return Data(out[0..<Int(opened.n)])
    }

    /// The proof that links the Nostr key of the wallet's words (NIP-06, m/44'/1237'/0'/0/0) to
    /// the account, for the opt-in "Use my wallet words for my Nostr identity": a NIP-98 event
    /// over url, as the API takes it, and the key's public half. Coax derives the key, signs and
    /// wipes it; the private key never reaches Swift.
    static func nostrProof(seed: Data, url: String, now: Date = Date()) throws -> (proof: String, pubkey: String)? {
        var s = [UInt8](seed), aux = try random(32), nonce = try random(16), out = [UInt8](repeating: 0, count: Int(metaverse_backup_number(8)))
        defer { wipe(&s); wipe(&aux) }
        let n = text(url) { u in bytes(&s) { sv in bytes(&aux) { av in bytes(&nonce) { nv in bytes(&out) { ov in
            metaverse_wallet_nostr_proof(sv, av, nv, Int64(now.timeIntervalSince1970), u, ov)
        } } } } }
        guard n > 0 else { return nil }
        let proof = String(decoding: out[0..<Int(n)], as: UTF8.self)
        guard let event = Data(base64Encoded: proof), let o = LightningAddress.object(event), let pubkey = o["pubkey"] as? String, pubkey.count == 64 else { return nil }
        return (proof, pubkey)
    }
}

/// The wallet's requests to openagents.com with the app's login. Tests give a fake `send`.
final class WalletBackupClient: @unchecked Sendable {
    typealias Send = AppAuth.Send
    typealias Authorize = @Sendable (URLRequest) async throws -> URLRequest

    let origin: String
    private let send: Send
    private let authorize: Authorize
    private let loggedIn: @Sendable () -> Bool
    private let accountID: @Sendable () -> String

    init(origin: String = AppAuth.shared.origin, send: @escaping Send = AppAuth.network,
         authorize: @escaping Authorize = { try await AppAuth.shared.authorize($0) },
         loggedIn: @escaping @Sendable () -> Bool = { AppAuth.shared.loggedIn },
         account: @escaping @Sendable () -> String = { AppAuth.shared.tokens?.account ?? "" }) {
        self.origin = origin; self.send = send; self.authorize = authorize; self.loggedIn = loggedIn; self.accountID = account
    }
    var isLoggedIn: Bool { loggedIn() }
    var account: String { accountID() }

    /// The answer's bytes and status; a refusal that asks for a fresh proof throws `.stepUp`.
    private func call(_ method: String, _ path: String, _ body: String? = nil) async throws -> Data {
        guard let url = URL(string: origin + path) else { throw WalletBackup.Failure.said("That didn't work. Try again.") }
        var r = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20)
        r.httpMethod = method
        r.setValue("application/json", forHTTPHeaderField: "Accept")
        if let body { r.httpBody = Data(body.utf8); r.setValue("application/json", forHTTPHeaderField: "Content-Type") }
        r = try await authorize(r)
        let data: Data, status: Int
        do { (data, status) = try await send(r) } catch { throw AppAuth.Failure.unreachable }
        guard (200..<300).contains(status) else {
            let words = AppAuth.errorWords(data) ?? (status == 401 ? "Log in again." : "That didn't work. Try again.")
            if status == 401, LightningAddress.object(data)?["step_up"] != nil { throw WalletBackup.Failure.stepUp(words) }
            throw WalletBackup.Failure.said(words)
        }
        return data
    }

    func summary() async throws -> WalletBackup.Summary {
        guard let s = WalletBackup.summary(try await call("GET", "/api/wallet")) else { throw WalletBackup.Failure.said("That didn't work. Try again.") }
        return s
    }
    /// The backup's ciphertext (needs a fresh proof); nil when the account has none.
    func export() async throws -> WalletBackup.Stored? {
        WalletBackup.stored(try await call("POST", "/api/wallet/backup/export", "{}"))
    }
    /// Stores a backup at the epoch after `after` (needs a fresh proof): the new epoch.
    func store(after: Int64, slots: Data, object: Data) async throws -> Int64 {
        let answer = try await call("POST", "/api/wallet/backup", WalletBackup.storeBody(after: after, slots: slots, object: object))
        guard let epoch = LightningAddress.int(LightningAddress.object(answer)?["epoch"]) else { throw WalletBackup.Failure.said("That didn't work. Try again.") }
        return epoch
    }
    func deleteBackup() async throws { _ = try await call("POST", "/api/wallet/backup/delete", "{}") }

    /// Sets the daily limit: lower counts now; higher needs a fresh proof and counts in 24 hours.
    /// The server's words for what happened.
    func setLimit(_ sats: Int64, current: Int64) async throws -> String {
        let answer = try await call("POST", sats > current ? "/api/wallet/limit" : "/api/wallet/limit/lower", "{\"daily_sat\":\"\(sats)\"}")
        return LightningAddress.object(answer)?["words"] as? String ?? ""
    }
    func cancelRaise() async throws { _ = try await call("POST", "/api/wallet/limit/cancel", "{}") }

    /// What the server answered to "may this payment go?": its status, and whether a refusal asked
    /// for a fresh proof. Status 0: it couldn't be asked.
    struct SpendAnswer: Equatable { var status: Int; var stepUp: Bool; var words: String; var daily: Int64? }
    func spend(sats: Int64, id: String, newDestination: Bool) async -> SpendAnswer {
        guard loggedIn(), let url = URL(string: origin + "/api/wallet/spend") else { return SpendAnswer(status: 0, stepUp: false, words: "", daily: nil) }
        var r = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 10)
        r.httpMethod = "POST"
        r.httpBody = Data("{\"sats\":\"\(sats)\",\"payment_id\":\"\(id)\"\(newDestination ? ",\"new_destination\":\"1\"" : "")}".utf8)
        r.setValue("application/json", forHTTPHeaderField: "Content-Type")
        guard let authed = try? await authorize(r), let (data, status) = try? await send(authed) else { return SpendAnswer(status: 0, stepUp: false, words: "", daily: nil) }
        let o = LightningAddress.object(data)
        return SpendAnswer(status: status, stepUp: o?["step_up"] != nil, words: AppAuth.errorWords(data) ?? "", daily: LightningAddress.int(o?["daily_sat"]))
    }
    /// The payment didn't go: it no longer counts.
    func void(id: String) async { _ = try? await call("POST", "/api/wallet/spend/void", "{\"payment_id\":\"\(id)\"}") }
}

/// What this phone paid in the last 24 hours and the limit it last heard, for when the server
/// can't be asked (pay/wallet-backup.coax's metaverse_limit_local). Sizes and times only.
struct SpendLedger {
    var defaults: UserDefaults = .standard
    var now: () -> Date = { Date() }

    private var rows: [[Double]] {
        get { (defaults.array(forKey: "wallet.spends") as? [[Double]] ?? []).filter { $0.count == 2 && $0[0] > now().timeIntervalSince1970 - 86_400 } }
        nonmutating set { defaults.set(newValue, forKey: "wallet.spends") }
    }
    /// Sats this phone paid in the last 24 hours.
    var spent: Int64 { Int64(rows.reduce(0) { $0 + $1[1] }) }
    /// Where the last limit this phone heard is kept: the Keychain in the app (beside the seed,
    /// so deleting the app and installing it again doesn't forget that this wallet has a limit).
    var keep: LimitStore = KeychainLimitStore()
    /// The daily limit this phone last heard from its account, or -1 when it never heard one.
    var limit: Int64 {
        get { keep.read() ?? -1 }
        nonmutating set { if newValue >= 0, keep.read() != newValue { keep.write(newValue) } }
    }
    func paid(_ sats: Int64) { rows = rows + [[now().timeIntervalSince1970, Double(sats)]] }
    /// Somewhere this phone hasn't paid before (kept as a digest, never the address itself).
    func isNew(_ destination: String) -> Bool { !(defaults.stringArray(forKey: "wallet.paid") ?? []).contains(SpendLedger.digest(destination)) }
    func remember(_ destination: String) {
        var all = defaults.stringArray(forKey: "wallet.paid") ?? []
        let d = SpendLedger.digest(destination)
        if !all.contains(d) { all.append(d); defaults.set(Array(all.suffix(500)), forKey: "wallet.paid") }
    }
    static func digest(_ s: String) -> String { AppAuth.b64url(Data(Data(SHA256.hash(data: Data(s.lowercased().utf8))).prefix(12))) }
}

/// Where the last daily limit this phone heard from its account is kept.
protocol LimitStore { func read() -> Int64?; func write(_ sats: Int64) }
/// In the Keychain like the seed (service `com.openagents.app.spark`, account `limit-v1`, this
/// device only): it stays when the app is deleted, so a wallet that had a limit still has one
/// after a reinstall and pays nothing until someone logs in. It is a number, not a secret.
struct KeychainLimitStore: LimitStore {
    private var base: [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: "com.openagents.app.spark",
         kSecAttrAccount as String: "limit-v1", kSecAttrSynchronizable as String: false]
    }
    func read() -> Int64? {
        var query = base; query[kSecReturnData as String] = true; query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess, let data = result as? Data else { return nil }
        return Int64(String(decoding: data, as: UTF8.self))
    }
    func write(_ sats: Int64) {
        let value = Data(String(sats).utf8)
        if SecItemUpdate(base as CFDictionary, [kSecValueData as String: value] as CFDictionary) == errSecItemNotFound {
            var item = base; item[kSecValueData as String] = value; item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            SecItemAdd(item as CFDictionary, nil)
        }
    }
}
/// For tests: a limit kept in memory.
final class MemoryLimitStore: LimitStore {
    var sats: Int64?
    func read() -> Int64? { sats }
    func write(_ s: Int64) { sats = s }
}
