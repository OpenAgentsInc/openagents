// Firm reason: 6 (JavaScriptCore/UIKit and URLSession SDK oracle).
import XCTest
import UIKit
@testable import OpenAgents

@MainActor final class NativeProjectTests:XCTestCase {
    func testReopenedProjectChatPinsMetadataWithoutReplacingTheSubmittedDraft()throws {
        let p=try NativeProgram(directory:XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil)),module:"app")
        let cid="cnv_0123456789abcdef0123456789abcdef",pid="prj_0123456789abcdef0123456789abcdef",null=NSNull()
        func answer(_ op:Int,_ status:Int,_ value:[String:Any])throws {try p.event(100+op,status,String(decoding:JSONSerialization.data(withJSONObject:value),as:UTF8.self));try p.frame()}
        func request(_ path:String)throws->[String:Any] {
            let action=try p.action();XCTAssertEqual(action.0,4,"Expected \(path)")
            let value=try XCTUnwrap(JSONSerialization.jsonObject(with:Data(action.1.utf8)) as? [String:Any]);XCTAssertEqual(value["path"] as? String,path);return value
        }
        try p.event(Int(cnw_enable_event()),0,"");try p.frame();_=try p.action();try answer(1,0,["loggedIn":true]);_=try p.action();try answer(4,200,["schema":"openagents.chat.history.v1","chats":[]])
        try p.input("nav.toggle");try p.frame();_=try request("/api/chat/environments/v1");try answer(4,200,["schema":"openagents.environment.catalog.v1alpha1","environments":[]])
        try p.input("nav.recent");try p.frame()
        try p.frame(elapsed:0.5);_=try request("/api/chat/conversations/v2alpha1?project_id=&cursor=")
        try answer(4,200,["schema":"openagents.chat.index.v2alpha1","items":[["id":cid,"title":"Saved Project chat","revision":1,"project_id":pid,"active_turn_id":null,"updated_at_ms":1]],"next_cursor":null])
        try p.input("nav.rich.0");try p.frame();_=try request("/api/chat/conversation/v2alpha1?id="+cid+"&after=0")
        try answer(4,200,["schema":"openagents.chat.conversation.v2alpha1","id":cid,"revision":1,"project_id":pid,"legacy_thread_id":null,"legacy_thread_revision":null,"messages":[],"active_turn_id":null,"created_at_ms":1,"updated_at_ms":1,"next_cursor":null])
        try p.input("chat.composer","Original submitted text");try p.frame();try p.input("chat.composer.send");try p.frame();_=try request("/api/projects/v1/"+pid)
        let composer=try XCTUnwrap(p.elements().first {$0.role=="composer"})
        XCTAssertTrue(try XCTUnwrap(composer.children.first {$0.id=="chat.composer"}).disabled)
        try answer(4,503,[:]);XCTAssertEqual(try p.action().0,0)
        let released=try XCTUnwrap(p.elements().first {$0.role=="composer"})
        XCTAssertFalse(try XCTUnwrap(released.children.first {$0.id=="chat.composer"}).disabled)
        XCTAssertTrue(p.nodes.values.contains {$0.text=="Could not refresh this Project. Your draft is kept. Try sending again."})
        try p.input("chat.composer.send");try p.frame();_=try request("/api/projects/v1/"+pid)
        // A text event queued by the native editor before it became disabled
        // must not alter the message whose Send has already been accepted.
        try p.input("chat.composer","Next draft");try p.frame()
        try answer(4,200,["schema":"openagents.projects.api.v1alpha1","data":["id":pid,"revision":7,"owner_account_id":"acct_one","name":"Research","description":"","instructions":["text":"Current instructions","revision":7,"updated_at_ms":1],"created_at_ms":1,"updated_at_ms":1,"counts":["knowledge":0,"conversations":1]]])
        XCTAssertEqual(try p.action().0,11);try answer(11,0,["command_id":"0123456789abcdef0123456789abcdef"])
        let http=try request("/api/chat/submit/v2alpha1"),raw=try XCTUnwrap(http["body"] as? String),submit=try XCTUnwrap(JSONSerialization.jsonObject(with:Data(raw.utf8)) as? [String:Any])
        XCTAssertEqual(submit["conversation_id"] as? String,cid);XCTAssertEqual(submit["expected_project_revision"] as? Int,7)
        XCTAssertEqual((submit["content"] as? [[String:Any]])?.first?["text"] as? String,"Original submitted text")
        let surface=NativeControls();surface.render(p.elements())
        XCTAssertEqual((surface.controls["chat.composer"] as? UITextView)?.text,"Next draft")
    }
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
        XCTAssertEqual(try p.action().0,11)
        try answer(11,["command_id":"3123456789abcdef0123456789abcdef"])
        func request(_ path:String)throws->[String:Any] {
            let action=try p.action();XCTAssertEqual(action.0,4)
            let http=try XCTUnwrap(JSONSerialization.jsonObject(with:Data(action.1.utf8)) as? [String:Any]);XCTAssertEqual(http["path"] as? String,path)
            guard let raw=http["body"] as? String else{return [:]}
            return try XCTUnwrap(JSONSerialization.jsonObject(with:Data(raw.utf8)) as? [String:Any])
        }
        let submit=try request("/api/chat/submit/v2alpha1")
        XCTAssertEqual(submit["project_id"] as? String,id);XCTAssertEqual(submit["expected_project_revision"] as? Int,3)
        XCTAssertFalse(String(describing:submit).contains("Cite my notes"))
        let cid="cnv_0123456789abcdef0123456789abcdef",tid="trn_0123456789abcdef0123456789abcdef",next="trn_1123456789abcdef0123456789abcdef",null=NSNull()
        func control(_ turn:String,_ revision:Int,_ state:String,_ stopped:Bool,_ cursor:Int)->[String:Any] {
            ["schema":"openagents.chat.control.v2alpha1","conversation_id":cid,"turn_id":turn,"conversation_revision":revision,"state":state,"stop_requested":stopped,"event_cursor":cursor]
        }
        func snapshot(_ turn:String,_ revision:Int,_ state:String,_ text:String,_ stopped:Bool=false)->[String:Any] {
            var reason:Any=null;if state=="completed" {reason="end_turn"}
            return ["schema":"openagents.chat.turn.v2alpha1","id":turn,"conversation_id":cid,"operation_id":"project-operation","state":state,"stop_requested":stopped,"stop_reason":reason,"usage":["input_tokens":0,"output_tokens":7,"cache_read_tokens":null,"cache_write_tokens":null,"reasoning_tokens":null],"selection":["model_id":"actual-model","provider_id":"actual-provider","effort":"low","route_decision_id":null,"reason":"Project context pinned"],"content":[["type":"text","text":text]],"event_cursor":3,"conversation_revision":revision,"context_snapshot_id":"ctx_0123456789abcdef0123456789abcdef","error":null,"created_at_ms":1,"updated_at_ms":2]
        }
        try answer(4,control(tid,1,"queued",false,0),status:202);_=try request("/api/chat/turn/v2alpha1?id="+tid)
        try answer(4,snapshot(tid,1,"completed","The code is violet."));_=try request("/api/chat/conversation/v2alpha1?id="+cid+"&after=0")
        let user:[String:Any]=["id":tid+".user","turn_id":tid,"role":"user","content":[["type":"text","text":"What is the code?"]],"created_at_ms":1]
        let reply:[String:Any]=["id":tid+".assistant","turn_id":tid,"role":"assistant","content":[["type":"text","text":"The code is violet."]],"created_at_ms":2]
        try answer(4,["schema":"openagents.chat.conversation.v2alpha1","id":cid,"revision":1,"project_id":id,"legacy_thread_id":null,"legacy_thread_revision":null,"messages":[user,reply],"active_turn_id":null,"created_at_ms":1,"updated_at_ms":2,"next_cursor":null])
        XCTAssertTrue(p.nodes.values.contains {$0.text=="The code is violet."});XCTAssertEqual(try p.action().0,0)
        try p.input("chat.composer","And next?");try p.frame();try press("chat.composer.send");XCTAssertEqual(try p.action().0,11)
        try answer(11,["command_id":"4123456789abcdef0123456789abcdef"]);let second=try request("/api/chat/submit/v2alpha1")
        XCTAssertEqual(second["conversation_id"] as? String,cid);XCTAssertEqual(second["expected_revision"] as? Int,1);XCTAssertEqual(second["expected_project_revision"] as? Int,3)
        try answer(4,control(next,2,"queued",false,0),status:202);_=try request("/api/chat/turn/v2alpha1?id="+next)
        try answer(4,snapshot(next,2,"running","A partial answer"));try press("chat.composer.send");XCTAssertEqual(try p.action().0,11)
        try answer(11,["command_id":"5123456789abcdef0123456789abcdef"]);let stop=try request("/api/chat/stop/v2alpha1")
        XCTAssertEqual(stop["turn_id"] as? String,next);XCTAssertEqual(stop["schema"] as? String,"openagents.chat.stop.v2alpha1")


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

// Opt in with TEST_RUNNER_OA_DURABLE_STAGING=1. Uses the existing phone signer,
// device proof and a separate in-memory token store; no production login changes.
@MainActor final class NativeDurableStagingTests:XCTestCase {
    func testLiveProjectContextStreamingStopAndReopen()async throws {
        guard ProcessInfo.processInfo.environment["OA_DURABLE_STAGING"]=="1" else {throw XCTSkip("Live staging qualification is opt in")}
        let auth=AppAuth(origin:"https://staging.openagents.com",store:AccountTokenOracle(nil))
        var evidence:[String:Any]=["origin":auth.origin,"requests":[[String:Any]]()],requests=[[String:Any]]()
        defer {evidence["requests"]=requests;let data=(try? JSONSerialization.data(withJSONObject:evidence,options:[.prettyPrinted,.sortedKeys])) ?? Data();let a=XCTAttachment(data:data,uniformTypeIdentifier:"public.json");a.name="Live native durable chat receipts";a.lifetime = .keepAlways;add(a)}
        _=try await auth.keyLogin();XCTAssertTrue(auth.loggedIn);evidence["account"]=auth.tokens?.account
        if let file=ProcessInfo.processInfo.environment["OA_DURABLE_CLI_APPROVAL_FILE"] {
            let code=try String(contentsOfFile:file,encoding:.utf8).trimmingCharacters(in:.whitespacesAndNewlines)
            var request=URLRequest(url:URL(string:auth.origin+"/api/auth/device/approve")!);request.httpMethod="POST";request.setValue(auth.origin,forHTTPHeaderField:"Origin");request.setValue("application/json",forHTTPHeaderField:"Content-Type");request.httpBody=try JSONSerialization.data(withJSONObject:["code":code])
            let authorized=try await auth.authorize(request),(_,status)=try await AppAuth.network(authorized)
            XCTAssertEqual(status,200);evidence["staging_cli_approved"]=status==200
        }
        let home=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer {try? FileManager.default.removeItem(at:home)}
        let host=NativeHost(updates:try NativeUpdates(home:home),auth:auth),directory=try XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil))
        var turn="",conversation="",state="",partial=false
        func http(_ path:String)async throws->[String:Any] {
            let result=try await host.call("http",["path":path]),reply=try XCTUnwrap(result as? [String:Any]);XCTAssertEqual(reply["status"] as? Int,200)
            return try XCTUnwrap(JSONSerialization.jsonObject(with:Data((reply["body"] as? String ?? "").utf8)) as? [String:Any])
        }
        func drain(_ p:NativeProgram)async throws {
            let names=[1:"auth.state",4:"http",8:"http.cancel",11:"environment.commandIDs",12:"environment.wait",18:"account.service"]
            for _ in 0..<64 {
                let (op,raw)=try p.action();if op==0 {return}
                let name=try XCTUnwrap(names[op]),args=try XCTUnwrap(JSONSerialization.jsonObject(with:Data(raw.utf8)) as? [String:Any])
                let result=try await host.call(name,args),reply=try XCTUnwrap(result as? [String:Any]);var body="",status=0
                if op==4 {
                    let path=args["path"] as? String ?? "";status=reply["status"] as? Int ?? 0;body=reply["body"] as? String ?? ""
                    requests.append(["path":path,"status":status]);guard (200..<300).contains(status) else {throw NativeFailure.refused("Staging request refused: \(path), HTTP \(status)")}
                    if let value=(try? JSONSerialization.jsonObject(with:Data(body.utf8))) as? [String:Any] {
                        if path=="/api/chat/submit/v2alpha1" {turn=value["turn_id"] as? String ?? "";conversation=value["conversation_id"] as? String ?? "";state=value["state"] as? String ?? "";evidence["turn"]=turn;evidence["conversation"]=conversation}
                        if path.hasPrefix("/api/chat/events/v2alpha1") || path.hasPrefix("/api/chat/turn/v2alpha1") {state=value["state"] as? String ?? state}
                        if path.hasPrefix("/api/chat/events/v2alpha1"),state=="running",let events=value["events"] as? [[String:Any]],events.contains(where:{($0["delta"] as? String)?.isEmpty==false}) {partial=true}
                    }
                }else if op==18 {body=reply["record"] as? String ?? ""}else {body=String(decoding:try JSONSerialization.data(withJSONObject:reply),as:UTF8.self)}
                try p.event(100+op,status,body);try p.frame()
            }
            throw NativeFailure.refused("Native effect chain exceeded its bound")
        }
        func press(_ p:NativeProgram,_ id:String,_ value:String?=nil)async throws {
            guard p.nodes.values.contains(where:{$0.id==id}) else {throw NativeFailure.refused("Missing native control \(id)")};try p.input(id,value);try p.frame();try await drain(p)
        }
        func boot()async throws->NativeProgram {let p=try NativeProgram(directory:directory,module:"app");try p.event(Int(cnw_enable_event()),0,"");try p.frame();try await drain(p);return p}
        let suffix=String(UUID().uuidString.prefix(8)),code="JADE-"+suffix,title="Native staging "+suffix,p=try await boot()
        try await press(p,"nav.toggle");try await press(p,"nav.projects");try await press(p,"projects.new")
        try await press(p,"private.name",title);try await press(p,"private.text","Use the selected Project knowledge for code questions.");try await press(p,"private.create")
        try await press(p,"private.knowledge");try await press(p,"private.knowledge.title","Native verification note");try await press(p,"private.text","The verification code is "+code+".");try await press(p,"private.add.text")
        try await press(p,"private.close.edit");try await press(p,"private.chat.new")
        let question="Native verification "+suffix+": Start with the verification code from Project knowledge, then write 200 numbered notes, each with three sentences about durable chat recovery."
        try await press(p,"chat.composer",question);try await press(p,"chat.composer.send")
        let deadline=Date().addingTimeInterval(90)
        while (!partial || !p.nodes.values.contains {$0.text.contains(code)}) && Date()<deadline && !["completed","failed","unknown"].contains(state) {try await Task.sleep(for:.milliseconds(500));try p.frame(elapsed:0.5);try await drain(p)}
        XCTAssertTrue(partial,"No live running text delta reached the native consumer");evidence["running_delta"]=partial
        try await press(p,"chat.composer.send")
        while !["cancelled","completed","failed","unknown"].contains(state) && Date()<deadline {try await Task.sleep(for:.milliseconds(500));try p.frame(elapsed:0.5);try await drain(p)}
        XCTAssertEqual(state,"cancelled");evidence["terminal_state"]=state
        let saved=try await http("/api/chat/turn/v2alpha1?id="+turn),text=(saved["content"] as? [[String:Any]])?.compactMap {$0["text"] as? String}.joined() ?? ""
        XCTAssertTrue(text.contains(code));XCTAssertEqual(saved["stop_reason"] as? String,"cancelled");evidence["knowledge_used"]=text.contains(code)
        let reopened=try await boot();try await press(reopened,"nav.toggle");try await press(reopened,"nav.projects")
        let project=try XCTUnwrap(reopened.elements().first {$0.id.hasPrefix("private.item.") && $0.label.contains(title)})
        try await press(reopened,project.id);try await press(reopened,"private.chats")
        for _ in 0..<4 {try reopened.frame(elapsed:0.5);try await drain(reopened)}
        let row=try XCTUnwrap(reopened.elements().first {$0.id.hasPrefix("private.rich.") && $0.label.contains(suffix)})
        try await press(reopened,row.id)
        let after=try await http("/api/chat/turn/v2alpha1?id="+turn),afterText=(after["content"] as? [[String:Any]])?.compactMap {$0["text"] as? String}.joined() ?? ""
        XCTAssertEqual(afterText,text);XCTAssertTrue(reopened.nodes.values.contains {$0.text.contains(code)})
        let surface=NativeControls();surface.render(reopened.elements());XCTAssertTrue(surface.controls.values.contains {$0 is NativeMessage});evidence["reopened_native"]=true
        evidence["conversation"]=conversation;evidence["partial_bytes"]=text.utf8.count
        evidence["stopped_turn"]=turn
        try await press(reopened,"chat.composer","Reply with only the verification code from the selected Project knowledge.");try await press(reopened,"chat.composer.send")
        let completedDeadline=Date().addingTimeInterval(60)
        while !["completed","failed","unknown","cancelled"].contains(state) && Date()<completedDeadline {try await Task.sleep(for:.milliseconds(500));try reopened.frame(elapsed:0.5);try await drain(reopened)}
        XCTAssertEqual(state,"completed")
        let completed=try await http("/api/chat/turn/v2alpha1?id="+turn)
        XCTAssertEqual(completed["stop_reason"] as? String,"end_turn")
        XCTAssertTrue(reopened.elements().contains {$0.id==turn+".assistant" && $0.textContent.contains(code)})
        evidence["completed_turn"]=turn;evidence["completed_usage"]=completed["usage"];evidence["actual_selection"]=completed["selection"]
        try await press(reopened,"project.chat.back");try await press(reopened,"private.chats")
        for _ in 0..<4 {try reopened.frame(elapsed:0.5);try await drain(reopened)}
        let completedRow=try XCTUnwrap(reopened.elements().first {$0.id.hasPrefix("private.rich.") && $0.label.contains(suffix)})
        try await press(reopened,completedRow.id)
        XCTAssertTrue(reopened.elements().contains {$0.id==turn+".assistant" && $0.textContent.contains(code)})
        evidence["completed_reopened_native"]=true
        evidence["project_conversation"]=conversation
        try await press(reopened,"nav.toggle");try await press(reopened,"nav.recent");try await press(reopened,"nav.compose");try await press(reopened,"nav.account.new")
        let marker="ORDINARY-"+suffix;partial=false
        try await press(reopened,"chat.composer","Start with "+marker+", then write 30 numbered notes about durable chat recovery, each with two full sentences.");try await press(reopened,"chat.composer.send")
        let ordinaryDeadline=Date().addingTimeInterval(90)
        while !["completed","failed","unknown","cancelled"].contains(state) && Date()<ordinaryDeadline {try await Task.sleep(for:.milliseconds(500));try reopened.frame(elapsed:0.5);try await drain(reopened)}
        XCTAssertTrue(partial);XCTAssertEqual(state,"completed")
        let ordinary=try await http("/api/chat/conversation/v2alpha1?id="+conversation+"&after=0")
        XCTAssertTrue(ordinary["project_id"] is NSNull)
        XCTAssertTrue(reopened.elements().contains {$0.id==turn+".assistant" && $0.textContent.contains(marker)})
        evidence["ordinary_conversation"]=conversation;evidence["ordinary_turn"]=turn;evidence["ordinary_running_delta"]=partial
        let cold=try await boot();try await press(cold,"nav.toggle");try await press(cold,"nav.recent")
        for _ in 0..<4 {try cold.frame(elapsed:0.5);try await drain(cold)}
        let ordinaryRow=try XCTUnwrap(cold.elements(navigation:true).first {$0.id.hasPrefix("nav.rich.") && $0.label.contains(marker)})
        try await press(cold,ordinaryRow.id)
        XCTAssertTrue(cold.elements().contains {$0.id==turn+".assistant" && $0.textContent.contains(marker)})
        evidence["ordinary_reopened_native"]=true
    }
}
