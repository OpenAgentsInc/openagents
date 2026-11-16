// Firm reason: 6 (controlled SDK input/storage/transport tests using the real compiled Coax codecs).
import XCTest
import Foundation

@MainActor private final class ProviderMemory: AppIdentityPendingStore {
    var values: [String: Data] = [:]
    var accepts = true
    func load(_ account: String) -> Data? { values[account] }
    func save(_ bytes: Data, account: String) -> Bool { guard accepts, values[account] == nil else { return false }; values[account] = bytes; return true }
    func remove(_ account: String) { values[account] = nil }
}
private final class ProviderTokens: TokenStore, @unchecked Sendable {
    func load() -> AppAuth.Tokens? { nil }
    func save(_ tokens: AppAuth.Tokens) {}
    func clear() {}
}
@MainActor private final class ProviderPeer: AppDeviceService {
    let host = String(repeating: "a", count: 64)
    var sent: [Data] = []
    var state: AppCompletion.State = .done
    var malformed = false
    var workspace = false
    var error = ""
    var before: (() -> Void)?
    func list() -> String { host + "\tFixture\n" }
    func pair(_ link: String, done: @escaping (Bool, String) -> Void) { done(false, "") }
    func call(host: String, body: Data, done: @escaping (AppCompletion.State, Data) -> Void) {
        before?(); sent.append(body)
        let q = (try? JSONSerialization.jsonObject(with: body)) as? [String: Any] ?? [:]
        let v: [String: Any] = ["schema": "openagents.environment.accounts.view.v1", "host_pubkey": host, "method": q["method"] ?? "", "command_id": q["command_id"] ?? "", "accounts": [], "login": ["login_id": "", "account_id": "", "state": "", "authorization_url": "", "expires_at_ms": 0], "error": error, "retry_after_ms": 0]
        if workspace {
            let view: [String: Any] = ["schema":"openagents.environment.workspace.v1","host":host,"method":q["method"] ?? "","command":q["command"] ?? "","workspace":q["workspace"] ?? "","revision":"","path":"","repository":"","page":0,"cursor":0,"next":0,"diff_cut":false,"branch":"","status":"","diff":"","error":"","github_error":"","worktrees":[],"branches":[],"pull_requests":[]]
            done(state, malformed ? Data("{}".utf8) : try! JSONSerialization.data(withJSONObject:view)); return
        }
        done(state, malformed ? Data("{}".utf8) : (try! JSONSerialization.data(withJSONObject: v)))
    }
}

@MainActor final class AppProviderTests: XCTestCase {
    private func auth() -> AppAuth { AppAuth(origin: "https://fixture.invalid", send: { _ in (Data(), 500) }, store: ProviderTokens(), device: nil) }
    private func request(_ method: String = "CompleteAccountLogin") -> Data {
        try! JSONSerialization.data(withJSONObject: ["schema": "openagents.environment.accounts.request.v1", "method": method, "provider": "claude-code", "account_id": "", "login_id": method == "CompleteAccountLogin" ? String(repeating: "1", count: 32) : "", "command_id": method == "ListAccounts" ? "" : String(repeating: "2", count: 32), "label": "", "code": method == "CompleteAccountLogin" ? "__native_private_code__" : "", "issued_at_ms": method == "ListAccounts" ? 0 : 1, "expires_at_ms": method == "ListAccounts" ? 0 : 300001, "refresh": false], options: [.sortedKeys])
    }
    private func run(_ sdk: AppProviderNative, _ peer: ProviderPeer, _ metadata: Data, mode: Int32 = 0) async -> (AppCompletion.State, Int32, Data) {
        await withCheckedContinuation { c in sdk.perform(mode: mode, host: peer.host, metadata: metadata) { c.resume(returning: ($0, $1, $2)) } }
    }
    func testPrivateCodeIsStoredBeforeTransportAndUnknownReopensForExactRetry() async {
        let peer = ProviderPeer(), store = ProviderMemory(), account = auth()
        let sdk = AppProviderNative(auth: account, devices: peer, store: store)
        var typed = "fixture-code#state"; sdk.input = { typed }; sdk.clearInput = { typed = "" }
        peer.before = { XCTAssertEqual(store.values.count, 1); XCTAssertTrue(typed.isEmpty) }; peer.state = .unknown
        let first = await run(sdk, peer, request())
        XCTAssertEqual(first.0, .unknown); XCTAssertTrue(first.2.isEmpty); XCTAssertEqual(peer.sent.count, 1)
        let original = peer.sent[0]
        let reopened = AppProviderNative(auth: account, devices: peer, store: store)
        let recover = await run(reopened, peer, Data(), mode: 2)
        XCTAssertEqual(recover.0, .done)
        let publicRecord = (try? JSONSerialization.jsonObject(with: recover.2)) as? [String: Any]
        XCTAssertEqual(publicRecord?["code"] as? String, "__native_private_code__")
        typed = "different-code"; reopened.input = { typed }; reopened.clearInput = { typed = "" }; peer.before = nil; peer.state = .done
        let settled = await run(reopened, peer, recover.2)
        XCTAssertEqual(settled.0, .done); XCTAssertTrue(store.values.isEmpty)
        XCTAssertEqual(peer.sent.count, 2); XCTAssertEqual(peer.sent[1], original)
        typed="fixture-code#state";peer.error="unknown"
        let unknownReceipt=await run(sdk,peer,request())
        XCTAssertEqual(unknownReceipt.0,.unknown);XCTAssertEqual(store.values.count,1)
    }
    func testMissingPrivateInputOrRefusedStorageNeverSends() async {
        let peer = ProviderPeer(), store = ProviderMemory(), sdk = AppProviderNative(auth: auth(), devices: peer, store: store)
        let missing = await run(sdk, peer, request()); XCTAssertEqual(missing.0, .failed); XCTAssertTrue(peer.sent.isEmpty)
        sdk.input = { "fixture-code#state" }; store.accepts = false
        let refused = await run(sdk, peer, request()); XCTAssertEqual(refused.1, 507); XCTAssertTrue(peer.sent.isEmpty)
    }
    func testReadDoesNotConsumeSecureInputAndMalformedWriteCannotSettle() async {
        let peer = ProviderPeer(), store = ProviderMemory(), sdk = AppProviderNative(auth: auth(), devices: peer, store: store)
        var typed = "fixture-code#state", reads = 0
        sdk.input = { reads += 1; return typed }; sdk.clearInput = { typed = "" }
        let read = await run(sdk, peer, request("ListAccounts")); XCTAssertEqual(read.0, .done); XCTAssertEqual(reads, 0); XCTAssertFalse(typed.isEmpty)
        peer.malformed = true
        let write = await run(sdk, peer, request()); XCTAssertEqual(write.0, .unknown); XCTAssertEqual(store.values.count, 1); XCTAssertTrue(write.2.isEmpty)
    }
    func testRawCodeMetadataAndAccountCallWithoutLoginAreRefused() async {
        let peer = ProviderPeer(), sdk = AppProviderNative(auth: auth(), devices: peer, store: ProviderMemory())
        var record = try! JSONSerialization.jsonObject(with: request()) as! [String: Any]; record["code"] = "private-code"
        let bad = await run(sdk, peer, try! JSONSerialization.data(withJSONObject: record)); XCTAssertEqual(bad.0, .failed)
        let noAccount = await run(sdk, peer, request("ListAccounts"), mode: 1); XCTAssertEqual(noAccount.1, 401)
        XCTAssertTrue(peer.sent.isEmpty)
    }
}

@MainActor final class AppWorkspaceTests: XCTestCase {
    private func request() -> Data {
        try! JSONSerialization.data(withJSONObject:["schema":"openagents.environment.workspace.v1","method":"CreatePullRequest","workspace":"default","branch":"","base":"main","title":"Native controls","body":String(repeating:"description\n",count:200),"number":0,"expected":String(repeating:"b",count:64),"command":String(repeating:"1",count:32),"issued_at":1,"expires_at":2,"page":0,"cursor":0])
    }
    private func run(_ sdk:AppWorkspaceNative,_ peer:ProviderPeer,_ raw:Data,mode:Int32=0) async -> (AppCompletion.State,Int32,Data) {
        await withCheckedContinuation { c in sdk.perform(mode:mode,host:peer.host,metadata:raw) { c.resume(returning:($0,$1,$2)) } }
    }
    func testDurableWorkspaceWriteCanRecoverAndRetryExactBytes() async {
        let peer=ProviderPeer(),store=ProviderMemory(),auth=AppAuth(origin:"https://fixture.invalid",send:{_ in (Data(),500)},store:ProviderTokens(),device:nil)
        peer.workspace=true;peer.state = .unknown
        let raw=request(),sdk=AppWorkspaceNative(auth:auth,devices:peer,store:store)
        peer.before={XCTAssertEqual(store.values.values.first,raw)}
        let first=await run(sdk,peer,raw);XCTAssertEqual(first.0,.unknown);XCTAssertEqual(store.values.count,1)
        let reopened=AppWorkspaceNative(auth:auth,devices:peer,store:store)
        let recovered=await run(reopened,peer,Data(),mode:2);XCTAssertEqual(recovered.2,raw)
        peer.state = .done
        let retry=await run(reopened,peer,recovered.2);XCTAssertEqual(retry.0,.done);XCTAssertTrue(store.values.isEmpty)
        XCTAssertEqual(peer.sent,[raw,raw])
    }
    func testWorkspaceRefusesStorageFailureAndMalformedRepliesRetainIntent() async {
        let peer=ProviderPeer(),store=ProviderMemory(),auth=AppAuth(origin:"https://fixture.invalid",send:{_ in (Data(),500)},store:ProviderTokens(),device:nil)
        peer.workspace=true;store.accepts=false
        let sdk=AppWorkspaceNative(auth:auth,devices:peer,store:store),raw=request()
        let refused=await run(sdk,peer,raw);XCTAssertEqual(refused.1,507);XCTAssertTrue(peer.sent.isEmpty)
        store.accepts=true;peer.malformed=true
        let unknown=await run(sdk,peer,raw);XCTAssertEqual(unknown.0,.unknown);XCTAssertEqual(store.values.count,1)
        var other=try! JSONSerialization.jsonObject(with:raw) as! [String:Any];other["command"]=String(repeating:"2",count:32)
        let blocked=await run(sdk,peer,try! JSONSerialization.data(withJSONObject:other));XCTAssertEqual(blocked.1,409);XCTAssertEqual(peer.sent.count,1)
    }
}
