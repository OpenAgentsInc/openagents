import Foundation
import LocalAuthentication

/// metareal/wallet.coax's enum codes, as Wallet.h defines them (Swift can't
/// import the header's cast macros). The header's layout checks pin Flow.
enum Pay { static let idle: coax_t_Pay = 0, quoted: coax_t_Pay = 1, sending: coax_t_Pay = 2, succeeded: coax_t_Pay = 3, notSent: coax_t_Pay = 4, unknown: coax_t_Pay = 5 }
enum Verdict { static let send: coax_t_Verdict = 0, overCap: coax_t_Verdict = 1, overLimit: coax_t_Verdict = 2, expired: coax_t_Verdict = 3, badAmount: coax_t_Verdict = 4, busy: coax_t_Verdict = 5 }
enum PayEvent { static let quote: coax_t_Event = 0, confirm: coax_t_Event = 1, cancel: coax_t_Event = 2, expire: coax_t_Event = 3, sent: coax_t_Event = 4, failed: coax_t_Event = 5, timedOut: coax_t_Event = 6, done: coax_t_Event = 7 }

/// Amounts as the wallet shows them: whole sats after ₿ (as the home does),
/// or BTC, with dollars alongside when the rate is known.
enum Money {
    nonisolated(unsafe) static var inBTC: Bool = UserDefaults.standard.bool(forKey: "wallet.btc") {
        didSet { UserDefaults.standard.set(inBTC, forKey: "wallet.btc") }
    }
    static func text(_ sats: UInt64) -> String {
        if inBTC { return SparkHost.btc(sats) + " BTC" }
        let number = NumberFormatter(); number.numberStyle = .decimal
        return "₿" + (number.string(from: NSNumber(value: sats)) ?? String(sats))
    }
    static func dollars(_ sats: UInt64, _ rate: Double?) -> String? {
        guard let rate, rate > 0 else { return nil }
        let f = NumberFormatter(); f.numberStyle = .currency; f.currencyCode = "USD"
        return f.string(from: NSNumber(value: Double(sats) / 100_000_000 * rate)).map { "≈ " + $0 }
    }
    /// Digits typed into an amount field, kept through metareal_wallet_digit:
    /// nil when it isn't a whole number of sats or passes all the bitcoin.
    static func typed(_ text: String) -> UInt64? {
        var sats: Int64 = 0
        for c in text where c != "," && c != " " {
            guard let d = c.wholeNumberValue, c.isASCII else { return nil }
            sats = metareal_wallet_digit(sats, Int64(d))
            if sats < 0 { return nil }
        }
        return UInt64(sats)
    }
}

/// The phone's owner check before money leaves: Face ID, Touch ID or the
/// passcode. A phone with no passcode has nothing to check with.
enum OwnerCheck {
    static func confirm(_ reason: String) async -> Bool {
        let context = LAContext()
        var error: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) else {
            return (error as? LAError)?.code == .passcodeNotSet
        }
        return (try? await context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: reason)) ?? false
    }
}
