// Firm reason: 6 (iOS @MainActor Spark Bitcoin & Lightning wallet controller backed by OASparkWallet, OAWalletKeychain, pay/wallet.coax, pay/wallet-backup.coax, and NIP-06).
import Foundation
import Security

@MainActor
final class MobileWalletController: ObservableObject {
    static let shared = MobileWalletController()

    private static let limitKey = "wallet.dailyLimitSats"
    private static let spentKey = "wallet.dailySpentSats"
    private static let spentDayKey = "wallet.dailySpentUTCDay"
    private static let backedUpKey = "wallet.backupConfirmed"

    let spark = OASparkWallet()

    @Published var ready: Bool = false
    @Published var busy: Bool = false
    @Published var hasSeed: Bool = false
    @Published var statusText: String = "Not connected"
    @Published var errorMessage: String?
    @Published var balanceSats: UInt64 = 0
    @Published var sparkAddress: String = ""
    @Published var bolt11Invoice: String = ""
    @Published var payments: [OASparkWallet.PaymentItem] = []
    @Published var preparedQuote: OASparkWallet.Quote?
    @Published var nostrNpub: String = ""
    @Published var nostrPubkeyHex: String = ""
    @Published var backupConfirmed: Bool {
        didSet { UserDefaults.standard.set(backupConfirmed, forKey: Self.backedUpKey) }
    }
    @Published var dailyLimitSats: UInt64 {
        didSet { UserDefaults.standard.set(NSNumber(value: Int64(clamping: dailyLimitSats)), forKey: Self.limitKey) }
    }

    init() {
        self.backupConfirmed = UserDefaults.standard.bool(forKey: Self.backedUpKey)
        let storedLimit = UserDefaults.standard.object(forKey: Self.limitKey) as? NSNumber
        self.dailyLimitSats = storedLimit.map { UInt64(max(0, $0.int64Value)) } ?? 100_000
        refreshKeychainAndNostr()
    }

    var sidebarLabel: String {
        if ready { return "Wallet · \(Self.formatSats(balanceSats))" }
        if hasSeed { return "Spark Wallet" }
        return "Spark Wallet"
    }

    var dailySpentSats: UInt64 {
        resetDailySpentIfNewUTCDay()
        return UInt64(max(0, UserDefaults.standard.integer(forKey: Self.spentKey)))
    }

    var dailyRemainingSats: UInt64 {
        let spent = dailySpentSats
        return dailyLimitSats > spent ? (dailyLimitSats - spent) : 0
    }

    private func addDailySpent(_ sats: UInt64) {
        resetDailySpentIfNewUTCDay()
        let next = dailySpentSats + sats
        UserDefaults.standard.set(Int(clamping: next), forKey: Self.spentKey)
    }

    private func resetDailySpentIfNewUTCDay() {
        let day = Int(Date().timeIntervalSince1970 / 86_400)
        let prev = UserDefaults.standard.integer(forKey: Self.spentDayKey)
        if prev != day {
            UserDefaults.standard.set(day, forKey: Self.spentDayKey)
            UserDefaults.standard.set(0, forKey: Self.spentKey)
        }
    }

    static func formatSats(_ sats: UInt64) -> String {
        let nf = NumberFormatter()
        nf.numberStyle = .decimal
        return (nf.string(from: NSNumber(value: sats)) ?? "\(sats)") + " sats"
    }

    static func formatBTC(_ sats: UInt64) -> String {
        let whole = sats / 100_000_000
        let frac = sats % 100_000_000
        return String(format: "%llu.%08llu BTC", whole, frac)
    }

    private func storageDirectory() -> URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        return base.appendingPathComponent("openagents/wallet/spark-mainnet", isDirectory: true)
    }

    func refreshKeychainAndNostr() {
        switch OAWalletKeychain.readSeed() {
        case .notFound:
            hasSeed = false
            nostrNpub = ""
            nostrPubkeyHex = ""
        case .unreadable, .unavailable:
            hasSeed = false
            nostrNpub = ""
            nostrPubkeyHex = ""
        case .found(let entropy):
            hasSeed = true
            deriveReadOnlyNostr(entropy: entropy)
        }
    }

    private func deriveReadOnlyNostr(entropy: Data) {
        guard let words = OABIP39.englishWords(),
              let mnemonic = OABIP39.mnemonic(entropy: entropy, words: words) else { return }
        var priv = [UInt8](repeating: 0, count: 32)
        defer { for i in 0..<priv.count { priv[i] = 0 } }
        let phraseBytes = Array(mnemonic.utf8)
        let ok = phraseBytes.withUnsafeBufferPointer { pBuf in
            priv.withUnsafeMutableBufferPointer { kBuf in
                nip06_key(
                    oa_wallet_string(data: pBuf.baseAddress, count: Int64(pBuf.count)),
                    oa_wallet_u8_slice(data: kBuf.baseAddress, count: Int64(kBuf.count))
                )
            }
        }
        guard ok else { return }
        var npubBuf = [UInt8](repeating: 0, count: 96)
        var hexBuf = [UInt8](repeating: 0, count: 64)
        let npubLen = priv.withUnsafeMutableBufferPointer { kBuf in
            npubBuf.withUnsafeMutableBufferPointer { oBuf in
                nip06_npub(
                    oa_wallet_u8_slice(data: kBuf.baseAddress, count: Int64(kBuf.count)),
                    oa_wallet_u8_slice(data: oBuf.baseAddress, count: Int64(oBuf.count))
                )
            }
        }
        let hexLen = priv.withUnsafeMutableBufferPointer { kBuf in
            hexBuf.withUnsafeMutableBufferPointer { oBuf in
                nip06_pubkey_hex(
                    oa_wallet_u8_slice(data: kBuf.baseAddress, count: Int64(kBuf.count)),
                    oa_wallet_u8_slice(data: oBuf.baseAddress, count: Int64(oBuf.count))
                )
            }
        }
        if npubLen > 0 && hexLen == 64 {
            nostrNpub = String(decoding: npubBuf.prefix(Int(npubLen)), as: UTF8.self)
            nostrPubkeyHex = String(decoding: hexBuf.prefix(64), as: UTF8.self)
        }
    }

    func createNewWallet() async {
        guard !hasSeed else {
            await connect()
            return
        }
        var entropy = Data(count: 16)
        let rc = entropy.withUnsafeMutableBytes { buf in
            SecRandomCopyBytes(kSecRandomDefault, 16, buf.baseAddress!)
        }
        guard rc == errSecSuccess else {
            errorMessage = "Could not generate secure randomness."
            return
        }
        guard OAWalletKeychain.addSeedIfMissing(entropy) else {
            errorMessage = "Could not save seed in iOS Keychain."
            return
        }
        refreshKeychainAndNostr()
        await connect()
    }

    func restoreWallet(mnemonic phrase: String) async {
        guard !hasSeed else {
            errorMessage = "A wallet seed is already stored on this device."
            return
        }
        guard let words = OABIP39.englishWords(),
              let entropy = OABIP39.entropy(mnemonic: phrase, words: words) else {
            errorMessage = "Invalid 12- or 24-word BIP-39 recovery phrase."
            return
        }
        guard OAWalletKeychain.addSeedIfMissing(entropy) else {
            errorMessage = "Could not save seed in iOS Keychain."
            return
        }
        backupConfirmed = true
        refreshKeychainAndNostr()
        await connect()
    }

    func connect() async {
        refreshKeychainAndNostr()
        guard hasSeed else {
            statusText = "Create or restore a Spark wallet"
            return
        }
        busy = true
        errorMessage = nil
        statusText = "Connecting to Spark mainnet…"
        defer { busy = false }
        do {
            try await spark.openFromKeychain(storageDir: storageDirectory())
            ready = true
            statusText = "Connected · Spark mainnet"
            await refreshBalanceAndHistory()
        } catch {
            ready = false
            errorMessage = error.localizedDescription
            statusText = "Connection failed"
        }
    }

    func refreshBalanceAndHistory() async {
        guard ready else { return }
        do {
            balanceSats = try await spark.syncAndBalance()
            sparkAddress = try await spark.receiveSpark().paymentRequest
            payments = try await spark.listPayments(limit: 25)
            statusText = "Connected · Spark mainnet"
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func generateBolt11Invoice(amountSats: UInt64?, note: String = "") async {
        guard ready else { return }
        busy = true
        errorMessage = nil
        defer { busy = false }
        do {
            let req = try await spark.receiveLightning(amountSats: amountSats, description: note.isEmpty ? "OpenAgents" : note)
            bolt11Invoice = req.paymentRequest
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func prepareSend(destination: String, amountSats: UInt64?) async {
        guard ready else {
            errorMessage = "Connect your Spark wallet first."
            return
        }
        busy = true
        errorMessage = nil
        preparedQuote = nil
        defer { busy = false }
        do {
            preparedQuote = try await spark.prepareSend(input: destination, amountSats: amountSats)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func confirmSend() async {
        guard ready, let quote = preparedQuote else { return }
        busy = true
        errorMessage = nil
        defer { busy = false }
        let feeCap = max(UInt64(1_000), quote.amountSats / 10)
        let outcome = await spark.sendPrepared(
            idempotencyKey: "ios-send-\(UUID().uuidString)",
            feeCapSats: feeCap,
            dailyRemainingSats: dailyRemainingSats,
            nowUnix: Int64(Date().timeIntervalSince1970)
        )
        switch outcome {
        case .sent(let item):
            addDailySpent(item.amountSats)
            preparedQuote = nil
            statusText = "Sent \(Self.formatSats(item.amountSats))"
            await refreshBalanceAndHistory()
        case .refused(let reason, _):
            errorMessage = reason
        case .failed(let msg):
            errorMessage = msg
        case .waiting(let msg):
            statusText = msg ?? "Payment pending confirmation…"
            preparedQuote = nil
            await refreshBalanceAndHistory()
        }
    }

    func cancelPreparedSend() async {
        preparedQuote = nil
    }

    func revealRecoveryPhrase() -> String? {
        guard case .found(let entropy) = OAWalletKeychain.readSeed(),
              let words = OABIP39.englishWords() else { return nil }
        return OABIP39.mnemonic(entropy: entropy, words: words)
    }
}
