// Firm reason: 6 (AppKit loopback HTTP wallet RPC and interactive spend-approval bridge over pay/wallet-rpc.coax).
import AppKit

/// Local loopback wallet RPC (`openagents.desktop.wallet-rpc.v1`, #13901).
/// Allows `openagents wallet` CLI and agent workers to query balance, create
/// receive invoices, prepare quotes, and request Bitcoin/Lightning/x402 spends.
///
/// Security invariants:
/// - Seeds and private keys NEVER leave `OASparkWallet` / macOS Keychain.
/// - Every spend request is gated by `pay/wallet-rpc.coax` (`#authority`) AND
///   a non-bypassable interactive user approval dialog in `OpenAgents.app`.
/// - Every approved or refused spend appends a structured receipt to
///   `~/.openagents/wallet/receipts.jsonl` for ATIF-v1.8 trace attachment.
extension CoderWindow {
    private static let wrpcSchema = "openagents.desktop.wallet-rpc.v1"

    private static var receiptsURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/wallet/receipts.jsonl")
    }

    private func appendWalletReceipt(_ receipt: [String: Any]) {
        let url = Self.receiptsURL
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        guard let data = try? JSONSerialization.data(withJSONObject: receipt, options: [.sortedKeys]),
              var line = String(data: data, encoding: .utf8) else { return }
        line += "\n"
        if FileManager.default.fileExists(atPath: url.path),
           let fh = try? FileHandle(forWritingTo: url) {
            defer { try? fh.close() }
            _ = try? fh.seekToEnd()
            try? fh.write(contentsOf: Data(line.utf8))
        } else {
            try? Data(line.utf8).write(to: url, options: .atomic)
        }
    }

    func handleWalletRPC(_ request: ForemanRequest, done: @escaping (ForemanResponse) -> Void) {
        let w = DesktopWalletController.shared
        switch (request.method, request.path) {
        case ("GET", "/v1/wallet/status"):
            done(ForemanResponse(status: 200, body: [
                "schema_version": Self.wrpcSchema,
                "ok": true,
                "status": w.ready ? "ready" : "not_ready",
                "detail": w.statusText,
                "balance_sats": Int64(clamping: w.balanceSats),
                "spark_address": w.sparkAddress,
                "daily_limit_sats": Int64(clamping: w.dailyLimitSats),
                "daily_spent_sats": Int64(clamping: w.dailySpentSats),
                "daily_remaining_sats": Int64(clamping: w.dailyRemainingSats)
            ]))

        case ("POST", "/v1/wallet/receive"):
            guard w.ready else {
                done(ForemanResponse(status: 503, body: [
                    "schema_version": Self.wrpcSchema, "ok": false, "status": "not_ready",
                    "error": "Spark wallet is not connected. Open Settings > Wallet to connect."
                ]))
                return
            }
            let kind = (request.payload["kind"] as? String)?.lowercased() ?? "lightning"
            let amountSats = (request.payload["amount_sats"] as? NSNumber).map { UInt64(max(0, $0.int64Value)) }
            let desc = (request.payload["description"] as? String) ?? "OpenAgents"
            Task {
                do {
                    let req: OASparkWallet.ReceiveRequest
                    switch kind {
                    case "spark": req = try await w.spark.receiveSpark()
                    case "bitcoin", "onchain": req = try await w.spark.receiveBitcoin()
                    default: req = try await w.spark.receiveLightning(amountSats: amountSats, description: desc)
                    }
                    await MainActor.run {
                        done(ForemanResponse(status: 200, body: [
                            "schema_version": Self.wrpcSchema,
                            "ok": true,
                            "status": "ready",
                            "kind": kind,
                            "payment_request": req.paymentRequest,
                            "fee_sats": Int64(clamping: req.feeSats)
                        ]))
                    }
                } catch {
                    await MainActor.run {
                        done(ForemanResponse(status: 400, body: [
                            "schema_version": Self.wrpcSchema, "ok": false, "status": "failed",
                            "error": error.localizedDescription
                        ]))
                    }
                }
            }

        case ("POST", "/v1/wallet/quote"):
            guard w.ready else {
                done(ForemanResponse(status: 503, body: [
                    "schema_version": Self.wrpcSchema, "ok": false, "status": "not_ready",
                    "error": "Spark wallet is not connected."
                ]))
                return
            }
            guard let dest = (request.payload["destination"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines),
                  !dest.isEmpty else {
                done(ForemanResponse(status: 400, body: [
                    "schema_version": Self.wrpcSchema, "ok": false, "status": "failed",
                    "error": "destination is required"
                ]))
                return
            }
            let amountSats = (request.payload["amount_sats"] as? NSNumber).map { UInt64(max(0, $0.int64Value)) }
            let comment = request.payload["comment"] as? String
            Task {
                do {
                    let q = try await w.spark.prepareSend(input: dest, amountSats: amountSats, comment: comment)
                    await MainActor.run {
                        done(ForemanResponse(status: 200, body: [
                            "schema_version": Self.wrpcSchema,
                            "ok": true,
                            "status": "quoted",
                            "rail": q.rail,
                            "recipient": q.recipient,
                            "amount_sats": Int64(clamping: q.amountSats),
                            "fee_sats": Int64(clamping: q.feeSats),
                            "expires_at": q.expiresAt ?? 0,
                            "high_fee_warning": q.highFeeWarning
                        ]))
                    }
                } catch {
                    await MainActor.run {
                        done(ForemanResponse(status: 400, body: [
                            "schema_version": Self.wrpcSchema, "ok": false, "status": "failed",
                            "error": error.localizedDescription
                        ]))
                    }
                }
            }

        case ("POST", "/v1/wallet/pay"):
            guard w.ready else {
                done(ForemanResponse(status: 503, body: [
                    "schema_version": Self.wrpcSchema, "ok": false, "status": "not_ready",
                    "error": "Spark wallet is not connected."
                ]))
                return
            }
            guard let dest = (request.payload["destination"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines),
                  !dest.isEmpty else {
                done(ForemanResponse(status: 400, body: [
                    "schema_version": Self.wrpcSchema, "ok": false, "status": "failed",
                    "error": "destination is required"
                ]))
                return
            }
            let amountSats = (request.payload["amount_sats"] as? NSNumber).map { UInt64(max(0, $0.int64Value)) }
            let comment = request.payload["comment"] as? String
            let reason = (request.payload["reason"] as? String) ?? "Agent/CLI payment request"
            let x402 = request.payload["x402_challenge"] as? String
            Task {
                do {
                    let quote = try await w.spark.prepareSend(input: dest, amountSats: amountSats, comment: comment)
                    await MainActor.run {
                        self.presentRPCSpendApproval(quote: quote, reason: reason, x402: x402, principal: request.principal.rawValue, done: done)
                    }
                } catch {
                    await MainActor.run {
                        done(ForemanResponse(status: 400, body: [
                            "schema_version": Self.wrpcSchema, "ok": false, "status": "failed",
                            "error": error.localizedDescription
                        ]))
                    }
                }
            }

        default:
            done(ForemanResponse(status: 404, body: [
                "schema_version": Self.wrpcSchema, "ok": false, "status": "failed",
                "error": "unknown wallet route"
            ]))
        }
    }

    /// Runs `pay/wallet-rpc.coax` (`#authority`) pre-check, then shows a non-bypassable
    /// interactive approval dialog before any satoshis can leave the wallet.
    private func presentRPCSpendApproval(
        quote: OASparkWallet.Quote,
        reason: String,
        x402: String?,
        principal: String,
        done: @escaping (ForemanResponse) -> Void
    ) {
        let w = DesktopWalletController.shared
        let now = Int64(Date().timeIntervalSince1970)
        let expires = quote.expiresAt ?? (now + 3600)
        let feeCap = max(quote.feeSats, 5_000)

        // Pre-check policy with approved=true to catch expired/over-limit/bad-amount before prompting.
        let preVerdict = wallet_rpc_spend_admit(
            w.ready, true,
            Int64(clamping: quote.amountSats),
            Int64(clamping: quote.feeSats),
            Int64(clamping: feeCap),
            Int64(clamping: w.dailyRemainingSats),
            now, expires
        )
        if preVerdict != 0 {
            let receipt: [String: Any] = [
                "schema_version": Self.wrpcSchema,
                "timestamp": now,
                "rail": quote.rail,
                "recipient": quote.recipient,
                "amount_sats": Int64(clamping: quote.amountSats),
                "fee_sats": Int64(clamping: quote.feeSats),
                "outcome": "refused_by_policy",
                "reason": reason,
                "principal": principal
            ]
            if wallet_rpc_writes_receipt(preVerdict) { appendWalletReceipt(receipt) }
            let httpStatus = Int(wallet_rpc_http_status(preVerdict))
            done(ForemanResponse(status: httpStatus, body: [
                "schema_version": Self.wrpcSchema, "ok": false, "status": "refused_by_policy",
                "receipt": receipt
            ]))
            return
        }

        let total = quote.amountSats &+ quote.feeSats
        let x402Line = x402.map { "\nx402 challenge: \($0)" } ?? ""
        let body = "Caller: \(principal)\nReason: \(reason)\(x402Line)\n\nRail: \(quote.rail)\nRecipient: \(quote.recipient)\nAmount: \(DesktopWalletController.formatSats(quote.amountSats))\nFee: \(DesktopWalletController.formatSats(quote.feeSats))\nTotal: \(DesktopWalletController.formatSats(total))\nDaily remaining: \(DesktopWalletController.formatSats(w.dailyRemainingSats))"
        var d = CoderDialog(size: .md, title: "Approve Agent Bitcoin Payment", body: body)
        d.buttons = [.init(title: "Deny"), .init(title: "Approve & Send \(DesktopWalletController.formatSats(total))", kind: .primary)]
        d.cancel = 0; d.confirm = 0  // default to Deny on accidental Return
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            let approved = (answer == 1)
            let checkNow = Int64(Date().timeIntervalSince1970)
            let verdict = wallet_rpc_spend_admit(
                w.ready, approved,
                Int64(clamping: quote.amountSats),
                Int64(clamping: quote.feeSats),
                Int64(clamping: feeCap),
                Int64(clamping: w.dailyRemainingSats),
                checkNow, expires
            )
            guard verdict == 0 else {
                let outcome = approved ? "refused_by_policy" : "refused_by_user"
                let receipt: [String: Any] = [
                    "schema_version": Self.wrpcSchema,
                    "timestamp": checkNow,
                    "rail": quote.rail,
                    "recipient": quote.recipient,
                    "amount_sats": Int64(clamping: quote.amountSats),
                    "fee_sats": Int64(clamping: quote.feeSats),
                    "outcome": outcome,
                    "reason": reason,
                    "principal": principal
                ]
                if wallet_rpc_writes_receipt(verdict) { self.appendWalletReceipt(receipt) }
                done(ForemanResponse(status: Int(wallet_rpc_http_status(verdict)), body: [
                    "schema_version": Self.wrpcSchema, "ok": false, "status": outcome,
                    "receipt": receipt
                ]))
                self.refocus()
                return
            }

            let key = UUID().uuidString
            Task {
                let sendOutcome = await w.spark.sendPrepared(
                    idempotencyKey: key,
                    feeCapSats: feeCap,
                    dailyRemainingSats: w.dailyRemainingSats
                )
                await MainActor.run {
                    switch sendOutcome {
                    case .sent(let item):
                        w.recordSpend(item.amountSats &+ item.feeSats)
                        let receipt: [String: Any] = [
                            "schema_version": Self.wrpcSchema,
                            "timestamp": item.timestamp,
                            "id": item.id,
                            "rail": item.rail,
                            "recipient": quote.recipient,
                            "amount_sats": Int64(clamping: item.amountSats),
                            "fee_sats": Int64(clamping: item.feeSats),
                            "outcome": "sent",
                            "reason": reason,
                            "principal": principal
                        ]
                        self.appendWalletReceipt(receipt)
                        self.walletRefresh()
                        done(ForemanResponse(status: 200, body: [
                            "schema_version": Self.wrpcSchema, "ok": true, "status": "sent",
                            "receipt": receipt
                        ]))
                    case .refused(let msg, _):
                        let receipt: [String: Any] = [
                            "schema_version": Self.wrpcSchema,
                            "timestamp": Int64(Date().timeIntervalSince1970),
                            "rail": quote.rail,
                            "recipient": quote.recipient,
                            "amount_sats": Int64(clamping: quote.amountSats),
                            "fee_sats": Int64(clamping: quote.feeSats),
                            "outcome": "refused_by_policy",
                            "reason": msg,
                            "principal": principal
                        ]
                        self.appendWalletReceipt(receipt)
                        done(ForemanResponse(status: 403, body: [
                            "schema_version": Self.wrpcSchema, "ok": false, "status": "refused_by_policy",
                            "error": msg, "receipt": receipt
                        ]))
                    case .failed(let msg):
                        done(ForemanResponse(status: 400, body: [
                            "schema_version": Self.wrpcSchema, "ok": false, "status": "failed",
                            "error": msg
                        ]))
                    case .waiting(let id):
                        done(ForemanResponse(status: 202, body: [
                            "schema_version": Self.wrpcSchema, "ok": true, "status": "pending",
                            "id": id ?? ""
                        ]))
                    }
                    self.refocus()
                }
            }
        }
    }
}
