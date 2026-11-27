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
    /// Whether this login attempt is using WorkOS Magic Auth (vs legacy /api/auth/email/*).
    private var pendingWorkOS = false
    /// Pending multi-organization selection token & orgs when WorkOS returns `need-org`.
    private(set) var pendingOrgToken: String?
    private(set) var pendingOrgs: [AppAuth.WorkOSOrg] = []
    /// Cached WorkOS config for this API origin.
    private(set) var workosConfig: AppAuth.WorkOSConfig?
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

    /// Loads /api/auth/workos/config (cached per account instance; falls back cleanly to disabled).
    @discardableResult
    func loadWorkOSConfig() async -> AppAuth.WorkOSConfig {
        if let cfg = workosConfig { return cfg }
        let cfg = (try? await auth.workosConfig()) ?? AppAuth.WorkOSConfig(enabled: false, clientID: "", providers: [])
        workosConfig = cfg
        return cfg
    }

    /// Sends the code to an email address (routes via WorkOS Magic Auth when enabled, else legacy email code).
    func emailStart(_ email: String) async -> Answer {
        attempt += 1; let mine = attempt; pending = nil; pendingWorkOS = false; pendingOrgToken = nil; pendingOrgs = []
        let trimmed = email.trimmingCharacters(in: .whitespacesAndNewlines)
        let useWorkOS = (workosConfig?.enabled == true)
        do {
            let handle = useWorkOS ? try await auth.workosMagicStart(trimmed) : try await auth.emailStart(trimmed)
            guard mine == attempt else { return .said("Start again.") }
            pending = handle; pendingWorkOS = useWorkOS; return .done
        } catch { return .said(AppAccount.words(error)) }
    }

    /// The code from the email. Done means logged in (or pendingOrgs populated when org selection is required).
    func emailVerify(_ code: String) async -> Answer {
        guard let handle = pending else { return .said("Ask for a new code.") }
        let mine = attempt
        let trimmed = code.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            if pendingWorkOS {
                switch try await auth.workosMagicVerify(code: trimmed, pending: handle) {
                case .loggedIn:
                    if mine == attempt { pending = nil; pendingWorkOS = false; pendingOrgToken = nil; pendingOrgs = [] }
                    return .done
                case .needOrg(let tok, let orgs):
                    if mine == attempt { pending = nil; pendingOrgToken = tok; pendingOrgs = orgs }
                    return .said("Choose an organization to finish signing in.")
                case .verifyEmail(let nextPending):
                    if mine == attempt { pending = nextPending; pendingWorkOS = true }
                    return .said("Enter the 6-digit verification code sent to your email.")
                }
            }
            _ = try await auth.emailVerify(code: trimmed, pending: handle)
            if mine == attempt { pending = nil }
            return .done
        } catch { return .said(AppAccount.words(error)) }
    }

    /// Completes WorkOS multi-organization selection after a `need-org` response.
    func selectWorkOSOrg(_ organizationID: String) async -> Answer {
        guard let tok = pendingOrgToken else { return .said("Start again.") }
        let mine = attempt
        do {
            _ = try await auth.workosSelectOrg(pending: tok, organizationID: organizationID)
            if mine == attempt { pendingOrgToken = nil; pendingOrgs = [] }
            return .done
        } catch { return .said(AppAccount.words(error)) }
    }

    /// Exchanges a validated PKCE callback code at /api/auth/workos/exchange.
    func exchangeWorkOSPKCE(code: String, pkce: AppAuth.WorkOSPKCE) async -> Answer {
        attempt += 1; let mine = attempt; pendingOrgToken = nil; pendingOrgs = []
        do {
            switch try await auth.workosExchangePKCE(code: code, pkce: pkce) {
            case .loggedIn:
                return .done
            case .needOrg(let tok, let orgs):
                if mine == attempt { pendingOrgToken = tok; pendingOrgs = orgs }
                return .said("Choose an organization to finish signing in.")
            case .verifyEmail(let nextPending):
                if mine == attempt { pending = nextPending; pendingWorkOS = true }
                return .said("Enter the 6-digit verification code sent to your email.")
            }
        } catch { return .said(AppAccount.words(error)) }
    }

    /// Signs in or creates an account with WorkOS Email + Password (handles verify-email and need-org).
    func passwordAuth(email: String, password: String, register: Bool) async -> Answer {
        attempt += 1; let mine = attempt; pending = nil; pendingWorkOS = false; pendingOrgToken = nil; pendingOrgs = []
        let trimmed = email.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            let res = register
                ? try await auth.workosPasswordRegister(email: trimmed, password: password)
                : try await auth.workosPasswordLogin(email: trimmed, password: password)
            switch res {
            case .loggedIn:
                return .done
            case .needOrg(let tok, let orgs):
                if mine == attempt { pendingOrgToken = tok; pendingOrgs = orgs }
                return .said("Choose an organization to finish signing in.")
            case .verifyEmail(let nextPending):
                if mine == attempt { pending = nextPending; pendingWorkOS = true }
                return .said("Enter the 6-digit verification code sent to your email.")
            }
        } catch { return .said(AppAccount.words(error)) }
    }

    /// Forgets an email attempt that was not finished.
    func emailCancel() { attempt += 1; pending = nil; pendingWorkOS = false; pendingOrgToken = nil; pendingOrgs = [] }

    /// Ends the login on this device and, when it can be reached, on the server.
    func logout() async { attempt += 1; pending = nil; pendingWorkOS = false; pendingOrgToken = nil; pendingOrgs = []; await auth.logout() }

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
