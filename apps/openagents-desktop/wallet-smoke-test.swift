import Foundation

@main
struct WalletSmokeTest {
    static func main() async {
        guard CommandLine.arguments.count >= 2 else {
            fputs("usage: wallet-smoke-test <path-to-bip39-english.txt>\n", stderr)
            exit(2)
        }
        let wordlistURL = URL(fileURLWithPath: CommandLine.arguments[1])

        // 1. Verify pinned BIP-39 wordlist & official 128-bit zero vector.
        guard let words = OABIP39.englishWords(url: wordlistURL) else {
            fputs("FAIL: bip39-english.txt failed SHA-256 verification\n", stderr)
            exit(1)
        }
        let zeroVector = OABIP39.mnemonic(entropy: Data(repeating: 0, count: 16), words: words)
        let expectedZero = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        guard zeroVector == expectedZero else {
            fputs("FAIL: BIP-39 zero vector mismatch\n", stderr)
            exit(1)
        }
        print("ok  BIP-39 wordlist SHA-256 + official zero vector")

        // 2. Verify pay/wallet.coax & pay/wallet-backup.coax #authority gates via C ABI.
        precondition(metaverse_wallet_sat_to_msat(1000) == 1_000_000)
        precondition(metaverse_wallet_sat_to_msat(-1) == -1)
        precondition(metaverse_wallet_amount_ok(500, 100, 1000, 600))
        precondition(!metaverse_wallet_amount_ok(700, 100, 1000, 600))

        var flow = metaverse_wallet_start()
        precondition(flow.state == OASparkWallet.PayCode.idle)
        flow = metaverse_wallet_step(flow, OASparkWallet.EventCode.quote)
        precondition(flow.state == OASparkWallet.PayCode.quoted)

        // Valid quote -> send
        precondition(metaverse_wallet_may_send(flow.state, 1000, 10, 50, 5000, 100, 200) == OASparkWallet.VerdictCode.send)
        // Expired quote -> expired
        precondition(metaverse_wallet_may_send(flow.state, 1000, 10, 50, 5000, 200, 200) == OASparkWallet.VerdictCode.expired)
        // Fee over cap -> overCap
        precondition(metaverse_wallet_may_send(flow.state, 1000, 51, 50, 5000, 100, 200) == OASparkWallet.VerdictCode.overCap)
        // Amount + fee over daily limit -> overLimit
        precondition(metaverse_wallet_may_send(flow.state, 1000, 10, 50, 1005, 100, 200) == OASparkWallet.VerdictCode.overLimit)
        // Backup card & daily limit check
        precondition(metaverse_backup_card(1, true, false, false, false, false) >= 0)
        print("ok  pay/wallet.coax + pay/wallet-backup.coax #authority C ABI gates")

        // 3. Generate throwaway 12-word seed in isolated temp dir (NEVER touches Keychain or owner funds).
        guard let entropy = OABIP39.generateEntropy16(),
              let mnemonic = OABIP39.mnemonic(entropy: entropy, words: words) else {
            fputs("FAIL: could not generate throwaway entropy\n", stderr)
            exit(1)
        }
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("oa-spark-smoke-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let wallet = OASparkWallet()
        do {
            try await wallet.open(mnemonic: mnemonic, storageDir: tempDir, readOnly: false)
            print("ok  Spark mainnet connected (throwaway seed, isolated temp storage)")

            let bal = try await wallet.syncAndBalance()
            precondition(bal == 0, "fresh throwaway wallet should have 0 balance")
            print("ok  syncAndBalance() -> \(bal) sats")

            let payments = try await wallet.listPayments(limit: 10)
            precondition(payments.isEmpty, "fresh throwaway wallet should have 0 payments")
            print("ok  listPayments() -> \(payments.count) payments")

            let spark = try await wallet.receiveSpark()
            precondition(spark.paymentRequest.hasPrefix("sp1") || spark.paymentRequest.hasPrefix("spark1"),
                         "unexpected spark address prefix: \(spark.paymentRequest.prefix(8))")
            print("ok  receiveSpark() -> \(spark.paymentRequest.prefix(16))… (len \(spark.paymentRequest.count))")

            let inv = try await wallet.receiveLightning(amountSats: 100, description: "OpenAgents macOS smoke test")
            precondition(inv.paymentRequest.lowercased().hasPrefix("lnbc"),
                         "unexpected BOLT11 prefix: \(inv.paymentRequest.prefix(8))")
            print("ok  receiveLightning(100 sats) -> \(inv.paymentRequest.prefix(16))… (len \(inv.paymentRequest.count))")

            let parsedInv = try await wallet.parse(inv.paymentRequest)
            switch parsedInv {
            case .bolt11(let amt, _, _):
                precondition(amt == 100, "expected parsed amount 100 sats, got \(String(describing: amt))")
                print("ok  parse(BOLT11) -> 100 sats verified")
            default:
                fputs("FAIL: expected .bolt11 from parse()\n", stderr)
                exit(1)
            }

            let parsedSpark = try await wallet.parse(spark.paymentRequest)
            switch parsedSpark {
            case .spark(let addr, _):
                precondition(addr == spark.paymentRequest)
                print("ok  parse(Spark) -> address verified")
            default:
                fputs("FAIL: expected .spark from parse()\n", stderr)
                exit(1)
            }

            await wallet.disconnect()
            print("ok  disconnect() clean")
            print("PASS: Desktop Wallet Phase 1 macOS Spark smoke test")
        } catch {
            await wallet.disconnect()
            fputs("FAIL: Spark smoke test error: \(error)\n", stderr)
            exit(1)
        }
    }
}
