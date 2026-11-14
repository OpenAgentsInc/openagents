// Firm reason: 6 (JavaScriptCore/UIKit and URLSession SDK oracle).
import XCTest
import UIKit
@testable import OpenAgents

@MainActor final class NativeProjectTests:XCTestCase {
    func testDraftReviewUsesNativeFieldsAndDoesNotOpenTheBrowser()throws {
        let p=try NativeProgram(directory:try XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil)),module:"app",diagnostic:{print("Project program:",$0)})
        func answer(_ op:Int,_ value:Any,status:Int=200)throws {let raw:String;if let text=value as? String {raw=text}else {raw=String(decoding:try JSONSerialization.data(withJSONObject:value),as:UTF8.self)};try p.event(100+op,status,raw);try p.frame()}
        try p.event(15,0,"");try p.frame();_ = try p.action();try answer(1,["loggedIn":true]);_ = try p.action();try answer(4,["schema":"openagents.chat.history.v1","chats":[]]);try p.input("nav.toggle");try p.frame();_ = try p.action();try answer(4,["schema":"openagents.environment.catalog.v1alpha1","environments":[]]);try p.input("nav.projects");try p.frame();_ = try p.action();try answer(4,"project mobile-kit Mobile Kit\n");try p.input("projects.new");try p.frame();XCTAssertEqual(try p.action().0,4)
        try answer(4,["may":true]);_ = try p.action();try answer(4,["drops":[["id":"d_fixture","status":"open","files":1,"project":NSNull()]]]);try p.input("project.resume.0");try p.frame();_ = try p.action();try answer(4,["files":[]]);_ = try p.action()
        let long=String(repeating:"A long design. ",count:200)
        try answer(4,["status":"ready","draft":["design":["markdown":long],"card":["name":"Native"]]])
        let row=try XCTUnwrap(p.elements().first {$0.id.hasPrefix("project.field.") && $0.label.contains("design markdown")})
        try p.input(row.id);try p.frame();let field=try XCTUnwrap(p.elements().first {$0.id=="project.edit"});XCTAssertEqual(field.value,long)
        let surface=NativeControls();surface.render(p.elements());XCTAssertEqual((surface.controls["project.edit"] as? UITextView)?.text,long)
        try p.input("project.edit","Edited on iPhone");try p.frame();try p.input("project.field.save");try p.frame();let save=try p.action();XCTAssertEqual(save.0,4);XCTAssertTrue(save.1.contains("draft/edit"));XCTAssertTrue(save.1.contains("Edited on iPhone"));let request=try XCTUnwrap(JSONSerialization.jsonObject(with:Data(save.1.utf8)) as? [String:Any]);let body=try XCTUnwrap(request["body"] as? String);XCTAssertNoThrow(try JSONSerialization.jsonObject(with:Data(body.utf8)))
    }
    func testDocumentLeaseIsBoundedAndForgettable()throws {
        let docs=NativeDocuments(),reply=try docs.stage(name:"notes.md",data:Data("Hello".utf8)),token=try XCTUnwrap(reply["token"] as? String)
        XCTAssertEqual(reply["bytes"] as? Int,5);XCTAssertEqual(docs.data(token),Data("Hello".utf8));docs.release();XCTAssertNil(docs.data(token))
        XCTAssertThrowsError(try docs.stage(name:"empty",data:Data()))
    }
    func testUploadUsesCredentialFreeNoRedirectSession()async throws {
        let config=URLSessionConfiguration.ephemeral;config.protocolClasses=[ProjectUploadOracle.self]
        let transport=NativeWorldSession(configuration:config);defer {transport.close()}
        let code=try await NativeDocuments.put(Data("Hello".utf8),url:"https://storage.googleapis.com/upload/fixture",session:transport.session)
        XCTAssertEqual(code,200);XCTAssertEqual(ProjectUploadOracle.method,"PUT");XCTAssertNil(ProjectUploadOracle.authorization);XCTAssertNil(ProjectUploadOracle.cookie)
    }
}
private final class ProjectUploadOracle:URLProtocol,@unchecked Sendable {
    static var method:String?,authorization:String?,cookie:String?
    override class func canInit(with request:URLRequest)->Bool {true}
    override class func canonicalRequest(for request:URLRequest)->URLRequest {request}
    override func startLoading() {
        Self.method=request.httpMethod;Self.authorization=request.value(forHTTPHeaderField:"Authorization");Self.cookie=request.value(forHTTPHeaderField:"Cookie")
        client?.urlProtocol(self,didReceive:HTTPURLResponse(url:request.url!,statusCode:200,httpVersion:nil,headerFields:nil)!,cacheStoragePolicy:.notAllowed);client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}
