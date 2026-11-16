// Firm reason: 6 (shared AuthenticationServices ceremony adapter, moved from SignInScreens.swift).
import AuthenticationServices

/// The system's passkey sheet (ASAuthorizationPlatformPublicKeyCredentialProvider) for one login:
/// the passkeys this device has for the site (apple-app-site-association's webcredentials), user
/// verification required. Its answer, or why there is none, in our words.
@MainActor
final class PasskeyAsk: NSObject, ASAuthorizationControllerDelegate, ASAuthorizationControllerPresentationContextProviding {
    private let anchor: ASPresentationAnchor?
    private var waiting: CheckedContinuation<AppAuth.PasskeyAssertion, Error>?
    init(anchor: ASPresentationAnchor?) { self.anchor = anchor }

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
        if #available(iOS 18.0, macOS 15.0, *), let prf { request.prf = .inputValues(.init(saltInput1: prf)) }
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
        if #available(iOS 18.0, macOS 15.0, *) { request.prf = .checkForSupport }
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
        var prf: Data?
        if #available(iOS 18.0, macOS 15.0, *) { prf = answered?.prf.map { $0.first.withUnsafeBytes { Data($0) } } }
        let made = (authorization.credential as? ASAuthorizationPlatformPublicKeyCredentialRegistration).flatMap { r in
            var supported = false
            if #available(iOS 18.0, macOS 15.0, *) { supported = r.prf?.isSupported ?? false }
            return r.rawAttestationObject.map { Registration(attestation: $0, clientData: r.rawClientDataJSON, credentialID: r.credentialID, prfSupported: supported) }
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
