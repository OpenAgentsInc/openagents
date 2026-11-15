// Firm reason: 6 (JavaScriptCore/UIKit oracle of the shipped Coax navigation).
import XCTest
import UIKit
@testable import OpenAgents

@MainActor final class NativeNavigationTests:XCTestCase {
    func testHeaderCentersTheTitleAndGroupsTrailingActions()throws {
        let surface=NativeControls();surface.view.frame=CGRect(x:0,y:0,width:402,height:874)
        let bar=NativeElement(id:"test.header",role:"toolbar",label:"",children:[NativeElement(id:"nav.toggle",role:"button",label:"Navigation",classes:"ib"),NativeElement(id:"test.title",role:"text",label:"Projects"),NativeElement(id:"test.action",role:"button",label:"Action",classes:"ib"),NativeElement(id:"projects.new",role:"button",label:"New project",classes:"ib")])
        surface.render([bar]);surface.view.layoutIfNeeded()
        let title=try XCTUnwrap(surface.controls["test.title"]),first=try XCTUnwrap(surface.controls["test.action"]),second=try XCTUnwrap(surface.controls["projects.new"])
        XCTAssertEqual(title.convert(title.bounds,to:surface.view).midX,201,accuracy:1)
        XCTAssertEqual(first.frame.maxX,second.frame.minX,accuracy:1)
        XCTAssertTrue((first as? NativeButton)?.glass == true)
        XCTAssertTrue((second as? NativeButton)?.glass == true)
    }
    func testToolbarUsesGlassOnlyForIconActions() throws {
        let surface=NativeControls()
        let toolbar=NativeElement(id:"toolbar",role:"toolbar",label:"",children:[NativeElement(id:"icon",role:"button",label:"Filter",classes:"ib"),NativeElement(id:"text",role:"button",label:"Done")])
        surface.render([toolbar])
        XCTAssertTrue(try XCTUnwrap(surface.controls["icon"] as? NativeButton).glass)
        XCTAssertFalse(try XCTUnwrap(surface.controls["text"] as? NativeButton).glass)
        let loading=NativeElement(id:"loading",role:"button",label:"",attrs:["data-state":"loading"])
        surface.render([NativeElement(id:"toolbar",role:"toolbar",label:"",children:[loading])])
        XCTAssertFalse(try XCTUnwrap(surface.controls["loading"] as? NativeButton).glass)
    }
    func testHistoryRevealsFixedWidthSingleLineRows()async throws {
        let detail=NativeControls(),workspace=NativeWorkspace(detail:detail);workspace.view.frame=CGRect(x:0,y:0,width:402,height:874)
        let row=NativeElement(id:"history.long",role:"button",label:String(repeating:"A long conversation title ",count:8),classes:"row")
        workspace.render([NativeElement(id:"nav.sidebar",role:"navigation",label:"",attrs:["data-state":"open","type":"rail"])],history:[]);workspace.view.layoutIfNeeded()
        workspace.render([NativeElement(id:"nav.sidebar",role:"navigation",label:"",attrs:["data-state":"open","type":"history"])],history:[row]);workspace.view.layoutIfNeeded()
        let button=try XCTUnwrap(workspace.sidebar.controls[row.id] as? NativeButton)
        for _ in 0..<4 {try await Task.sleep(for:.milliseconds(40));XCTAssertEqual(workspace.sidebar.view.bounds.width,338,accuracy:0.1);XCTAssertLessThanOrEqual(button.intrinsicContentSize.height,44)}
        XCTAssertEqual(button.accessibilityLabel,row.label)
    }
    private let mac=String(repeating:"a",count:64),coder=String(repeating:"b",count:64)
    func testInstalledElapsedClockDrivesReadSynchronization()throws {
        let p=try loaded();try p.input("nav.projects");try p.frame();_ = try p.action()
        try p.event(104,200,"");try p.frame();_ = try p.action()
        try answer(p,["schema":"openagents.projects.api.v1alpha1","data":[],"next_cursor":NSNull()])
        try p.frame(elapsed:9);XCTAssertEqual(try p.action().0,0)
        try p.frame(elapsed:1);let request=try p.action();XCTAssertEqual(request.0,4)
        let body=try XCTUnwrap(try JSONSerialization.jsonObject(with:Data(request.1.utf8)) as? [String:String])
        XCTAssertEqual(body["path"],"/api/projects");XCTAssertEqual(body["method"],"GET")
    }
    private func answer(_ p:NativeProgram,_ body:[String:Any])throws {
        try p.event(104,200,String(decoding:JSONSerialization.data(withJSONObject:body),as:UTF8.self));try p.frame();_ = try p.action()
    }
    private func view(_ host:String,_ method:String)->[String:Any] {
        ["schema":"openagents.environment.runtime.v1alpha1","host_pubkey":host,"method":method,"display_name":host==mac ? "Mac":"CoderOS","sessions":[["session_id":"session-1","workspace_id":"default","title":host==mac ? "Mac task":"CoderOS task","origin":"managed","control":"available","generation":1,"revision":1,"busy":false,"interaction_id":"","updated_at_ms":1]],"items":method=="ReadSession" ? [["item_id":"answer-1","role":"assistant","text":"**Ready.**","tool_name":"","complete":true]]:[],"error":"","complete":true,"next_cursor":"","command_id":"","command_status":"","source_epoch":"fixture","models":[["model_id":"default","display_name":"Claude default","efforts":["low","high"]]],"efforts":[["effort_id":"low","display_name":"Low"],["effort_id":"high","display_name":"High"]],"default_effort":"high"]
    }
    private func loaded(count:Int=1)throws->NativeProgram {
        let p=try NativeProgram(directory:XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil)),module:"app",diagnostic:{print("Navigation program:",$0)})
        try p.event(Int(cnw_enable_event()),0,"");try p.frame();_ = try p.action()
        try p.event(101,0,"{\"loggedIn\":true,\"worldsAvailable\":true}");try p.frame();_ = try p.action()
        try answer(p,["schema":"openagents.chat.history.v1","chats":[]])
        XCTAssertEqual(p.elements().first {$0.role=="navigation"}?.attrs["data-state"],"closed")
        try p.input("nav.toggle");try p.frame();_ = try p.action();try p.input("nav.recent");try p.frame()
        try answer(p,["schema":"openagents.environment.catalog.v1alpha1","environments":[["host_pubkey":mac,"display_name":"Mac"],["host_pubkey":coder,"display_name":"CoderOS"]]])
        var page=view(mac,"ListSessions")
        if count>1 {let base=(page["sessions"] as! [[String:Any]])[0];page["sessions"]=(0..<count).map {i -> [String:Any] in var row=base;row["session_id"]="session-\(i)";row["title"]="Mac task \(i)";return row}}
        try answer(p,page);try answer(p,view(coder,"ListSessions"));return p
    }
    func testHomeDrawerBackdropProjectsAndStreamedWorldShortcuts()throws {
        let p=try loaded(),detail=NativeControls(),workspace=NativeWorkspace(detail:detail)
        let window=UIWindow(frame:CGRect(x:0,y:0,width:402,height:874));window.rootViewController=workspace;window.makeKeyAndVisible();defer {window.isHidden=true}
        func render()throws {try p.frame();workspace.render(p.elements(),history:p.elements(navigation:true));workspace.view.layoutIfNeeded();detail.view.layoutIfNeeded()}
        try p.input("nav.home");try render()
        XCTAssertTrue(p.elements().contains {$0.id=="home.suggestions" && $0.role=="welcome"})
        XCTAssertNotNil(detail.controls["chat.composer"] as? UITextView)
        XCTAssertGreaterThan(try XCTUnwrap(detail.controls["home.title"]).convert(.zero,to:detail.view).y,150)
        try p.input("chat.composer","Home draft");try render()
        let model=try XCTUnwrap(detail.controls["home.microduck"] as? NativeButton)
        XCTAssertEqual(model.accessibilityLabel,"Microduck");XCTAssertTrue(model.isDescendant(of:detail.view))
        XCTAssertTrue(detail.view.bounds.contains(model.convert(CGPoint(x:10,y:10),to:detail.view)))
        let home=UIGraphicsImageRenderer(bounds:window.bounds).image {ctx in window.layer.render(in:ctx.cgContext)}
        let picture=XCTAttachment(image:home);picture.name="Home with Microduck";picture.lifetime = .keepAlways;add(picture)
        try p.input("home.microduck");try render();let modelOpen=try p.action()
        XCTAssertEqual(modelOpen.0,13)
        let modelRequest=try JSONSerialization.jsonObject(with:Data(modelOpen.1.utf8)) as! [String:Any]
        XCTAssertEqual(modelRequest["realm"] as? String,"microduck");XCTAssertEqual(modelRequest["path"] as? String,"/microduck")
        try p.event(113,0,"{}");try render();XCTAssertEqual((detail.controls["chat.composer"] as? UITextView)?.text,"Home draft")
        try p.input("nav.toggle");try render()
        XCTAssertEqual(workspace.sidebar.view.frame.width,70,accuracy:1)
        XCTAssertEqual(detail.view.frame.minX,70,accuracy:1)
        try p.input("nav.recent");try render();XCTAssertEqual(workspace.sidebar.view.frame.width,338,accuracy:1)
        XCTAssertTrue(["nav.home","nav.projects","world.library"].allSatisfy {workspace.sidebar.controls[$0] != nil})
        let shade=try XCTUnwrap(workspace.view.subviews.first {$0.accessibilityIdentifier=="workspace.backdrop"})
        XCTAssertFalse(shade.isHidden);XCTAssertEqual(shade.frame.width-workspace.sidebar.view.frame.width,64,accuracy:1)
        try p.input("nav.projects");try render();let request=try p.action();XCTAssertEqual(request.0,4);XCTAssertTrue(request.1.contains("/api/projects"))
        try p.event(104,200,"project mobile-kit Mobile Kit\n");try render();_ = try p.action()
        try answer(p,["schema":"openagents.projects.api.v1alpha1","data":[],"next_cursor":NSNull()]);try render()
        XCTAssertTrue(p.elements().contains {$0.id=="projects.item.0" && $0.label.contains("Mobile Kit")})
        let project=try XCTUnwrap(detail.controls["projects.item.0"]);XCTAssertTrue(project.isDescendant(of:detail.view));XCTAssertTrue(detail.view.bounds.contains(project.convert(CGPoint(x:10,y:10),to:detail.view)))
        try p.input("projects.item.0");try render();let member=try p.action();XCTAssertEqual(member.0,4);XCTAssertTrue(member.1.contains("/projects/mobile-kit/api/state"))
        try p.event(104,200,"Y|Ada|member\nP|Mobile Kit|Project summary|1|now|Ada\n");try render();_ = try p.action();try p.event(104,404,"{}");try render();try p.input("nav.home");try render()
        XCTAssertEqual((detail.controls["chat.composer"] as? UITextView)?.text,"Home draft")
        try p.input("home.everglade");try render();let world=try p.action();XCTAssertEqual(world.0,13);XCTAssertTrue(world.1.contains("everglade2"))
        try p.event(113,0,"{}");try render();try p.input("nav.toggle");try render()
        XCTAssertTrue(workspace.accessibilityPerformEscape())
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
        func render()throws {try p.frame();workspace.render(p.elements(),history:p.elements(navigation:true));workspace.view.layoutIfNeeded();detail.view.layoutIfNeeded()}
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
        XCTAssertFalse(p.elements().contains {$0.id=="env.model" || $0.id=="env.effort" || $0.id=="env.refresh"})
        try p.input("env.composer","Unsent draft");try render()
        let editor=try XCTUnwrap(detail.controls["env.composer"] as? UITextView)
        try p.input("nav.toggle");try render();try p.input("nav.close");try render()
        XCTAssertTrue(editor === detail.controls["env.composer"]);XCTAssertEqual(editor.text,"Unsent draft")
        try p.input("env.effort");try render();XCTAssertTrue(p.elements(dialog:true).isEmpty)
        try p.input("env.effort.0");try render();XCTAssertTrue(p.elements(dialog:true).isEmpty)
        try p.input("nav.toggle");try render();try p.input("nav.recent");try render()
        let coderID=try XCTUnwrap(p.elements(navigation:true).first {$0.label=="CoderOS task"}?.id)
        try p.input(coderID);try render();_ = try p.action();try answer(p,view(coder,"ReadSession"))
        try p.input("nav.toggle");try render();try p.input("nav.recent");try render();try p.input(macID);try render();_ = try p.action();try answer(p,view(mac,"ReadSession"));try render()
        XCTAssertEqual((detail.controls["env.composer"] as? UITextView)?.text,"Unsent draft")
        try p.input("nav.toggle");try render();try p.input("nav.settings");try render()
        XCTAssertTrue(p.elements(dialog:true).contains {$0.id=="app.updates"});XCTAssertFalse(p.elements().contains {$0.id=="app.updates"})
        XCTAssertEqual(workspace_layout(1024,true,false).list,328)
    }
}
