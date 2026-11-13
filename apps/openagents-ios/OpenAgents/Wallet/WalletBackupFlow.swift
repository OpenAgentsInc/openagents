import Foundation
import UIKit

/// Where the wallet's seed is on this phone. Adding never replaces: a phone that has a wallet keeps it.
protocol SeedStore: Sendable {
    func read() -> Data?
    func add(_ seed: Data) -> Bool
}
struct KeychainSeedStore: SeedStore {
    func read() -> Data? { if case .found(let bytes) = WalletKeychain.readSeed() { return bytes }; return nil }
    func add(_ seed: Data) -> Bool { WalletKeychain.addSeed(seed) }
}

/// The system's passkey sheet, as the wallet's flows use it. Tests give a software passkey.
@MainActor
protocol PasskeyAsking: AnyObject {
    func assert(_ c: AppAuth.PasskeyChallenge, prf: Data?) async throws -> AppAuth.PasskeyAssertion
    func register(_ c: AppAuth.PasskeyNew) async throws -> PasskeyAsk.Registration
    var prfOutput: Data? { get }
}
extension PasskeyAsk: PasskeyAsking {}

/// Backing the wallet up, bringing it back, and asking before a payment (#13217). Every decision
/// is Coax's or the server's; this runs the steps in order and words what happened.
///
///   backUp    one passkey prompt (two when the account has no passkey yet: one makes it), then
///             the phone seals the seed, checks that the passkey and the new recovery code both
///             open it, stores the ciphertext, and hands back the code's 24 words to show once
///   restore   on a phone with no wallet: the passkey prompt that logged in, or the recovery
///             code typed in, opens the account's backup and the seed goes into the Keychain
///   maySpend  asks the server before a payment; past the daily limit it asks for the passkey
@MainActor
final class WalletBackupFlow {
    let auth: AppAuth
    let client: WalletBackupClient
    let seeds: SeedStore
    let memory: WalletBackup.Memory
    let ledger: SpendLedger
    /// A fresh sheet for each prompt, anchored to the screen that asks.
    var asker: () -> PasskeyAsking

    init(auth: AppAuth = .shared, client: WalletBackupClient = WalletBackupClient(), seeds: SeedStore = KeychainSeedStore(),
         memory: WalletBackup.Memory = WalletBackup.Memory(), ledger: SpendLedger = SpendLedger(), asker: @escaping () -> PasskeyAsking) {
        self.auth = auth; self.client = client; self.seeds = seeds; self.memory = memory; self.ledger = ledger; self.asker = asker
    }

    /// The flow for a screen: its passkey prompts come up over that screen's window.
    static func on(_ screen: UIViewController?) -> WalletBackupFlow {
        WalletBackupFlow(asker: { [weak screen] in PasskeyAsk(anchor: screen?.view.window) })
    }
    /// Posted when a wallet came back to this phone, so the home screen reads its balance.
    static let restored = Notification.Name("OpenAgentsWalletRestored")

    private func said(_ words: String) -> WalletBackup.Failure { .said(words) }

    /// Proves it is you with a passkey of this account, evaluating the wallet backup's PRF input
    /// in the same prompt: the passkey's credential id, its site, and the PRF output when the
    /// passkey can make one. With no passkey on the account, makes one first.
    struct Proved { let credential: Data; let rpID: String; let prf: Data? }
    func prove() async throws -> Proved {
        if try await auth.passkeyCount() == 0 {
            let new = try await auth.passkeyRegisterChallenge()
            let made = try await asker().register(new)
            try await auth.passkeyRegister(new, attestation: made.attestation, clientData: made.clientData, prf: made.prfSupported)
        }
        let challenge = try await auth.passkeyStepUpChallenge()
        let ask = asker()
        let answer = try await ask.assert(challenge, prf: WalletVault.prfInput)
        try await auth.passkeyStepUp(challenge, answer)
        return Proved(credential: answer.credentialID, rpID: challenge.rpID, prf: ask.prfOutput)
    }

    /// Backs this phone's wallet up to the account: the new recovery code's words, to show once.
    func backUp(label: String = UIDevice.current.name) async throws -> String {
        guard let seed = seeds.read() else { throw said("This phone has no wallet to back up.") }
        let account = client.account
        guard client.isLoggedIn, !account.isEmpty else { throw said("Log in first.") }
        let proved = try await prove()
        guard let prf = proved.prf, prf.count == 32 else {
            throw said("This passkey can't open a wallet backup on this phone, so nothing was saved. Update your phone, then try again.")
        }
        let after = try await client.summary().backup?.epoch ?? 0
        let clean = String(label.unicodeScalars.filter { $0.isASCII && $0.value >= 32 && $0 != "\"" && $0 != "\\" }.prefix(60))
        let made = try await WalletVault.onBigStack {
            try WalletVault.make(account: account, epoch: after + 1, seed: seed, prf: prf, credential: proved.credential, rp: proved.rpID, label: clean.isEmpty ? "iPhone" : clean)
        }
        let epoch = try await client.store(after: after, slots: made.slots, object: made.object)
        memory.remember(vault: made.vault, epoch: epoch)
        return made.words
    }

    /// Brings the account's backed-up wallet to a phone that has none. false: the account has no backup.
    func restore(_ key: WalletVault.Key) async throws -> Bool {
        guard seeds.read() == nil else { throw said("This phone already has a wallet. It was left as it is.") }
        let account = client.account
        guard client.isLoggedIn, !account.isEmpty else { throw said("Log in first.") }
        guard let stored = try await client.export() else { return false }
        let seen = memory.vault == stored.vault ? memory.seen : 0
        let seed = try await WalletVault.onBigStack { try WalletVault.open(stored, account: account, key: key, seen: seen) }
        guard seeds.add(seed) else { throw said("This phone couldn't keep the wallet. Nothing was changed.") }
        memory.remember(vault: stored.vault, epoch: stored.epoch)
        NotificationCenter.default.post(name: WalletBackupFlow.restored, object: nil)
        return true
    }
    /// Right after a passkey login: when this phone has no wallet and the passkey made a PRF
    /// output, the account's backup is opened with it. true when a wallet came back.
    func restoreAfterLogin(credential: Data, prf: Data?) async -> Bool {
        guard seeds.read() == nil, let prf, prf.count == 32 else { return false }
        return (try? await restore(.passkey(credential: credential, prf: prf))) ?? false
    }

    // MARK: payments

    enum Spend: Equatable {
        /// Pay; the id names the payment to the server (report it paid or not paid).
        case pay(String)
        case refuse(String)
    }
    /// Asks before a payment of sats to destination leaves the wallet.
    func maySpend(sats: Int64, destination: String) async -> Spend {
        let id = UUID().uuidString
        let new = ledger.isNew(destination)
        var stepped = false
        while true {
            let answer = await client.spend(sats: sats, id: id, newDestination: new)
            if let daily = answer.daily { ledger.limit = daily }
            // A 401 that asks for no proof means the server no longer takes this login (it ended, or
            // the account was frozen, which ends every login): the phone is logged out.
            let loggedIn = auth.loggedIn && !(answer.status == 401 && !answer.stepUp)
            let local = metaverse_limit_local(sats, ledger.spent, ledger.limit, loggedIn)
            switch metaverse_limit_next(Int64(answer.status), answer.stepUp, stepped, auth.loggedIn, local) {
            case 0: return .pay(id)
            case 1:
                do { _ = try await prove(); stepped = true } catch { return .refuse(error.localizedDescription) }
            case 2:
                return .refuse("This is over your daily limit of \(WalletBackup.sats(ledger.limit)) sats. Confirm it's you with your passkey to pay it.")
            case 4:
                return .refuse("Log in to pay. This wallet's daily limit is kept by your account.")
            default:
                return .refuse(answer.words.isEmpty ? "This payment can't be made right now." : answer.words)
            }
        }
    }
    func paid(id: String, sats: Int64, destination: String) { ledger.paid(sats); ledger.remember(destination) }
    func notPaid(id: String) async { await client.void(id: id) }

    // MARK: one seed for the wallet and Nostr (decision D5: off unless the person turns it on)

    /// Links the Nostr key of the wallet's words to the account. The screen shows the warning first.
    func useWalletWordsForNostr() async throws {
        guard let seed = seeds.read() else { throw said("This phone has no wallet.") }
        _ = try await prove()
        let url = try await auth.proofChallenge(purpose: "link")
        guard let made = try await WalletVault.onBigStack({ try WalletVault.nostrProof(seed: seed, url: url) }) else { throw said("This phone couldn't sign with your wallet's words. Nothing was changed.") }
        try await auth.linkNostr(url: url, proof: made.proof, label: "Wallet words")
        memory.oneSeedKey = made.pubkey
    }
    func stopUsingWalletWordsForNostr() async throws {
        guard let pubkey = memory.oneSeedKey else { return }
        _ = try await prove()
        try await auth.unlinkNostr(pubkey: pubkey)
        memory.oneSeedKey = nil
    }
}
