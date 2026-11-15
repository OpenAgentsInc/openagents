// Firm reason: 6 (JavaScriptCore/UIKit and URLSession SDK oracle).
import XCTest
import UIKit
@testable import OpenAgents

@MainActor final class NativeProjectTests:XCTestCase {
    func testDraftReviewUsesNativeFieldsAndDoesNotOpenTheBrowser()throws {
        let p=try NativeProgram(directory:try XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil)),module:"app",diagnostic:{print("Project program:",$0)})
        func answer(_ op:Int,_ value:Any,status:Int=200)throws {let raw:String;if let text=value as? String {raw=text}else {raw=String(decoding:try JSONSerialization.data(withJSONObject:value),as:UTF8.self)};try p.event(100+op,status,raw);try p.frame()}
        try p.event(15,0,"");try p.frame();_ = try p.action();try answer(1,["loggedIn":true]);_ = try p.action();try answer(4,["schema":"openagents.chat.history.v1","chats":[]]);try p.input("nav.toggle");try p.frame();_ = try p.action();try answer(4,["schema":"openagents.environment.catalog.v1alpha1","environments":[]]);try p.input("nav.projects");try p.frame();_ = try p.action();try answer(4,"project mobile-kit Mobile Kit\n");_ = try p.action();try answer(4,["schema":"openagents.projects.api.v1alpha1","data":[],"revision":NSNull(),"next_cursor":NSNull()]);try p.input("projects.publish.new");try p.frame();XCTAssertEqual(try p.action().0,4)
        try answer(4,["may":true]);_ = try p.action();try answer(4,["drops":[["id":"d_fixture","status":"open","files":1,"project":NSNull()]]]);try p.input("project.resume.0");try p.frame();_ = try p.action();try answer(4,["files":[]]);_ = try p.action()
        let long=String(repeating:"A long design. ",count:200)
        try answer(4,["status":"ready","draft":["design":["markdown":long],"card":["name":"Native"]]])
        let row=try XCTUnwrap(p.elements().first {$0.id.hasPrefix("project.field.") && $0.label.contains("Design document")})
        try p.input(row.id);try p.frame();let field=try XCTUnwrap(p.elements().first {$0.id=="project.edit"});XCTAssertEqual(field.value,long)
        let surface=NativeControls();surface.render(p.elements());XCTAssertEqual((surface.controls["project.edit"] as? UITextView)?.text,long)
        try p.input("project.edit","Edited on iPhone");try p.frame();try p.input("project.field.save");try p.frame();let save=try p.action();XCTAssertEqual(save.0,4);XCTAssertTrue(save.1.contains("draft/edit"));XCTAssertTrue(save.1.contains("Edited on iPhone"));let request=try XCTUnwrap(JSONSerialization.jsonObject(with:Data(save.1.utf8)) as? [String:Any]);let body=try XCTUnwrap(request["body"] as? String);XCTAssertNoThrow(try JSONSerialization.jsonObject(with:Data(body.utf8)))
    }
    func testPrivateProjectInstructionsKnowledgeAndSavedChatUseTheInstalledProgram()throws {
        let p=try NativeProgram(directory:try XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil)),module:"app",diagnostic:{print("Private Project:",$0)})
        func answer(_ op:Int,_ value:Any,status:Int=200)throws {let raw=String(decoding:try JSONSerialization.data(withJSONObject:value),as:UTF8.self);
        try p.event(100+op,status,raw);
        try p.frame()}
        func press(_ id:String)throws {try p.input(id);
        try p.frame()}
        let id="prj_0123456789abcdef0123456789abcdef"
        func project(_ revision:Int,_ instructions:String)->[String:Any] {["schema":"openagents.projects.api.v1alpha1","data":["id":id,"revision":revision,"owner_account_id":"acct_one","name":"Research","description":"","instructions":["text":instructions,"revision":revision,"updated_at_ms":1],"created_at_ms":1,"updated_at_ms":1,"counts":["knowledge":0,"conversations":0]]]}
        try p.event(15,0,"");
        try p.frame();
        _ = try p.action();
        try answer(1,["loggedIn":true]);
        _ = try p.action();
        try answer(4,["schema":"openagents.chat.history.v1","chats":[]]);
        try press("nav.toggle");
        _ = try p.action();
        try answer(4,["schema":"openagents.environment.catalog.v1alpha1","environments":[]]);
        try press("nav.projects");
        _ = try p.action();
        try answer(4,[:]);
        _ = try p.action();
        try answer(4,["schema":"openagents.projects.api.v1alpha1","data":[],"next_cursor":NSNull()])
        try press("projects.new");
        try p.input("private.name","Research");
        try p.input("private.text","Be precise");
        try p.frame();
        try press("private.create");
        XCTAssertEqual(try p.action().0,11);
        try answer(11,["command_id":"0123456789abcdef0123456789abcdef"]);
        let create=try p.action();
        XCTAssertTrue(create.1.contains("/api/projects/v1"));
        XCTAssertTrue(create.1.contains("Research"));
        try answer(4,project(1,"Be precise"),status:201)
        try press("private.instructions");
        try p.input("private.text","Cite my notes");
        try p.frame();
        try press("private.save.instructions");
        _ = try p.action();
        try answer(11,["command_id":"1123456789abcdef0123456789abcdef"]);
        XCTAssertTrue(try p.action().1.contains("/instructions"));
        try answer(4,["schema":"openagents.projects.api.v1alpha1","data":["text":"Cite my notes"]]);
        _ = try p.action();
        try answer(4,project(2,"Cite my notes"))
        try press("private.knowledge");
        _ = try p.action();
        try answer(4,["schema":"openagents.projects.api.v1alpha1","data":[]]);
        try p.input("private.knowledge.title","Launch");
        try p.input("private.text","The code is violet.");
        try p.frame();
        try press("private.add.text");
        _ = try p.action();
        try answer(11,["command_id":"2123456789abcdef0123456789abcdef"]);
        let knowledge=try p.action();
        XCTAssertTrue(knowledge.1.contains("/knowledge"));
        XCTAssertTrue(knowledge.1.contains("violet"));
        try answer(4,["schema":"openagents.projects.api.v1alpha1","data":["id":"knw_0123456789abcdef0123456789abcdef"]],status:201);
        _ = try p.action();
        try answer(4,project(3,"Cite my notes"));
        _ = try p.action();
        try answer(4,["schema":"openagents.projects.api.v1alpha1","data":[["id":"knw_0123456789abcdef0123456789abcdef","title":"Launch","selected":true]]]);
        XCTAssertTrue(try p.elements().contains {$0.id=="private.source.0"})
        try press("private.close.edit");
        try press("private.chat.new");
        try p.input("chat.composer","What is the code?");
        try p.frame();
        try press("chat.composer.send");
        XCTAssertTrue(try p.action().1.contains("/api/chat/thread/v1"));
        let tid="m_00000000000000000001";
        let user:[String:Any]=["role":"user","content":"What is the code?"]
        try answer(4,["schema":"openagents.chat.thread.v1","id":tid,"revision":1,"messages":[user]]);
        XCTAssertTrue(try p.action().1.contains(id+"/chats"));
        try answer(4,project(4,"Cite my notes"));
        let complete=try p.action();
        XCTAssertTrue(complete.1.contains(id+"/complete"));
        XCTAssertFalse(complete.1.contains("Cite my notes"));
        try answer(4,["choices":[["message":["content":"The code is violet."]]]]);
        XCTAssertTrue(try p.action().1.contains("/api/chat/thread/v1?id="));
        try answer(4,["schema":"openagents.chat.thread.v1","id":tid,"revision":2,"messages":[user,["role":"assistant","content":"The code is violet."]]]);
        XCTAssertTrue(p.nodes.values.contains {$0.text=="The code is violet."});
        XCTAssertEqual(try p.action().0,0)
        try p.input("chat.composer","And next?");try p.frame();try press("chat.composer.send");_ = try p.action()
        try answer(4,["schema":"openagents.chat.thread.v1","id":tid,"revision":3,"messages":[user,["role":"assistant","content":"The code is violet."],["role":"user","content":"And next?"]]])
        let second=try p.action();XCTAssertTrue(second.1.contains(id+"/complete"));XCTAssertFalse(second.1.contains(id+"/chats"))
        try answer(4,["schema":"openagents.projects.api.v1alpha1","error":["message":"This turn is still running."]],status:409)
        XCTAssertTrue(p.nodes.values.contains {$0.text=="This turn is still running."})
        try press("chat.retry");_ = try p.action();try press("chat.composer.send");XCTAssertEqual(try p.action().0,8)
        try answer(8,[:],status:0);XCTAssertTrue(p.nodes.values.contains {$0.text.contains("Stopped waiting.")})


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
