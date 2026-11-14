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
}
