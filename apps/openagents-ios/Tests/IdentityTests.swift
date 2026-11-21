// Firm reason: 6 (checks the installed Swift SDK carrier and its real compiled Coax crypto).
import XCTest
import Foundation
#if canImport(OpenAgents)
@testable import OpenAgents
#endif

private final class IdentityNetwork:URLProtocol {
    nonisolated(unsafe) static var seen:[Data]=[]
    nonisolated(unsafe) static var reply:(URLRequest,Data)->(Int,Data,URLError.Code?)={_,_ in (500,Data(),nil)}
    override class func canInit(with request:URLRequest)->Bool {true}
    override class func canonicalRequest(for request:URLRequest)->URLRequest {request}
    override func stopLoading() {}
    override func startLoading() {
        var body=request.httpBody ?? Data()
        if let stream=request.httpBodyStream {
            stream.open();defer {stream.close()};var bytes=[UInt8](repeating:0,count:2048)
            while stream.hasBytesAvailable {let n=stream.read(&bytes,maxLength:bytes.count);if n<=0 {break};body.append(contentsOf:bytes.prefix(n))}
        }
        let captured=body
        DispatchQueue.main.async {
            if self.request.httpMethod == "POST" {Self.seen.append(captured)}
            let (status,data,error)=Self.reply(self.request,captured)
            if let error {self.client?.urlProtocol(self,didFailWithError:URLError(error));return}
            let response=HTTPURLResponse(url:self.request.url!,statusCode:status,httpVersion:"HTTP/1.1",headerFields:["Content-Type":"application/json"])!
            self.client?.urlProtocol(self,didReceive:response,cacheStoragePolicy:.notAllowed)
            self.client?.urlProtocol(self,didLoad:data);self.client?.urlProtocolDidFinishLoading(self)
        }
    }
}
private final class IdentityTokens:TokenStore,@unchecked Sendable {
    var value:AppAuth.Tokens?=AppAuth.Tokens(access:"oa_at_disposable_fixture",refresh:"oa_rt_disposable_fixture",accessUntil:Date().addingTimeInterval(3600),account:"fixture")
    func load()->AppAuth.Tokens? {value}
    func save(_ tokens:AppAuth.Tokens) {value=tokens}
    func clear() {value=nil}
}
@MainActor private final class IdentityMemory:AppIdentityPendingStore {
    var held:Data?;var accepts=true
    func load(_ account:String)->Data? {held}
    func save(_ bytes:Data,account:String)->Bool {if !accepts || held != nil {return false};held=bytes;return true}
    func remove(_ account:String) {held=nil}
}

@MainActor final class IdentityTests:XCTestCase {
    private let pub="79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
    private var metadata:Data {try! JSONSerialization.data(withJSONObject:["schema":"openagents.account.identity.v1","account":"fixture","pubkey":pub,"source":"account"])}
    private var state:Data {try! JSONSerialization.data(withJSONObject:["schema":"openagents.account.identity.state.v1","account":"fixture","pubkey":pub,"source":"account","revision":0])}
    private func auth()->AppAuth {AppAuth(origin:"https://fixture.invalid",send:{_ in (Data("{}".utf8),200)},store:IdentityTokens(),device:nil)}
    private func run(_ sdk:AppIdentityNative,_ action:String,_ key:String) async ->(AppCompletion.State,Int32,Data) {
        await withCheckedContinuation {k in sdk.perform(action:action,expected:key) {state,status,body in k.resume(returning:(state,status,body))}}
    }
    func testBackupPublishesOnlyPublicMetadataAndChecksItsScalar() async {
        IdentityNetwork.seen=[];let account=auth(),store=IdentityMemory()
        let sdk=AppIdentityNative(auth:account,protocols:[IdentityNetwork.self],store:store,proof:{},localProof:{true})
        let secret=String(repeating:"0",count:63)+"1"
        let backup=try! JSONSerialization.data(withJSONObject:["schema":"openagents.account.identity.backup.v1","pubkey":pub,"secret":secret])
        IdentityNetwork.reply={_,_ in (200,backup,nil)}
        let result=await run(sdk,"backup",pub)
        XCTAssertTrue(result.0 == .done && result.1 == 200)
        let publicResult=(try? JSONSerialization.jsonObject(with:result.2)) as? [String:Any]
        XCTAssertTrue(publicResult?["pubkey"] as? String == pub && publicResult?["secret"] == nil)
        XCTAssertTrue(sdk.backup.hasPrefix("nsec1"));sdk.clear();XCTAssertTrue(sdk.backup.isEmpty)
        let bad=try! JSONSerialization.data(withJSONObject:["schema":"openagents.account.identity.backup.v1","pubkey":pub,"secret":String(repeating:"0",count:63)+"2"])
        IdentityNetwork.reply={_,_ in (200,bad,nil)}
        let refused=await run(sdk,"backup",pub)
        XCTAssertTrue(refused.0 == .failed && sdk.backup.isEmpty && refused.2.isEmpty)
    }
    func testUnknownRestoreKeepsAndRetriesExactlyOneRequest() async {
        IdentityNetwork.seen=[];let account=auth(),store=IdentityMemory()
        let sdk=AppIdentityNative(auth:account,protocols:[IdentityNetwork.self],store:store,proof:{})
        var typed=String(repeating:"0",count:63)+"1";sdk.input={typed};sdk.clearInput={typed=""}
        let identityState=state
        IdentityNetwork.reply={q,_ in q.httpMethod == "GET" ? (200,identityState,nil):(0,Data(),.networkConnectionLost)}
        let first=await run(sdk,"restore",pub)
        XCTAssertTrue(first.0 == .unknown && store.held != nil && typed.isEmpty && first.2.isEmpty)
        let original=store.held
        let publicBody=metadata;IdentityNetwork.reply={_,_ in (200,publicBody,nil)}
        let second=await run(sdk,"retry",pub)
        XCTAssertTrue(second.0 == .done && second.1 == 200 && store.held == nil)
        XCTAssertTrue(IdentityNetwork.seen.count==2 && IdentityNetwork.seen[0]==original && IdentityNetwork.seen[1]==original)
    }
    func testDiscardSettlesUnknownWithoutResendingPrivateInput() async {
        IdentityNetwork.seen=[];let account=auth(),store=IdentityMemory()
        let sdk=AppIdentityNative(auth:account,protocols:[IdentityNetwork.self],store:store,proof:{})
        sdk.input={String(repeating:"0",count:63)+"1"}
        let identityState=state,publicBody=metadata
        IdentityNetwork.reply={q,_ in q.httpMethod == "GET" ? (200,identityState,nil):(409,Data(),nil)}
        let refused=await run(sdk,"restore",pub)
        XCTAssertTrue(refused.0 == .unknown && store.held != nil)
        IdentityNetwork.reply={_,_ in (200,publicBody,nil)}
        let settled=await run(sdk,"discard",pub)
        XCTAssertTrue(settled.0 == .done && store.held == nil && IdentityNetwork.seen.count==2)
        let cancel=(try? JSONSerialization.jsonObject(with:IdentityNetwork.seen.last ?? Data())) as? [String:Any]
        XCTAssertTrue(cancel?["schema"] as? String == "openagents.account.identity.cancel.v1" && cancel?.count == 2 && cancel?["secret"] == nil)
    }
    func testRefusedStorageAndChangedAccountNeverSendAKey() async {
        IdentityNetwork.seen=[];let account=auth(),store=IdentityMemory();store.accepts=false
        let sdk=AppIdentityNative(auth:account,protocols:[IdentityNetwork.self],store:store,proof:{})
        let identityState=state;IdentityNetwork.reply={_,_ in (200,identityState,nil)}
        let result=await run(sdk,"rotate",pub)
        XCTAssertTrue(result.0 == .failed && IdentityNetwork.seen.isEmpty)
        let changed=AppIdentityNative(auth:account,protocols:[IdentityNetwork.self],store:store,proof:{await account.logout()})
        let cancelled=await run(changed,"rotate",pub)
        XCTAssertTrue(cancelled.0 == .cancelled && IdentityNetwork.seen.isEmpty && changed.backup.isEmpty)
        let corrupt=IdentityMemory();corrupt.held=Data("not a command".utf8)
        let invalid=AppIdentityNative(auth:auth(),protocols:[IdentityNetwork.self],store:corrupt,proof:{})
        let refused=await run(invalid,"retry",pub)
        XCTAssertTrue(refused.0 == .failed && IdentityNetwork.seen.isEmpty && corrupt.held != nil)
    }
    #if os(macOS)
    func testDesktopPendingFileSurvivesUnknownAndReplaysExactBytes() async throws {
        let folder=FileManager.default.temporaryDirectory.appendingPathComponent("oa-pending-\(UUID().uuidString)")
        defer {try? FileManager.default.removeItem(at:folder)}
        let store=AppFileIdentityPending(folder:folder),account=auth()
        IdentityNetwork.seen=[]
        let identityState=state,publicIdentity=metadata
        IdentityNetwork.reply={request,_ in request.httpMethod == "GET" ? (200,identityState,nil):(200,Data(),.networkConnectionLost)}
        let first=AppIdentityNative(auth:account,protocols:[IdentityNetwork.self],store:store,proof:{})
        first.input={String(repeating:"0",count:63)+"1"}
        let uncertain=await run(first,"restore",pub)
        XCTAssertEqual(uncertain.0,.unknown)
        let kept=try XCTUnwrap(store.load("fixture"))
        XCTAssertEqual(IdentityNetwork.seen.first,kept)
        XCTAssertFalse(store.save(Data("changed".utf8),account:"fixture"))
        XCTAssertNil(store.load("../other-account"))
        IdentityNetwork.reply={_,_ in (200,publicIdentity,nil)}
        let restarted=AppIdentityNative(auth:account,protocols:[IdentityNetwork.self],store:store,proof:{})
        let replayed=await run(restarted,"retry",pub)
        XCTAssertEqual(replayed.0,.done)
        XCTAssertEqual(IdentityNetwork.seen.count,2)
        XCTAssertEqual(IdentityNetwork.seen.last,kept)
        XCTAssertNil(store.load("fixture"))
    }
    func testThreadProofEffectiveModelHonestyAndPrivateByDefaultSync() {
        let fb = IOSThreadProof.formatEffectiveBadge(
            provider: "Antigravity",
            requestedModel: "gemini-4-neon",
            effectiveModel: "gemini-4-flash",
            execMode: "cloud"
        )
        XCTAssertTrue(fb.fallback)
        XCTAssertEqual(fb.badge, "Antigravity / gemini-4-flash [cloud] (fallback from gemini-4-neon)")

        let exact = IOSThreadProof.formatEffectiveBadge(
            provider: "Psionic Metal",
            requestedModel: "inkling-2-mini",
            effectiveModel: "inkling-2-mini",
            execMode: "local"
        )
        XCTAssertFalse(exact.fallback)
        XCTAssertEqual(exact.badge, "Psionic Metal / inkling-2-mini [local]")

        UserDefaults.standard.removeObject(forKey: AppServices.traceSyncOptInKey)
        XCTAssertFalse(AppServices.traceSyncOptIn)
        XCTAssertEqual(AppServices.tracePrivacyStatus, "Local Only (Private by Default)")
        AppServices.traceSyncOptIn = true
        XCTAssertTrue(AppServices.traceSyncOptIn)
        XCTAssertEqual(AppServices.tracePrivacyStatus, "Encrypted Sync Enabled (Opt-In)")
        UserDefaults.standard.removeObject(forKey: AppServices.traceSyncOptInKey)
    }
    #endif

}
