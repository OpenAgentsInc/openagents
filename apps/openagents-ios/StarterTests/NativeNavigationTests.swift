// Firm reason: 6 (JavaScriptCore/UIKit oracle of the shipped Coax navigation).
import XCTest
import UIKit
@testable import OpenAgents

@MainActor final class NativeNavigationTests:XCTestCase {
    private let mac=String(repeating:"a",count:64),coder=String(repeating:"b",count:64)
    private func answer(_ p:NativeProgram,_ body:[String:Any])throws {
        try p.event(104,200,String(decoding:JSONSerialization.data(withJSONObject:body),as:UTF8.self));try p.frame();_ = try p.action()
    }
    private func view(_ host:String,_ method:String)->[String:Any] {
        ["schema":"openagents.environment.runtime.v1alpha1","host_pubkey":host,"method":method,"display_name":host==mac ? "Mac":"CoderOS","sessions":[["session_id":"session-1","workspace_id":"default","title":host==mac ? "Mac task":"CoderOS task","origin":"managed","control":"available","generation":1,"revision":1,"busy":false,"interaction_id":"","updated_at_ms":1]],"items":method=="ReadSession" ? [["item_id":"answer-1","role":"assistant","text":"**Ready.**","tool_name":"","complete":true]]:[],"error":"","complete":true,"next_cursor":"","command_id":"","command_status":"","source_epoch":"fixture","models":[["model_id":"default","display_name":"Claude default","efforts":["low","high"]]],"efforts":[["effort_id":"low","display_name":"Low"],["effort_id":"high","display_name":"High"]],"default_effort":"high"]
    }
    private func loaded(count:Int=1)throws->NativeProgram {
        let p=try NativeProgram(directory:XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil)),module:"app")
        try p.event(Int(cnw_enable_event()),0,"");try p.frame();_ = try p.action()
        try p.event(101,0,"{\"loggedIn\":true,\"worldsAvailable\":true}");try p.frame();_ = try p.action()
        try answer(p,["schema":"openagents.chat.history.v1","chats":[]])
        try answer(p,["schema":"openagents.environment.catalog.v1alpha1","environments":[["host_pubkey":mac,"display_name":"Mac"],["host_pubkey":coder,"display_name":"CoderOS"]]])
        var page=view(mac,"ListSessions")
        if count>1 {let base=(page["sessions"] as! [[String:Any]])[0];page["sessions"]=(0..<count).map {i -> [String:Any] in var row=base;row["session_id"]="session-\(i)";row["title"]="Mac task \(i)";return row}}
        try answer(p,page);try answer(p,view(coder,"ListSessions"));return p
    }
    func testLargeHistoryKeepsBothComputersAccessibleAndPagesCachedRows()throws {
        let p=try loaded(count:100)
        XCTAssertTrue(p.elements(navigation:true).contains {$0.label=="CoderOS task"})
        XCTAssertTrue(p.elements(navigation:true).contains {$0.label=="Mac task 0"})
        try p.input("nav.more.0");try p.frame();XCTAssertEqual(try p.action().0,0)
        XCTAssertTrue(p.elements(navigation:true).contains {$0.label=="Mac task 48"})
        XCTAssertFalse(p.elements(navigation:true).contains {$0.label=="Mac task 0"})
        try p.input("nav.latest.0");try p.frame();XCTAssertTrue(p.elements(navigation:true).contains {$0.label=="Mac task 0"})
        try p.input("nav.search","Mac task 99");try p.frame();XCTAssertTrue(p.elements(navigation:true).contains {$0.label=="Mac task 99"})
    }
    func testChangingComposerDomainKeepsItsReusedViewAttached() {
        let surface=NativeControls();surface.loadViewIfNeeded()
        func composer(_ id:String)->NativeElement {NativeElement(id:id+".frame",role:"composer",label:"Message",children:[NativeElement(id:id,role:"field",label:"Message",value:"draft"),NativeElement(id:id+".send",role:"button",label:"Send")])}
        surface.render([composer("env.composer")]);surface.render([composer("chat.composer")])
        XCTAssertTrue(surface.controls["chat.composer"]?.isDescendant(of:surface.view)==true)
    }
    func testHistoryProjectionSearchSelectionDraftAndFixedComposerControls()throws {
        let p=try loaded(),detail=NativeControls(),workspace=NativeWorkspace(detail:detail)
        workspace.loadViewIfNeeded();workspace.view.frame=CGRect(x:0,y:0,width:402,height:874)
        func render()throws {try p.frame();workspace.render(p.elements(),history:p.elements(navigation:true));workspace.view.layoutIfNeeded()}
        try render();XCTAssertTrue(p.elements().contains {$0.role=="navigation" && $0.attrs["data-state"]=="open"})
        XCTAssertFalse(p.elements().contains {$0.id.hasPrefix("nav.chat.")})
        XCTAssertEqual(p.elements(navigation:true).filter {$0.id.hasPrefix("nav.chat.")}.count,2)
        XCTAssertNotNil(workspace.sidebar.controls["nav.search"] as? NativeField)
        XCTAssertNotNil(workspace.sidebar.controls["nav.compose"] as? NativeButton)
        let search=try XCTUnwrap(workspace.sidebar.controls["nav.search"])
        try p.input("nav.search","coder");try render()
        XCTAssertTrue(search === workspace.sidebar.controls["nav.search"])
        XCTAssertEqual(p.elements(navigation:true).filter {$0.id.hasPrefix("nav.chat.")}.map(\.label),["CoderOS task"])
        try p.input("nav.search","");try render()
        let macID=try XCTUnwrap(p.elements(navigation:true).first {$0.label=="Mac task"}?.id)
        try p.input(macID);try render();let read=try p.action();XCTAssertEqual(read.0,4);XCTAssertTrue(read.1.contains("ReadSession"))
        try answer(p,view(mac,"ReadSession"));try render()
        XCTAssertFalse(p.elements().contains {$0.id=="env.sessions"})
        XCTAssertEqual(p.elements().first {$0.id=="env.model"}?.attrs["_nativePlacement"],"dock")
        try p.input("env.composer","Unsent draft");try render()
        let editor=try XCTUnwrap(detail.controls["env.composer"] as? UITextView)
        try p.input("nav.toggle");try render();try p.input("nav.close");try render()
        XCTAssertTrue(editor === detail.controls["env.composer"]);XCTAssertEqual(editor.text,"Unsent draft")
        try p.input("env.effort");try render();XCTAssertTrue(p.elements(dialog:true).contains {$0.id=="env.effort.0"})
        try p.input("env.effort.0");try render();XCTAssertTrue(p.elements(dialog:true).isEmpty)
        try p.input("nav.toggle");try render()
        let coderID=try XCTUnwrap(p.elements(navigation:true).first {$0.label=="CoderOS task"}?.id)
        try p.input(coderID);try render();_ = try p.action();try answer(p,view(coder,"ReadSession"))
        try p.input("nav.toggle");try render();try p.input(macID);try render();_ = try p.action();try answer(p,view(mac,"ReadSession"));try render()
        XCTAssertEqual((detail.controls["env.composer"] as? UITextView)?.text,"Unsent draft")
        try p.input("nav.toggle");try render();try p.input("nav.settings");try render()
        XCTAssertTrue(p.elements(dialog:true).contains {$0.id=="app.updates"});XCTAssertFalse(p.elements().contains {$0.id=="app.updates"})
        XCTAssertEqual(workspace_layout(1024,true,false).list,328)
    }
}
