import Foundation
import BreezSdkSpark

/// The person's Spark wallet on Bitcoin mainnet, opened from the existing
/// Keychain seed the same way the earlier app opened it: the entropy's
/// English BIP-39 words, no passphrase, Breez's default mainnet account.
///
/// This is the only file that imports BreezSdkSpark
/// (docs/lightning/coax-integration.md). It does the effects (Keychain, SDK
/// calls, events) and hands the screens plain values; verse/wallet.coax
/// decides whether a quote may be paid and how a payment moves. It never
/// writes, replaces or shows the seed.
actor SparkHost {
    enum State: Equatable {
        case opening
        case balance(UInt64)
        case noWallet
        case locked
        case unreadable
        case failed(String)
    }

    /// One payment in the activity list.
    struct Activity: Equatable, Sendable {
        enum Status: Sendable { case done, pending, failed }
        var id: String
        var incoming: Bool
        var amount: UInt64
        var fee: UInt64
        var status: Status
        var date: Date
        /// "Lightning", "Spark" or "On-chain".
        var rail: String
        var note: String?
        /// The invoice, Lightning address or transaction id, for copying.
        var reference: String?
        /// An on-chain transaction id, for a block explorer.
        var txid: String?
    }

    /// What a pasted or scanned payment request is, as far as the screens
    /// need to know. The SDK parsed it; nothing here re-parses Lightning.
    enum Destination: Sendable {
        /// A BOLT11 invoice: its amount (nil: the payer chooses), note and expiry.
        case invoice(amount: UInt64?, note: String?, expires: Date)
        /// A Lightning address or LNURL-pay: whole-sat bounds, comment length.
        case lnurl(name: String, low: UInt64, high: UInt64, comment: Int)
        case bitcoin(address: String, amount: UInt64?)
        case spark(address: String, amount: UInt64?)
        /// An LNURL-withdraw: money comes in, not out.
        case withdraw(note: String, low: UInt64, high: UInt64)
        case unsupported(String)
    }

    /// A send quote, held until it is paid or replaced.
    struct Quote: Sendable {
        var amount: UInt64
        var fee: UInt64
        var expires: Date?
        /// On-chain only: the fee at each speed, fastest first.
        var speeds: [UInt64]
        var recipient: String
    }

    enum Outcome: Sendable {
        case sent(Activity)
        /// Refused before any money moved.
        case notSent(String)
        /// Handed over, but not settled yet (or no answer): wait for the SDK.
        case waiting(String?)
    }

    /// A request to be paid: the text to share and what goes in the QR.
    struct Request: Sendable {
        var text: String
        var qr: String
        var fee: UInt64
    }

    enum Receive: Sendable {
        /// One QR for both: a BIP21 URI with an on-chain address and a
        /// Lightning invoice (bitcoin.design's unified request).
        case unified(amount: UInt64?, note: String)
        case lightning(amount: UInt64?, note: String)
        case bitcoin
        case spark
    }

    /// An on-chain deposit the wallet hasn't claimed.
    struct Deposit: Sendable {
        var txid: String
        var vout: UInt32
        var amount: UInt64
        var mature: Bool
        var problem: String?
    }

    enum Change: Sendable {
        case updated
        case received(Activity)
        case settled(Activity)
        case failed(Activity)
    }

    private var sdk: BreezSdk?
    #if DEBUG
    private var throwaway: Data?
    private let throwawayFolder = "spark-mainnet-throwaway-" + UUID().uuidString
    #endif
    private var listener: String?
    private var parsed: InputType?
    private var prepared: PrepareSendPaymentResponse?
    private var preparedLnurl: PrepareLnurlPayResponse?
    private var continuations: [UUID: AsyncStream<Change>.Continuation] = [:]

    // MARK: Opening

    func balance() async -> State {
        var entropy = Data()
        var folder = "spark-mainnet"
        #if DEBUG
        // UI tests only: a fresh, empty wallet from random words held in
        // memory for this launch, never the Keychain, in its own folder.
        // Release builds have no such path.
        if ProcessInfo.processInfo.environment["OPENAGENTS_THROWAWAY_WALLET"] == "1" {
            if throwaway == nil {
                var bytes = [UInt8](repeating: 0, count: 16)
                guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else { return .unreadable }
                throwaway = Data(bytes)
            }
            entropy = throwaway!
            folder = throwawayFolder
        }
        #endif
        if entropy.isEmpty {
            switch WalletKeychain.readSeed() {
            case .found(let bytes): entropy = bytes
            case .notFound: return .noWallet
            case .unreadable: return .unreadable
            case .unavailable(let status):
                NSLog("OpenAgents wallet: Keychain status %d", status)
                if status == errSecInteractionNotAllowed { return .locked }
                return .failed("We couldn't open this phone's Keychain.")
            }
        }
        guard let words = BIP39.englishWords(), let mnemonic = BIP39.mnemonic(entropy: entropy, words: words) else {
            return .unreadable
        }
        do {
            let sdk = try await open(mnemonic: mnemonic, folder: folder)
            let info = try await sdk.getInfo(request: GetInfoRequest(ensureSynced: true))
            return .balance(info.balanceSats)
        } catch {
            NSLog("OpenAgents wallet: %@", String(describing: error))
            return .failed("We couldn't reach Spark. Check the connection and reopen OpenAgents.")
        }
    }

    func disconnect() async {
        if let sdk, let listener { _ = await sdk.removeEventListener(id: listener) }
        try? await sdk?.disconnect()
        sdk = nil; listener = nil
    }

    private func open(mnemonic: String, folder: String) async throws -> BreezSdk {
        if let sdk { return sdk }
        var config = defaultConfig(network: .mainnet)
        config.apiKey = breezApiKey
        // Encrypted multi-device sync to Breez's server stays off.
        config.realTimeSyncServerUrl = nil
        // As before: claim on-chain deposits at maturity for up to the
        // network's recommended rate plus 1 sat/vB.
        config.maxDepositClaimFee = MaxFee(onchain: .networkRecommended(leewaySatPerVbyte: 1))
        let directory = try FileManager.default
            .url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
            .appendingPathComponent(folder, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let builder = SdkBuilder(config: config, seed: .mnemonic(mnemonic: mnemonic, passphrase: nil))
        await builder.withDefaultStorage(storageDir: directory.path)
        let sdk = try await builder.build()
        self.sdk = sdk
        listener = await sdk.addEventListener(listener: Relay(host: self))
        return sdk
    }

    /// An SDK error in words a person can act on; the SDK's own text is
    /// logged, never shown.
    static func plain(_ error: Error) -> Error {
        if error is HostError { return error }
        NSLog("OpenAgents wallet: %@", String(describing: error))
        guard let e = error as? SdkError else { return HostError.said("Something went wrong. Nothing was sent; try again.") }
        let text = String(describing: e).lowercased()
        switch e {
        case .InsufficientFunds: return HostError.said("There isn't enough in your wallet for this payment and its fee.")
        case .InvalidInput: return HostError.said("That request isn't valid. Check the amount and the request, then try again.")
        default:
            if text.contains("insufficient funds") { return HostError.said("There isn't enough in your wallet for this payment and its fee.") }
            if text.contains("network") || text.contains("timeout") || text.contains("connect") {
                return HostError.said("We couldn't reach Spark. Check the connection and try again.")
            }
            return HostError.said("Spark couldn't do that right now. Nothing was sent; try again.")
        }
    }

    private func connected() throws -> BreezSdk {
        guard let sdk else { throw HostError.closed }
        return sdk
    }

    enum HostError: LocalizedError {
        case closed, unsupported, noQuote, said(String)
        var errorDescription: String? {
            switch self {
            case .said(let text): return text
            case .closed: return "Your wallet isn't open. Close this and try again."
            case .unsupported: return "OpenAgents can't pay this kind of request yet."
            case .noQuote: return "That quote is gone. Start the payment again."
            }
        }
    }

    // MARK: Events

    /// Balance and payment changes, as they happen.
    func changes() -> AsyncStream<Change> {
        let id = UUID()
        return AsyncStream { continuation in
            continuations[id] = continuation
            continuation.onTermination = { _ in Task { await self.drop(id) } }
        }
    }
    private func drop(_ id: UUID) { continuations[id] = nil }

    fileprivate func handle(_ event: SdkEvent) {
        let change: Change
        switch event {
        case .paymentSucceeded(let payment):
            let a = Self.activity(payment)
            change = a.incoming ? .received(a) : .settled(a)
        case .paymentFailed(let payment): change = .failed(Self.activity(payment))
        case .synced, .paymentPending, .claimedDeposits, .newDeposits, .unclaimedDeposits: change = .updated
        default: return
        }
        for c in continuations.values { c.yield(change) }
    }

    private final class Relay: EventListener, @unchecked Sendable {
        weak var host: SparkHost?
        init(host: SparkHost) { self.host = host }
        func onEvent(event: SdkEvent) async { await host?.handle(event) }
    }

    // MARK: Reading

    func spendable() async throws -> UInt64 {
        do { return try await spendableRaw() } catch { throw Self.plain(error) }
    }
    private func spendableRaw() async throws -> UInt64 {
        try await connected().getInfo(request: GetInfoRequest(ensureSynced: false)).balanceSats
    }

    func activity(limit: UInt32 = 100) async throws -> [Activity] {
        do { return try await activityRaw(limit: limit) } catch { throw Self.plain(error) }
    }
    private func activityRaw(limit: UInt32) async throws -> [Activity] {
        try await connected().listPayments(request: ListPaymentsRequest(limit: limit, sortAscending: false)).payments.map(Self.activity)
    }

    /// One payment, by id: how an unknown send is resolved.
    func payment(_ id: String) async throws -> Activity {
        Self.activity(try await connected().getPayment(request: GetPaymentRequest(paymentId: id)).payment)
    }

    /// Dollars per bitcoin, if the SDK's rate service answers.
    func dollarsPerBitcoin() async -> Double? {
        guard let rates = try? await connected().listFiatRates().rates else { return nil }
        return rates.first { $0.coin == "USD" }?.value
    }

    func lightningAddress() async -> String? {
        try? await connected().getLightningAddress()?.lightningAddress
    }

    // MARK: Sending

    /// What `text` asks for. A BIP21 URI becomes the best method in it:
    /// Lightning first, then Spark, then on-chain.
    func parse(_ text: String) async throws -> Destination {
        do { return try await parseRaw(text) } catch { throw Self.plain(error) }
    }
    private func parseRaw(_ text: String) async throws -> Destination {
        var input = try await connected().parse(input: text.trimmingCharacters(in: .whitespacesAndNewlines))
        if case .bip21(let uri) = input {
            let order: [(InputType) -> Bool] = [
                { if case .bolt11Invoice = $0 { return true }; return false },
                { if case .sparkAddress = $0 { return true }; return false },
                { if case .bitcoinAddress = $0 { return true }; return false },
            ]
            guard let best = order.lazy.compactMap({ test in uri.paymentMethods.first(where: test) }).first else {
                return .unsupported("This payment link has no method OpenAgents can pay.")
            }
            input = best
            parsed = best
            switch best {
            case .bitcoinAddress(let a): return .bitcoin(address: a.address, amount: uri.amountSat)
            case .sparkAddress(let a): return .spark(address: a.address, amount: uri.amountSat)
            default: break
            }
        }
        parsed = input
        switch input {
        case .bolt11Invoice(let d):
            if d.network != .bitcoin { return .unsupported("This invoice is for a test network, not Bitcoin.") }
            return .invoice(amount: d.amountMsat.map { $0 / 1000 }, note: d.description,
                            expires: Date(timeIntervalSince1970: TimeInterval(d.timestamp + d.expiry)))
        case .lightningAddress(let d):
            return lnurl(d.address, d.payRequest)
        case .lnurlPay(let d):
            return lnurl(d.address ?? d.domain, d)
        case .bitcoinAddress(let a):
            if a.network != .bitcoin { return .unsupported("This address is for a test network, not Bitcoin.") }
            return .bitcoin(address: a.address, amount: nil)
        case .sparkAddress(let a):
            if a.network != .bitcoin { return .unsupported("This Spark address is for a test network.") }
            return .spark(address: a.address, amount: nil)
        case .lnurlWithdraw(let d):
            return .withdraw(note: d.defaultDescription, low: UInt64(max(0, verse_wallet_msat_up(Int64(clamping: d.minWithdrawable)))),
                             high: UInt64(max(0, verse_wallet_msat_down(Int64(clamping: d.maxWithdrawable)))))
        case .bolt12Offer, .bolt12Invoice, .bolt12InvoiceRequest:
            return .unsupported("This is a BOLT12 offer. Ask for a Lightning invoice or address instead.")
        case .lnurlAuth:
            return .unsupported("This is a sign-in code, not a payment.")
        case .silentPaymentAddress:
            return .unsupported("Silent payment addresses aren't supported yet. Ask for a regular bitcoin address.")
        case .url:
            return .unsupported("This is a web link, not a payment request.")
        default:
            return .unsupported("OpenAgents can't pay this kind of request yet.")
        }
    }

    private func lnurl(_ name: String, _ d: LnurlPayRequestDetails) -> Destination {
        .lnurl(name: name, low: UInt64(max(1, verse_wallet_msat_up(Int64(clamping: d.minSendable)))),
               high: UInt64(max(0, verse_wallet_msat_down(Int64(clamping: d.maxSendable)))), comment: Int(d.commentAllowed))
    }

    /// A quote for the request last parsed. `amount` is required unless the
    /// request carries one. `all` sends everything, the fee taken from it.
    func quote(text: String, amount: UInt64?, comment: String?, all: Bool = false) async throws -> Quote {
        do { return try await quoteRaw(text: text, amount: amount, comment: comment, all: all) } catch { throw Self.plain(error) }
    }
    private func quoteRaw(text: String, amount: UInt64?, comment: String?, all: Bool) async throws -> Quote {
        let sdk = try connected()
        prepared = nil; preparedLnurl = nil
        switch parsed {
        case .lightningAddress(let d)?:
            return try await quoteLnurl(sdk, d.payRequest, d.address, amount, comment)
        case .lnurlPay(let d)?:
            return try await quoteLnurl(sdk, d, d.address ?? d.domain, amount, comment)
        case nil, .lnurlWithdraw?, .lnurlAuth?:
            throw HostError.unsupported
        default: break
        }
        let paymentRequest: String
        switch parsed {
        case .bolt11Invoice(let d)?: paymentRequest = d.invoice.bolt11
        case .bitcoinAddress(let a)?: paymentRequest = a.address
        case .sparkAddress(let a)?: paymentRequest = a.address
        default: throw HostError.unsupported
        }
        let response = try await sdk.prepareSendPayment(request: PrepareSendPaymentRequest(
            paymentRequest: .input(input: paymentRequest), amount: amount.map { U128(Int($0)) },
            feePolicy: all ? .feesIncluded : .feesExcluded))
        prepared = response
        let sent = Self.sats(response.amount)
        switch response.paymentMethod {
        case .bolt11Invoice(let d, _, let fee):
            return Quote(amount: sent, fee: fee, expires: Date(timeIntervalSince1970: TimeInterval(d.timestamp + d.expiry)), speeds: [], recipient: Self.short(d.invoice.bolt11))
        case .bitcoinAddress(let a, let q):
            let speeds = [q.speedFast, q.speedMedium, q.speedSlow].map { $0.userFeeSat + $0.l1BroadcastFeeSat }
            return Quote(amount: sent, fee: speeds[1], expires: Date(timeIntervalSince1970: TimeInterval(q.expiresAt)), speeds: speeds, recipient: a.address)
        case .sparkAddress(let address, let fee, _):
            return Quote(amount: sent, fee: Self.sats(fee), expires: nil, speeds: [], recipient: address)
        case .sparkInvoice(_, let fee, _):
            return Quote(amount: sent, fee: Self.sats(fee), expires: nil, speeds: [], recipient: Self.short(paymentRequest))
        default:
            prepared = nil
            throw HostError.unsupported
        }
    }

    private func quoteLnurl(_ sdk: BreezSdk, _ d: LnurlPayRequestDetails, _ name: String, _ amount: UInt64?, _ comment: String?) async throws -> Quote {
        guard let amount else { throw HostError.unsupported }
        let response = try await sdk.prepareLnurlPay(request: PrepareLnurlPayRequest(
            amount: U128(Int(amount)), payRequest: d, comment: (comment?.isEmpty ?? true) ? nil : comment, validateSuccessActionUrl: true))
        preparedLnurl = response
        let i = response.invoiceDetails
        return Quote(amount: response.amountSats, fee: response.feeSats,
                     expires: Date(timeIntervalSince1970: TimeInterval(i.timestamp + i.expiry)), speeds: [], recipient: name)
    }

    /// Pays the held quote once, under `key` (a UUID made for this quote).
    /// `speed` picks the on-chain fee: 0 fast, 1 medium, 2 slow.
    func send(key: String, speed: Int) async -> Outcome {
        do {
            let sdk = try connected()
            if let lnurl = preparedLnurl {
                let response = try await sdk.lnurlPay(request: LnurlPayRequest(prepareResponse: lnurl, idempotencyKey: key))
                preparedLnurl = nil
                return Self.outcome(response.payment)
            }
            guard let prepared else { return .notSent(HostError.noQuote.localizedDescription) }
            let options: SendPaymentOptions?
            switch prepared.paymentMethod {
            case .bitcoinAddress: options = .bitcoinAddress(confirmationSpeed: [.fast, .medium, .slow][min(max(speed, 0), 2)])
            case .bolt11Invoice: options = .bolt11Invoice(preferSpark: false, completionTimeoutSecs: 30)
            default: options = nil
            }
            let response = try await sdk.sendPayment(request: SendPaymentRequest(prepareResponse: prepared, options: options, idempotencyKey: key))
            self.prepared = nil
            return Self.outcome(response.payment)
        } catch let error as SdkError {
            // Refusals the SDK makes before moving anything; anything else
            // may have gone out, so the payment waits for the SDK to say.
            switch error {
            case .InsufficientFunds: return .notSent("There isn't enough in your wallet for this payment and its fee.")
            case .InvalidInput(let why): return .notSent(why)
            default:
                NSLog("OpenAgents wallet: send: %@", String(describing: error))
                return .waiting(nil)
            }
        } catch {
            NSLog("OpenAgents wallet: send: %@", String(describing: error))
            return .waiting(nil)
        }
    }

    private static func outcome(_ p: Payment) -> Outcome {
        let a = activity(p)
        switch p.status {
        case .completed: return .sent(a)
        case .failed: return .notSent("The payment failed. No money left your wallet.")
        case .pending: return .waiting(a.id)
        }
    }

    // MARK: Receiving

    func receive(_ kind: Receive) async throws -> Request {
        do { return try await receiveRaw(kind) } catch { throw Self.plain(error) }
    }
    private func receiveRaw(_ kind: Receive) async throws -> Request {
        let sdk = try connected()
        func ask(_ m: ReceivePaymentMethod) async throws -> ReceivePaymentResponse {
            try await sdk.receivePayment(request: ReceivePaymentRequest(paymentMethod: m))
        }
        // 24 hours, for requests sent in messages (bitcoin.design).
        let day: UInt32 = 86_400
        switch kind {
        case .lightning(let amount, let note):
            let r = try await ask(.bolt11Invoice(description: note, amountSats: amount, expirySecs: day, paymentHash: nil, receiverIdentityPublicKey: nil))
            return Request(text: r.paymentRequest, qr: r.paymentRequest.uppercased(), fee: Self.sats(r.fee))
        case .bitcoin:
            let r = try await ask(.bitcoinAddress(newAddress: nil))
            return Request(text: r.paymentRequest, qr: "bitcoin:" + r.paymentRequest.uppercased(), fee: Self.sats(r.fee))
        case .spark:
            let r = try await ask(.sparkAddress)
            return Request(text: r.paymentRequest, qr: r.paymentRequest, fee: Self.sats(r.fee))
        case .unified(let amount, let note):
            let address = try await ask(.bitcoinAddress(newAddress: nil)).paymentRequest
            let invoice = try await ask(.bolt11Invoice(description: note, amountSats: amount, expirySecs: day, paymentHash: nil, receiverIdentityPublicKey: nil))
            var query = [String]()
            if let amount { query.append("amount=" + Self.btc(amount)) }
            if !note.isEmpty, let m = note.addingPercentEncoding(withAllowedCharacters: .alphanumerics) { query.append("message=" + m) }
            let text = "bitcoin:\(address)?" + (query + ["lightning=" + invoice.paymentRequest]).joined(separator: "&")
            let qr = "bitcoin:\(address.uppercased())?" + (query + ["lightning=" + invoice.paymentRequest.uppercased()]).joined(separator: "&")
            return Request(text: text, qr: qr, fee: Self.sats(invoice.fee))
        }
    }

    /// Takes an LNURL-withdraw's money into this wallet.
    func withdraw(amount: UInt64) async throws {
        do { try await withdrawRaw(amount: amount) } catch { throw Self.plain(error) }
    }
    private func withdrawRaw(amount: UInt64) async throws {
        guard case .lnurlWithdraw(let d)? = parsed else { throw HostError.unsupported }
        _ = try await connected().lnurlWithdraw(request: LnurlWithdrawRequest(amountSats: amount, withdrawRequest: d, completionTimeoutSecs: 30))
    }

    func deposits() async throws -> [Deposit] {
        try await connected().listUnclaimedDeposits(request: ListUnclaimedDepositsRequest()).deposits.map { d in
            var problem: String?
            switch d.claimError {
            case .maxDepositClaimFeeExceeded(_, _, _, let need)?: problem = "Claiming it costs \(need) sats right now, more than the limit. It will be claimed when fees fall."
            case .depositTooSmall?: problem = "It is too small to claim."
            case .missingUtxo?: problem = "The transaction can't be found on the network."
            case .generic(let message)?: problem = message
            case nil: problem = nil
            }
            return Deposit(txid: d.txid, vout: d.vout, amount: d.amountSats, mature: d.isMature, problem: problem)
        }
    }

    /// Claims a deposit now, paying up to the network's fast rate.
    func claim(_ d: Deposit) async throws {
        do { try await claimRaw(d) } catch { throw Self.plain(error) }
    }
    private func claimRaw(_ d: Deposit) async throws {
        _ = try await connected().claimDeposit(request: ClaimDepositRequest(txid: d.txid, vout: d.vout,
                                                                             maxFee: MaxFee(onchain: .networkRecommended(leewaySatPerVbyte: 5))))
    }

    // MARK: Values

    private static func sats(_ v: U128) -> UInt64 { UInt64(v.description) ?? 0 }

    private static func short(_ s: String) -> String {
        s.count > 24 ? String(s.prefix(12)) + "…" + String(s.suffix(8)) : s
    }

    /// A whole-sat amount in BTC for a BIP21 URI: 8 decimals, trailing zeros cut.
    static func btc(_ sats: UInt64) -> String {
        var s = String(format: "%d.%08d", sats / 100_000_000, sats % 100_000_000)
        while s.hasSuffix("0") { s.removeLast() }
        if s.hasSuffix(".") { s.removeLast() }
        return s
    }

    private static func activity(_ p: Payment) -> Activity {
        var a = Activity(id: p.id, incoming: p.paymentType == .receive, amount: sats(p.amount), fee: sats(p.fees),
                         status: p.status == .completed ? .done : p.status == .pending ? .pending : .failed,
                         date: Date(timeIntervalSince1970: TimeInterval(p.timestamp)), rail: "Spark")
        switch p.method {
        case .lightning: a.rail = "Lightning"
        case .deposit, .withdraw: a.rail = "On-chain"
        default: break
        }
        switch p.details {
        case .lightning(let description, let invoice, _, _, let lnurlPay, _, _, _)?:
            a.note = description; a.reference = lnurlPay?.lnAddress ?? invoice
            if let comment = lnurlPay?.comment, !comment.isEmpty { a.note = comment }
        case .spark(let invoice, _, _)?:
            a.note = invoice?.description
        case .withdraw(let txid)?: a.txid = txid; a.reference = txid
        case .deposit(let txid, _)?: a.txid = txid; a.reference = txid
        default: break
        }
        return a
    }
}
