// Firm reason: 6 (Keychain custody, system passkeys/biometrics, and the secure platform editor).
// The private side of the compiled GPU Settings screen. Coax owns admission, codecs and crypto.
// Only strict public identity-v1 metadata returns through the effect outbox.
import Foundation
import AuthenticationServices
import LocalAuthentication
import Security

@MainActor protocol AppIdentityPendingStore:AnyObject {
    func load(_ account:String)->Data?
    func save(_ bytes:Data,account:String)->Bool
    func remove(_ account:String)
}

@MainActor final class AppIdentityNative: AppIdentityService {
    private let auth: AppAuth
    private let http: AppHTTP
    var anchor: () -> ASPresentationAnchor? = { nil }
    var captured: () -> Bool = { false }
    var input: () -> String = { "" }
    var clearInput: () -> Void = {}
    private(set) var backup = ""
    private var running = false
    private var epoch = 0
    private var serial: UInt64 = 0
    private var watch:NSObjectProtocol?
    private let store:AppIdentityPendingStore
    private let proof:(() async throws -> Void)?
    private let localProof:(() async throws -> Bool)?

    init(auth: AppAuth,protocols:[AnyClass]=[],store:AppIdentityPendingStore?=nil,proof:(() async throws -> Void)?=nil,localProof:(() async throws -> Bool)?=nil) {
        self.auth = auth
        #if os(macOS)
        self.store=store ?? AppFileIdentityPending()
        #else
        self.store=store ?? KeychainIdentityPending()
        #endif
        self.proof=proof;self.localProof=localProof
        http = AppHTTP(policy: .init(origin: auth.origin, admit: { method, path, bytes in
            oi_http(method == "GET" ? 1 : method == "POST" ? 2 : 0, path, Int32(bytes)) != 0
        }), authorize: AppAccount.authorizer(auth),protocols:protocols)
        watch=NotificationCenter.default.addObserver(forName:AppAuth.didChangeSession,object:auth,queue:.main) { [weak self] _ in MainActor.assumeIsolated {self?.clear()} }
    }
    deinit {if let watch {NotificationCenter.default.removeObserver(watch)}}
    func clear() { epoch += 1; backup = ""; clearInput() }
    func hideBackup() { backup = "" }

    func perform(action: String, expected: String, done: @escaping (AppCompletion.State, Int32, Data) -> Void) {
        guard !running, auth.loggedIn else { done(.failed, 401, Data()); return }
        running = true
        let owner = auth.sessionOwner, account = auth.tokens?.account ?? "", mine = epoch
        var typed = action == "restore" ? input() : ""
        clearInput()
        Task {
            defer { running = false }
            do {
                // A metadata read provisions existing keyless sessions too. It never opens a secret.
                if action == "read" {
                    let (state, status, raw) = await exchange("GET", "/api/auth/identity/v1", Data())
                    guard auth.sessionOwner == owner else { done(.cancelled, 0, Data()); return }
                    let publicBody = sanitize(raw)
                    done(state, Int32(status == 200 && pending(account) != nil ? 202 : status), publicBody)
                    return
                }
                guard ["backup", "rotate", "restore", "retry", "discard"].contains(action), expected.utf8.count == 64 else { done(.failed, 400, Data()); return }
                // Private input is copied only in response to the person's confirmed restore action.
                defer { typed = "" }
                clearInput(); backup = ""
                if let proof {try await proof()}else {
                 let ask = PasskeyAsk(anchor: anchor())
                 if try await auth.passkeyCount() == 0 {
                    let challenge = try await auth.passkeyRegisterChallenge()
                    let made = try await ask.register(challenge)
                    guard auth.sessionOwner == owner, epoch == mine else { done(.cancelled, 0, Data()); return }
                    try await auth.passkeyRegister(challenge, attestation: made.attestation, clientData: made.clientData, prf: made.prfSupported)
                 }
                 let challenge = try await auth.passkeyStepUpChallenge()
                 let assertion = try await ask.assert(challenge)
                 guard auth.sessionOwner == owner, epoch == mine else { done(.cancelled, 0, Data()); return }
                 try await auth.passkeyStepUp(challenge, assertion)
                }
                guard auth.sessionOwner == owner, epoch == mine else { done(.cancelled, 0, Data()); return }
                if action == "backup" {
                    let verified:Bool
                    if let localProof {verified=try await localProof()}else {verified=try await LAContext().evaluatePolicy(.deviceOwnerAuthentication,localizedReason:"Show your Nostr backup key")}
                    guard verified else {done(.cancelled,0,Data());return}
                    var (state, status, raw) = await exchange("POST", "/api/auth/identity/backup/v1", Data("{}".utf8))
                    defer { raw.resetBytes(in: 0..<raw.count) }
                    guard auth.sessionOwner == owner, epoch == mine, !captured() else { done(.cancelled, 0, Data()); return }
                    var text = [CChar](repeating: 0, count: 128)
                    defer { text.withUnsafeMutableBytes { $0.initializeMemory(as: UInt8.self, repeating: 0) } }
                    let count = raw.withUnsafeBytes { bytes in oi_backup(bytes.bindMemory(to: UInt8.self).baseAddress, Int32(raw.count), expected, &text, Int32(text.count)) }
                    guard state == .done, status == 200, count > 0 else { done(state == .done ? .failed : state, Int32(status), Data()); return }
                    backup = String(cString: text)
                    // The public label must name this backup even if another device rotated meanwhile.
                    var metadata=[UInt8](repeating:0,count:512)
                    let size=oi_identity(account,expected,&metadata,Int32(metadata.count))
                    guard size>0 else {clear();done(.failed,0,Data());return}
                    done(.done,200,Data(metadata.prefix(Int(size))));return
                }
                var body = Data()
                defer { body.resetBytes(in: 0..<body.count) }
                if action == "retry" || action == "discard" {
                    guard let kept = pending(account) else { done(.failed, 409, Data()); return }
                    // A private-store read still goes through the one strict
                    // command codec before either replay or cancellation.
                    var out=[UInt8](repeating:0,count:256)
                    let size=kept.withUnsafeBytes {bytes in oi_cancel(bytes.bindMemory(to:UInt8.self).baseAddress,Int32(kept.count),&out,Int32(out.count))}
                    guard size>0 else {done(.failed,0,Data());return}
                    if action == "discard" {
                        body=Data(out.prefix(Int(size)))
                    }else {body = kept}
                } else {
                    // Never replace an unresolved operation with a new command or different key.
                    guard pending(account) == nil else { done(.unknown, 0, Data()); return }
                    var bytes = [CChar](repeating: 0, count: 1024)
                    defer { bytes.withUnsafeMutableBytes { $0.initializeMemory(as: UInt8.self, repeating: 0) } }
                    let command = UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
                    let (readState,readStatus,identityState)=await exchange("GET","/api/auth/identity/state/v1",Data())
                    guard readState == .done,readStatus == 200,auth.sessionOwner == owner,epoch == mine else {done(.failed,Int32(readStatus),Data());return}
                    let count = identityState.withUnsafeBytes { state in oi_change(action == "rotate" ? 1 : 2, command, expected,account,state.bindMemory(to:UInt8.self).baseAddress,Int32(identityState.count),typed,&bytes,Int32(bytes.count)) }
                    typed = ""
                    guard count > 0 else { done(.failed,count == -2 ? 409:count == -3 ? 503:400,Data());return }
                    body = bytes.withUnsafeBytes { Data($0.prefix(Int(count))) }
                    guard keep(body, account: account) else { done(.failed, 0, Data()); return }
                }
                guard auth.sessionOwner == owner, epoch == mine else { done(.cancelled, 0, Data()); return }
                let (state, status, raw) = await exchange("POST", action == "discard" ? "/api/auth/identity/cancel/v1":"/api/auth/identity/change/v1", body)
                guard auth.sessionOwner == owner else { done(.cancelled, 0, Data()); return }
                let publicBody=sanitize(raw)
                // A refusal or a server failure cannot settle an earlier unknown commit.
                let settled=state == .done && status == 200 && !publicBody.isEmpty
                if settled {forget(account)}
                done(settled ? .done:.unknown,Int32(status),publicBody)
            } catch {
                // No exception text or SDK request is sent to diagnostics or the component tree.
                done(.failed, 0, Data())
            }
        }
    }
    private func exchange(_ method: String, _ path: String, _ body: Data) async -> (AppCompletion.State, Int, Data) {
        serial += 1; let op = serial
        return await withCheckedContinuation { continuation in
            http.start(op: op, generation: 1, .init(method: method, path: path, body: body, limit: 4096, timeoutMs: 20000)) { outcome in
                switch outcome {
                case .answer(let status, let bytes, _): continuation.resume(returning: (.done, status, bytes))
                case .failed: continuation.resume(returning: (.failed, 0, Data()))
                case .unknown: continuation.resume(returning: (.unknown, 0, Data()))
                case .part: break
                }
            }
        }
    }
    private func sanitize(_ raw: Data) -> Data {
        var out = [UInt8](repeating: 0, count: 512)
        let count = raw.withUnsafeBytes { bytes in oi_public(bytes.bindMemory(to: UInt8.self).baseAddress, Int32(raw.count), &out, Int32(out.count)) }
        return count > 0 ? Data(out.prefix(Int(count))) : Data()
    }
    private func pending(_ account:String)->Data? {store.load(account)}
    private func keep(_ bytes:Data,account:String)->Bool {store.save(bytes,account:account)}
    private func forget(_ account:String) {store.remove(account)}
}

// Exact pending change request: device-only protected storage, not preferences or a core draft.
#if os(iOS)
@MainActor private final class KeychainIdentityPending:AppIdentityPendingStore {
    private func query(_ account: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: "com.openagents.app.identity.pending.v1", kSecAttrAccount as String: account, kSecAttrSynchronizable as String: false]
    }
    func load(_ account: String) -> Data? {
        var q = query(account); q[kSecReturnData as String] = true; q[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?; guard SecItemCopyMatching(q as CFDictionary, &item) == errSecSuccess else { return nil }; return item as? Data
    }
    func save(_ bytes: Data, account: String) -> Bool {
        var q = query(account); q[kSecValueData as String] = bytes; q[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        return SecItemAdd(q as CFDictionary, nil) == errSecSuccess
    }
    func remove(_ account: String) { SecItemDelete(query(account) as CFDictionary) }
}
#endif
