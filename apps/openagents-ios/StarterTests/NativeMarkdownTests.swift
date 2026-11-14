// Firm reason: 6 (TextKit semantic rendering, selection and native-layout checks).
import XCTest
import UIKit
@testable import OpenAgents

@MainActor final class NativeMarkdownTests:XCTestCase {
    static let markdown="## A heading\n\n**bold** and *italic* and ~~gone~~. [Docs](https://openagents.com/#chat).\n\n- [x] done\n  - child\n\n| Left | Right |\n| :--- | ---: |\n| one | two |\n\n> A quote\n\n```coax\nx := 1;\n```\n\n---"
    func loaded()throws->NativeProgram {
        let root=try XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil));let p=try NativeProgram(directory:root,module:"app")
        try p.frame();_=try p.action();try p.event(101,0,"{\"loggedIn\":true}");try p.frame();_=try p.action()
        let id="m_12345678901234567890"
        let list:[String:Any]=["schema":"openagents.chat.history.v1","chats":[["id":id,"title":"Markdown fixture","model":"m","folder":"","updated_ms":1,"busy":false]]]
        try p.event(104,200,String(decoding:JSONSerialization.data(withJSONObject:list),as:UTF8.self));try p.frame();try p.input("chat.0");try p.frame();_=try p.action()
        let thread:[String:Any]=["schema":"openagents.chat.thread.v1","id":id,"revision":2,"messages":[["role":"user","content":"A short question"],["role":"assistant","content":Self.markdown]]]
        try p.event(104,200,String(decoding:JSONSerialization.data(withJSONObject:thread),as:UTF8.self));try p.frame();return p
    }
    func descendants(_ view:UIView)->[UIView] {[view]+view.subviews.flatMap {descendants($0)}}
    func testSharedParserBecomesSelectableNativeBlocksAndPreservesSelection()throws {
        let p=try loaded(),surface=NativeControls();surface.render(p.elements());surface.view.frame=CGRect(x:0,y:0,width:402,height:874);surface.view.layoutIfNeeded();surface.view.layoutIfNeeded()
        let message=try XCTUnwrap(surface.controls["message.1"] as? NativeMessage)
        let texts=descendants(message).compactMap {$0 as? NativeRichText}
        let heading=try XCTUnwrap(texts.first {$0.text=="A heading"});XCTAssertEqual(heading.font?.pointSize ?? (heading.attributedText.attribute(.font,at:0,effectiveRange:nil) as? UIFont)?.pointSize,19)
        let body=try XCTUnwrap(texts.first {$0.text.contains("bold and italic")})
        let raw=body.text as NSString
        let bold=try XCTUnwrap(body.attributedText.attribute(.font,at:raw.range(of:"bold").location,effectiveRange:nil) as? UIFont)
        let italic=try XCTUnwrap(body.attributedText.attribute(.font,at:raw.range(of:"italic").location,effectiveRange:nil) as? UIFont)
        XCTAssertTrue(bold.fontName.contains("Bold"));XCTAssertTrue(italic.fontDescriptor.symbolicTraits.contains(.traitItalic))
        XCTAssertNotNil(body.attributedText.attribute(.strikethroughStyle,at:raw.range(of:"gone").location,effectiveRange:nil))
        XCTAssertNotNil(body.attributedText.attribute(.link,at:raw.range(of:"Docs").location,effectiveRange:nil))
        // The checkbox includes VS15 to request text presentation. Swift's
        // grapheme-prefix comparison does not match its bare scalar.
        XCTAssertTrue(texts.contains {$0.text.unicodeScalars.first?.value==0x2611});XCTAssertTrue(texts.contains {$0.text=="x := 1;"})
        XCTAssertFalse(body.isEditable);XCTAssertTrue(body.isSelectable)
        body.selectedRange=NSRange(location:0,length:4)
        try p.input("chat.composer","next draft");try p.frame();surface.render(p.elements())
        XCTAssertTrue(message === surface.controls["message.1"]);XCTAssertEqual(body.selectedRange,NSRange(location:0,length:4))
        let shot=XCTAttachment(image:UIGraphicsImageRenderer(bounds:surface.view.bounds).image {_ in surface.view.drawHierarchy(in:surface.view.bounds,afterScreenUpdates:true)});shot.name="Native shared Markdown blocks";shot.lifetime = .keepAlways;add(shot)
    }
    func testCopyUsesOriginalMessageOrRawCodeAndUnsafeLinksHaveNoEffect()throws {
        let p=try loaded()
        try p.input("message.1.copy");try p.frame();let action=try p.action();XCTAssertEqual(action.0,9)
        let object=try JSONSerialization.jsonObject(with:Data(action.1.utf8)) as? [String:String];XCTAssertEqual(object?["text"],Self.markdown)
        try p.event(109,0,"{\"copied\":true}");try p.frame()
        let copy=try XCTUnwrap(p.nodes.values.first {$0.id.hasPrefix("message.1.body.") && $0.id.hasSuffix(".copy")})
        try p.input(copy.id);try p.frame();let code=try p.action();XCTAssertEqual(code.0,9)
        XCTAssertEqual((try JSONSerialization.jsonObject(with:Data(code.1.utf8)) as? [String:String])?["text"],"x := 1;")
        try p.event(109,0,"{}");try p.frame()
        XCTAssertFalse("javascript:alert(1)".withCString {nt_link(coax_string(data:$0,count:19))})
        let link=try XCTUnwrap(p.nodes.values.first {$0.tag=="a"})
        try p.input(link.id);try p.frame();let open=try p.action();XCTAssertEqual(open.0,10)
        XCTAssertEqual((try JSONSerialization.jsonObject(with:Data(open.1.utf8)) as? [String:String])?["url"],"https://openagents.com/#chat")
    }
}
