// Firm reason: 6 (JavaScriptCore/UIKit oracle of the actual generated transcript).
import XCTest
import UIKit
@testable import OpenAgents

@MainActor final class NativeTranscriptTests:XCTestCase {
    private func descendants(_ v:UIView)->[UIView] {[v]+v.subviews.flatMap(descendants)}
    private let output="## Tool output is literal\n"+String(repeating:"42  let code = **not Markdown**;\n",count:160)
    private func loaded()throws->(NativeProgram,[String:Any]) {
        let p=try NativeProgram(directory:XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil)),module:"app")
        func answer(_ value:[String:Any])throws {try p.event(104,200,String(decoding:JSONSerialization.data(withJSONObject:value),as:UTF8.self));try p.frame();_ = try p.action()}
        try p.frame();_ = try p.action();try p.event(101,0,"{\"loggedIn\":true}");try p.frame();_ = try p.action()
        try answer(["schema":"openagents.chat.history.v1","chats":[]]);try p.input("environments.open");try p.frame();_ = try p.action()
        let host=String(repeating:"a",count:64),session="fixture-session"
        try answer(["schema":"openagents.environment.catalog.v1alpha1","environments":[["host_pubkey":host,"display_name":"Mac"]]])
        try p.input("environment.0");try p.frame();_ = try p.action()
        let summary:[String:Any]=["session_id":session,"workspace_id":"default","title":"Transcript fixture","origin":"managed","control":"available","generation":1,"revision":1,"busy":true,"interaction_id":"","updated_at_ms":1]
        var view:[String:Any]=["schema":"openagents.environment.runtime.v1alpha1","host_pubkey":host,"display_name":"Mac","method":"ListSessions","sessions":[summary],"items":[],"error":"","complete":false,"next_cursor":"1000001","command_id":"","command_status":"","source_epoch":"fixture"]
        try answer(view);try p.input("environment.session.0");try p.frame();_ = try p.action()
        view["method"]="ReadSession"
        view["models"]=[["model_id":"default","display_name":"Claude default","efforts":["low","high"]]]
        view["efforts"]=[["effort_id":"low","display_name":"Low"],["effort_id":"high","display_name":"High"]];view["default_effort"]="high"
        view["items"]=[
            ["item_id":"user-1","role":"user","text":"Fix the transcript","tool_name":"","complete":true],
            ["item_id":"tool-1","role":"tool","text":output,"tool_name":"Read","complete":true],
            ["item_id":"tool-2","role":"tool","text":"Updated file successfully.","tool_name":"Edit","complete":true],
            ["item_id":"answer-1","role":"assistant","text":"**Done.** The transcript is compact.","tool_name":"","complete":true]]
        try answer(view);return (p,view)
    }
    func testToolsCollapseAndExpandWithoutMarkdownAndKeepTheirIdentityAcrossPolling()throws {
        let (p,view)=try loaded(),surface=NativeControls();surface.view.frame=CGRect(x:0,y:0,width:402,height:874);surface.render(p.elements());surface.view.layoutIfNeeded()
        let tools=p.elements().filter {$0.role=="tool"};XCTAssertEqual(tools.count,2)
        let e=try XCTUnwrap(tools.first),tool=try XCTUnwrap(surface.controls[e.id] as? NativeTool)
        XCTAssertTrue(p.elements().contains {$0.id=="env.model"});XCTAssertTrue(p.elements().contains {$0.id=="env.effort"})
        XCTAssertEqual(tool.frame.height,36,accuracy:1);XCTAssertFalse(descendants(tool).contains {$0 is NativeRichText})
        let action=try XCTUnwrap(tool.header.accessibilityIdentifier);try p.input(action);try p.frame();surface.render(p.elements());surface.view.layoutIfNeeded()
        let text=try XCTUnwrap(descendants(tool).compactMap {$0 as? NativeRichText}.first)
        XCTAssertEqual(text.text,output);XCTAssertTrue(text.isSelectable);XCTAssertFalse(text.isEditable)
        let detail=try XCTUnwrap(descendants(tool).compactMap {$0 as? UIScrollView}.first);XCTAssertLessThanOrEqual(detail.frame.height,240);XCTAssertGreaterThan(detail.contentSize.height,detail.bounds.height)
        let screenshot=XCTAttachment(image:UIGraphicsImageRenderer(bounds:surface.view.bounds).image {_ in surface.view.drawHierarchy(in:surface.view.bounds,afterScreenUpdates:true)});screenshot.name="Expanded literal tool output with bounded scrolling";screenshot.lifetime = .keepAlways;add(screenshot)
        try p.input(action+".copy");try p.frame();let copied=try p.action();XCTAssertEqual(copied.0,9)
        XCTAssertEqual((try JSONSerialization.jsonObject(with:Data(copied.1.utf8)) as? [String:String])?["text"],output)
        try p.event(109,0,"{}");try p.frame();_ = try p.action()
        text.selectedRange=NSRange(location:3,length:4)
        try p.event(112,0,"{}");try p.frame();_ = try p.action()
        try p.event(104,200,String(decoding:JSONSerialization.data(withJSONObject:view),as:UTF8.self));try p.frame();_ = try p.action();surface.render(p.elements());surface.view.layoutIfNeeded()
        XCTAssertTrue(tool === surface.controls[e.id]);XCTAssertEqual(text.selectedRange,NSRange(location:3,length:4));XCTAssertEqual(tool.header.accessibilityValue,"Expanded")
        // A header's name is drawn in the skin's upper-case label type; what it says to a reader is the name as given.
        XCTAssertEqual((surface.controls["env.executing"] as? UILabel)?.accessibilityLabel,"Mac");XCTAssertEqual((surface.controls["env.executing"] as? UILabel)?.text,"MAC")
        try p.input(action);try p.frame()
        XCTAssertEqual(p.elements().first {$0.id==e.id}?.children.first {$0.role=="button"}?.attrs["aria-expanded"],"false")
        surface.render(p.elements());XCTAssertEqual(tool.header.accessibilityValue,"Collapsed")
        XCTAssertEqual(tool.constraints.first {$0.firstAttribute == .height}?.constant,36)
        surface.view.layoutIfNeeded();XCTAssertEqual(tool.frame.height,36,accuracy:1)
    }
    func testRefreshLivesInTheHeaderWithoutAFlashingDisabledButton()throws {
        let (p,view)=try loaded(),surface=NativeControls();surface.view.frame=CGRect(x:0,y:0,width:402,height:874);surface.render(p.elements());surface.view.layoutIfNeeded()
        let refresh=try XCTUnwrap(surface.controls["env.refresh"] as? NativeButton)
        XCTAssertTrue(refresh.isEnabled);XCTAssertNotNil(refresh.symbol);XCTAssertEqual(refresh.variant,.icon)
        XCTAssertFalse(p.elements().contains {$0.id=="env.history.partial"})
        try p.event(112,0,"{}");try p.frame();_ = try p.action();surface.render(p.elements())
        XCTAssertFalse(refresh.busy);XCTAssertTrue(refresh.isEnabled)
        try p.event(104,200,String(decoding:JSONSerialization.data(withJSONObject:view),as:UTF8.self));try p.frame();_ = try p.action()
        try p.input("env.composer","Keep this draft");try p.frame();_ = try p.action()
        try p.input("env.refresh");try p.frame();surface.render(p.elements());surface.view.layoutIfNeeded()
        XCTAssertTrue(refresh === surface.controls["env.refresh"]);XCTAssertTrue(refresh.isEnabled);XCTAssertTrue(refresh.busy)
        XCTAssertEqual((surface.controls["env.composer"] as? UITextView)?.text,"Keep this draft")
        let screenshot=XCTAttachment(image:UIGraphicsImageRenderer(bounds:surface.view.bounds).image {_ in surface.view.drawHierarchy(in:surface.view.bounds,afterScreenUpdates:true)});screenshot.name="Compact T3 work rows and header refresh";screenshot.lifetime = .keepAlways;add(screenshot)
    }
    func testEmptyNativeHistoryRecordsNeverRenderBubblesOrCopyOnlyRows()throws {
        let (p,initial)=try loaded();var view=initial
        var items=try XCTUnwrap(view["items"] as? [[String:Any]])
        for i in 0..<12 {items.insert(["item_id":"blank-\(i)","role":i%2==0 ? "user":"assistant","text":i%3==0 ? " \n\t":"","tool_name":"","complete":true],at:1)}
        view["items"]=items
        try p.event(104,200,String(decoding:JSONSerialization.data(withJSONObject:view),as:UTF8.self));try p.frame();_ = try p.action()
        let elements=p.elements(),messages=elements.filter {$0.role=="message"}
        XCTAssertEqual(messages.count,2);XCTAssertEqual(elements.filter {$0.role=="tool"}.count,2)
        let surface=NativeControls();surface.view.frame=CGRect(x:0,y:0,width:402,height:874);surface.render(elements)
        surface.view.layoutIfNeeded();surface.view.layoutIfNeeded()
        for message in messages {
            let native=try XCTUnwrap(surface.controls[message.id] as? NativeMessage)
            let text=descendants(native).compactMap {$0 as? NativeRichText}
            XCTAssertTrue(text.contains {!$0.text.trimmingCharacters(in:.whitespacesAndNewlines).isEmpty})
            XCTAssertTrue(text.allSatisfy {$0.bounds.height>0})
        }
        let shot=XCTAttachment(image:UIGraphicsImageRenderer(bounds:surface.view.bounds).image {_ in surface.view.drawHierarchy(in:surface.view.bounds,afterScreenUpdates:true)})
        shot.name="Native history without empty bubbles or orphan copy icons";shot.lifetime = .keepAlways;add(shot)
    }
}
