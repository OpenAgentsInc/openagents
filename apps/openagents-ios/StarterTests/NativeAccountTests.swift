// Firm reason: 6 (Keychain, UIKit private fields and authenticated SDK exchange oracle).
import XCTest
import UIKit
@testable import OpenAgents

@MainActor final class NativeAccountTests:XCTestCase {
    private let pub="79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
    private let fixtureKey=String(repeating:"0",count:63)+"1" // Public test scalar, never a real account.
    func testHexAndNsecRoundTripRejectsPublicKeysAndZero()throws {
        var key=try NativeAccountCodec.key(fixtureKey);defer {key.resetBytes(in:0..<key.count)}
        XCTAssertEqual(try NativeAccountCodec.key(NativeAccountCodec.encode(key,secret:true)),key)
        XCTAssertThrowsError(try NativeAccountCodec.key(NativeAccountCodec.encode(NativeAccountCodec.publicKey(key),secret:false)))
        XCTAssertThrowsError(try NativeAccountCodec.key(String(repeating:"0",count:64)))
        XCTAssertThrowsError(try NativeAccountCodec.key("bad"))
    }
    func testAdditionalKeyInventoryDoesNotReplaceAnExistingKey()throws {
        var vault=NativeLoginKeys();vault.service="com.openagents.test.login."+UUID().uuidString
        let key=try NativeAccountCodec.key(fixtureKey);defer {try? vault.remove(pub)}
        try vault.add(key,pub:pub,account:"test");try vault.add(Data(repeating:2,count:32),pub:pub,account:"other")
        XCTAssertEqual(try vault.read(pub),key)
        XCTAssertNil(try vault.read(String(repeating:"b",count:64)))
    }
    func testPrivateInputUsesNativeBindingWithoutSendingAnUpdateableUIEvent() {
        let surface=NativeControls();var publicEdits=0,privateEdits=0
        surface.edit={_,_ in publicEdits+=1};surface.secretEdit={_,_ in privateEdits+=1}
        surface.render([NativeElement(id:"settings.secret.input",role:"field",label:"Private login key",type:"secret")])
        let field=surface.controls["settings.secret.input"] as! UITextField
        field.text="fixture";field.sendActions(for:.editingChanged)
        XCTAssertEqual(publicEdits,0);XCTAssertEqual(privateEdits,1);XCTAssertTrue(field.isSecureTextEntry)
        NotificationCenter.default.post(name:UIApplication.willResignActiveNotification,object:nil)
        XCTAssertTrue(field.text?.isEmpty != false)
    }
    func testBackupRequiresServerAdmissionAndNeverEntersPublicObservation()async throws {
        let service=try fixture(),presenter=UIViewController();var authenticated=0
        service.authenticate={authenticated+=1}
        _ = try await service.perform(["action":"snapshot"],presenter:presenter)
        let result=try await service.perform(["action":"backup"],presenter:presenter),record=try XCTUnwrap(result["record"] as? String)
        XCTAssertEqual(authenticated,1);XCTAssertFalse(record.contains(fixtureKey));XCTAssertFalse(record.contains("nsec1"));XCTAssertTrue(NativeAccountCodec.flag(record,"backup"))
        let field=UITextField();service.bindSecret(field,id:"settings.secret.nsec");XCTAssertTrue(field.isSecureTextEntry);XCTAssertTrue(field.text?.hasPrefix("nsec1")==true)
        service.clear();XCTAssertTrue(field.text?.isEmpty != false);XCTAssertNil(service.secret(id:"settings.secret.hex"))
    }
    func testFailedStrongProofCannotBecomeLocalBackupFallback()async throws {
        let service=try fixture(refuseBackup:true),presenter=UIViewController();service.authenticate={}
        _ = try await service.perform(["action":"snapshot"],presenter:presenter)
        do {_ = try await service.perform(["action":"backup"],presenter:presenter);XCTFail("An L1 session exported a key")}catch {}
        XCTAssertNil(service.secret(id:"settings.secret.hex"))
    }
    private func fixture(refuseBackup:Bool=false)throws->NativeAccountServices {
        let initial=try NativeAccountCodec.write {nas_initial($0)},pub=self.pub,key=fixtureKey
        let send:AppAuth.Send={request in
            guard request.value(forHTTPHeaderField:"Authorization")=="Bearer fixture" else {throw NativeFailure.refused("Missing authorization")}
            let path=request.url!.path,raw:String
            switch path {
            case "/api/auth/settings/v1":raw=initial
            case "/api/auth/identity/v1":raw="{\"pubkey\":\"\(pub)\",\"source\":\"account\"}"
            case "/api/auth/account":raw="{\"account\":\"fixture\",\"primary\":\"ada@example.com\"}"
            case "/api/auth/ways":raw="{\"level\":\"L2\",\"keys\":[],\"passkeys\":[]}"
            case "/api/auth/sessions":raw="{\"sessions\":[{\"id\":\"s1\",\"name\":\"iPhone\",\"this\":true}]}"
            case "/api/auth/identity/backup/v1":if refuseBackup {return (Data("{\"error\":\"Verify first\"}".utf8),401)};raw="{\"pubkey\":\"\(pub)\",\"secret\":\"\(key)\"}"
            default:throw NativeFailure.refused("Unexpected fixture request")
            };return (Data(raw.utf8),200)
        }
        let store=AccountTokenOracle(),auth=AppAuth(origin:"https://example.com",send:send,store:store,device:nil)
        return NativeAccountServices(auth:auth,send:send)
    }
}
final class AccountTokenOracle:TokenStore,@unchecked Sendable {
    private var value:AppAuth.Tokens?
    init(_ value:AppAuth.Tokens? = .init(access:"fixture",refresh:"refresh",accessUntil:Date().addingTimeInterval(3600),account:"fixture")) {self.value=value}
    func load()->AppAuth.Tokens? {value}
    func save(_ tokens:AppAuth.Tokens) {value=tokens}
    func clear() {value=nil}
}
