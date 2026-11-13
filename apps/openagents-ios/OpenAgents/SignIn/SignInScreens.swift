import AuthenticationServices
import UIKit

/// Where sign-in codes arrive (a scan, a paste, an opened link) and the
/// screens that ask before the phone signs anything.
enum SignInRouter {
    /// Shows the approval sheet for text if it's one of our codes; false if it isn't.
    @discardableResult
    static func open(_ text: String, from presenter: UIViewController) -> Bool {
        guard let code = PhoneSignIn(text) else { return false }
        var top = presenter
        while let shown = top.presentedViewController { top = shown }
        if let sheet = top as? ApproveSignInViewController { sheet.dismiss(animated: false) }
        let sheet = ApproveSignInViewController(code: code)
        let nav = UINavigationController(rootViewController: sheet)
        nav.overrideUserInterfaceStyle = .dark
        nav.view.tintColor = .white
        top.present(nav, animated: true)
        return true
    }
}

/// Your key and the ways in: scan a code, or paste a link. Drawn in the new
/// landing's HUD (HudChrome.swift): its masthead, panels and buttons.
final class SignInViewController: WalletForm {
    private let npub = Look.label("", .monospacedSystemFont(ofSize: 13, weight: .regular), Hud.light)
    private let note = Look.label("", .preferredFont(forTextStyle: .footnote), Hud.mid)
    private lazy var account = AccountLogin(form: self)

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Log in"
        let close = HudButton("Done", "signin.close")
        close.addAction(UIAction { [weak self] _ in self?.dismiss(animated: true) }, for: .touchUpInside)
        let head = HudMasthead(title: "Log in", close: close)
        let about = Look.label("Scan the code on openagents.com/login to log in there, or the LINK A PHONE code on your project page.", .preferredFont(forTextStyle: .body), Hud.light)
        npub.accessibilityIdentifier = "signin.npub"; npub.lineBreakMode = .byCharWrapping
        npub.font = UIFontMetrics(forTextStyle: .body).scaledFont(for: .monospacedSystemFont(ofSize: 13, weight: .regular))
        note.accessibilityIdentifier = "signin.note"
        let copy = HudButton("Copy", "signin.copy")
        copy.addAction(UIAction { [weak self] _ in
            guard let self, let text = self.npub.text, text.hasPrefix("npub1") else { return }
            UIPasteboard.general.string = text
            self.note.text = "Copied."
        }, for: .touchUpInside)
        let scan = HudButton("Scan a code", "signin.scan", main: true)
        scan.addAction(UIAction { [weak self] _ in
            let scanner = ScannerViewController { code in self?.take(code) }
            self?.present(scanner, animated: true)
        }, for: .touchUpInside)
        let paste = HudButton("Paste a link", "signin.paste")
        paste.addAction(UIAction { [weak self] _ in self?.take(UIPasteboard.general.string ?? "") }, for: .touchUpInside)
        let keyNote = Look.label("This key is made on this phone and stays in its Keychain. It signs only after you approve.", .preferredFont(forTextStyle: .footnote), Hud.mid)
        let ways = HudPanel([HudHeading("Log in on a computer"), about, scan, paste, note])
        ways.column.setCustomSpacing(18, after: about)
        let key = HudPanel([HudHeading("Your key"), npub, copy, keyNote])
        // Security: the account's notices, a waiting recovery and Freeze (what a notice's push opens).
        let security = HudButton("Security", "signin.security")
        security.addAction(UIAction { [weak self] _ in self?.navigationController?.pushViewController(SecurityViewController(), animated: true) }, for: .touchUpInside)
        let secure = HudPanel([HudHeading("Security"), security])
        secure.isHidden = !AppAuth.shared.loggedIn
        let stack = Look.stack([head, account.panel, secure, ways, key], spacing: 20)
        stack.setCustomSpacing(28, after: head)
        column.addArrangedSubview(stack)
        do { npub.text = try PhoneSignIn.npub() } catch { npub.text = error.localizedDescription }
        account.show()
    }
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        navigationController?.setNavigationBarHidden(true, animated: false)
    }

    private func take(_ text: String) {
        if !SignInRouter.open(text, from: self) {
            note.text = "That isn't an openagents.com login code or project link."
        } else { note.text = "" }
    }
}

/// Security (#13233): what a security notice's push opens, and "Security" on the Log in screen.
/// It shows the account's newest notices and a waiting recovery, and this login can cancel the
/// recovery or freeze the account (`AppAuth.security`, `cancelRecovery`, `freeze`).
final class SecurityViewController: WalletForm {
    private let auth: AppAuth
    private let recovery = Look.label("", .preferredFont(forTextStyle: .body), Hud.light)
    private let cancel = HudButton("Cancel recovery", "security.cancel", main: true)
    private let notices = Look.label("", .preferredFont(forTextStyle: .footnote), Hud.light)
    private let freeze = HudButton("Freeze account", "security.freeze")
    private let result = Look.label("", .preferredFont(forTextStyle: .body), Hud.light)
    init(auth: AppAuth = .shared) { self.auth = auth; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Security"
        let close = HudButton("Done", "security.close")
        close.addAction(UIAction { [weak self] _ in self?.dismiss(animated: true) }, for: .touchUpInside)
        recovery.accessibilityIdentifier = "security.recovery"
        notices.accessibilityIdentifier = "security.notices"
        result.accessibilityIdentifier = "security.result"
        cancel.isHidden = true
        cancel.addAction(UIAction { [weak self] _ in self?.cancelRecovery() }, for: .touchUpInside)
        freeze.addAction(UIAction { [weak self] _ in self?.askFreeze() }, for: .touchUpInside)
        let frozen = Look.label("Freezing ends every login on the account, this one too, and cancels any recovery.", .preferredFont(forTextStyle: .footnote), Hud.mid)
        let stack = Look.stack([HudMasthead(title: "Security", close: close),
                                HudPanel([HudHeading("Recovery"), recovery, cancel, result], spacing: 14),
                                HudPanel([HudHeading("Notices"), notices]),
                                HudPanel([HudHeading("Freeze"), frozen, freeze])], spacing: 20)
        column.addArrangedSubview(stack)
        load()
    }
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        navigationController?.setNavigationBarHidden(true, animated: false)
    }

    private func load() {
        Task { @MainActor in
            do {
                let s = try await auth.security()
                recovery.text = s.recoveryWords()
                cancel.isHidden = !s.recoveryOpen
                notices.text = s.noticeWords()
            } catch { recovery.text = error.localizedDescription }
        }
    }
    private func cancelRecovery() {
        busy(cancel) { [weak self] in
            guard let self else { return }
            do {
                try await self.auth.cancelRecovery()
                self.result.text = AppAuth.recoveryCancelledWords
            } catch { self.result.text = error.localizedDescription }
            self.load()
        }
    }
    private func askFreeze() {
        let ask = UIAlertController(title: AppAuth.freezeAskTitle, message: AppAuth.freezeAskWords, preferredStyle: .alert)
        ask.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        ask.addAction(UIAlertAction(title: "Freeze", style: .destructive) { [weak self] _ in self?.freezeNow() })
        present(ask, animated: true)
    }
    private func freezeNow() {
        busy(freeze) { [weak self] in
            guard let self else { return }
            do {
                try await self.auth.freeze()
                self.cancel.isHidden = true; self.freeze.isHidden = true
                self.recovery.text = AppAuth.frozenWords
                self.result.text = ""
            } catch { self.result.text = error.localizedDescription }
        }
    }
}

/// "Log in to openagents.com?" with Approve and Cancel; then what happened.
final class ApproveSignInViewController: WalletForm {
    private let code: PhoneSignIn
    private let question = Look.label("", Hud.chrome(15, .headline))
    private let result = Look.label("", .preferredFont(forTextStyle: .body), Hud.light)
    private let approve = HudButton("Approve", "signin.approve", main: true)
    private let cancel = HudButton("Cancel", "signin.cancel")
    init(code: PhoneSignIn) { self.code = code; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = code.kind == .signin ? "Log in" : "Link this phone"
        question.text = code.question; question.accessibilityIdentifier = "signin.question"
        result.accessibilityIdentifier = "signin.result"
        let detail = Look.label(code.kind == .signin
            ? "Approve only if you just opened the login page yourself. The phone signs with its key and the page logs in as you."
            : "Approve only if the code came from your own project page or an invite sent to you. This phone's key becomes one of your keys for that project.",
            .preferredFont(forTextStyle: .footnote), Hud.mid)
        approve.addAction(UIAction { [weak self] _ in self?.go() }, for: .touchUpInside)
        cancel.addAction(UIAction { [weak self] _ in self?.dismiss(animated: true) }, for: .touchUpInside)
        let buttons = UIStackView(arrangedSubviews: [cancel, approve]); buttons.spacing = 12; buttons.distribution = .fillEqually
        let panel = HudPanel([question, detail, buttons, result], spacing: 18)
        let stack = Look.stack([HudMasthead(title: title ?? "", close: nil), panel], spacing: 28)
        column.addArrangedSubview(stack)
    }
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        navigationController?.setNavigationBarHidden(true, animated: false)
    }

    private func go() {
        busy(approve) { [weak self] in
            guard let self else { return }
            self.cancel.isEnabled = false
            let said = await self.code.approve()
            self.result.text = said
            UIAccessibility.post(notification: .announcement, argument: said)
            self.approve.isHidden = true
            self.cancel.setTitle("Done", for: .normal); self.cancel.accessibilityIdentifier = "signin.done"; self.cancel.isEnabled = true
        }
    }
}

/// Logging this app in to your openagents.com account (AppAuth): a passkey, an email code, or this
/// phone's key. Logged in, it says so and offers Log out.
@MainActor
final class AccountLogin {
    private weak var form: WalletForm?
    private let auth: AppAuth
    private let state = Look.label("", .preferredFont(forTextStyle: .body), Hud.light)
    private let message = Look.label("", .preferredFont(forTextStyle: .footnote), Hud.mid)
    private let email = Look.field("Email address", "login.email")
    private let code = Look.field("6-digit code", "login.code")
    private let send = HudButton("Send code", "login.send", main: true)
    private let verify = HudButton("Log in", "login.verify", main: true)
    private let again = HudButton("Send a new code", "login.again")
    private let key = HudButton("Log in with this phone's key", "login.key")
    private let passkey = HudButton("Log in with a passkey", "login.passkey", main: true)
    private var ask: PasskeyAsk?
    private let logout = HudButton("Log out", "login.logout")
    /// Logged in on a phone with no wallet: bring the account's backed-up wallet here (#13217).
    private let restorePasskey = HudButton("Bring my wallet back with my passkey", "restore.passkey", main: true)
    private let restoreCode = HudButton("Bring my wallet back with my recovery code", "restore.code")
    private var hasBackup = false
    private var pending: String?
    lazy var panel = HudPanel([HudHeading("Your account"), state, passkey, email, send, code, verify, again, key, restorePasskey, restoreCode, logout, message])

    init(form: WalletForm, auth: AppAuth = .shared) {
        self.form = form; self.auth = auth
        state.accessibilityIdentifier = "login.state"; message.accessibilityIdentifier = "login.message"
        email.keyboardType = .emailAddress; email.textContentType = .emailAddress
        email.autocapitalizationType = .none; email.autocorrectionType = .no
        code.keyboardType = .numberPad; code.textContentType = .oneTimeCode
        send.addAction(UIAction { [weak self] _ in self?.start() }, for: .touchUpInside)
        again.addAction(UIAction { [weak self] _ in self?.start() }, for: .touchUpInside)
        verify.addAction(UIAction { [weak self] _ in self?.check() }, for: .touchUpInside)
        key.addAction(UIAction { [weak self] _ in self?.withKey() }, for: .touchUpInside)
        passkey.addAction(UIAction { [weak self] _ in self?.withPasskey() }, for: .touchUpInside)
        logout.addAction(UIAction { [weak self] _ in self?.end() }, for: .touchUpInside)
        restorePasskey.addAction(UIAction { [weak self] _ in self?.bringBack() }, for: .touchUpInside)
        restoreCode.addAction(UIAction { [weak self] _ in self?.bringBackWithCode() }, for: .touchUpInside)
    }
    /// Whether this account has a wallet backup this phone could bring back (it has no wallet).
    private func lookForBackup() {
        guard auth.loggedIn, case .notFound = WalletKeychain.readSeed() else { hasBackup = false; return }
        Task { @MainActor [weak self] in
            guard let self else { return }
            self.hasBackup = (try? await WalletBackupClient().summary())?.backup != nil
            self.restorePasskey.isHidden = !self.hasBackup; self.restoreCode.isHidden = !self.hasBackup
        }
    }
    private func bringBack() {
        form?.busy(restorePasskey) { [weak self] in
            guard let self else { return }
            do {
                let flow = WalletBackupFlow.on(self.form)
                let proved = try await flow.prove()
                guard let prf = proved.prf else { throw AppAuth.Failure.said("This passkey can't open the backup on this phone. Use your recovery code.") }
                guard try await flow.restore(.passkey(credential: proved.credential, prf: prf)) else { throw AppAuth.Failure.said("This account has no wallet backup.") }
                self.message.text = "Your wallet is back on this phone."
            } catch { self.say(error) }
            self.show()
        }
    }
    private func bringBackWithCode() {
        let screen = RestoreCodeViewController(flow: WalletBackupFlow.on(form))
        screen.restored = { [weak self] in self?.message.text = "Your wallet is back on this phone."; self?.show() }
        let sheet = UINavigationController(rootViewController: screen); sheet.overrideUserInterfaceStyle = .dark
        form?.present(sheet, animated: true)
    }

    /// The panel for the current state: logged in, waiting for a code, or the ways in.
    func show() {
        let inNow = auth.loggedIn
        state.text = inNow ? "Logged in." : (pending == nil ? "Log in with a passkey, your email or this phone's key." : "We sent a code. It works for 10 minutes and 5 tries.")
        email.isHidden = inNow || pending != nil
        send.isHidden = inNow || pending != nil
        code.isHidden = inNow || pending == nil
        verify.isHidden = inNow || pending == nil
        again.isHidden = inNow || pending == nil
        key.isHidden = inNow
        passkey.isHidden = inNow || pending != nil
        logout.isHidden = !inNow
        restorePasskey.isHidden = true; restoreCode.isHidden = true
        if inNow { lookForBackup() }
    }
    private func say(_ error: Error) {
        message.text = error.localizedDescription
        UIAccessibility.post(notification: .announcement, argument: message.text)
    }

    private func start() {
        let address = (email.text ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        guard !address.isEmpty else { message.text = "Type your email address."; return }
        form?.busy(pending == nil ? send : again) { [weak self] in
            guard let self else { return }
            do { self.pending = try await self.auth.emailStart(address); self.message.text = ""; self.code.text = "" } catch { self.say(error) }
            self.show()
        }
    }
    private func check() {
        guard let pending else { return }
        let typed = (code.text ?? "").filter { $0.isNumber }
        guard typed.count == 6 else { message.text = "Type the 6 digits from the email."; return }
        form?.busy(verify) { [weak self] in
            guard let self else { return }
            do { try await self.auth.emailVerify(code: typed, pending: pending); self.pending = nil; self.message.text = "" } catch { self.say(error) }
            self.show()
        }
    }
    private func withKey() {
        form?.busy(key) { [weak self] in
            guard let self else { return }
            do { try await self.auth.keyLogin(); self.message.text = "" } catch { self.say(error) }
            self.show()
        }
    }
    private func withPasskey() {
        form?.busy(passkey) { [weak self] in
            guard let self else { return }
            do {
                let challenge = try await self.auth.passkeyChallenge()
                let ask = PasskeyAsk(anchor: self.form?.view.window)
                self.ask = ask
                defer { self.ask = nil }
                // The same prompt evaluates the wallet backup's PRF input (#13217): on a phone with
                // no wallet, that opens the account's backup, and the wallet is back.
                let answer = try await ask.assert(challenge, prf: WalletVault.prfInput)
                try await self.auth.passkeyLogin(challenge, answer)
                let back = await WalletBackupFlow.on(self.form).restoreAfterLogin(credential: answer.credentialID, prf: ask.prfOutput)
                self.message.text = back ? "Your wallet is back on this phone." : ""
            } catch { self.say(error) }
            self.show()
        }
    }
    private func end() {
        form?.busy(logout) { [weak self] in
            guard let self else { return }
            await self.auth.logout(); self.pending = nil; self.message.text = ""
            self.show()
        }
    }
}

/// The system's passkey sheet (ASAuthorizationPlatformPublicKeyCredentialProvider) for one login:
/// the passkeys this device has for the site (apple-app-site-association's webcredentials), user
/// verification required. Its answer, or why there is none, in our words.
@MainActor
final class PasskeyAsk: NSObject, ASAuthorizationControllerDelegate, ASAuthorizationControllerPresentationContextProviding {
    private let anchor: UIWindow?
    private var waiting: CheckedContinuation<AppAuth.PasskeyAssertion, Error>?
    init(anchor: UIWindow?) { self.anchor = anchor }

    /// The PRF output of the last answer, when its request asked for one and the passkey can
    /// make it (WebAuthn's prf extension, hmac-secret). It is made inside the authenticator and
    /// stays on this phone: it opens the wallet's encrypted backup and is never sent anywhere.
    private(set) var prfOutput: Data?
    /// A new passkey's answer.
    struct Registration { let attestation: Data; let clientData: Data; let credentialID: Data; let prfSupported: Bool }
    private var registering: CheckedContinuation<Registration, Error>?

    /// prf: the PRF input to evaluate with the same prompt (the wallet backup's, WalletVault.prfInput).
    func assert(_ c: AppAuth.PasskeyChallenge, prf: Data? = nil) async throws -> AppAuth.PasskeyAssertion {
        let request = ASAuthorizationPlatformPublicKeyCredentialProvider(relyingPartyIdentifier: c.rpID).createCredentialAssertionRequest(challenge: c.challenge)
        request.userVerificationPreference = .required
        if let prf { request.prf = .inputValues(.init(saltInput1: prf)) }
        prfOutput = nil
        let controller = ASAuthorizationController(authorizationRequests: [request])
        controller.delegate = self; controller.presentationContextProvider = self
        return try await withCheckedThrowingContinuation { k in waiting = k; controller.performRequests() }
    }
    /// Makes a passkey for this account on this phone (synced by the person's password manager),
    /// asking whether it can do PRF.
    func register(_ c: AppAuth.PasskeyNew) async throws -> Registration {
        let request = ASAuthorizationPlatformPublicKeyCredentialProvider(relyingPartyIdentifier: c.rpID)
            .createCredentialRegistrationRequest(challenge: c.challenge, name: c.userName, userID: c.userID)
        request.userVerificationPreference = .required
        request.prf = .checkForSupport
        let controller = ASAuthorizationController(authorizationRequests: [request])
        controller.delegate = self; controller.presentationContextProvider = self
        return try await withCheckedThrowingContinuation { k in registering = k; controller.performRequests() }
    }
    nonisolated func presentationAnchor(for controller: ASAuthorizationController) -> ASPresentationAnchor {
        MainActor.assumeIsolated { anchor ?? ASPresentationAnchor() }
    }
    nonisolated func authorizationController(controller: ASAuthorizationController, didCompleteWithAuthorization authorization: ASAuthorization) {
        let answered = authorization.credential as? ASAuthorizationPlatformPublicKeyCredentialAssertion
        let got = answered.map {
            AppAuth.PasskeyAssertion(credentialID: $0.credentialID, authenticatorData: $0.rawAuthenticatorData, clientData: $0.rawClientDataJSON,
                                     signature: $0.signature, userHandle: $0.userID)
        }
        let prf = answered?.prf.map { $0.first.withUnsafeBytes { Data($0) } }
        let made = (authorization.credential as? ASAuthorizationPlatformPublicKeyCredentialRegistration).flatMap { r in
            r.rawAttestationObject.map { Registration(attestation: $0, clientData: r.rawClientDataJSON, credentialID: r.credentialID, prfSupported: r.prf?.isSupported ?? false) }
        }
        MainActor.assumeIsolated {
            prfOutput = prf
            if let k = registering {
                registering = nil
                if let made { k.resume(returning: made) } else { k.resume(throwing: AppAuth.Failure.said("The passkey wasn't made. Try again.")) }
                return
            }
            if let got { waiting?.resume(returning: got) } else { waiting?.resume(throwing: AppAuth.Failure.said("Your passkey didn't answer. Try again.")) }
            waiting = nil
        }
    }
    nonisolated func authorizationController(controller: ASAuthorizationController, didCompleteWithError error: Error) {
        let words = PasskeyAsk.words(error)
        MainActor.assumeIsolated {
            if let k = registering { registering = nil; k.resume(throwing: AppAuth.Failure.said((error as? ASAuthorizationError)?.code == .canceled ? "No passkey was made." : "The passkey wasn't made. Try again.")); return }
            waiting?.resume(throwing: AppAuth.Failure.said(words)); waiting = nil
        }
    }
    /// Why no passkey answered, in our words.
    nonisolated static func words(_ error: Error) -> String {
        switch (error as? ASAuthorizationError)?.code {
        case .canceled: return "No passkey was used. Try again."
        case .notHandled, .failed: return "This phone has no passkey for openagents.com. Log in another way, then add one in Security."
        default: return "Your passkey didn't answer. Try again."
        }
    }
}
