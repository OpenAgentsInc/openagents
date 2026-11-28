// Firm reason: 6 (iOS @MainActor account controller bridging AppAuth, AppAccount, PasskeyAsk, WorkOS PKCE via ASWebAuthenticationSession, and Cloud credit balance).
import AuthenticationServices
import Foundation
import SwiftUI
import UIKit

private final class EphemeralTokenStore: TokenStore, @unchecked Sendable {
    private let lock = NSLock()
    private var tokens: AppAuth.Tokens?
    func load() -> AppAuth.Tokens? { lock.lock(); defer { lock.unlock() }; return tokens }
    func save(_ tokens: AppAuth.Tokens) { lock.lock(); defer { lock.unlock() }; self.tokens = tokens }
    func clear() { lock.lock(); defer { lock.unlock() }; tokens = nil }
}

@MainActor
final class MobileAccountController: NSObject, ObservableObject, ASWebAuthenticationPresentationContextProviding {
    static let shared = MobileAccountController()

    private static func defaultAuth() -> AppAuth {
        if ProcessInfo.processInfo.arguments.contains("-OAResetStateForTests") {
            return AppAuth(origin: CloudAPIBase.origin, store: EphemeralTokenStore(), device: nil)
        }
        return AppAuth(origin: CloudAPIBase.origin)
    }

    let auth: AppAuth
    let account: AppAccount

    @Published var isLoggedIn: Bool = false
    @Published var email: String = ""
    @Published var accountID: String = ""
    @Published var creditMsat: Int64?
    @Published var promoMicros: Int64 = 0
    @Published var promoExpiresAt: String = ""
    @Published var workosEnabled: Bool = false
    @Published var workosClientID: String = ""
    @Published var pendingOrgs: [AppAuth.WorkOSOrg] = []
    @Published var awaitingEmailCode: Bool = false
    @Published var pendingEmailAddress: String = ""
    @Published var isBusy: Bool = false
    @Published var errorMessage: String?
    @Published var statusMessage: String?

    var onAuthChanged: ((Bool) -> Void)?

    private var webAuthSession: ASWebAuthenticationSession?

    init(auth: AppAuth? = nil) {
        let resolvedAuth = auth ?? Self.defaultAuth()
        self.auth = resolvedAuth
        self.account = AppAccount(auth: resolvedAuth)
        super.init()
        self.isLoggedIn = resolvedAuth.loggedIn
        self.accountID = resolvedAuth.tokens?.account ?? ""
        #if DEBUG
        if UserDefaults.standard.bool(forKey: "OADemoPreview") {
            enableDebugDemoSignedIn()
        }
        #endif
        self.account.changed = { [weak self] in
            guard let self else { return }
            let nowLoggedIn = self.auth.loggedIn
            self.isLoggedIn = nowLoggedIn
            self.accountID = self.auth.tokens?.account ?? ""
            if !nowLoggedIn {
                self.email = ""
                self.creditMsat = nil
                self.promoMicros = 0
                self.promoExpiresAt = ""
                self.pendingOrgs = []
            }
            self.onAuthChanged?(nowLoggedIn)
        }
    }

    var sidebarLabel: String {
        if !isLoggedIn { return "Log in & Cloud" }
        if !email.isEmpty { return email }
        return "Account · Cloud"
    }

    var formattedCredits: String { formattedPaidCredits }

    var formattedPaidCredits: String {
        guard let msat = creditMsat else { return "—" }
        // Display in USD only (no bitcoin/sats units in v1.0 UI).
        return CloudUsageSnapshot.formatUsd(micros: max(0, msat / 1000))
    }

    var formattedPromoCredits: String {
        CloudUsageSnapshot.formatUsd(micros: max(0, promoMicros))
    }

    var formattedPromoExpiry: String {
        let trimmed = promoExpiresAt.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count >= 10, trimmed != "-" else { return "" }
        return String(trimmed.prefix(10))
    }

    static func parseCreditFields(_ obj: [String: Any]) -> (creditMsat: Int64?, promoMicros: Int64?, promoExpiresAt: String?) {
        let msat = (obj["credit_msat"] as? NSNumber).map { max(0, $0.int64Value) }
        let promo = (obj["promo_micros"] as? NSNumber).map { max(0, $0.int64Value) }
        let exp = (obj["promo_expires_at"] as? String).flatMap { $0.isEmpty || $0 == "-" ? nil : $0 }
        return (msat, promo, exp)
    }

    func refresh() async {
        #if DEBUG
        if UserDefaults.standard.bool(forKey: "OADemoPreview") && !auth.loggedIn && isLoggedIn {
            return
        }
        #endif
        let cfg = await account.loadWorkOSConfig()
        workosEnabled = cfg.enabled
        workosClientID = cfg.clientID
        isLoggedIn = auth.loggedIn
        accountID = auth.tokens?.account ?? ""
        guard isLoggedIn else {
            email = ""
            creditMsat = nil
            promoMicros = 0
            promoExpiresAt = ""
            return
        }
        _ = await account.refresh()
        isLoggedIn = auth.loggedIn
        guard isLoggedIn else { return }
        if let info = try? await auth.account() {
            email = info.emails.first ?? ""
            if !info.account.isEmpty { accountID = info.account }
        }
        var loadedLife = false
        if let url = URL(string: auth.origin + "/api/auth/life") {
            var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 15)
            req.httpMethod = "GET"
            if let authed = try? await auth.authorize(req),
               let (data, resp) = try? await URLSession.shared.data(for: authed),
               (resp as? HTTPURLResponse)?.statusCode == 200,
               let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                let parsed = Self.parseCreditFields(obj)
                if let msat = parsed.creditMsat { creditMsat = msat }
                if let promo = parsed.promoMicros { promoMicros = promo }
                promoExpiresAt = parsed.promoExpiresAt ?? ""
                loadedLife = true
            }
        }
        if !loadedLife, let msat = try? await auth.creditMsat() {
            creditMsat = max(0, msat)
        }
    }

    func sendEmailCode(to address: String) async {
        let trimmed = address.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            errorMessage = "Enter your email address."
            return
        }
        isBusy = true
        errorMessage = nil
        statusMessage = nil
        defer { isBusy = false }
        _ = await account.loadWorkOSConfig()
        let ans = await account.emailStart(trimmed)
        switch ans {
        case .done:
            pendingEmailAddress = trimmed
            awaitingEmailCode = true
            statusMessage = "Enter the 6-digit code sent to \(trimmed)."
        case .said(let msg):
            errorMessage = msg
        }
    }

    func verifyEmailCode(_ code: String) async {
        let trimmed = code.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            errorMessage = "Enter the verification code."
            return
        }
        isBusy = true
        errorMessage = nil
        statusMessage = nil
        defer { isBusy = false }
        let ans = await account.emailVerify(trimmed)
        switch ans {
        case .done:
            awaitingEmailCode = false
            pendingEmailAddress = ""
            pendingOrgs = []
            statusMessage = "Logged in."
            await refresh()
            onAuthChanged?(true)
        case .said(let msg):
            pendingOrgs = account.pendingOrgs
            errorMessage = msg
        }
    }

    func passwordSignIn(email: String, password: String, register: Bool) async {
        let trimmed = email.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, !password.isEmpty else {
            errorMessage = "Enter both email and password."
            return
        }
        isBusy = true
        errorMessage = nil
        statusMessage = nil
        defer { isBusy = false }
        let ans = await account.passwordAuth(email: trimmed, password: password, register: register)
        switch ans {
        case .done:
            pendingOrgs = []
            statusMessage = "Logged in."
            await refresh()
            onAuthChanged?(true)
        case .said(let msg):
            pendingOrgs = account.pendingOrgs
            if msg.lowercased().contains("verification code") {
                pendingEmailAddress = trimmed
                awaitingEmailCode = true
                statusMessage = msg
            } else {
                errorMessage = msg
            }
        }
    }

    func selectOrganization(_ orgID: String) async {
        isBusy = true
        errorMessage = nil
        defer { isBusy = false }
        let ans = await account.selectWorkOSOrg(orgID)
        switch ans {
        case .done:
            pendingOrgs = []
            statusMessage = "Logged in."
            await refresh()
            onAuthChanged?(true)
        case .said(let msg):
            errorMessage = msg
        }
    }

    func signInWithPasskey() async {
        isBusy = true
        errorMessage = nil
        statusMessage = nil
        defer { isBusy = false }
        do {
            let challenge = try await auth.passkeyChallenge()
            let ask = PasskeyAsk(anchor: keyWindow())
            let assertion = try await ask.assert(challenge)
            _ = try await auth.passkeyLogin(challenge, assertion)
            statusMessage = "Logged in with Passkey."
            await refresh()
            onAuthChanged?(true)
        } catch {
            errorMessage = AppAccount.words(error)
        }
    }

    func addPasskey() async {
        guard isLoggedIn else { return }
        isBusy = true
        errorMessage = nil
        statusMessage = nil
        defer { isBusy = false }
        do {
            let challenge = try await auth.passkeyRegisterChallenge()
            let ask = PasskeyAsk(anchor: keyWindow())
            let reg = try await ask.register(challenge)
            try await auth.passkeyRegister(
                challenge,
                attestation: reg.attestation,
                clientData: reg.clientData,
                prf: reg.prfSupported
            )
            statusMessage = "Passkey added to your account."
        } catch {
            errorMessage = AppAccount.words(error)
        }
    }

    #if DEBUG
    func enableDebugDemoSignedIn() {
        isLoggedIn = true
        email = "demo@openagents.com"
        accountID = "acct_demo_preview"
        creditMsat = 25_000_000
        promoMicros = 5_000_000
        promoExpiresAt = "2027-02-25T00:00:00Z"
    }
    #endif

    func signInWithWorkOS(provider: String, emailHint: String = "") async {
        isBusy = true
        errorMessage = nil
        statusMessage = nil
        defer { isBusy = false }
        let cfg = await account.loadWorkOSConfig()
        guard cfg.enabled, !cfg.clientID.isEmpty else {
            let label = provider == "AppleOAuth" ? "Apple" : provider == "GoogleOAuth" ? "Google" : "OAuth"
            errorMessage = "\(label) login is temporarily unavailable. Use Personal or work email to log in."
            return
        }
        do {
            let pkce = try AppAuth.workosPKCEStart(
                clientID: cfg.clientID,
                provider: provider,
                loginHint: emailHint.isEmpty ? nil : emailHint,
                redirectURI: "openagents://auth/callback"
            )
            let code = try await AppAuth.workosWebAuthorize(pkce: pkce, anchor: keyWindow())
            let ans = await account.exchangeWorkOSPKCE(code: code, pkce: pkce)
            switch ans {
            case .done:
                pendingOrgs = []
                statusMessage = "Logged in."
                await refresh()
                onAuthChanged?(true)
            case .said(let msg):
                pendingOrgs = account.pendingOrgs
                errorMessage = msg
            }
        } catch {
            let msg = AppAccount.words(error)
            if (error as? ASWebAuthenticationSessionError)?.code != .canceledLogin && msg != "Sign-in cancelled." {
                errorMessage = msg
            }
        }
    }

    func signOut() async {
        isBusy = true
        errorMessage = nil
        statusMessage = nil
        defer { isBusy = false }
        await account.logout()
        isLoggedIn = false
        email = ""
        accountID = ""
        creditMsat = nil
        promoMicros = 0
        promoExpiresAt = ""
        awaitingEmailCode = false
        pendingEmailAddress = ""
        pendingOrgs = []
        onAuthChanged?(false)
    }

    func cancelEmailFlow() {
        account.emailCancel()
        awaitingEmailCode = false
        pendingEmailAddress = ""
        pendingOrgs = []
        errorMessage = nil
        statusMessage = nil
    }

    private func runWebAuth(url: URL, callbackScheme: String) async throws -> URL {
        try await withCheckedThrowingContinuation { cont in
            let session = ASWebAuthenticationSession(url: url, callbackURLScheme: callbackScheme) { [weak self] cbURL, err in
                self?.webAuthSession = nil
                if let cbURL {
                    cont.resume(returning: cbURL)
                } else {
                    cont.resume(throwing: err ?? AppAuth.Failure.said("Login cancelled."))
                }
            }
            session.presentationContextProvider = self
            session.prefersEphemeralWebBrowserSession = false
            self.webAuthSession = session
            if !session.start() {
                self.webAuthSession = nil
                cont.resume(throwing: AppAuth.Failure.said("Could not start login session."))
            }
        }
    }

    private func keyWindow() -> UIWindow? {
        UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .flatMap { $0.windows }
            .first { $0.isKeyWindow }
    }

    nonisolated func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        MainActor.assumeIsolated { keyWindow() ?? ASPresentationAnchor() }
    }
}
