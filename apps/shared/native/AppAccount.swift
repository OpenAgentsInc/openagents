// Firm reason: 6 (the native account adapter: Keychain tokens and the device key stay behind it).
// The account, as a compiled Coax app sees it (GitHub #13645): whether someone is logged in, and
// the steps of an email login. AppAuth (apps/shared/host/apple/AppAuth.swift) does the work and
// keeps the tokens (Keychain service `com.openagents.app.auth`, device-bound refresh). This file
// keeps what else must not enter Coax: the email attempt's pending handle. Coax sends an email
// address or a code and hears "done" or the server's own words.
// Main thread only.
//   apps/shared/native/app-services-check.sh
import Foundation

@MainActor final class AppAccount {
    /// What a step answered: done, or words to show as given.
    enum Answer: Equatable { case done, said(String) }
    let auth: AppAuth
    /// The email attempt's handle from the server. It goes back with the code and nowhere else.
    private var pending: String?
    /// Counts logins and logouts, so an answer from an earlier attempt is dropped.
    private var attempt: UInt64 = 0
    /// Called when the login starts or ends, here or by the server (a refresh it refused).
    var changed: (() -> Void)?
    private var watch: NSObjectProtocol?

    init(auth: AppAuth) {
        self.auth = auth
        watch = NotificationCenter.default.addObserver(forName: AppAuth.didChangeSession, object: auth, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.changed?() }
        }
    }
    deinit { if let watch { NotificationCenter.default.removeObserver(watch) } }

    var loggedIn: Bool { auth.loggedIn }

    /// Sends the code to an email address.
    func emailStart(_ email: String) async -> Answer {
        attempt += 1; let mine = attempt; pending = nil
        do {
            let handle = try await auth.emailStart(email.trimmingCharacters(in: .whitespacesAndNewlines))
            guard mine == attempt else { return .said("Start again.") }
            pending = handle; return .done
        } catch { return .said(AppAccount.words(error)) }
    }

    /// The code from the email. Done means logged in.
    func emailVerify(_ code: String) async -> Answer {
        guard let handle = pending else { return .said("Ask for a new code.") }
        let mine = attempt
        do {
            _ = try await auth.emailVerify(code: code.trimmingCharacters(in: .whitespacesAndNewlines), pending: handle)
            if mine == attempt { pending = nil }
            return .done
        } catch { return .said(AppAccount.words(error)) }
    }

    /// Forgets an email attempt that was not finished.
    func emailCancel() { attempt += 1; pending = nil }

    /// Ends the login on this device and, when it can be reached, on the server.
    func logout() async { attempt += 1; pending = nil; await auth.logout() }

    /// Renews the access token when it is stale. Answers whether someone is still logged in.
    func refresh() async -> Bool {
        guard auth.loggedIn else { return false }
        if case .refresh = AppAuth.next(auth.tokens, now: Date()) { _ = try? await auth.refreshed() }
        return auth.loggedIn
    }

    /// The request with the account session for AppHTTP: nil when nobody is logged in.
    nonisolated static func authorizer(_ auth: AppAuth) -> AppHTTP.Authorize {
        { request in
            guard auth.loggedIn else { return nil }
            do { return try await auth.authorize(request) }
            catch AppAuth.Failure.unreachable { throw AppHTTP.Blocked.offline }
            catch { throw AppHTTP.Blocked.login }
        }
    }

    nonisolated static func words(_ error: Error) -> String {
        (error as? AppAuth.Failure)?.errorDescription ?? "That didn't work. Try again."
    }
}
