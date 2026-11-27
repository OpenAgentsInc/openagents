// Firm reason: 6 (AppKit dialogs and async Spark wallet bridge over pay/wallet.coax and OASparkWallet).
import AppKit

/// Shared desktop wallet state backed by `OASparkWallet` (Breez Spark mainnet)
/// and gated by `pay/wallet.coax` + `pay/wallet-backup.coax` (`#authority`).
/// Seeds live exclusively in macOS Keychain (`com.openagents.app.spark` / `seed-v1`)
/// and are never logged or passed into Coax.
final class DesktopWalletController {
    static let shared = DesktopWalletController()

    private static let limitKey = "wallet.dailyLimitSats"
    private static let spentKey = "wallet.dailySpentSats"
    private static let spentDayKey = "wallet.dailySpentUTCDay"
    private static let backedUpKey = "wallet.backupConfirmed"

    let spark = OASparkWallet()

    private(set) var ready: Bool = false
    private(set) var statusText: String = "Not connected"
    private(set) var balanceSats: UInt64 = 0
    private(set) var sparkAddress: String = ""
    private(set) var historySummary: String = "No payments yet."
    private(set) var busy: Bool = false
    private(set) var hasSeed: Bool = false
    private(set) var nostrNpub: String = ""
    private(set) var nostrPubkeyHex: String = ""
    private(set) var nostrCustody: String = "No seed in macOS Keychain yet"
    private(set) var nostrStatus: String = "Read-only · Create or connect wallet to derive NIP-06 key pair"
    private var lastDerivedEntropy: Data? = nil

    static let nostrDerivationPath = "m/44'/1237'/0'/0/0 (NIP-06)"

    var dailyLimitSats: UInt64 {
        get {
            let stored = UserDefaults.standard.object(forKey: Self.limitKey) as? NSNumber
            return stored.map { UInt64(max(0, $0.int64Value)) } ?? 100_000
        }
        set {
            UserDefaults.standard.set(NSNumber(value: Int64(clamping: newValue)), forKey: Self.limitKey)
        }
    }

    var dailySpentSats: UInt64 {
        get {
            resetDailySpentIfNewUTCDay()
            return UInt64(max(0, UserDefaults.standard.integer(forKey: Self.spentKey)))
        }
        set {
            resetDailySpentIfNewUTCDay()
            UserDefaults.standard.set(Int(clamping: newValue), forKey: Self.spentKey)
        }
    }

    var dailyRemainingSats: UInt64 {
        let limit = dailyLimitSats
        let spent = dailySpentSats
        return limit > spent ? (limit - spent) : 0
    }

    var backupConfirmed: Bool {
        get { UserDefaults.standard.bool(forKey: Self.backedUpKey) }
        set { UserDefaults.standard.set(newValue, forKey: Self.backedUpKey) }
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
        return (nf.string(from: NSNumber(value: sats)) ?? String(sats)) + " sats"
    }

    static func formatBTC(_ sats: UInt64) -> String {
        let whole = sats / 100_000_000
        let frac = sats % 100_000_000
        return String(format: "%llu.%08llu BTC", whole, frac)
    }

    /// Derives only the public read-only NIP-06 Nostr identity (`npub` + x-only hex pubkey)
    /// from the macOS Keychain BIP-39 seed. The temporary private key buffer is zeroed before returning.
    func refreshReadOnlyNostrInfo() {
        switch OAWalletKeychain.readSeed() {
        case .notFound:
            hasSeed = false
            lastDerivedEntropy = nil
            nostrNpub = ""
            nostrPubkeyHex = ""
            nostrCustody = "No seed in macOS Keychain yet"
            nostrStatus = "Read-only · Create or connect wallet to derive NIP-06 key pair"
        case .unreadable:
            hasSeed = false
            lastDerivedEntropy = nil
            nostrNpub = ""
            nostrPubkeyHex = ""
            nostrCustody = "Keychain item unreadable (com.openagents.app.spark / seed-v1)"
            nostrStatus = "Read-only · Keychain seed could not be decoded"
        case .unavailable(let s):
            hasSeed = false
            nostrNpub = ""
            nostrPubkeyHex = ""
            nostrCustody = "Keychain locked (OSStatus \(s))"
            nostrStatus = "Read-only · Unlock macOS Keychain to view Nostr identity"
        case .found(let entropy):
            hasSeed = true
            nostrCustody = "macOS Keychain (com.openagents.app.spark / seed-v1)"
            if lastDerivedEntropy == entropy && !nostrNpub.isEmpty && nostrPubkeyHex.count == 64 {
                nostrStatus = "Read-only · NIP-06 key pair paired with Spark wallet seed"
                return
            }
            guard let words = OABIP39.englishWords(),
                  let mnemonic = OABIP39.mnemonic(entropy: entropy, words: words) else {
                nostrNpub = ""
                nostrPubkeyHex = ""
                nostrStatus = "Read-only · BIP-39 wordlist unavailable"
                return
            }
            var priv = [UInt8](repeating: 0, count: 32)
            defer {
                for i in 0..<priv.count { priv[i] = 0 }
            }
            let phraseBytes = Array(mnemonic.utf8)
            let ok: Bool = phraseBytes.withUnsafeBufferPointer { pBuf in
                priv.withUnsafeMutableBufferPointer { kBuf in
                    nip06_key(
                        oa_wallet_string(data: pBuf.baseAddress, count: Int64(pBuf.count)),
                        oa_wallet_u8_slice(data: kBuf.baseAddress, count: Int64(kBuf.count))
                    )
                }
            }
            guard ok else {
                nostrNpub = ""
                nostrPubkeyHex = ""
                nostrStatus = "Read-only · Could not derive NIP-06 key pair"
                return
            }
            var npubBuf = [UInt8](repeating: 0, count: 96)
            var hexBuf = [UInt8](repeating: 0, count: 64)
            let npubLen: Int64 = priv.withUnsafeMutableBufferPointer { kBuf in
                npubBuf.withUnsafeMutableBufferPointer { oBuf in
                    nip06_npub(
                        oa_wallet_u8_slice(data: kBuf.baseAddress, count: Int64(kBuf.count)),
                        oa_wallet_u8_slice(data: oBuf.baseAddress, count: Int64(oBuf.count))
                    )
                }
            }
            let hexLen: Int64 = priv.withUnsafeMutableBufferPointer { kBuf in
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
                lastDerivedEntropy = entropy
                nostrStatus = "Read-only · NIP-06 key pair paired with Spark wallet seed"
            } else {
                nostrNpub = ""
                nostrPubkeyHex = ""
                nostrStatus = "Read-only · Could not encode Nostr public key"
            }
        }
    }

    static func parseTypedSats(_ text: String) -> UInt64? {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        var sats: Int64 = 0
        for ch in trimmed where ch != "," && ch != " " && ch != "_" {
            guard let d = ch.wholeNumberValue, ch.isASCII else { return nil }
            sats = metaverse_wallet_digit(sats, Int64(d))
            if sats < 0 { return nil }
        }
        return UInt64(sats)
    }

    func limitSummary() -> String {
        "\(Self.formatSats(dailyLimitSats)) / day (\(Self.formatSats(dailySpentSats)) spent)"
    }

    func backupSummary() -> String {
        switch OAWalletKeychain.readSeed() {
        case .notFound:
            return "No seed in Keychain yet"
        case .unreadable:
            return "Keychain item unreadable"
        case .unavailable(let s):
            return "Keychain locked (\(s))"
        case .found:
            let card = metaverse_backup_card(Int64(clamping: balanceSats), false, backupConfirmed, true, true, false)
            if backupConfirmed { return "Backed up (Keychain seed-v1)" }
            return card > 0 ? "Backup recommended (holds funds)" : "Keychain seed-v1 (not backed up)"
        }
    }

    func npubSummary() -> String {
        if !nostrNpub.isEmpty { return nostrNpub }
        switch OAWalletKeychain.readSeed() {
        case .found: return "Keychain seed-v1 (NIP-06 paired)"
        case .notFound: return "No Nostr key pair yet"
        case .unreadable: return "Keychain seed unreadable"
        case .unavailable: return "Keychain locked"
        }
    }

    func nostrHexDisplay() -> String {
        nostrPubkeyHex.isEmpty ? "—" : nostrPubkeyHex
    }

    func balanceDisplay() -> String {
        ready ? Self.formatSats(balanceSats) : (hasSeed ? "0 sats" : "—")
    }

    func btcBalanceDisplay() -> String {
        ready ? Self.formatBTC(balanceSats) : (hasSeed ? "0.00000000 BTC" : "Not connected")
    }

    func addressDisplay() -> String {
        if !sparkAddress.isEmpty { return sparkAddress }
        return ready ? "Fetching Spark address…" : "Connect wallet to view Spark address"
    }

    func connectAndSync(onUpdate: @escaping () -> Void) {
        guard !busy else { return }
        busy = true
        statusText = "Connecting to Spark mainnet…"
        onUpdate()

        Task {
            do {
                if await !spark.isConnected() {
                    try await spark.openFromKeychain()
                }
                let bal = try await spark.syncAndBalance()
                let sparkReq = try await spark.receiveSpark()
                let payments = try await spark.listPayments(limit: 5)
                let hist = Self.summarizePayments(payments)
                await MainActor.run {
                    self.balanceSats = bal
                    self.sparkAddress = sparkReq.paymentRequest
                    self.historySummary = hist
                    self.ready = true
                    self.statusText = "Connected (Spark mainnet)"
                    self.busy = false
                    onUpdate()
                }
            } catch {
                await MainActor.run {
                    self.ready = false
                    self.statusText = error.localizedDescription
                    self.busy = false
                    onUpdate()
                }
            }
        }
    }

    func refresh(onUpdate: @escaping () -> Void) {
        connectAndSync(onUpdate: onUpdate)
    }

    func recordSpend(_ totalSats: UInt64) {
        dailySpentSats = dailySpentSats &+ totalSats
    }

    private static func summarizePayments(_ items: [OASparkWallet.PaymentItem]) -> String {
        guard !items.isEmpty else { return "No payments yet." }
        return items.prefix(3).map { p in
            let dir = p.incoming ? "+" : "−"
            let st = p.status == .done ? "" : " (\(p.status.rawValue))"
            return "\(dir)\(formatSats(p.amountSats)) \(p.rail)\(st)"
        }.joined(separator: " · ")
    }
}

extension CoderWindow {
    func syncWalletSettings() {
        let w = DesktopWalletController.shared
        w.refreshReadOnlyNostrInfo()
        cd_settings_wallet(
            page,
            w.statusText,
            w.balanceDisplay(),
            w.addressDisplay(),
            w.npubSummary(),
            w.limitSummary(),
            w.backupSummary(),
            w.historySummary,
            w.ready ? 1 : 0
        )
        cd_wallet_info(
            page,
            w.btcBalanceDisplay(),
            w.nostrHexDisplay(),
            DesktopWalletController.nostrDerivationPath,
            w.nostrCustody,
            w.nostrStatus,
            w.hasSeed ? 1 : 0
        )
    }

    func openWalletPane() {
        closeGym()
        syncWalletSettings()
        let w = DesktopWalletController.shared
        if w.hasSeed && !w.ready && !w.busy {
            w.connectAndSync { [weak self] in
                self?.syncWalletSettings()
                self?.wake()
            }
        }
        wake()
    }

    func walletCopyNpub() {
        let w = DesktopWalletController.shared
        guard !w.nostrNpub.isEmpty else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(w.nostrNpub, forType: .string)
        cd_toast(page, "Copied Nostr npub", 0)
        wake()
    }

    func walletCopyHex() {
        let w = DesktopWalletController.shared
        guard !w.nostrPubkeyHex.isEmpty else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(w.nostrPubkeyHex, forType: .string)
        cd_toast(page, "Copied Nostr public key (hex)", 0)
        wake()
    }

    func walletCopySparkAddress() {
        let w = DesktopWalletController.shared
        guard !w.sparkAddress.isEmpty else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(w.sparkAddress, forType: .string)
        cd_toast(page, "Copied Spark address", 0)
        wake()
    }

    func walletConnectOrCreate() {
        switch OAWalletKeychain.readSeed() {
        case .found:
            DesktopWalletController.shared.connectAndSync { [weak self] in
                self?.syncWalletSettings()
                self?.wake()
            }
        case .notFound:
            var d = CoderDialog(
                size: .sm,
                title: "Create Bitcoin Wallet?",
                body: "No Spark wallet seed is stored in this Mac's Keychain yet. OpenAgents will generate a new 12-word BIP-39 seed in Keychain (com.openagents.app.spark, this device only) and connect to Spark mainnet."
            )
            d.buttons = [.init(title: "Cancel"), .init(title: "Create Wallet", kind: .primary)]
            d.cancel = 0; d.confirm = 1
            present(d) { [weak self] answer, _ in
                guard let self else { return }
                guard answer == 1 else { self.refocus(); return }
                guard let entropy = OABIP39.generateEntropy16(),
                      OAWalletKeychain.addSeedIfMissing(entropy) else {
                    self.present(.notice("Could not create wallet", "Failed to store new seed in macOS Keychain."))
                    return
                }
                DesktopWalletController.shared.connectAndSync { [weak self] in
                    self?.syncWalletSettings()
                    self?.wake()
                }
                self.refocus()
            }
        case .unreadable:
            present(.notice("Wallet seed unreadable", "The existing Keychain item com.openagents.app.spark/seed-v1 is not 16 or 32 bytes. It was left untouched."))
        case .unavailable(let status):
            present(.notice("Keychain unavailable", "Could not read macOS Keychain (OSStatus \(status)). Unlock your Mac Keychain and try again."))
        }
    }

    func walletRefresh() {
        DesktopWalletController.shared.refresh { [weak self] in
            self?.syncWalletSettings()
            self?.wake()
        }
    }

    func walletReceiveDialog() {
        let w = DesktopWalletController.shared
        guard w.ready else { walletConnectOrCreate(); return }
        var d = CoderDialog(
            size: .md,
            title: "Receive Bitcoin",
            body: "Enter an optional amount in sats to create a 24-hour BOLT11 Lightning invoice, or copy your Spark address directly."
        )
        d.fields = [
            .init(label: "Amount (sats, optional)", value: "", hint: "e.g. 1000"),
            .init(label: "Note (optional)", value: "", hint: "OpenAgents")
        ]
        d.buttons = [
            .init(title: "Copy Spark Address", aside: true),
            .init(title: "Cancel"),
            .init(title: "Create Invoice", kind: .primary)
        ]
        d.cancel = 1; d.confirm = 2
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == 0 {
                if !w.sparkAddress.isEmpty {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(w.sparkAddress, forType: .string)
                    self.present(.notice("Spark address copied", w.sparkAddress))
                }
                return
            }
            guard answer == 2 else { self.refocus(); return }
            let rawAmt = values.first?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            var amt: UInt64? = nil
            if !rawAmt.isEmpty {
                guard let parsed = DesktopWalletController.parseTypedSats(rawAmt), parsed > 0 else {
                    self.present(.notice("Invalid amount", "Enter a whole number of sats."))
                    return
                }
                amt = parsed
            }
            let note = values.count > 1 ? values[1].trimmingCharacters(in: .whitespacesAndNewlines) : ""
            Task {
                do {
                    let req = try await w.spark.receiveLightning(amountSats: amt, description: note.isEmpty ? "OpenAgents" : note)
                    await MainActor.run {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(req.paymentRequest, forType: .string)
                        let feeNote = req.feeSats > 0 ? "\nEstimated receive fee: \(DesktopWalletController.formatSats(req.feeSats))" : ""
                        self.present(.notice("Lightning invoice copied to clipboard", req.paymentRequest + feeNote))
                    }
                } catch {
                    await MainActor.run {
                        self.present(.notice("Could not create invoice", error.localizedDescription))
                    }
                }
            }
        }
    }

    func walletSendDialog() {
        let w = DesktopWalletController.shared
        guard w.ready else { walletConnectOrCreate(); return }
        var d = CoderDialog(
            size: .md,
            title: "Send Bitcoin",
            body: "Paste a BOLT11 invoice, Lightning Address, LNURL-pay, Spark address, or Bitcoin address. You will review the exact fee quote before any sats leave."
        )
        d.fields = [
            .init(label: "Destination", value: "", hint: "lnbc... / name@domain / spark1..."),
            .init(label: "Amount (sats, if not in invoice)", value: "", hint: "e.g. 1000")
        ]
        d.buttons = [.init(title: "Cancel"), .init(title: "Review Quote", kind: .primary)]
        d.cancel = 0; d.confirm = 1
        present(d) { [weak self] answer, values in
            guard let self else { return }
            guard answer == 1 else { self.refocus(); return }
            let dest = values.first?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            guard !dest.isEmpty else {
                self.present(.notice("Destination required", "Paste a BOLT11 invoice, Lightning Address, or Spark address."))
                return
            }
            let rawAmt = values.count > 1 ? values[1].trimmingCharacters(in: .whitespacesAndNewlines) : ""
            var amt: UInt64? = nil
            if !rawAmt.isEmpty {
                guard let parsed = DesktopWalletController.parseTypedSats(rawAmt), parsed > 0 else {
                    self.present(.notice("Invalid amount", "Enter a whole number of sats."))
                    return
                }
                amt = parsed
            }
            Task {
                do {
                    let quote = try await w.spark.prepareSend(input: dest, amountSats: amt)
                    await MainActor.run { self.confirmWalletQuote(quote) }
                } catch {
                    await MainActor.run {
                        self.present(.notice("Could not prepare quote", error.localizedDescription))
                    }
                }
            }
        }
    }

    private func confirmWalletQuote(_ quote: OASparkWallet.Quote) {
        let w = DesktopWalletController.shared
        let total = quote.amountSats &+ quote.feeSats
        let warn = quote.highFeeWarning ? "\n\nWarning: fee is high relative to amount (pay/wallet.coax)." : ""
        let body = "Rail: \(quote.rail)\nRecipient: \(quote.recipient)\nAmount: \(DesktopWalletController.formatSats(quote.amountSats))\nFee: \(DesktopWalletController.formatSats(quote.feeSats))\nTotal: \(DesktopWalletController.formatSats(total))\nDaily remaining: \(DesktopWalletController.formatSats(w.dailyRemainingSats))\(warn)"
        var d = CoderDialog(size: .md, title: "Confirm Bitcoin Payment", body: body)
        d.buttons = [.init(title: "Cancel"), .init(title: "Send \(DesktopWalletController.formatSats(total))", kind: .primary)]
        d.cancel = 0; d.confirm = 1
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            guard answer == 1 else { self.refocus(); return }
            let feeCap = max(quote.feeSats, 5_000)
            let key = UUID().uuidString
            Task {
                let outcome = await w.spark.sendPrepared(
                    idempotencyKey: key,
                    feeCapSats: feeCap,
                    dailyRemainingSats: w.dailyRemainingSats
                )
                await MainActor.run {
                    switch outcome {
                    case .sent(let item):
                        w.recordSpend(item.amountSats &+ item.feeSats)
                        self.present(.notice("Payment sent", "\(DesktopWalletController.formatSats(item.amountSats)) sent via \(item.rail) (fee \(DesktopWalletController.formatSats(item.feeSats)))."))
                        self.walletRefresh()
                    case .refused(let reason, _):
                        self.present(.notice("Payment refused by pay/wallet.coax", reason))
                    case .failed(let reason):
                        self.present(.notice("Payment failed", reason))
                    case .waiting(let id):
                        self.present(.notice("Payment pending", "Payment is in flight (\(id ?? "awaiting network confirmation")). Do not resend until synced."))
                        self.walletRefresh()
                    }
                }
            }
        }
    }

    func walletLimitDialog() {
        let w = DesktopWalletController.shared
        var d = CoderDialog(
            size: .sm,
            title: "Daily Spend Limit",
            body: "Maximum satoshis allowed per UTC day before pay/wallet.coax refuses a payment. Current: \(w.limitSummary())."
        )
        d.fields = [.init(label: "Daily limit (sats)", value: String(w.dailyLimitSats), hint: "100000")]
        d.buttons = [.init(title: "Cancel"), .init(title: "Save Limit", kind: .primary)]
        d.cancel = 0; d.confirm = 1
        present(d) { [weak self] answer, values in
            guard let self else { return }
            guard answer == 1 else { self.refocus(); return }
            guard let raw = values.first, let parsed = DesktopWalletController.parseTypedSats(raw), parsed > 0 else {
                self.present(.notice("Invalid limit", "Enter a positive whole number of sats."))
                return
            }
            w.dailyLimitSats = parsed
            self.syncWalletSettings()
            self.refocus()
        }
    }

    func walletBackupDialog() {
        let w = DesktopWalletController.shared
        switch OAWalletKeychain.readSeed() {
        case .notFound:
            present(.notice("No wallet yet", "Create or connect your wallet first before backing up recovery words."))
            return
        case .unreadable, .unavailable:
            present(.notice("Wallet seed unavailable", w.backupSummary()))
            return
        case .found(let entropy):
            var warn = CoderDialog(
                size: .sm,
                title: "Reveal Recovery Words?",
                body: "Anyone with your 12 BIP-39 words can spend your Bitcoin and sign as your Nostr key. Never share them or paste them into a chat. Words are never logged."
            )
            warn.buttons = [.init(title: "Cancel"), .init(title: "Show Words Once", kind: .danger)]
            warn.cancel = 0; warn.confirm = 0
            present(warn) { [weak self] answer, _ in
                guard let self else { return }
                guard answer == 1 else { self.refocus(); return }
                guard let words = OABIP39.englishWords(),
                      let mnemonic = OABIP39.mnemonic(entropy: entropy, words: words) else {
                    self.present(.notice("Wordlist unavailable", "Could not verify bundled bip39-english.txt."))
                    return
                }
                var show = CoderDialog(
                    size: .md,
                    title: "Your 12 Recovery Words",
                    body: "Write these down offline and store them safely:\n\n\(mnemonic)"
                )
                show.buttons = [.init(title: "Done — I Wrote Them Down", kind: .primary)]
                show.cancel = 0; show.confirm = 0
                self.present(show) { [weak self] _, _ in
                    w.backupConfirmed = true
                    self?.syncWalletSettings()
                    self?.refocus()
                }
            }
        }
    }
}
