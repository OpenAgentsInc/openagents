import Foundation
import UIKit

/// The one wallet for the app. It doesn't depend on sign-in: the wallet is
/// the person's own, kept by this phone.
final class WalletModel {
    static let shared = WalletModel()

    let wallet: CoreWallet
    private var observers: [UUID: () -> Void] = [:]

    private init() {
        #if DEBUG && targetEnvironment(simulator)
        // Simulator captures: an offline wallet that holds no money and never
        // touches the Keychain.
        if ProcessInfo.processInfo.arguments.contains("-wallet-fixture") {
            let home = FileManager.default.temporaryDirectory.appendingPathComponent("wallet-fixture", isDirectory: true)
            wallet = CoreWallet.fixture(home: home.path, listener: WalletBridge())
            return
        }
        #endif
        wallet = CoreWallet(home: Self.home().path, store: WalletKeychain(), listener: WalletBridge())
    }

    /// Where the earlier app kept the wallet's files:
    /// `Application Support/openagents-v1/spark`, kept off backups. Reusing
    /// it keeps the last balance it showed and Breez's records.
    private static func home() -> URL {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        var state = support.appendingPathComponent("openagents-v1", isDirectory: true)
        try? FileManager.default.createDirectory(
            at: state, withIntermediateDirectories: true,
            attributes: [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication])
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try? state.setResourceValues(values)
        return state.appendingPathComponent("spark", isDirectory: true)
    }

    /// Read the seed and start the wallet; a repeat retries what failed.
    func open() { wallet.open() }

    @discardableResult
    func observe(_ handler: @escaping () -> Void) -> AnyObject {
        let id = UUID()
        observers[id] = handler
        return Token { [weak self] in self?.observers[id] = nil }
    }

    fileprivate func changed() { observers.values.forEach { $0() } }

    private final class Token {
        let cancel: () -> Void
        init(_ cancel: @escaping () -> Void) { self.cancel = cancel }
        deinit { cancel() }
    }
}

private final class WalletBridge: WalletListener, @unchecked Sendable {
    func onWalletChanged() {
        DispatchQueue.main.async { WalletModel.shared.changed() }
    }
}

extension AppModel {
    func observeWallet(_ handler: @escaping () -> Void) -> AnyObject {
        WalletModel.shared.observe(handler)
    }
}
