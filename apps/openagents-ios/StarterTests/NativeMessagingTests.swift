// Firm reason: 6 (native custody, Keychain and HTTP SDK integration oracle).
import XCTest
import UIKit
@testable import OpenAgents

@MainActor final class NativeMessagingTests:XCTestCase {
    // Opt-in: real API + Postgres, real relay, and the normally compiled phone
    // module. The loopback fixture provides disposable sessions only; neither
    // credentials nor decrypted payloads are attached to the test report.
    func testLiveCompiledInboxExchangesWithWebAndRecoversAfterColdLaunch()async throws {
        guard let address=ProcessInfo.processInfo.environment["OA_DM_LIVE_FIXTURE"],let fixture=URL(string:address) else {throw XCTSkip("Requires the isolated live messaging fixture")}
        let (data,_)=try await URLSession.shared.data(from:fixture.appendingPathComponent("native"))
        let value=try XCTUnwrap(JSONSerialization.jsonObject(with:data) as? [String:Any])
        let origin=try XCTUnwrap(value["origin"] as? String),tokens=try XCTUnwrap(value["tokens"] as? [String:Any]),peer=try XCTUnwrap(value["peer"] as? String)
        let text=try XCTUnwrap(value["text"] as? String),reply=try XCTUnwrap(value["reply"] as? String)
        let login=AppAuth.Tokens(access:try XCTUnwrap(tokens["access_token"] as? String),refresh:try XCTUnwrap(tokens["refresh_token"] as? String),accessUntil:Date().addingTimeInterval(3600),account:try XCTUnwrap(tokens["account"] as? String))
        func progress(_ stage:String)async throws {var q=URLRequest(url:fixture.appendingPathComponent("progress"));q.httpMethod="POST";q.httpBody=Data(stage.utf8);_=try await URLSession.shared.data(for:q)}
        let first=try LiveInbox(origin:origin,tokens:login)
        defer {first.stop()}
        try await first.open(peer:peer)
        try first.program.input("dm.composer",text);try first.program.frame()
        try await first.until("send readiness") {first.program.nodes.values.contains {$0.id=="dm.composer.send" && $0.attrs["disabled"] != "true"}}
        try first.program.input("dm.composer.send");try first.program.frame()
        try await first.until("recipient relay acceptance") {first.program.nodes.values.contains {$0.id.hasPrefix("dm.status.") && $0.attrs["aria-label"]=="Accepted by relay"}}
        try await progress("native-sent")
        try await first.until("web reply") {first.has(reply)}
        XCTAssertEqual(first.count(text),1);XCTAssertEqual(first.count(reply),1)
        first.stop();try await progress("native-received")
        let cold=try LiveInbox(origin:origin,tokens:login);defer {cold.stop()}
        try await cold.open(peer:peer);try await cold.until("cold journal convergence") {cold.has(text) && cold.has(reply)}
        XCTAssertEqual(cold.count(text),1);XCTAssertEqual(cold.count(reply),1)
        try await progress("native-cold-recovered")
    }
    @MainActor private final class LiveInbox {
        let program:NativeProgram,host:NativeHost,messaging:NativeMessagingServices,relays:NativeRelayServices
        init(origin:String,tokens:AppAuth.Tokens)throws {
            let auth=AppAuth(origin:origin,store:AccountTokenOracle(tokens),device:nil)
            host=NativeHost(updates:try NativeUpdates(home:FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)),auth:auth)
            messaging=NativeMessagingServices(auth:auth,account:host.accountServices);relays=NativeRelayServices(auth:auth)
            relays.messaging=messaging;let relay=relays
            messaging.admitCustody={ [weak relay] raw in guard let relay else {throw CancellationError()};return try await relay.authorizeCustody(raw)}
            program=try NativeProgram(directory:XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil)),module:"app")
            try program.event(Int(cnw_enable_event()),0,"");try program.frame()
        }
        func stop() {messaging.retire();relays.reset();host.accountServices.clear()}
        func count(_ text:String)->Int {program.nodes.values.filter {$0.text==text}.count}
        func has(_ text:String)->Bool {count(text)>0}
        func until(_ stage:String,_ predicate:()->Bool)async throws {
            let end=Date().addingTimeInterval(100)
            while Date()<end {
                if predicate() {return};try await step();try await Task.sleep(for:.milliseconds(25))
            }
            let note=program.nodes.values.first {$0.id=="dm.note"}?.text ?? ""
            XCTFail("Live messaging timed out during \(stage): \(note)");throw NativeFailure.refused("Live messaging timed out")
        }
        func open(peer:String)async throws {
            try await until("home") {program.nodes.values.contains {$0.id=="chat.composer"}}
            try program.input("nav.toggle");try program.frame();try program.input("nav.recent");try program.frame()
            try await until("messages navigation") {program.nodes.values.contains {$0.id=="dm.open"}}
            try program.input("dm.open");try program.frame()
            var enabled=false
            try await until("inbox setup") {
                if program.nodes.values.contains(where:{$0.id=="dm.enable"}) && !enabled {enabled=true;try? program.input("dm.enable");try? program.frame()}
                return program.nodes.values.contains {$0.id=="dm.empty" || $0.id.hasPrefix("dm.room.")}
            }
            try program.input("dm.new");try program.frame();try program.input("dm.recipient",peer);try program.frame();try program.input("dm.recipient.open");try program.frame()
        }
        func step()async throws {
            try program.frame(elapsed:0.05);let (op,raw)=try program.action();if op==0 {return}
            let args=(try? JSONSerialization.jsonObject(with:Data(raw.utf8))) as? [String:Any] ?? [:]
            let result:[String:Any]
            do {
                switch op {
                case 21:result=try await messaging.custody(raw)
                case 22:result=try await messaging.store(raw)
                case 23:result=try await relays.performSession(raw)
                default:
                    let names=[1:"auth.state",4:"http",8:"http.cancel",11:"environment.commandIDs",12:"environment.wait",18:"account.service"]
                    guard let name=names[op] else {throw NativeFailure.refused("Unexpected live integration effect \(op)")}
                    result=try await host.call(name,args) as? [String:Any] ?? [:]
                }
                let body=op==18 || op>=21 ? result["record"] as? String ?? "":op==4 ? result["body"] as? String ?? "":String(decoding:try JSONSerialization.data(withJSONObject:result),as:UTF8.self)
                try program.event(100+op,result["status"] as? Int ?? 0,body)
            }catch {try program.event(200+op,0,"{}")}
            try program.frame()
        }
    }
    private let pub="79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
    private let peer="dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"
    private let op=String(repeating:"0",count:63)+"1"
    private func json(_ value:Any)throws->String {String(decoding:try JSONSerialization.data(withJSONObject:value,options:.sortedKeys),as:UTF8.self)}
    private func request(_ action:String,payload:String="")throws->String {try json(["schema":"coax.native.nostr-custody.v1alpha1","action":action,"operation":op,"identity":pub,"payload":payload])}
    private func seal()throws->String {try json(["schema":"coax.native.nostr-seal.v1alpha1","created_at":Int(Date().timeIntervalSince1970),"participants":[peer],"content":"private native message","subject":"","reply":""])}
    private func record(_ value:[String:Any])throws->String {try XCTUnwrap(value["record"] as? String)}
    private final class Server:@unchecked Sendable {
        let pub:String;var source="linked",wrongIdentity=false,dropNextCommit=false,denySecondProbe=false,probes=0
        private let lock=NSLock();private var receipts=[String:(String,String)](),bodies=[String](),events=[String:String]()
        private let inboxes:[String:[String:Any]]
        init(_ pub:String) {self.pub=pub;let url=Bundle(for:NativeMessagingTests.self).url(forResource:"nostr-inboxes",withExtension:"json")!;inboxes=(try! JSONSerialization.jsonObject(with:Data(contentsOf:url))) as! [String:[String:Any]]}
        var requests:[String] {lock.lock();defer {lock.unlock()};return bodies}
        func envelope(_ operation:String)->String? {lock.lock();defer {lock.unlock()};guard let manifest=receipts[operation]?.1,let data=manifest.data(using:.utf8),let value=(try? JSONSerialization.jsonObject(with:data)) as? [String:Any],let id=value["self"] as? String else {return nil};return events[id]}
        func prepared(_ operation:String)->String? {lock.lock();defer {lock.unlock()};return receipts[operation]?.1}
        func send(_ request:URLRequest)async throws->(Data,Int) {try answer(request)}
        private func answer(_ request:URLRequest)throws->(Data,Int) {
            lock.lock();defer {lock.unlock()}
            let body=String(decoding:request.httpBody ?? Data(),as:UTF8.self);bodies.append(body)
            if request.url?.path=="/api/auth/identity/v1" {return (try JSONSerialization.data(withJSONObject:["schema":"openagents.account.identity.v1","account":"fixture","pubkey":pub,"source":source]),200)}
            if request.url?.path=="/api/auth/settings/v1" {return (Data(try NativeAccountCodec.write {nas_initial($0)}.utf8),200)}
            let q=(try? JSONSerialization.jsonObject(with:Data(body.utf8))) as? [String:Any] ?? [:]
            var service="store",action=q["action"] as? String ?? "",payload="",status="ok"
            if request.url?.path=="/api/auth/nostr/store/v1alpha1" {
                probes += 1;if denySecondProbe && probes==2 {return (Data("{\"error\":\"Session revoked\"}".utf8),401)}
                var items=[[String:Any]]()
                if action=="index",let filter=(q["payload"] as? String)?.data(using:.utf8),let value=(try? JSONSerialization.jsonObject(with:filter)) as? [String:Any],let authors=value["authors"] as? [String],let author=authors.first,let event=inboxes[author] {
                    items=[["id":event["id"]!,"author":author,"kind":10050,"created_at":event["created_at"]!,"pending":"0000000000000000","accepted":"0000000000000000","refused":"0000000000000000"]]
                }
                if action=="event",let id=q["payload"] as? String,let event=inboxes.values.first(where:{$0["id"] as? String==id}) {payload=String(decoding:try JSONSerialization.data(withJSONObject:event,options:.sortedKeys),as:UTF8.self)}
                else {payload=String(decoding:try JSONSerialization.data(withJSONObject:["schema":"coax.native.nostr-index.v1alpha1","revision":0,"cursor":0,"complete":true,"relays":[],"items":items]),as:UTF8.self)}
            }else if request.url?.path=="/api/auth/nostr/send/v1alpha1" {
                service="custody";action="seal";let operation=q["operation"] as? String ?? "",digest=q["digest"] as? String ?? "",copies=q["events"] as? [[String:Any]] ?? []
                guard copies.count==2,let last=copies.last,let own=last["id"] as? String else {return (Data("{}".utf8),400)}
                if let old=receipts[operation] {status=old.0==digest ? "ok":"conflict";payload=status=="ok" ? old.1:""}
                else {
                    for event in copies {events[event["id"] as! String]=String(decoding:try JSONSerialization.data(withJSONObject:event,options:.sortedKeys),as:UTF8.self)}
                    payload=String(decoding:try JSONSerialization.data(withJSONObject:["schema":"coax.native.nostr-sealed.v1alpha1","events":copies.map {$0["id"] as! String},"self":own,"queued_revision":1]),as:UTF8.self)
                    receipts[operation]=(digest,payload);if dropNextCommit {dropNextCommit=false;throw URLError(.networkConnectionLost)}
                }
            }else if request.url?.path=="/api/auth/nostr/operation/v1alpha1" {
                service="custody";let operation=q["operation"] as? String ?? "",digest=q["digest"] as? String ?? "",candidate=q["result"] as? String ?? ""
                if let old=receipts[operation] {status=old.0==digest ? "ok":"conflict";payload=status=="ok" ? old.1:""}
                else if candidate.isEmpty {status="not-ready"}
                else {receipts[operation]=(digest,candidate);payload=candidate;if dropNextCommit {dropNextCommit=false;throw URLError(.networkConnectionLost)}}
            }else if request.url?.path=="/api/auth/nostr/custody/v1alpha1" {
                service="custody";action="capabilities";payload="{\"source\":\"managed-server\"}"
            }else {return (Data("{}".utf8),404)}
            let answer:[String:Any]=["schema":"coax.native.nostr-result.v1alpha1","service":service,"action":action,"operation":q["operation"] ?? "","identity":wrongIdentity ? String(repeating:"b",count:64):pub,"status":status,"revision":0,"cursor":0,"payload":payload]
            return (try JSONSerialization.data(withJSONObject:answer),200)
        }
    }
    func testLinkedCustodyRecoversExactCiphertextAcrossSDKRestartWithoutSendingPlaintextToTheServer()async throws {
        for lostResponse in [false,true] {
        let server=Server(pub),auth=AppAuth(origin:"https://example.test",send:{try await server.send($0)},store:AccountTokenOracle(),device:nil)
        server.dropNextCommit=lostResponse
        let account=NativeAccountServices(auth:auth);account.vault.service="com.openagents.test.custody."+UUID().uuidString
        var key=Data(repeating:0,count:32);key[31]=1;defer {key.resetBytes(in:0..<key.count);try? account.vault.remove(pub)}
        try account.vault.add(key,pub:pub,account:"fixture")
        let firstSDK=NativeMessagingServices(auth:auth,account:account,send:{try await server.send($0)})
        let intent=try request("seal",payload:seal())
        do {let first=try record(await firstSDK.custody(intent));XCTAssertFalse(lostResponse);XCTAssertEqual(NativeAccountCodec.part(first,"status"),"ok")}
        catch {if !lostResponse {throw error}}
        let wrap=try XCTUnwrap(server.envelope(op));XCTAssertFalse(wrap.isEmpty);XCTAssertFalse(wrap.contains("private native message"))
        firstSDK.retire()
        let nextSDK=NativeMessagingServices(auth:auth,account:account,send:{try await server.send($0)})
        let retry=try record(await nextSDK.custody(intent));XCTAssertEqual(NativeAccountCodec.part(retry,"payload",cap:65536),server.prepared(op))
        let opened=try record(await nextSDK.custody(request("open",payload:wrap)))
        XCTAssertEqual(NativeAccountCodec.part(opened,"status"),"ok")
        XCTAssertEqual(NativeAccountCodec.part(NativeAccountCodec.part(opened,"payload",cap:65536),"content"),"private native message")
        server.probes=0;server.denySecondProbe=true
        do {_ = try await nextSDK.custody(request("open",payload:wrap));XCTFail("Revoked context exposed decrypted private content")}catch {}
        XCTAssertEqual(server.probes,2)
        XCTAssertFalse(server.requests.contains {$0.contains("private native message") || $0.contains("nsec1")})
        await auth.logout()
        do {_ = try await nextSDK.custody(intent);XCTFail("Logged-out SDK decrypted or signed")}catch {}
        }
    }
    func testManagedCustodyRefusesMismatchedResultOwnershipAndNeverFetchesBackup()async throws {
        let server=Server(pub);server.source="account"
        let auth=AppAuth(origin:"https://example.test",send:{try await server.send($0)},store:AccountTokenOracle(),device:nil),account=NativeAccountServices(auth:auth)
        let sdk=NativeMessagingServices(auth:auth,account:account,send:{try await server.send($0)})
        let value=try record(await sdk.custody(request("capabilities")));XCTAssertEqual(NativeAccountCodec.part(value,"status"),"ok")
        server.wrongIdentity=true
        do {_ = try await sdk.custody(request("capabilities"));XCTFail("Another identity's callback was accepted")}catch {}
        XCTAssertFalse(server.requests.contains {$0.contains("backup") || $0.contains("secret")})
    }
    func testAUTHAndListSigningRequireInstalledRelayObservations()async throws {
        let server=Server(pub);server.source="account"
        let auth=AppAuth(origin:"https://example.test",send:{try await server.send($0)},store:AccountTokenOracle(),device:nil),account=NativeAccountServices(auth:auth)
        let sdk=NativeMessagingServices(auth:auth,account:account,send:{try await server.send($0)})
        for action in ["relay-auth","publish-list"] {
            do {_ = try await sdk.custody(request(action,payload:"{}"));XCTFail("An updateable request manufactured live relay permission")}catch {}
        }
    }
    func testRetirementCancelsAnInFlightHTTPProviderBeforeItsCallback()async throws {
        let server=Server(pub);server.source="account"
        let started=expectation(description:"provider started"),cancelled=expectation(description:"provider cancelled")
        let send:AppAuth.Send={request in
            if request.url?.path=="/api/auth/nostr/custody/v1alpha1" {
                started.fulfill()
                do {try await Task.sleep(for:.seconds(5))}catch {cancelled.fulfill();throw error}
            }
            return try await server.send(request)
        }
        let auth=AppAuth(origin:"https://example.test",send:send,store:AccountTokenOracle(),device:nil),account=NativeAccountServices(auth:auth)
        let sdk=NativeMessagingServices(auth:auth,account:account,send:send),raw=try request("capabilities")
        let pending=Task {try await sdk.custody(raw)}
        await fulfillment(of:[started],timeout:2);sdk.retire()
        await fulfillment(of:[cancelled],timeout:2)
        do {_ = try await pending.value;XCTFail("Retired provider delivered a callback")}catch {}
    }
    func testLiveAUTHReconnectUsesTheActualChallengeAndPublicReadCannotUseItsSocket()async throws {
        // BIP-340's public test scalar, never an account/wallet key. AUTH is
        // ephemeral relay authentication; this test publishes no Nostr event.
        let server=Server(pub),auth=AppAuth(origin:"https://example.test",send:{try await server.send($0)},store:AccountTokenOracle(),device:nil)
        let account=NativeAccountServices(auth:auth);account.vault.service="com.openagents.test.relay."+UUID().uuidString
        var key=Data(repeating:0,count:32);key[31]=1;try account.vault.add(key,pub:pub,account:"fixture")
        let sdk=NativeMessagingServices(auth:auth,account:account,send:{try await server.send($0)}),relays=NativeRelayServices(auth:auth)
        defer {key.resetBytes(in:0..<key.count);try? account.vault.remove(pub);sdk.retire();relays.reset()}
        relays.messaging=sdk;sdk.admitCustody={ [weak relays] raw in guard let relays else {throw CancellationError()};return try await relays.authorizeCustody(raw)}
        let url="wss://relay.openagents.com";var previous=""
        for generation in 1...2 {
            var ack=0
            func call(_ action:String,_ payload:String="")async throws->[String:Any] {
                let raw=try json(["schema":"coax.native.relay-session.v1alpha1","action":action,"slot":0,"generation":generation,"sequence":action=="poll" || action=="close" ? ack:0,"identity":pub,"payload":payload])
                let response=try record(await relays.performSession(raw)),value=try XCTUnwrap(JSONSerialization.jsonObject(with:Data(response.utf8)) as? [String:Any])
                ack=max(ack,(value["sequence"] as? NSNumber)?.intValue ?? 0);return value
            }
            func wait(_ predicate:([Any])->Bool)async throws->[Any] {
                let deadline=Date().addingTimeInterval(15)
                while Date()<deadline {
                    let observation=try await call("poll")
                    if let payload=observation["payload"] as? String,let values=(try? JSONSerialization.jsonObject(with:Data(payload.utf8))) as? [Any],predicate(values) {return values}
                    if let kind=observation["kind"] as? String,["closed","failed","overflow"].contains(kind) {throw NativeFailure.refused("Live relay closed")}
                    try await Task.sleep(for:.milliseconds(50))
                };throw NativeFailure.refused("Live relay timed out")
            }
            _ = try await call("open",url)
            let readonly=try json(["schema":"coax.native.relay-read.v1","action":"poll","slot":0,"generation":generation,"sequence":0,"payload":""])
            do {_ = try await relays.perform(readonly);XCTFail("Public read accessed an authenticated slot")}catch {}
            let challenge=try await wait {$0.first as? String=="AUTH"},text=try XCTUnwrap(challenge[1] as? String)
            let purpose=try json(["schema":"coax.native.nostr-auth.v1alpha1","url":url,"generation":generation,"challenge":text])
            let operation=String(repeating:"0",count:63)+String(generation)
            let wire=try json(["schema":"coax.native.nostr-custody.v1alpha1","action":"relay-auth","operation":operation,"identity":pub,"payload":purpose])
            let result=try record(await sdk.custody(wire)),event=NativeAccountCodec.part(result,"payload",cap:65536),eventID=NativeAccountCodec.part(event,"id")
            XCTAssertEqual(NativeAccountCodec.part(result,"status"),"ok");XCTAssertFalse(eventID.isEmpty)
            _ = try await call("send","[\"AUTH\","+event+"]")
            let accepted=try await wait {$0.first as? String=="OK" && $0.count>=3 && $0[1] as? String==eventID}
            XCTAssertEqual(accepted[2] as? Bool,true)
            if !previous.isEmpty {XCTAssertNotEqual(previous,eventID)};previous=eventID
            _ = try await call("close")
            do {_ = try await sdk.custody(wire);XCTFail("A retired socket acquired another AUTH signature")}catch {}
        }
    }
}
