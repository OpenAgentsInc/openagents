// Firm reason: 6 (JavaScriptCore and UIKit SDK integration checks).
import XCTest
import UIKit
@testable import OpenAgents

@MainActor final class NativeProgramTests:XCTestCase {
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
        XCTAssertEqual(composer.intrinsicContentSize.height,160,accuracy:1);XCTAssertTrue(editor.isScrollEnabled)
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
        XCTAssertTrue(p.elements().contains {$0.id=="env.reconnect"})
        try p.input("env.reconnect");try p.frame()
        XCTAssertEqual(try p.action().1,request.1)
    }
}
