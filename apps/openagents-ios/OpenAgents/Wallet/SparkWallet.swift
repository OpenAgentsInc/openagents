import Foundation
import BreezSdkSpark

/// The person's Spark wallet on Bitcoin mainnet, opened from the existing
/// Keychain seed the same way the earlier app opened it: the entropy's
/// English BIP-39 words, no passphrase, Breez's default mainnet account.
/// It only reads the balance; it never makes, replaces or sends anything.
actor SparkWallet {
    enum State: Equatable {
        case opening
        case balance(UInt64)
        case noWallet
        case locked
        case unreadable
        case failed(String)
    }

    private var sdk: BreezSdk?

    func balance() async -> State {
        let entropy: Data
        switch WalletKeychain.readSeed() {
        case .found(let bytes): entropy = bytes
        case .notFound: return .noWallet
        case .unreadable: return .unreadable
        case .unavailable(let status):
            NSLog("OpenAgents wallet: Keychain status %d", status)
            if status == errSecInteractionNotAllowed { return .locked }
            return .failed("We couldn't open this phone's Keychain.")
        }
        guard let words = BIP39.englishWords(), let mnemonic = BIP39.mnemonic(entropy: entropy, words: words) else {
            return .unreadable
        }
        do {
            let sdk = try await open(mnemonic: mnemonic)
            let info = try await sdk.getInfo(request: GetInfoRequest(ensureSynced: true))
            return .balance(info.balanceSats)
        } catch {
            NSLog("OpenAgents wallet: %@", String(describing: error))
            return .failed("We couldn't reach Spark. Check the connection and reopen OpenAgents.")
        }
    }

    func disconnect() async {
        try? await sdk?.disconnect()
        sdk = nil
    }

    private func open(mnemonic: String) async throws -> BreezSdk {
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
            .appendingPathComponent("spark-mainnet", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let builder = SdkBuilder(config: config, seed: .mnemonic(mnemonic: mnemonic, passphrase: nil))
        await builder.withDefaultStorage(storageDir: directory.path)
        let sdk = try await builder.build()
        self.sdk = sdk
        return sdk
    }
}
