// Firm reason: 6 (iOS @MainActor account controller bridging AppAuth, AppAccount, PasskeyAsk, WorkOS PKCE via ASWebAuthenticationSession, and Cloud credit balance).
import AuthenticationServices
import Foundation
import SwiftUI
import UIKit

@MainActor
final class MobileAccountController: NSObject, ObservableObject, ASWebAuthenticationPresentationContextProviding {
    static let shared = MobileAccountController()

    let auth: AppAuth
    let account: AppAccount

    @Published var isLoggedIn: Bool = false
    @Published var email: String = ""
    @Published var accountID: String = ""
    @Published var creditMsat: Int64?
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

    init(auth: AppAuth = AppAuth.shared) {
        self.auth = auth
        self.account = AppAccount(auth: auth)
        super.init()
        self.isLoggedIn = auth.loggedIn
        self.accountID = auth.tokens?.account ?? ""
        self.account.changed = { [weak self] in
            guard let self else { return }
            let nowLoggedIn = self.auth.loggedIn
            self.isLoggedIn = nowLoggedIn
            self.accountID = self.auth.tokens?.account ?? ""
            if !nowLoggedIn {
                self.email = ""
                self.creditMsat = nil
                self.pendingOrgs = []
            }
            self.onAuthChanged?(nowLoggedIn)
        }
    }

    var sidebarLabel: String {
        if !isLoggedIn { return "Sign In & Cloud" }
        if !email.isEmpty { return email }
        return "Account · Cloud"
    }

    var formattedCredits: String {
        guard let msat = creditMsat else { return "—" }
        let sats = max(0, msat / 1000)
        let nf = NumberFormatter()
        nf.numberStyle = .decimal
        return (nf.string(from: NSNumber(value: sats)) ?? "\(sats)") + " sats"
    }

    func refresh() async {
        let cfg = await account.loadWorkOSConfig()
        workosEnabled = cfg.enabled
        workosClientID = cfg.clientID
        isLoggedIn = auth.loggedIn
        accountID = auth.tokens?.account ?? ""
        guard isLoggedIn else {
            email = ""
            creditMsat = nil
            return
        }
        _ = await account.refresh()
        isLoggedIn = auth.loggedIn
        guard isLoggedIn else { return }
        if let info = try? await auth.account() {
            email = info.emails.first ?? ""
            if !info.account.isEmpty { accountID = info.account }
        }
        if let msat = try? await auth.creditMsat() {
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
            statusMessage = "Signed in."
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
            statusMessage = "Signed in."
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
            statusMessage = "Signed in."
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
            statusMessage = "Signed in with Passkey."
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

    func signInWithWorkOS(provider: String, emailHint: String = "") async {
        isBusy = true
        errorMessage = nil
        statusMessage = nil
        defer { isBusy = false }
        let cfg = await account.loadWorkOSConfig()
        guard cfg.enabled, !cfg.clientID.isEmpty else {
            errorMessage = "OAuth sign-in is not enabled on this server."
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
                statusMessage = "Signed in."
                await refresh()
                onAuthChanged?(true)
            case .said(let msg):
                pendingOrgs = account.pendingOrgs
                errorMessage = msg
            }
        } catch {
            if (error as? ASWebAuthenticationSessionError)?.code != .canceledLogin {
                errorMessage = AppAccount.words(error)
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
                    cont.resume(throwing: err ?? AppAuth.Failure.said("Sign-in cancelled."))
                }
            }
            session.presentationContextProvider = self
            session.prefersEphemeralWebBrowserSession = false
            self.webAuthSession = session
            if !session.start() {
                self.webAuthSession = nil
                cont.resume(throwing: AppAuth.Failure.said("Could not start sign-in session."))
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
