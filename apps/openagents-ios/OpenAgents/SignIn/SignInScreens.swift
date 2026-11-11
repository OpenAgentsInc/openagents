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
        let stack = Look.stack([head, account.panel, ways, key], spacing: 20)
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

/// Logging this app in to your openagents.com account (AppAuth): an email code, or this phone's
/// key. Logged in, it says so and offers Log out.
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
    private let logout = HudButton("Log out", "login.logout")
    private var pending: String?
    lazy var panel = HudPanel([HudHeading("Your account"), state, email, send, code, verify, again, key, logout, message])

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
        logout.addAction(UIAction { [weak self] _ in self?.end() }, for: .touchUpInside)
    }

    /// The panel for the current state: logged in, waiting for a code, or the ways in.
    func show() {
        let inNow = auth.loggedIn
        state.text = inNow ? "Logged in." : (pending == nil ? "Log in with your email or this phone's key." : "We sent a code. It works for 10 minutes and 5 tries.")
        email.isHidden = inNow || pending != nil
        send.isHidden = inNow || pending != nil
        code.isHidden = inNow || pending == nil
        verify.isHidden = inNow || pending == nil
        again.isHidden = inNow || pending == nil
        key.isHidden = inNow
        logout.isHidden = !inNow
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
    private func end() {
        form?.busy(logout) { [weak self] in
            guard let self else { return }
            await self.auth.logout(); self.pending = nil; self.message.text = ""
            self.show()
        }
    }
}
