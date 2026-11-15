// Firm reason: 6 (JavaScriptCore and UIKit SDK integration checks).
import XCTest
import UIKit
@testable import OpenAgents

@MainActor final class NativeProgramTests:XCTestCase {
    func testDurableTurnStreamsResumesStopsAndCopiesCompleteReply()throws {
        let p=try NativeProgram(directory:XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil)),module:"app")
        let conversation="cnv_0123456789abcdef0123456789abcdef",turn="trn_0123456789abcdef0123456789abcdef",nilValue=NSNull()
        func answer(_ op:Int,_ status:Int,_ value:[String:Any])throws {
            try p.event(100+op,status,String(decoding:JSONSerialization.data(withJSONObject:value),as:UTF8.self));try p.frame()
        }
        func request(_ expected:String)throws->[String:Any] {
            let note=p.nodes.values.filter {$0.id.hasPrefix("account.note") || $0.id.hasPrefix("rich.")}.map {$0.text}.joined(separator:" | ")
            let action=try p.action();XCTAssertEqual(action.0,4,"Expected \(expected); note: \(note)")
            let body=try XCTUnwrap(JSONSerialization.jsonObject(with:Data(action.1.utf8)) as? [String:Any]);XCTAssertEqual(body["path"] as? String,expected);return body
        }
        let unknown:[String:Any]=["input_tokens":nilValue,"output_tokens":nilValue,"cache_read_tokens":nilValue,"cache_write_tokens":nilValue,"reasoning_tokens":nilValue]
        var snapshot:[String:Any]=["schema":"openagents.chat.turn.v2alpha1","id":turn,"conversation_id":conversation,"operation_id":"0123456789abcdef0123456789abcdef","state":"queued","stop_requested":false,"stop_reason":nilValue,"usage":unknown,"selection":["model_id":"actual-model","provider_id":"actual-provider","effort":"low","route_decision_id":nilValue,"reason":"Actual selection"],"content":[],"event_cursor":0,"conversation_revision":1,"context_snapshot_id":nilValue,"error":nilValue,"created_at_ms":1,"updated_at_ms":1]
        try p.event(Int(cnw_enable_event()),0,"");try p.frame();_=try p.action();try answer(1,0,["loggedIn":true]);_=try p.action();try answer(4,200,["schema":"openagents.chat.history.v1","chats":[]])
        try p.input("chat.composer","Hello");try p.frame();try p.input("chat.composer.send");try p.frame();XCTAssertEqual(try p.action().0,11)
        try answer(11,0,["command_id":"0123456789abcdef0123456789abcdef"])
        let submit=try request("/api/chat/submit/v2alpha1"),write=try XCTUnwrap(submit["body"] as? String)
        XCTAssertTrue(write.contains("openagents.chat.submit.v2alpha1"))
        try p.event(204,0,"{}");try p.frame();XCTAssertTrue(p.elements().contains {$0.id=="rich.reconcile"})
        try p.input("rich.reconcile");try p.frame();let retry=try request("/api/chat/submit/v2alpha1");XCTAssertEqual(retry["body"] as? String,write)
        let control:[String:Any]=["schema":"openagents.chat.control.v2alpha1","conversation_id":conversation,"turn_id":turn,"conversation_revision":1,"state":"queued","stop_requested":false,"event_cursor":0]
        try answer(4,202,control);_=try request("/api/chat/turn/v2alpha1?id="+turn);try answer(4,200,snapshot);_=try request("/api/chat/conversation/v2alpha1?id="+conversation+"&after=0")
        let user:[String:Any]=["id":turn+".user","turn_id":turn,"role":"user","content":[["type":"text","text":"Hello"]],"created_at_ms":1]
        let assistant:[String:Any]=["id":turn+".assistant","turn_id":turn,"role":"assistant","content":[],"created_at_ms":1]
        let history:[String:Any]=["schema":"openagents.chat.conversation.v2alpha1","id":conversation,"revision":1,"project_id":nilValue,"legacy_thread_id":nilValue,"legacy_thread_revision":nilValue,"messages":[user,assistant],"active_turn_id":turn,"created_at_ms":1,"updated_at_ms":1,"next_cursor":nilValue]
        try answer(4,200,history);XCTAssertEqual(p.nodes.values.first {$0.id=="chat.composer"}?.value,"")
        func event(_ sequence:Int,_ type:String,_ text:Any,_ index:Any)->[String:Any] {
            ["schema":"openagents.chat.event.v2alpha1","conversation_id":conversation,"turn_id":turn,"sequence":sequence,"type":type,"state":"running","stop_requested":false,"block_index":index,"delta":text,"usage":nilValue,"stop_reason":nilValue,"error":nilValue]
        }
        let page:[String:Any]=["schema":"openagents.chat.events.v2alpha1","conversation_id":conversation,"turn_id":turn,"after":0,"events":[event(1,"started",nilValue,nilValue),event(2,"text_delta","A partial answer",0)],"next_cursor":2,"has_more":false,"state":"running"]
        try p.frame(elapsed:0.5);_=try request("/api/chat/events/v2alpha1?id="+turn+"&after=0");try answer(4,200,page)
        XCTAssertTrue(p.nodes.values.contains {$0.text=="A partial answer"})
        try p.input("chat.composer.send");try p.frame();XCTAssertEqual(try p.action().0,11)
        try answer(11,0,["command_id":"1123456789abcdef0123456789abcdef"]);let stop=try request("/api/chat/stop/v2alpha1");XCTAssertTrue((stop["body"] as? String ?? "").contains(turn))
        var stopped=control;stopped["state"]="running";stopped["stop_requested"]=true;stopped["event_cursor"]=3
        try answer(4,200,stopped);_=try request("/api/chat/turn/v2alpha1?id="+turn)
        let full=String(repeating:"x",count:131071)+"!"
        snapshot["state"]="cancelled";snapshot["stop_requested"]=true;snapshot["stop_reason"]="cancelled";snapshot["content"]=[["type":"text","text":full]];snapshot["event_cursor"]=4;snapshot["updated_at_ms"]=2
        try answer(4,200,snapshot);_=try request("/api/chat/conversation/v2alpha1?id="+conversation+"&after=0")
        var saved=history;var savedAssistant=assistant;savedAssistant["content"]=[["type":"text","text":full]];saved["messages"]=[user,savedAssistant];saved["active_turn_id"]=nilValue;saved["updated_at_ms"]=2;try answer(4,200,saved)
        XCTAssertEqual(p.nodes.values.first {$0.id==turn+".assistant.plain"}?.text,full)
        let surface=NativeControls();surface.render(p.elements());surface.view.frame=CGRect(x:0,y:0,width:402,height:874);surface.view.layoutIfNeeded()
        XCTAssertNotNil(surface.controls[turn+".assistant"] as? NativeMessage)
        try p.input(turn+".assistant.copy");try p.frame();let copy=try p.action();XCTAssertEqual(copy.0,19)
        XCTAssertEqual((try JSONSerialization.jsonObject(with:Data(copy.1.utf8)) as? [String:String])?["text"],full)
        XCTAssertFalse(nt_copy_admit(1,131072));XCTAssertTrue(nt_copy_admit(2,131072));XCTAssertFalse(nt_copy_admit(2,131073))
    }
    func testCompiledCoaxOwnsNativeEmailAndCodeFields()throws {
        let directory=try XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil))
        let p=try NativeProgram(directory:directory,module:"app")
        try p.frame();XCTAssertEqual(try p.action().0,1)
        try p.event(101,0,"{\"loggedIn\":false}");try p.frame()
        XCTAssertEqual(p.elements().first {$0.id=="login.email"}?.type,"email")
        XCTAssertEqual(p.elements().first {$0.id=="login.send"}?.label,"Send code")
        try p.input("login.email","ada@example.com");try p.frame()
        try p.input("login.send");try p.frame()
        let request=try p.action();XCTAssertEqual(request.0,2)
        XCTAssertEqual((try JSONSerialization.jsonObject(with:Data(request.1.utf8)) as? [String:String])?["email"],"ada@example.com")
        try p.event(102,0,"{\"pending\":\"fixture\"}");try p.frame()
        let surface=NativeControls();surface.render(p.elements())
        let field=try XCTUnwrap(surface.controls["login.code"] as? UITextField)
        XCTAssertEqual(field.textContentType,.oneTimeCode)
        XCTAssertEqual(field.keyboardType,.numberPad)
        try p.input("login.code","415926");try p.frame();surface.render(p.elements())
        XCTAssertTrue(field === surface.controls["login.code"])
        XCTAssertEqual(field.text,"415926")
    }
    func testBadComponentFrameIsRefusedWithoutCastingOrTrapping()throws {
        let directory=try XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil))
        let p=try NativeProgram(directory:directory,module:"app")
        p.context.evaluateScript("cnFrame = () => [[1]]")
        XCTAssertThrowsError(try p.frame())
    }
    func testSharedComposerRetainsEditorAndMatchesReferenceGeometry()throws {
        let directory=try XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil))
        let p=try NativeProgram(directory:directory,module:"app")
        try p.frame();_ = try p.action();try p.event(101,0,"{\"loggedIn\":true}");try p.frame();_ = try p.action()
        try p.event(104,200,"{\"schema\":\"openagents.chat.history.v1\",\"chats\":[]}");try p.frame()
        try p.input("chat.new");try p.frame()
        let surface=NativeControls();surface.render(p.elements());surface.view.frame=CGRect(x:0,y:0,width:402,height:874);surface.view.layoutIfNeeded()
        let editor=try XCTUnwrap(surface.controls["chat.composer"] as? UITextView)
        let composer=try XCTUnwrap(surface.controls["chat.composer.frame"] as? NativeComposer)
        composer.layoutIfNeeded()
        XCTAssertEqual(composer.frame.width,378,accuracy:1);XCTAssertEqual(composer.intrinsicContentSize.height,48,accuracy:1)
        XCTAssertFalse(composer.send.isEnabled);XCTAssertEqual(editor.textContainer.lineFragmentPadding,0)
        try p.input("chat.composer","Hello");try p.frame();surface.render(p.elements());surface.view.layoutIfNeeded()
        XCTAssertTrue(editor === surface.controls["chat.composer"]);XCTAssertTrue(composer.send.isEnabled)
        editor.selectedRange=NSRange(location:2,length:1);surface.render(p.elements());XCTAssertEqual(editor.selectedRange,NSRange(location:2,length:1))
        try p.input("chat.composer","First line\nSecond line\nThird line\nFourth line\nFifth line");try p.frame();surface.render(p.elements());surface.view.layoutIfNeeded();composer.layoutIfNeeded()
        XCTAssertEqual(composer.intrinsicContentSize.height,116,accuracy:1);XCTAssertTrue(editor.isScrollEnabled)
    }

    func testEnvironmentTargetsAndUncertainCommandsUseTheGeneratedNativeProgram()throws {
        let directory=try XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil))
        let p=try NativeProgram(directory:directory,module:"app")
        func answer(_ op:Int,_ status:Int,_ value:[String:Any])throws {
            try p.event(100+op,status,String(decoding:JSONSerialization.data(withJSONObject:value),as:UTF8.self));try p.frame()
        }
        try p.frame();_ = try p.action();try answer(1,0,["loggedIn":true]);_ = try p.action()
        try answer(4,200,["schema":"openagents.chat.history.v1","chats":[]])
        try p.input("environments.open");try p.frame()
        XCTAssertEqual(try p.action().0,4)
        let host=String(repeating:"a",count:64)
        try answer(4,200,["schema":"openagents.environment.catalog.v1alpha1","environments":[["host_pubkey":host,"display_name":"Mac"]]])
        try p.input("environment.0");try p.frame();_ = try p.action()
        let view:[String:Any]=["schema":"openagents.environment.runtime.v1alpha1","host_pubkey":host,"display_name":"Mac","method":"ListSessions","sessions":[],"items":[],"error":"","complete":true,"next_cursor":"","command_id":"","command_status":"","source_epoch":""]
        try answer(4,200,view);try p.input("env.new");try p.frame()
        let surface=NativeControls();surface.render(p.elements())
        XCTAssertNotNil(surface.controls["env.composer"] as? UITextView)
        try p.input("env.composer","Hello from the phone");try p.frame();try p.input("env.composer.send");try p.frame()
        XCTAssertEqual(try p.action().0,11)
        try answer(11,0,["command_id":String(repeating:"1",count:32),"session_id":String(repeating:"2",count:32),"now_ms":1_700_000_000_000])
        let request=try p.action();XCTAssertEqual(request.0,4)
        let http=try XCTUnwrap(JSONSerialization.jsonObject(with:Data(request.1.utf8)) as? [String:Any])
        let raw=try XCTUnwrap(http["body"] as? String)
        let command=try XCTUnwrap(JSONSerialization.jsonObject(with:Data(raw.utf8)) as? [String:Any])
        XCTAssertEqual(command["method"] as? String,"OpenSession")
        XCTAssertEqual(command["text"] as? String,"Hello from the phone")
        try p.event(204,0,"{}");try p.frame()
        XCTAssertTrue(p.elements().contains {$0.role=="toolbar" && $0.children.contains {$0.id=="env.refresh" && $0.attrs["data-state"]=="offline"}})
        try p.input("env.refresh");try p.frame()
        XCTAssertEqual(try p.action().1,request.1)
    }
    func testUsageCodeCompletesThroughTheInstalledInterpreter()throws {
        let p=try NativeProgram(directory:try XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil)),module:"app")
        func answer(_ op:Int,_ value:[String:Any])throws {try p.event(100+op,op==4 ? 200:0,String(decoding:JSONSerialization.data(withJSONObject:value),as:UTF8.self));try p.frame()}
        func takeAction()throws->(Int,String) {for _ in 0..<64 {let a=try p.action();if a.0 != 12 {return a};try answer(12,[:])};throw NativeFailure.refused("Unbounded timer chain")}
        let host=String(repeating:"a",count:64),flow=String(repeating:"1",count:32)
        try p.event(15,0,"");try p.frame();_ = try takeAction();try answer(1,["loggedIn":true]);_ = try takeAction();try answer(4,["schema":"openagents.chat.history.v1","chats":[]])
        try p.input("nav.settings");try p.frame();XCTAssertEqual(try takeAction().0,18);try p.event(218,0,"{\"error\":\"Fixture has no settings service\"}");try p.frame();try p.input("usage.open");try p.frame();XCTAssertEqual(try takeAction().0,4)
        try answer(4,["schema":"openagents.environment.catalog.v1alpha1","environments":[["host_pubkey":host,"display_name":"Mac"]]]);XCTAssertEqual(try takeAction().0,4)
        func view(_ method:String,_ command:String,_ login:[String:Any])->[String:Any] {["schema":"openagents.environment.accounts.view.v1","host_pubkey":host,"method":method,"command_id":command,"accounts":[],"login":login,"error":"","retry_after_ms":0]}
        let noLogin:[String:Any]=["login_id":"","account_id":"","state":"","authorization_url":"","expires_at_ms":0]
        try answer(4,view("ListAccounts","",noLogin));try p.input("usage.add.0");try p.frame();try p.input("usage.label","Work");try p.frame();try p.input("usage.begin");try p.frame();XCTAssertEqual(try takeAction().0,11)
        try answer(11,["command_id":flow,"now_ms":1_700_000_000_000]);XCTAssertEqual(try takeAction().0,4)
        let login:[String:Any]=["login_id":flow,"account_id":flow,"state":"awaiting_code","authorization_url":"https://claude.com/cai/oauth/authorize?fixture=1","expires_at_ms":1_700_000_600_000]
        try answer(4,view("BeginAccountLogin",flow,login));try p.input("usage.code","fixture-code");try p.frame()
        XCTAssertFalse(try XCTUnwrap(p.elements(dialog:true).first {$0.id=="usage.complete"}).disabled)
        try p.input("usage.complete");try p.frame();XCTAssertEqual(try takeAction().0,11)
        try answer(11,["command_id":String(repeating:"2",count:32),"now_ms":1_700_000_000_001]);let request=try takeAction();XCTAssertEqual(request.0,4)
        let http=try XCTUnwrap(JSONSerialization.jsonObject(with:Data(request.1.utf8)) as? [String:Any]);let body=try XCTUnwrap(JSONSerialization.jsonObject(with:Data((http["body"] as! String).utf8)) as? [String:Any]);XCTAssertEqual(body["method"] as? String,"CompleteAccountLogin");XCTAssertEqual(body["code"] as? String,"fixture-code")
    }

}
