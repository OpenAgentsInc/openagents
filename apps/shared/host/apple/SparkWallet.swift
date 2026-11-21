import CryptoKit
import Foundation
import BreezSdkSpark

/// Shared Apple Spark wallet host for macOS (`apps/openagents-desktop`) and
/// iOS (`apps/openagents-ios`).
///
/// This is the only Swift file that imports `BreezSdkSpark`. It performs
/// platform effects (Keychain, SDK networking, storage) and hands plain numbers
/// to `pay/wallet.coax` (`#authority`) to decide whether a send is allowed.
/// Words and keys never cross into Coax or logs.
///
/// Bitcoin-only rule: Bitcoin, Lightning, and Spark only; USD/cross-chain
/// features remain disabled.
public actor OASparkWallet {
    public enum PayCode {
        public static let idle: coax_t_Pay = 0
        public static let quoted: coax_t_Pay = 1
        public static let sending: coax_t_Pay = 2
        public static let succeeded: coax_t_Pay = 3
        public static let notSent: coax_t_Pay = 4
        public static let unknown: coax_t_Pay = 5
    }

    public enum VerdictCode {
        public static let send: coax_t_Verdict = 0
        public static let overCap: coax_t_Verdict = 1
        public static let overLimit: coax_t_Verdict = 2
        public static let expired: coax_t_Verdict = 3
        public static let badAmount: coax_t_Verdict = 4
        public static let busy: coax_t_Verdict = 5
    }

    public enum EventCode {
        public static let quote: coax_t_Event = 0
        public static let confirm: coax_t_Event = 1
        public static let cancel: coax_t_Event = 2
        public static let expire: coax_t_Event = 3
        public static let sent: coax_t_Event = 4
        public static let failed: coax_t_Event = 5
        public static let timedOut: coax_t_Event = 6
        public static let done: coax_t_Event = 7
    }

    public enum SparkNetwork: String, Sendable {
        case mainnet
        case regtest
        case signet

        var sdkNetwork: Network {
            switch self {
            case .mainnet: return .mainnet
            case .regtest: return .regtest
            case .signet: return .signet
            }
        }
    }

    public struct PaymentItem: Equatable, Sendable {
        public enum Status: String, Sendable { case done, pending, failed }
        public var id: String
        public var incoming: Bool
        public var amountSats: UInt64
        public var feeSats: UInt64
        public var status: Status
        public var timestamp: Int64
        public var rail: String
        public var note: String?
        public var reference: String?
        public var paymentHash: String? = nil
    }

    public enum Destination: Sendable {
        case bolt11(amountSats: UInt64?, note: String?, expiresAt: Int64)
        case lnurlPay(name: String, minSats: UInt64, maxSats: UInt64, commentAllowed: Int)
        case spark(address: String, amountSats: UInt64?)
        case bitcoin(address: String, amountSats: UInt64?)
        case unsupported(String)
    }

    public struct Quote: Sendable {
        public var amountSats: UInt64
        public var feeSats: UInt64
        public var expiresAt: Int64?
        public var recipient: String
        public var rail: String
        public var highFeeWarning: Bool
    }

    public struct ReceiveRequest: Sendable {
        public var paymentRequest: String
        public var feeSats: UInt64
    }

    public enum SendOutcome: Sendable {
        case sent(PaymentItem)
        case refused(reason: String, verdict: coax_t_Verdict)
        case failed(String)
        case waiting(String?)
    }

    public enum WalletError: LocalizedError, Sendable {
        case notConnected
        case noWordlist
        case noSeed
        case keychainUnavailable(OSStatus)
        case invalidSeed
        case unsupportedDestination(String)
        case noPreparedQuote
        case sdk(String)

        public var errorDescription: String? {
            switch self {
            case .notConnected: return "Spark wallet is not connected."
            case .noWordlist: return "BIP-39 English wordlist missing or checksum mismatch."
            case .noSeed: return "No wallet seed stored in Keychain."
            case .keychainUnavailable(let s): return "Keychain unavailable (OSStatus \(s))."
            case .invalidSeed: return "Stored wallet seed is invalid."
            case .unsupportedDestination(let s): return "Unsupported payment destination: \(s)"
            case .noPreparedQuote: return "No prepared send quote."
            case .sdk(let msg): return msg
            }
        }
    }

    private var sdk: BreezSdk?
    private var flow: oa_coax_t_Flow = metaverse_wallet_start()
    private var prepared: PrepareSendPaymentResponse?
    private var preparedLnurl: PrepareLnurlPayResponse?
    private var currentQuote: Quote?
    public private(set) var lastSendError: String?

    public init() {}

    /// Opens the wallet from the Keychain seed (`com.openagents.app.spark` / `seed-v1`)
    /// into `~/.openagents/wallet/spark-mainnet` (or an explicit storageDir).
    public func openFromKeychain(
        wordlistURL: URL? = nil,
        storageDir: URL? = nil,
        readOnly: Bool = false
    ) async throws {
        let words: [String]
        if let url = wordlistURL, let loaded = OABIP39.englishWords(url: url) {
            words = loaded
        } else if let loaded = OABIP39.englishWords() {
            words = loaded
        } else {
            throw WalletError.noWordlist
        }

        let entropy: Data
        switch OAWalletKeychain.readSeed() {
        case .found(let data): entropy = data
        case .notFound: throw WalletError.noSeed
        case .unreadable: throw WalletError.invalidSeed
        case .unavailable(let status): throw WalletError.keychainUnavailable(status)
        }
        guard let mnemonic = OABIP39.mnemonic(entropy: entropy, words: words) else {
            throw WalletError.invalidSeed
        }

        let dir: URL
        if let storageDir {
            dir = storageDir
        } else {
            dir = FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent(".openagents/wallet/spark-mainnet", isDirectory: true)
        }
        try await open(mnemonic: mnemonic, storageDir: dir, readOnly: readOnly)
    }

    /// Opens Spark with an explicit mnemonic and storage directory (used by
    /// `openFromKeychain`, settlement helpers, and isolated throwaway-seed tests).
    public func open(
        mnemonic: String,
        storageDir: URL,
        readOnly: Bool = false,
        network: SparkNetwork = .mainnet
    ) async throws {
        if sdk != nil {
            await disconnect()
        }
        try FileManager.default.createDirectory(at: storageDir, withIntermediateDirectories: true)
        let sdkNet = network.sdkNetwork
        var config = readOnly ? defaultServerConfig(network: sdkNet) : defaultConfig(network: sdkNet)
        config.apiKey = (network == .mainnet) ? oaBreezApiKey : nil
        config.realTimeSyncServerUrl = nil
        config.maxDepositClaimFee = readOnly ? nil : MaxFee(onchain: .networkRecommended(leewaySatPerVbyte: 1))

        let builder = SdkBuilder(config: config, seed: .mnemonic(mnemonic: mnemonic, passphrase: nil))
        await builder.withDefaultStorage(storageDir: storageDir.path)
        do {
            self.sdk = try await builder.build()
            self.flow = metaverse_wallet_start()
        } catch {
            throw WalletError.sdk(String(describing: error))
        }
    }

    public func identityPubkey() async throws -> String {
        let sdk = try requireSdk()
        do {
            let info = try await sdk.getInfo(request: GetInfoRequest(ensureSynced: false))
            return info.identityPubkey
        } catch {
            throw WalletError.sdk(String(describing: error))
        }
    }

    public func isConnected() -> Bool {
        sdk != nil
    }

    public func disconnect() async {
        if let sdk {
            try? await sdk.disconnect()
        }
        self.sdk = nil
        self.prepared = nil
        self.preparedLnurl = nil
        self.currentQuote = nil
        self.flow = metaverse_wallet_start()
    }

    private func requireSdk() throws -> BreezSdk {
        guard let sdk else { throw WalletError.notConnected }
        return sdk
    }

    public func syncAndBalance() async throws -> UInt64 {
        let sdk = try requireSdk()
        do {
            _ = try await sdk.syncWallet(request: SyncWalletRequest())
            let info = try await sdk.getInfo(request: GetInfoRequest(ensureSynced: false))
            return info.balanceSats
        } catch {
            throw WalletError.sdk(String(describing: error))
        }
    }

    public func balanceFast() async throws -> UInt64 {
        let sdk = try requireSdk()
        do {
            let info = try await sdk.getInfo(request: GetInfoRequest(ensureSynced: false))
            return info.balanceSats
        } catch {
            throw WalletError.sdk(String(describing: error))
        }
    }

    public func receiveLightning(amountSats: UInt64? = nil, description: String = "OpenAgents") async throws -> ReceiveRequest {
        let sdk = try requireSdk()
        do {
            let resp = try await sdk.receivePayment(request: ReceivePaymentRequest(
                paymentMethod: .bolt11Invoice(
                    description: description,
                    amountSats: amountSats,
                    expirySecs: 86_400,
                    paymentHash: nil,
                    receiverIdentityPublicKey: nil
                )
            ))
            return ReceiveRequest(paymentRequest: resp.paymentRequest, feeSats: Self.u64(resp.fee))
        } catch {
            throw WalletError.sdk(String(describing: error))
        }
    }

    public func receiveSpark() async throws -> ReceiveRequest {
        let sdk = try requireSdk()
        do {
            let resp = try await sdk.receivePayment(request: ReceivePaymentRequest(paymentMethod: .sparkAddress))
            return ReceiveRequest(paymentRequest: resp.paymentRequest, feeSats: Self.u64(resp.fee))
        } catch {
            throw WalletError.sdk(String(describing: error))
        }
    }

    public func receiveBitcoin() async throws -> ReceiveRequest {
        let sdk = try requireSdk()
        do {
            let resp = try await sdk.receivePayment(request: ReceivePaymentRequest(paymentMethod: .bitcoinAddress(newAddress: nil)))
            return ReceiveRequest(paymentRequest: resp.paymentRequest, feeSats: Self.u64(resp.fee))
        } catch {
            throw WalletError.sdk(String(describing: error))
        }
    }

    public func parse(_ input: String) async throws -> Destination {
        let sdk = try requireSdk()
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            let parsed = try await sdk.parse(input: trimmed)
            switch parsed {
            case .bolt11Invoice(let details):
                let amount = details.amountMsat.map { UInt64(metaverse_wallet_msat_up(Int64(clamping: $0))) }
                return .bolt11(
                    amountSats: amount,
                    note: details.description,
                    expiresAt: Int64(details.timestamp + details.expiry)
                )
            case .lnurlPay(let details):
                let low = UInt64(max(0, metaverse_wallet_msat_up(Int64(clamping: details.minSendable))))
                let high = UInt64(max(0, metaverse_wallet_msat_down(Int64(clamping: details.maxSendable))))
                return .lnurlPay(name: details.domain, minSats: low, maxSats: high, commentAllowed: Int(details.commentAllowed))
            case .lightningAddress(let details):
                let low = UInt64(max(0, metaverse_wallet_msat_up(Int64(clamping: details.payRequest.minSendable))))
                let high = UInt64(max(0, metaverse_wallet_msat_down(Int64(clamping: details.payRequest.maxSendable))))
                return .lnurlPay(name: details.address, minSats: low, maxSats: high, commentAllowed: Int(details.payRequest.commentAllowed))
            case .sparkAddress(let details):
                return .spark(address: details.address, amountSats: nil)
            case .sparkInvoice(let details):
                return .spark(address: details.identityPublicKey, amountSats: details.amount.map { Self.u64($0) })
            case .bitcoinAddress(let details):
                return .bitcoin(address: details.address, amountSats: nil)
            default:
                return .unsupported("Unsupported Bitcoin/Lightning payment format")
            }
        } catch {
            throw WalletError.sdk(String(describing: error))
        }
    }

    /// Prepares a quote for a BOLT11, Spark, Bitcoin, or Lightning Address destination.
    /// Advances `pay/wallet.coax` state machine to `quoted`.
    public func prepareSend(input: String, amountSats: UInt64? = nil, comment: String? = nil) async throws -> Quote {
        let sdk = try requireSdk()
        self.prepared = nil
        self.preparedLnurl = nil
        self.currentQuote = nil
        self.flow = metaverse_wallet_start()

        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            let parsed = try await sdk.parse(input: trimmed)
            switch parsed {
            case .lightningAddress(let details):
                guard let amountSats else { throw WalletError.unsupportedDestination("Amount required for Lightning Address") }
                let resp = try await sdk.prepareLnurlPay(request: PrepareLnurlPayRequest(
                    amount: U128(Int(amountSats)),
                    payRequest: details.payRequest,
                    comment: (comment?.isEmpty ?? true) ? nil : comment,
                    validateSuccessActionUrl: true
                ))
                self.preparedLnurl = resp
                let exp = Int64(resp.invoiceDetails.timestamp + resp.invoiceDetails.expiry)
                let q = Quote(
                    amountSats: resp.amountSats,
                    feeSats: resp.feeSats,
                    expiresAt: exp,
                    recipient: details.address,
                    rail: "Lightning",
                    highFeeWarning: metaverse_wallet_fee_high(Int64(clamping: resp.amountSats), Int64(clamping: resp.feeSats))
                )
                self.currentQuote = q
                self.flow = metaverse_wallet_step(self.flow, EventCode.quote)
                return q

            case .lnurlPay(let details):
                guard let amountSats else { throw WalletError.unsupportedDestination("Amount required for LNURL-pay") }
                let resp = try await sdk.prepareLnurlPay(request: PrepareLnurlPayRequest(
                    amount: U128(Int(amountSats)),
                    payRequest: details,
                    comment: (comment?.isEmpty ?? true) ? nil : comment,
                    validateSuccessActionUrl: true
                ))
                self.preparedLnurl = resp
                let exp = Int64(resp.invoiceDetails.timestamp + resp.invoiceDetails.expiry)
                let q = Quote(
                    amountSats: resp.amountSats,
                    feeSats: resp.feeSats,
                    expiresAt: exp,
                    recipient: details.domain,
                    rail: "Lightning",
                    highFeeWarning: metaverse_wallet_fee_high(Int64(clamping: resp.amountSats), Int64(clamping: resp.feeSats))
                )
                self.currentQuote = q
                self.flow = metaverse_wallet_step(self.flow, EventCode.quote)
                return q

            default:
                let resp = try await sdk.prepareSendPayment(request: PrepareSendPaymentRequest(
                    paymentRequest: .input(input: trimmed),
                    amount: amountSats.map { U128(Int($0)) },
                    feePolicy: .feesExcluded
                ))
                self.prepared = resp
                let sent = Self.u64(resp.amount)
                let q: Quote
                switch resp.paymentMethod {
                case .bolt11Invoice(let d, _, let fee):
                    q = Quote(
                        amountSats: sent,
                        feeSats: fee,
                        expiresAt: Int64(d.timestamp + d.expiry),
                        recipient: Self.short(d.invoice.bolt11),
                        rail: "Lightning",
                        highFeeWarning: metaverse_wallet_fee_high(Int64(clamping: sent), Int64(clamping: fee))
                    )
                case .sparkAddress(let addr, let fee, _):
                    let f = Self.u64(fee)
                    q = Quote(
                        amountSats: sent,
                        feeSats: f,
                        expiresAt: nil,
                        recipient: addr,
                        rail: "Spark",
                        highFeeWarning: metaverse_wallet_fee_high(Int64(clamping: sent), Int64(clamping: f))
                    )
                case .sparkInvoice(_, let fee, _):
                    let f = Self.u64(fee)
                    q = Quote(
                        amountSats: sent,
                        feeSats: f,
                        expiresAt: nil,
                        recipient: Self.short(trimmed),
                        rail: "Spark",
                        highFeeWarning: metaverse_wallet_fee_high(Int64(clamping: sent), Int64(clamping: f))
                    )
                case .bitcoinAddress(let addr, let feeQuote):
                    let f = feeQuote.speedMedium.userFeeSat + feeQuote.speedMedium.l1BroadcastFeeSat
                    q = Quote(
                        amountSats: sent,
                        feeSats: f,
                        expiresAt: Int64(feeQuote.expiresAt),
                        recipient: addr.address,
                        rail: "On-chain",
                        highFeeWarning: metaverse_wallet_fee_high(Int64(clamping: sent), Int64(clamping: f))
                    )
                default:
                    self.prepared = nil
                    throw WalletError.unsupportedDestination("Only Bitcoin, Lightning, and Spark payments are supported.")
                }
                self.currentQuote = q
                self.flow = metaverse_wallet_step(self.flow, EventCode.quote)
                return q
            }
        } catch let e as WalletError {
            throw e
        } catch {
            throw WalletError.sdk(String(describing: error))
        }
    }

    /// Executes the held quote ONLY if `pay/wallet.coax` (`metaverse_wallet_may_send`)
    /// returns `Verdict.send`. Never bypasses the Coax authority gate.
    public func sendPrepared(
        idempotencyKey: String,
        feeCapSats: UInt64,
        dailyRemainingSats: UInt64,
        nowUnix: Int64 = Int64(Date().timeIntervalSince1970)
    ) async -> SendOutcome {
        guard let quote = currentQuote else {
            return .refused(reason: "No prepared quote.", verdict: VerdictCode.busy)
        }
        let expires = quote.expiresAt ?? (nowUnix + 3600)
        let verdict = metaverse_wallet_may_send(
            flow.state,
            Int64(clamping: quote.amountSats),
            Int64(clamping: quote.feeSats),
            Int64(clamping: feeCapSats),
            Int64(clamping: dailyRemainingSats),
            nowUnix,
            expires
        )
        guard verdict == VerdictCode.send else {
            return .refused(reason: Self.verdictMessage(verdict), verdict: verdict)
        }

        flow = metaverse_wallet_step(flow, EventCode.confirm)
        self.lastSendError = nil
        let sdkKey = Self.sparkIdempotencyUUID(idempotencyKey)
        do {
            let sdk = try requireSdk()
            if let lnurl = preparedLnurl {
                let resp = try await sdk.lnurlPay(request: LnurlPayRequest(prepareResponse: lnurl, idempotencyKey: sdkKey))
                preparedLnurl = nil
                currentQuote = nil
                return finishSend(resp.payment)
            }
            guard let prepared else {
                flow = metaverse_wallet_step(flow, EventCode.failed)
                return .failed("No prepared quote.")
            }
            let options: SendPaymentOptions?
            switch prepared.paymentMethod {
            case .bolt11Invoice:
                options = .bolt11Invoice(preferSpark: false, completionTimeoutSecs: 30)
            case .bitcoinAddress:
                options = .bitcoinAddress(confirmationSpeed: .medium)
            default:
                options = nil
            }
            let resp = try await sdk.sendPayment(request: SendPaymentRequest(
                prepareResponse: prepared,
                options: options,
                idempotencyKey: sdkKey
            ))
            self.prepared = nil
            self.currentQuote = nil
            return finishSend(resp.payment)
        } catch let error as SdkError {
            self.lastSendError = String(describing: error)
            switch error {
            case .InsufficientFunds:
                flow = metaverse_wallet_step(flow, EventCode.failed)
                return .failed("Insufficient funds for this payment and its fee.")
            case .InvalidInput(let why):
                flow = metaverse_wallet_step(flow, EventCode.failed)
                return .failed(why)
            default:
                flow = metaverse_wallet_step(flow, EventCode.timedOut)
                return .waiting(nil)
            }
        } catch {
            self.lastSendError = String(describing: error)
            flow = metaverse_wallet_step(flow, EventCode.timedOut)
            return .waiting(nil)
        }
    }

    private func finishSend(_ p: Payment) -> SendOutcome {
        let item = Self.paymentItem(p)
        switch p.status {
        case .completed:
            flow = metaverse_wallet_step(flow, EventCode.sent)
            flow = metaverse_wallet_step(flow, EventCode.done)
            return .sent(item)
        case .failed:
            flow = metaverse_wallet_step(flow, EventCode.failed)
            return .failed("Payment failed before settlement.")
        case .pending:
            flow = metaverse_wallet_step(flow, EventCode.timedOut)
            return .waiting(item.id)
        }
    }

    public func listPayments(limit: UInt32 = 50) async throws -> [PaymentItem] {
        let sdk = try requireSdk()
        do {
            let resp = try await sdk.listPayments(request: ListPaymentsRequest(limit: limit))
            return resp.payments.map(Self.paymentItem)
        } catch {
            throw WalletError.sdk(String(describing: error))
        }
    }

    public static func verdictMessage(_ v: coax_t_Verdict) -> String {
        switch v {
        case VerdictCode.send: return "Allowed"
        case VerdictCode.overCap: return "Fee exceeds the configured fee cap."
        case VerdictCode.overLimit: return "Payment plus fee exceeds the remaining spend limit."
        case VerdictCode.expired: return "Payment quote has expired."
        case VerdictCode.badAmount: return "Invalid payment amount or fee."
        default: return "Wallet is busy or has no active quote."
        }
    }

    public static func sparkIdempotencyUUID(_ raw: String) -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if let existing = UUID(uuidString: trimmed) {
            return existing.uuidString.lowercased()
        }
        var bytes = Array(SHA256.hash(data: Data("openagents.spark-idempotency.v1:\(trimmed)".utf8)).prefix(16))
        bytes[6] = (bytes[6] & 0x0F) | 0x50
        bytes[8] = (bytes[8] & 0x3F) | 0x80
        let hex = bytes.map { String(format: "%02x", $0) }.joined()
        let i0 = hex.startIndex
        let i1 = hex.index(i0, offsetBy: 8)
        let i2 = hex.index(i1, offsetBy: 4)
        let i3 = hex.index(i2, offsetBy: 4)
        let i4 = hex.index(i3, offsetBy: 4)
        return "\(hex[i0..<i1])-\(hex[i1..<i2])-\(hex[i2..<i3])-\(hex[i3..<i4])-\(hex[i4...])"
    }

    private static func u64(_ v: U128) -> UInt64 {
        UInt64(v.description) ?? 0
    }

    private static func short(_ s: String) -> String {
        s.count > 24 ? String(s.prefix(12)) + "…" + String(s.suffix(8)) : s
    }

    private static func paymentItem(_ p: Payment) -> PaymentItem {
        var rail = "Spark"
        switch p.method {
        case .lightning: rail = "Lightning"
        case .deposit, .withdraw: rail = "On-chain"
        default: break
        }
        var note: String?
        var ref: String?
        var paymentHash: String?
        switch p.details {
        case .lightning(let desc, let invoice, _, let htlcDetails, let lnurlPay, _, _, _)?:
            note = desc
            ref = lnurlPay?.lnAddress ?? invoice
            paymentHash = htlcDetails.paymentHash
            if let c = lnurlPay?.comment, !c.isEmpty { note = c }
        case .spark(let inv, _, _)?:
            note = inv?.description
        case .withdraw(let txid)?: ref = txid
        case .deposit(let txid, _)?: ref = txid
        default: break
        }
        let status: PaymentItem.Status = p.status == .completed ? .done : (p.status == .pending ? .pending : .failed)
        return PaymentItem(
            id: p.id,
            incoming: p.paymentType == .receive,
            amountSats: u64(p.amount),
            feeSats: u64(p.fees),
            status: status,
            timestamp: Int64(p.timestamp),
            rail: rail,
            note: note,
            reference: ref,
            paymentHash: paymentHash
        )
    }
}
