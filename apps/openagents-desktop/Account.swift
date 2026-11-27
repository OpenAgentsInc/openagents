// Firm reason: 6 (a Swift-only SDK: AppKit's sheets, CryptoKit, UserNotifications and the Secure Enclave through AppAuth.swift and DeviceKey.swift)
import AppKit
import CryptoKit
import Foundation
import UserNotifications

/// The Mac app's account login (docs/v1/desktop.md §2.4):
/// OpenAgents ▸ Log In… uses WorkOS (Magic Auth 6-digit email code, Email & Password, Google,
/// GitHub, Apple, Enterprise SSO, B2B Organizations) or first-party Passkey.
/// Session tokens (`oa_at_`/`oa_rt_`) and the Secure Enclave device key blob live in 0600 files
/// under `~/.openagents/desktop/`, never the Keychain, so rebuilt apps never prompt for Keychain.
///
/// OPENAGENTS_API_ORIGIN points it at staging (https://staging.openagents.com) or a local API on
/// 127.0.0.1; anything else is openagents.com.

/// The app's one login, what its menu says, and the sheets.
final class MacAccount: @unchecked Sendable {
    static let shared = MacAccount()

    /// $HOME, as the CLI and settings.json read it (a check runs the app in a home of its own).
    static var home: URL {
        let h = ProcessInfo.processInfo.environment["HOME"] ?? ""
        return h.hasPrefix("/") ? URL(fileURLWithPath: h, isDirectory: true) : FileManager.default.homeDirectoryForCurrentUser
    }
    static var folder: URL { home.appendingPathComponent(".openagents/desktop", isDirectory: true) }

    static var origin: String {
        let o = ProcessInfo.processInfo.environment["OPENAGENTS_API_ORIGIN"] ?? ""
        if o == "https://staging.openagents.com" || o.hasPrefix("http://127.0.0.1:") || o.hasPrefix("http://localhost:") { return o }
        return "https://openagents.com"
    }

    let store: TokenStore
    let auth: AppAuth
    private let send: AppAuth.Send
    private let lock = NSLock()
    private var shown = ""          // what the account answered: the masked email
    private var known = false
    private var cachedCreditMsat: Int64? = nil
    private var cachedPromoMicros: Int64 = 0
    private var cachedPromoExpiresAt: String = ""
    private var cachedOrgName: String = "Personal"
    private var cachedGitHubStatus: String = "Not connected"
    private var cachedProjectsSummary: String = "None synced"
    var changed: (() -> Void)?

    init(store: TokenStore = FileTokenStore(url: MacAccount.folder.appendingPathComponent("login.json")),
         device: DeviceKeys? = DeviceKeys(store: FileDeviceKeyStore(url: MacAccount.folder.appendingPathComponent("device-key"))),
         origin: String = MacAccount.origin, send: @escaping AppAuth.Send = AppAuth.network) {
        self.store = store
        self.send = send
        auth = AppAuth(origin: origin, send: send, store: store, device: device)
    }

    var loggedIn: Bool { auth.loggedIn }
    var email: String { lock.lock(); defer { lock.unlock() }; return shown }
    var orgName: String { lock.lock(); defer { lock.unlock() }; return cachedOrgName }
    var creditMsat: Int64? { lock.lock(); defer { lock.unlock() }; return cachedCreditMsat }
    var promoMicros: Int64 { lock.lock(); defer { lock.unlock() }; return cachedPromoMicros }
    var promoExpiresAt: String { lock.lock(); defer { lock.unlock() }; return cachedPromoExpiresAt }
    var githubStatus: String { lock.lock(); defer { lock.unlock() }; return cachedGitHubStatus }
    var projectsSummary: String { lock.lock(); defer { lock.unlock() }; return cachedProjectsSummary }

    var paidCreditLabel: String {
        guard let msat = creditMsat else { return loggedIn ? "0 sats" : "Sign in to view" }
        let sats = max(0, msat / 1000)
        return "\(sats) sats"
    }

    var promoCreditLabel: String {
        let dollars = Double(max(0, promoMicros)) / 1_000_000.0
        return String(format: "$%.2f USD", dollars)
    }

    var promoExpiryLabel: String {
        let exp = promoExpiresAt
        if exp.isEmpty {
            return "Non-withdrawable service credit; spends oldest expiry first on eligible models."
        }
        return "Nearest expiry: \(exp) · Non-withdrawable service credit."
    }

    var title: String {
        guard loggedIn else { return "Log In…" }
        lock.lock(); defer { lock.unlock() }
        if !known { return "Logged in" }
        return shown.isEmpty ? "Logged in" : "Logged in as \(shown)"
    }

    /// Fetches the user's two-line Paid Credit (msat) and Promo Credit (USD micros + nearest expiry)
    /// from GET /api/auth/life using the signed-in account session.
    @discardableResult
    func fetchCreditMsat() async -> Int64? {
        guard loggedIn else { return nil }
        if let url = URL(string: auth.origin + "/api/auth/life") {
            var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 15)
            req.httpMethod = "GET"
            if let authed = try? await auth.authorize(req),
               let (data, code) = try? await send(authed),
               code == 200,
               let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                let msat = (obj["credit_msat"] as? NSNumber)?.int64Value ?? 0
                let promo = (obj["promo_micros"] as? NSNumber)?.int64Value ?? 0
                let exp = (obj["promo_expires_at"] as? String) ?? ""
                setCredits(msat: max(0, msat), promoMicros: max(0, promo), promoExpiresAt: exp)
                return max(0, msat)
            }
        }
        if let msat = try? await auth.creditMsat() {
            setCreditMsat(msat)
            return msat
        }
        return creditMsat
    }

    /// Fetches WorkOS Pipes GitHub connection status (GET /api/integrations/github/status).
    func fetchGitHubStatus() async {
        guard loggedIn, let url = URL(string: auth.origin + "/api/integrations/github/status") else { return }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 12)
        req.httpMethod = "GET"
        guard let authed = try? await auth.authorize(req),
              let (data, code) = try? await send(authed),
              code == 200,
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
        let connected = (obj["connected"] as? Bool) == true
        let login = (obj["login"] as? String) ?? (obj["username"] as? String) ?? ""
        lock.lock()
        cachedGitHubStatus = connected ? (login.isEmpty ? "Connected" : "Connected (@\(login))") : "Not connected"
        lock.unlock()
    }

    /// Fetches synced Cloud Projects count (GET /api/projects/v1).
    func fetchCloudProjects() async -> [[String: Any]] {
        guard loggedIn, let url = URL(string: auth.origin + "/api/projects/v1") else { return [] }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 12)
        req.httpMethod = "GET"
        guard let authed = try? await auth.authorize(req),
              let (data, code) = try? await send(authed),
              code == 200,
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let list = obj["projects"] as? [[String: Any]] else { return [] }
        lock.lock()
        cachedProjectsSummary = list.isEmpty ? "0 projects" : (list.count == 1 ? "1 project" : "\(list.count) projects")
        lock.unlock()
        return list
    }

    /// Starts WorkOS Pipes GitHub OAuth connection in the system browser (POST /api/integrations/github/connect).
    @MainActor func connectGitHubPipes() async {
        guard loggedIn else { return await showLogin() }
        guard let url = URL(string: auth.origin + "/api/integrations/github/connect") else { return }
        var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 15)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = Data("{\"return_to\":\"/settings\"}".utf8)
        do {
            let authed = try await auth.authorize(req)
            let (data, code) = try await send(authed)
            if (200..<300).contains(code),
               let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let authURLStr = (obj["url"] as? String) ?? (obj["authorize_url"] as? String),
               let authURL = URL(string: authURLStr) {
                NSWorkspace.shared.open(authURL)
            } else {
                note("Open openagents.com/settings to connect GitHub via WorkOS Pipes.")
            }
        } catch {
            say(error)
        }
    }

    /// Asks the server who is logged in (an authorized request: a stale access token is renewed
    /// first); a login the server ended is forgotten.
    func look() async {
        guard loggedIn else {
            tell()
            return
        }
        if let a = try? await auth.account() {
            setShown(a.emails.first ?? "")
        }
        _ = await fetchCreditMsat()
        await fetchGitHubStatus()
        _ = await fetchCloudProjects()
        tell()
        await registerForPush()
    }
    private func setShown(_ s: String) { lock.lock(); shown = s; known = true; lock.unlock() }
    private func setCreditMsat(_ msat: Int64?) { lock.lock(); cachedCreditMsat = msat; lock.unlock() }
    private func setCredits(msat: Int64?, promoMicros: Int64, promoExpiresAt: String) {
        lock.lock()
        cachedCreditMsat = msat
        cachedPromoMicros = promoMicros
        cachedPromoExpiresAt = promoExpiresAt
        lock.unlock()
    }
    func forgetShown() {
        lock.lock()
        shown = ""; known = false; cachedCreditMsat = nil
        cachedPromoMicros = 0; cachedPromoExpiresAt = ""
        cachedOrgName = "Personal"; cachedGitHubStatus = "Not connected"; cachedProjectsSummary = "None synced"
        lock.unlock()
    }
    func tell() { DispatchQueue.main.async { self.changed?() } }

    func logout() async {
        await auth.logout()
        forgetShown()
        tell()
    }

    // MARK: the sheets

    @MainActor func showLogin() async {
        let wos = (try? await auth.workosConfig()) ?? AppAuth.WorkOSConfig(enabled: false, clientID: "", providers: [])
        let alert = NSAlert()
        alert.messageText = "Log in to OpenAgents"
        alert.informativeText = wos.enabled
            ? "Enter your email for a 6-digit code, or continue with Email & Password, Google, GitHub, Apple, Enterprise SSO, or Passkey."
            : "Enter your email and we'll send you a code."
        let email = NSTextField(frame: NSRect(x: 0, y: 0, width: 280, height: 24))
        email.placeholderString = "you@example.com"
        email.setAccessibilityIdentifier("account.email")
        alert.accessoryView = email
        alert.addButton(withTitle: "Send Code")
        if wos.enabled { alert.addButton(withTitle: "Password / Google / GitHub / Apple / SSO…") }
        alert.addButton(withTitle: "Log In with Passkey")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = email
        let resp = alert.runModal()
        let idx = resp.rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
        if idx == 0 {
            let address = email.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !address.isEmpty else { return await showLogin() }
            do {
                if wos.enabled {
                    let pending = try await auth.workosMagicStart(address)
                    await askCode(address: address, pending: pending, workos: true)
                } else {
                    let pending = try await auth.emailStart(address)
                    await askCode(address: address, pending: pending, workos: false)
                }
            } catch { say(error) }
            return
        }
        if wos.enabled && idx == 1 {
            await showWorkOSProviders(clientID: wos.clientID, emailHint: email.stringValue.trimmingCharacters(in: .whitespacesAndNewlines))
            return
        }
        let passkeyIdx = wos.enabled ? 2 : 1
        if idx == passkeyIdx { await loginWithPasskey() }
    }

    @MainActor private func showWorkOSProviders(clientID: String, emailHint: String) async {
        let alert = NSAlert()
        alert.messageText = "Continue with…"
        alert.informativeText = "Choose Email & Password, or an OAuth/SSO provider (system browser sheet using PKCE)."
        alert.addButton(withTitle: "Email & Password…")
        alert.addButton(withTitle: "Google")
        alert.addButton(withTitle: "GitHub")
        alert.addButton(withTitle: "Apple")
        alert.addButton(withTitle: "Enterprise SSO")
        alert.addButton(withTitle: "Cancel")
        let idx = alert.runModal().rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
        switch idx {
        case 0: await askPassword(emailHint: emailHint)
        case 1: await loginWithWorkOSOAuth(clientID: clientID, provider: "GoogleOAuth", connectionID: nil, loginHint: emailHint)
        case 2: await loginWithWorkOSOAuth(clientID: clientID, provider: "GitHubOAuth", connectionID: nil, loginHint: emailHint)
        case 3: await loginWithWorkOSOAuth(clientID: clientID, provider: "AppleOAuth", connectionID: nil, loginHint: emailHint)
        case 4:
            var addr = emailHint
            if addr.isEmpty {
                let ask = NSAlert()
                ask.messageText = "Enterprise SSO"
                ask.informativeText = "Enter your work email address."
                let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 260, height: 24))
                field.placeholderString = "you@company.com"
                ask.accessoryView = field
                ask.addButton(withTitle: "Continue")
                ask.addButton(withTitle: "Cancel")
                guard ask.runModal() == .alertFirstButtonReturn else { return }
                addr = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            }
            guard !addr.isEmpty else { return }
            do {
                let disc = try await auth.workosDiscoverSSO(email: addr)
                guard disc.sso, !disc.connectionID.isEmpty else {
                    say(AppAuth.Failure.said("No Enterprise SSO connection found for that email domain."))
                    return
                }
                await loginWithWorkOSOAuth(clientID: clientID, provider: nil, connectionID: disc.connectionID, loginHint: addr)
            } catch { say(error) }
        default: return
        }
    }

    @MainActor private func askPassword(emailHint: String) async {
        let alert = NSAlert()
        alert.messageText = "Email & Password"
        alert.informativeText = "Sign in or create an account with your email and password."
        let stack = NSStackView(frame: NSRect(x: 0, y: 0, width: 280, height: 56))
        stack.orientation = .vertical
        stack.spacing = 8
        let email = NSTextField(frame: NSRect(x: 0, y: 0, width: 280, height: 24))
        email.placeholderString = "you@example.com"
        email.stringValue = emailHint
        let pw = NSSecureTextField(frame: NSRect(x: 0, y: 0, width: 280, height: 24))
        pw.placeholderString = "Password"
        stack.addArrangedSubview(email)
        stack.addArrangedSubview(pw)
        alert.accessoryView = stack
        alert.addButton(withTitle: "Log In")
        alert.addButton(withTitle: "Create Account")
        alert.addButton(withTitle: "Reset Password")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = emailHint.isEmpty ? email : pw
        let idx = alert.runModal().rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
        let addr = email.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        let secret = pw.stringValue
        pw.stringValue = ""
        guard !addr.isEmpty else { return }
        do {
            if idx == 0 || idx == 1 {
                let res = idx == 0
                    ? try await auth.workosPasswordLogin(email: addr, password: secret)
                    : try await auth.workosPasswordRegister(email: addr, password: secret)
                switch res {
                case .loggedIn:
                    forgetShown()
                    await look()
                case .needOrg(let pending, let orgs):
                    await pickWorkOSOrg(pending: pending, organizations: orgs)
                case .verifyEmail(let pending):
                    await askCode(address: addr, pending: pending, workos: true)
                }
            } else if idx == 2 {
                try await auth.workosPasswordReset(email: addr)
                note("If an account exists for that email, we sent a password reset link.")
            }
        } catch { say(error) }
    }

    @MainActor func loginWithWorkOSOAuth(clientID: String, provider: String?, connectionID: String?, loginHint: String?) async {
        do {
            let pkce = try AppAuth.workosPKCEStart(
                clientID: clientID, provider: provider, connectionID: connectionID,
                loginHint: (loginHint?.isEmpty == false) ? loginHint : nil
            )
            let anchor = NSApp?.keyWindow ?? NSApp?.mainWindow
            let code = try await AppAuth.workosWebAuthorize(pkce: pkce, anchor: anchor)
            switch try await auth.workosExchangePKCE(code: code, pkce: pkce) {
            case .loggedIn:
                forgetShown()
                await look()
            case .needOrg(let pending, let orgs):
                await pickWorkOSOrg(pending: pending, organizations: orgs)
            case .verifyEmail(let pending):
                await askCode(address: loginHint ?? "your email", pending: pending, workos: true)
            }
        } catch {
            if case AppAuth.Failure.said(let msg) = error, msg == "Sign-in cancelled." { return }
            say(error)
        }
    }

    @MainActor private func pickWorkOSOrg(pending: String, organizations: [AppAuth.WorkOSOrg]) async {
        guard !organizations.isEmpty else { say(AppAuth.Failure.said("No organization available.")); return }
        let alert = NSAlert()
        alert.messageText = "Choose an organization"
        alert.informativeText = "Select the organization to sign into."
        let popup = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 280, height: 26))
        for org in organizations { popup.addItem(withTitle: org.name) }
        alert.accessoryView = popup
        alert.addButton(withTitle: "Continue")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        let i = max(0, min(popup.indexOfSelectedItem, organizations.count - 1))
        do {
            _ = try await auth.workosSelectOrg(pending: pending, organizationID: organizations[i].id)
            forgetShown()
            await look()
        } catch { say(error) }
    }

    @MainActor func switchOrganization() async {
        guard loggedIn else { return await showLogin() }
        do {
            let orgs = try await auth.workosOrgs()
            guard !orgs.isEmpty else { note("No organizations linked to this account."); return }
            let alert = NSAlert()
            alert.messageText = "Switch organization"
            alert.informativeText = "Choose the active organization."
            let popup = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 280, height: 26))
            for org in orgs { popup.addItem(withTitle: org.name) }
            alert.accessoryView = popup
            alert.addButton(withTitle: "Switch")
            alert.addButton(withTitle: "Cancel")
            guard alert.runModal() == .alertFirstButtonReturn else { return }
            let i = max(0, min(popup.indexOfSelectedItem, orgs.count - 1))
            _ = try await auth.workosSwitchOrg(orgs[i].id)
            lock.lock(); cachedOrgName = orgs[i].name; lock.unlock()
            note("Switched to \(orgs[i].name).")
            tell()
        } catch { say(error) }
    }

    @MainActor private func askCode(address: String, pending: String, workos: Bool = false) async {
        let alert = NSAlert()
        alert.messageText = "Enter your code"
        alert.informativeText = "We sent a 6-digit code to \(address)."
        let code = NSTextField(frame: NSRect(x: 0, y: 0, width: 160, height: 24))
        code.placeholderString = "123456"
        code.setAccessibilityIdentifier("account.code")
        alert.accessoryView = code
        alert.addButton(withTitle: "Log In")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = code
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        let entered = code.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            if workos {
                switch try await auth.workosMagicVerify(code: entered, pending: pending) {
                case .loggedIn:
                    forgetShown()
                    await look()
                case .needOrg(let orgPending, let orgs):
                    await pickWorkOSOrg(pending: orgPending, organizations: orgs)
                case .verifyEmail(let nextPending):
                    await askCode(address: address, pending: nextPending, workos: true)
                }
            } else {
                try await auth.emailVerify(code: entered, pending: pending)
                forgetShown()
                await look()
            }
        } catch {
            say(error)
            // A wrong code leaves the code good for its other tries; a dead one says so and stops.
            if case AppAuth.Failure.said(let words)? = error as? AppAuth.Failure, words.contains("tries left") {
                await askCode(address: address, pending: pending, workos: workos)
            }
        }
    }

    @MainActor func loginWithPasskey() async {
        do {
            let challenge = try await auth.passkeyChallenge()
            let anchor = NSApp?.keyWindow ?? NSApp?.mainWindow
            let assertion = try await PasskeyAsk(anchor: anchor).assert(challenge)
            _ = try await auth.passkeyLogin(challenge, assertion)
            forgetShown()
            await look()
        } catch {
            if case AppAuth.Failure.said(let msg) = error, msg.hasPrefix("No passkey was used") { return }
            say(error)
        }
    }

    /// Handles universal links matched by apple-app-site-association:
    /// /login#c=..., /login#link=..., /projects/...#invite=...
    @discardableResult
    func handleUniversalLink(_ url: URL) -> Bool {
        guard let host = url.host?.lowercased(),
              host == "openagents.com" || host == "staging.openagents.com" else { return false }
        let path = url.path
        let fragment = url.fragment ?? ""
        if path == "/login" && (fragment.hasPrefix("c=") || fragment.hasPrefix("link=")) {
            Task { @MainActor in
                NSApp?.activate(ignoringOtherApps: true)
                if !self.loggedIn { await self.showLogin() }
                else { await self.look() }
            }
            return true
        }
        if path.hasPrefix("/projects/") && fragment.hasPrefix("invite=") {
            Task { @MainActor in
                NSApp?.activate(ignoringOtherApps: true)
                await self.look()
            }
            return true
        }
        return false
    }

    @MainActor private func say(_ error: Error) {
        let alert = NSAlert()
        alert.messageText = (error as? LocalizedError)?.errorDescription ?? "That didn't work. Try again."
        alert.addButton(withTitle: "OK")
        alert.runModal()
        tell()
    }
}

// MARK: security notices (#13233)

extension MacAccount {
    /// What the Security sheet says (pure, for the check): the recovery, then the newest notices.
    static func securityWords(_ s: AppAuth.Security, zone: TimeZone = .current) -> String {
        s.recoveryWords(zone: zone) + "\n\nNotices\n" + s.noticeWords(most: 3, zone: zone)
    }

    /// OpenAgents ▸ Security…, and what a security notice's push opens: the account's newest
    /// notices and a waiting recovery, which this login can cancel, and Freeze.
    @MainActor func showSecurity() async {
        guard loggedIn else { return await showLogin() }
        let s: AppAuth.Security
        do { s = try await auth.security() } catch { say(error); return }
        let alert = NSAlert()
        alert.messageText = "Security"
        alert.informativeText = MacAccount.securityWords(s)
        let choices = (s.recoveryOpen ? ["Cancel Recovery"] : []) + ["Freeze Account…", "Close"]
        for c in choices { alert.addButton(withTitle: c) }
        let picked = alert.runModal().rawValue - NSApplication.ModalResponse.alertFirstButtonReturn.rawValue
        guard picked >= 0, picked < choices.count else { return }
        switch choices[picked] {
        case "Cancel Recovery":
            do { try await auth.cancelRecovery(); note(AppAuth.recoveryCancelledWords) } catch { say(error) }
        case "Freeze Account…":
            let ask = NSAlert()
            ask.messageText = AppAuth.freezeAskTitle
            ask.informativeText = AppAuth.freezeAskWords
            ask.addButton(withTitle: "Freeze")
            ask.addButton(withTitle: "Cancel")
            guard ask.runModal() == .alertFirstButtonReturn else { return }
            do { try await auth.freeze(); forgetShown(); tell(); note(AppAuth.frozenWords) } catch { say(error) }
        default: return
        }
    }
    @MainActor private func note(_ words: String) {
        let alert = NSAlert()
        alert.messageText = words
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    /// Asks Apple for this install's push token when someone is logged in, so the account's
    /// security notices reach this Mac. Apple gives a token only to a build signed with the push
    /// entitlement; a development build is signed ad hoc, gets none, and nothing is asked or shown.
    /// OPENAGENTS_PUSH=0 turns it off.
    @MainActor func registerForPush() {
        guard loggedIn, Bundle.main.bundleIdentifier != nil, ProcessInfo.processInfo.environment["OPENAGENTS_PUSH"] != "0", let app = NSApp else { return }
        if app.delegate == nil { app.delegate = MacPush.shared }
        guard app.delegate === MacPush.shared || MacPush.delegateForwards else { return }
        if UNUserNotificationCenter.current().delegate == nil {
            UNUserNotificationCenter.current().delegate = MacPush.shared
        }
        app.registerForRemoteNotifications()
    }
}

/// The Mac app's side of Apple's push service: the token goes to the server
/// (`AppAuth.pushTarget`), and a clicked notice opens the Security sheet.
final class MacPush: NSObject, NSApplicationDelegate, UNUserNotificationCenterDelegate {
    static let shared = MacPush()
    /// Set true by CoderDelegate when it forwards didRegisterForRemoteNotificationsWithDeviceToken to MacPush.shared.
    static var delegateForwards = false
    func application(_ application: NSApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        Task {
            // Once the server keeps the token for this Mac, macOS asks (once) whether notices may show.
            do { try await MacAccount.shared.auth.pushTarget(deviceToken) } catch { return }
            _ = try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound])
        }
    }
    func application(_ application: NSApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {}
    func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification) async -> UNNotificationPresentationOptions { [.banner, .list, .sound] }
    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        guard AppAuth.noticeOpens(response.notification.request.content.userInfo) else { return }
        await MacAccount.shared.showSecurity()
    }
}

// MARK: main.c's calls (account-mac.h)

@_cdecl("oa_account_start")
func oa_account_start(_ changed: (@convention(c) () -> Void)?) {
    MacAccount.shared.changed = { changed?() }
    Task.detached { await MacAccount.shared.look() }
}

@_cdecl("oa_account_login")
func oa_account_login() {
    Task { @MainActor in await MacAccount.shared.showLogin() }
}

@_cdecl("oa_account_logout")
func oa_account_logout() {
    Task.detached { await MacAccount.shared.logout() }
}

@_cdecl("oa_account_security")
func oa_account_security() {
    Task { @MainActor in await MacAccount.shared.showSecurity() }
}

@_cdecl("oa_account_logged_in")
func oa_account_logged_in() -> Int32 { MacAccount.shared.loggedIn ? 1 : 0 }

@_cdecl("oa_account_title")
func oa_account_title(_ out: UnsafeMutablePointer<CChar>?, _ cap: Int32) -> Int32 {
    copyOut(MacAccount.shared.title, out, cap)
}

private func copyOut(_ s: String, _ out: UnsafeMutablePointer<CChar>?, _ cap: Int32) -> Int32 {
    guard let out, cap > 0 else { return 0 }
    let bytes = Array(s.utf8.prefix(Int(cap) - 1))
    for (i, b) in bytes.enumerated() { out[i] = CChar(bitPattern: b) }
    out[bytes.count] = 0
    return Int32(bytes.count)
}

/// coax drive's account operations (main.c's on_drive), for checks against staging without a sheet:
///   account state     logged-in 0|1 and the menu's title
///   account key       log in with this Mac's key (made first when there is none)
///   account refresh   mark the access token stale, then ask who is logged in (a refresh, with its proof)
///   account logout    log out
///   account security  what the Security sheet says: the recovery, then the newest notices
@_cdecl("oa_account_drive")
func oa_account_drive(_ request: UnsafePointer<CChar>?, _ out: UnsafeMutablePointer<CChar>?, _ cap: Int32) -> Int32 {
    let op = request.map { String(cString: $0) } ?? ""
    let me = MacAccount.shared
    let done = DispatchSemaphore(value: 0)
    let said = SaidBox()
    Task.detached {
        do {
            switch op {
            case "refresh":
                guard var t = me.store.load() else { throw AppAuth.Failure.said("Log in first.") }
                t.accessUntil = Date(timeIntervalSince1970: 0); me.store.save(t)
                await me.look()
            case "logout": await me.logout()
            case "security": said.set("account: security\n" + MacAccount.securityWords(try await me.auth.security()) + "\n")
            case "state": break
            default: said.set("account: state | refresh | logout | security\n")
            }
        } catch { said.set("account: \((error as? LocalizedError)?.errorDescription ?? "\(error)")\n") }
        done.signal()
    }
    // The answer waits for the server, without holding the main thread's own work (the requests run elsewhere).
    while done.wait(timeout: .now() + 0.05) == .timedOut { RunLoop.current.run(until: Date().addingTimeInterval(0.05)) }
    let text = said.get() ?? "account: logged-in \(me.loggedIn ? 1 : 0) \(me.title)\n"
    return copyOut(text, out, cap)
}

private final class SaidBox: @unchecked Sendable {
    private let lock = NSLock(); private var text: String?
    func set(_ s: String) { lock.lock(); text = s; lock.unlock() }
    func get() -> String? { lock.lock(); defer { lock.unlock() }; return text }
}
