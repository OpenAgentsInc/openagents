import AuthenticationServices
import UIKit

/// One-tap sign-in on the phone (#11980), shared by every sign-in surface
/// (the sign-in screen, the cloud chat's sign-in prompt):
///
/// - `open(_:)` shows the website's approval page for the waiting code in an
///   in-app sheet (`ASWebAuthenticationSession`). The page sends the sheet to
///   `openagents://signed-in` after Approve or Deny, which closes it; the
///   account's own poll then signs the app in. `close()` dismisses it when the
///   account changes (AppModel calls it). The code stays on screen as the
///   fallback.
/// - `appleButton(app:)` is the Sign in with Apple button: Apple's sheet with
///   a fresh nonce from the Rust core, then `AppModel.signInWithApple`, which
///   hands the identity token to the same account controller.
///
/// The nonce, the sheet's address, and the token exchange live in Rust
/// (`crates/mobile/src/client_ffi/apple_sign_in.rs`).
final class SignInSheet: NSObject {
    static let shared = SignInSheet()

    private var web: ASWebAuthenticationSession?
    private var webURL: String?
    private weak var app: AppModel?
    private var nonce: AppleNonce?

    /// Show the approval page for `verificationURL`. False when the sheet
    /// can't start, so the caller opens the browser instead.
    @discardableResult
    func open(_ verificationURL: String) -> Bool {
        if web != nil, webURL == verificationURL { return true }
        close()
        guard let target = URL(string: signInSheetUrl(verificationUrl: verificationURL)) else { return false }
        let session = ASWebAuthenticationSession(url: target, callback: .customScheme(signInSheetScheme())) { [weak self] _, _ in
            // Approve, Deny, or Close: the account's poll decides what's next.
            DispatchQueue.main.async {
                self?.web = nil
                self?.webURL = nil
            }
        }
        session.presentationContextProvider = self
        web = session
        webURL = verificationURL
        guard session.start() else {
            web = nil
            webURL = nil
            return false
        }
        return true
    }

    /// Dismiss the approval sheet, if it's showing.
    func close() {
        web?.cancel()
        web = nil
        webURL = nil
    }

    /// Apple's Sign in with Apple button, wired to `startApple(app:)`.
    static func appleButton(app: AppModel, traits: UITraitCollection) -> UIControl {
        let style: ASAuthorizationAppleIDButton.Style = traits.userInterfaceStyle == .dark ? .white : .black
        let button = ASAuthorizationAppleIDButton(authorizationButtonType: .signIn, authorizationButtonStyle: style)
        button.cornerRadius = 25
        button.accessibilityIdentifier = "account-sign-in-apple"
        button.addAction(UIAction { [weak app] _ in
            guard let app else { return }
            SignInSheet.shared.startApple(app: app)
        }, for: .touchUpInside)
        button.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            button.heightAnchor.constraint(equalToConstant: 50),
            button.widthAnchor.constraint(equalToConstant: 280),
        ])
        return button
    }

    /// Show Apple's sheet; on success the account signs in through Rust.
    func startApple(app: AppModel) {
        close()
        self.app = app
        let nonce = appleSignInNonce()
        self.nonce = nonce
        let request = ASAuthorizationAppleIDProvider().createRequest()
        request.requestedScopes = [.fullName, .email]
        request.nonce = nonce.hashed
        let controller = ASAuthorizationController(authorizationRequests: [request])
        controller.delegate = self
        controller.presentationContextProvider = self
        controller.performRequests()
    }

    private static var window: UIWindow {
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        let active = scenes.first { $0.activationState == .foregroundActive } ?? scenes.first
        return active?.windows.first { $0.isKeyWindow } ?? active?.windows.first ?? UIWindow()
    }
}

extension SignInSheet: ASWebAuthenticationPresentationContextProviding, ASAuthorizationControllerPresentationContextProviding {
    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor { Self.window }
    func presentationAnchor(for controller: ASAuthorizationController) -> ASPresentationAnchor { Self.window }
}

extension SignInSheet: ASAuthorizationControllerDelegate {
    func authorizationController(controller: ASAuthorizationController, didCompleteWithAuthorization authorization: ASAuthorization) {
        guard let credential = authorization.credential as? ASAuthorizationAppleIDCredential,
              let data = credential.identityToken, let token = String(data: data, encoding: .utf8),
              let nonce else { return }
        self.nonce = nil
        let name = credential.fullName.flatMap { PersonNameComponentsFormatter.localizedString(from: $0, style: .default) }
        app?.signInWithApple(identityToken: token, nonce: nonce.raw, name: name?.isEmpty == false ? name : nil)
    }

    func authorizationController(controller: ASAuthorizationController, didCompleteWithError error: Error) {
        // Closing Apple's sheet leaves the sign-in screen as it was.
        nonce = nil
    }
}
