// Firm reason: 6 (Swift-only Breez Spark SDK settlement bridge over pay/wallet.coax and OASparkWallet; no C API for BreezSdkSpark).
import CryptoKit
import Foundation

/// Non-interactive Breez Spark settlement bridge for Executor W4.3 (#13922).
///
/// Security invariants:
/// - Reads mnemonic ONLY from `OA_SPARK_MNEMONIC` (or generates an ephemeral
///   in-memory throwaway seed on non-mainnet test networks when `--ephemeral`
///   is passed). NEVER touches macOS Keychain and NEVER logs seeds/words.
/// - Refuses mainnet spends unless BOTH `--arm-mainnet` AND
///   `OA_EXECUTOR_MAINNET_ARMED=1` are explicitly set by the owner.
/// - Every spend routes strictly through `OASparkWallet.prepareSend` +
///   `sendPrepared` -> `pay/wallet.coax` (`#authority` `metaverse_wallet_may_send`).
/// - Outputs ONLY sanitized settlement JSON (`settlement_commitment` =
///   `SHA-256("openagents.pylon-settlement.v1:<NET>:<PAYMENT_HASH_OR_ID>")`,
///   `real_bitcoin_moved: bool`). NEVER emits preimages, private keys, or raw
///   invoices in settlement receipts.
@main
struct SparkSettleHelper {
    static func main() async {
        let args = Array(CommandLine.arguments.dropFirst())
        guard let verb = args.first, ["status", "receive", "pay"].contains(verb) else {
            emitJSON([
                "ok": false,
                "real_bitcoin_moved": false,
                "error": "usage: spark-settle-helper <status|receive|pay> [--network regtest|signet|bitcoin] [--storage-dir DIR] [--wordlist PATH] [--ephemeral] [--kind spark|lightning|bitcoin] [--dest ADDR] [--amount-sats N] [--idempotency-key KEY] [--fee-cap-sats N] [--daily-remaining-sats N] [--arm-mainnet]"
            ])
            exit(2)
        }

        let netArg = flagValue(args, "--network") ?? "regtest"
        guard let (sparkNet, wireNet) = parseNetwork(netArg) else {
            emitJSON(["ok": false, "real_bitcoin_moved": false, "error": "unsupported network (use regtest, signet, or bitcoin)"])
            exit(2)
        }
        let ephemeral = args.contains("--ephemeral")
        let armMainnet = args.contains("--arm-mainnet")
        let storagePath = flagValue(args, "--storage-dir")
            ?? FileManager.default.temporaryDirectory
                .appendingPathComponent("oa-spark-settle-\(wireNet)-\(UUID().uuidString)", isDirectory: true).path
        let wordlistPath = flagValue(args, "--wordlist")

        if sparkNet == .mainnet && ephemeral {
            emitJSON(["ok": false, "real_bitcoin_moved": false, "error": "--ephemeral is refused on mainnet"])
            exit(2)
        }

        if verb == "pay" && sparkNet == .mainnet {
            let envArmed = ProcessInfo.processInfo.environment["OA_EXECUTOR_MAINNET_ARMED"] == "1"
            if !armMainnet || !envArmed {
                emitJSON([
                    "ok": false,
                    "real_bitcoin_moved": false,
                    "status": "held_for_owner",
                    "network": wireNet,
                    "error": "mainnet auto-settlement is not armed (requires explicit owner sign-off in NEEDS_OWNER.md, --arm-mainnet, and OA_EXECUTOR_MAINNET_ARMED=1)"
                ])
                exit(3)
            }
        }

        let mnemonic: String
        if let envMnemonic = ProcessInfo.processInfo.environment["OA_SPARK_MNEMONIC"]?.trimmingCharacters(in: .whitespacesAndNewlines),
           !envMnemonic.isEmpty {
            mnemonic = envMnemonic
        } else if ephemeral {
            let wordlistURL = wordlistPath.map { URL(fileURLWithPath: $0) }
            guard let words = (wordlistURL.flatMap { OABIP39.englishWords(url: $0) } ?? OABIP39.englishWords()),
                  let entropy = OABIP39.generateEntropy16(),
                  let generated = OABIP39.mnemonic(entropy: entropy, words: words) else {
                emitJSON(["ok": false, "real_bitcoin_moved": false, "error": "failed to generate ephemeral BIP-39 test seed (check --wordlist)"])
                exit(1)
            }
            mnemonic = generated
        } else {
            emitJSON([
                "ok": false,
                "real_bitcoin_moved": false,
                "status": "no_wallet_seed",
                "network": wireNet,
                "error": "OA_SPARK_MNEMONIC is not set (or pass --ephemeral on regtest/signet)"
            ])
            exit(3)
        }

        let storageURL = URL(fileURLWithPath: storagePath, isDirectory: true)
        let wallet = OASparkWallet()

        do {
            try await wallet.open(mnemonic: mnemonic, storageDir: storageURL, readOnly: false, network: sparkNet)
        } catch {
            emitJSON([
                "ok": false,
                "real_bitcoin_moved": false,
                "status": "connect_failed",
                "network": wireNet,
                "error": error.localizedDescription
            ])
            exit(1)
        }

        switch verb {
        case "status":
            do {
                let bal = try await wallet.syncAndBalance()
                let idPub = try await wallet.identityPubkey()
                let sp = try await wallet.receiveSpark()
                await wallet.disconnect()
                emitJSON([
                    "ok": true,
                    "status": "ready",
                    "network": wireNet,
                    "balance_sats": Int64(clamping: bal),
                    "spark_address": sp.paymentRequest,
                    "identity_pubkey": idPub,
                    "real_bitcoin_moved": false
                ])
                exit(0)
            } catch {
                await wallet.disconnect()
                emitJSON([
                    "ok": false,
                    "real_bitcoin_moved": false,
                    "status": "sync_failed",
                    "network": wireNet,
                    "error": error.localizedDescription
                ])
                exit(1)
            }

        case "receive":
            let kind = (flagValue(args, "--kind") ?? "spark").lowercased()
            let amt = flagValue(args, "--amount-sats").flatMap { UInt64($0) }
            let note = flagValue(args, "--note") ?? "OpenAgents Executor"
            do {
                _ = try await wallet.syncAndBalance()
                let req: OASparkWallet.ReceiveRequest
                switch kind {
                case "lightning": req = try await wallet.receiveLightning(amountSats: amt, description: note)
                case "bitcoin": req = try await wallet.receiveBitcoin()
                default: req = try await wallet.receiveSpark()
                }
                await wallet.disconnect()
                emitJSON([
                    "ok": true,
                    "status": "ready",
                    "network": wireNet,
                    "kind": kind,
                    "payment_request": req.paymentRequest,
                    "fee_sats": Int64(clamping: req.feeSats),
                    "real_bitcoin_moved": false
                ])
                exit(0)
            } catch {
                await wallet.disconnect()
                emitJSON([
                    "ok": false,
                    "real_bitcoin_moved": false,
                    "status": "receive_failed",
                    "network": wireNet,
                    "error": error.localizedDescription
                ])
                exit(1)
            }

        case "pay":
            guard let dest = flagValue(args, "--dest"), !dest.isEmpty,
                  let amtStr = flagValue(args, "--amount-sats"), let amt = UInt64(amtStr), amt > 0 else {
                await wallet.disconnect()
                emitJSON(["ok": false, "real_bitcoin_moved": false, "error": "--dest and positive --amount-sats required"])
                exit(2)
            }
            let idemKey = flagValue(args, "--idempotency-key") ?? UUID().uuidString
            let feeCap = flagValue(args, "--fee-cap-sats").flatMap { UInt64($0) } ?? 500
            let dailyRem = flagValue(args, "--daily-remaining-sats").flatMap { UInt64($0) } ?? (amt &+ feeCap)

            do {
                let bal = try await wallet.syncAndBalance()
                if bal < amt {
                    await wallet.disconnect()
                    emitJSON([
                        "ok": false,
                        "real_bitcoin_moved": false,
                        "status": "insufficient_funds",
                        "network": wireNet,
                        "balance_sats": Int64(clamping: bal),
                        "required_sats": Int64(clamping: amt),
                        "error": "wallet balance (\(bal) sats) is below required payout (\(amt) sats)"
                    ])
                    exit(4)
                }

                let quote = try await wallet.prepareSend(input: dest, amountSats: amt)
                let outcome = await wallet.sendPrepared(
                    idempotencyKey: idemKey,
                    feeCapSats: feeCap,
                    dailyRemainingSats: dailyRem
                )
                let lastErr = await wallet.lastSendError ?? ""
                await wallet.disconnect()

                switch outcome {
                case .sent(let item):
                    let moved = (item.status == .done && item.amountSats > 0 && !item.id.isEmpty)
                    let hashSource = (item.paymentHash?.isEmpty == false) ? item.paymentHash! : item.id
                    let commitment = moved ? settlementCommitment(network: wireNet, paymentRef: hashSource) : ""
                    emitJSON([
                        "ok": moved,
                        "real_bitcoin_moved": moved,
                        "status": moved ? "settled" : "not_settled",
                        "network": wireNet,
                        "rail": item.rail,
                        "amount_sats": Int64(clamping: item.amountSats),
                        "fee_sats": Int64(clamping: item.feeSats),
                        "settled_at": moved ? max(item.timestamp, Int64(Date().timeIntervalSince1970)) : 0,
                        "settlement_commitment": commitment
                    ])
                    exit(moved ? 0 : 1)

                case .refused(let reason, _):
                    emitJSON([
                        "ok": false,
                        "real_bitcoin_moved": false,
                        "status": "refused_by_policy",
                        "network": wireNet,
                        "error": reason
                    ])
                    exit(3)

                case .failed(let reason):
                    emitJSON([
                        "ok": false,
                        "real_bitcoin_moved": false,
                        "status": "failed",
                        "network": wireNet,
                        "error": reason
                    ])
                    exit(1)

                case .waiting(let id):
                    emitJSON([
                        "ok": false,
                        "real_bitcoin_moved": false,
                        "status": "pending",
                        "network": wireNet,
                        "pending_id": id ?? "",
                        "error": lastErr
                    ])
                    exit(5)
                }
            } catch {
                await wallet.disconnect()
                emitJSON([
                    "ok": false,
                    "real_bitcoin_moved": false,
                    "status": "pay_failed",
                    "network": wireNet,
                    "error": error.localizedDescription
                ])
                exit(1)
            }

        default:
            await wallet.disconnect()
            exit(2)
        }
    }

    private static func parseNetwork(_ raw: String) -> (OASparkWallet.SparkNetwork, String)? {
        switch raw.lowercased() {
        case "regtest": return (.regtest, "regtest")
        case "signet": return (.signet, "signet")
        case "bitcoin", "mainnet": return (.mainnet, "bitcoin")
        default: return nil
        }
    }

    private static func flagValue(_ args: [String], _ flag: String) -> String? {
        guard let idx = args.firstIndex(of: flag), idx + 1 < args.count else { return nil }
        return args[idx + 1]
    }

    /// Domain-separated settlement commitment per NIP-PYLON §Service receipt:
    /// SHA-256("openagents.pylon-settlement.v1:<NETWORK>:<PAYMENT_HASH_OR_ID>")
    private static func settlementCommitment(network: String, paymentRef: String) -> String {
        let preimage = "openagents.pylon-settlement.v1:\(network):\(paymentRef)"
        let digest = SHA256.hash(data: Data(preimage.utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    private static func emitJSON(_ obj: [String: Any]) {
        if let data = try? JSONSerialization.data(withJSONObject: obj, options: [.sortedKeys]),
           let text = String(data: data, encoding: .utf8) {
            print(text)
        }
    }
}
