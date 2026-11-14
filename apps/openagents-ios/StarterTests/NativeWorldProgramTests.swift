// Firm reason: 6 (JavaScriptCore/UIKit oracle of the shared mobile authoring programs).
import XCTest
@testable import OpenAgents

@MainActor final class NativeWorldProgramTests:XCTestCase {
    private func program(_ module:String)throws->NativeProgram {try NativeProgram(directory:XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil)),module:module)}
    private func object(_ text:String)throws->[String:Any] {try XCTUnwrap(JSONSerialization.jsonObject(with:Data(text.utf8)) as? [String:Any])}
    func testLibraryUsesTheSharedBuilderAndNativeWorldService()throws {
        let p=try program("worlds");try p.frame();XCTAssertEqual(try p.action().0,4)
        try p.event(104,200,"world my-forest My forest\n");try p.frame()
        XCTAssertTrue(p.elements().contains {$0.id=="worlds.edit.0" && $0.label=="My forest"})
        try p.input("worlds.splice");try p.frame();let open=try p.action()
        XCTAssertEqual(open.0,13);XCTAssertEqual(try object(open.1)["realm"] as? String,"splice-bots")
        try p.event(113,0,"{}");try p.frame();try p.input("worlds.create");try p.frame()
        XCTAssertTrue(p.elements().contains {$0.id=="worlds.question"})
        for _ in 0..<5 {try p.input("worlds.skip");try p.frame()}
        try p.input("worlds.prompt","A forest with a dragon");try p.frame();try p.input("worlds.plan");try p.frame()
        let plan=try p.action();XCTAssertEqual(plan.0,4);XCTAssertTrue(plan.1.contains("A forest with a dragon"))
    }
    private func model(_ revision:Int=7)->String {"editor.model.v1 splice dev \(revision) 2 0 1\ncols x z yaw\nk 1001 Cargo crate\ne 1 1001 350 0 -1200 90 100\n"}
    func testEditorProjectsControlsAndFencesTypedPlansWithoutBrowserContent()throws {
        let p=try program("editor");try p.event(16,0,"");try p.event(6,0,model());try p.frame()
        XCTAssertTrue(p.elements().contains {$0.id=="agent"})
        // Drain SDK fetches (scripts/revisions/assets) without fabricating an editing grant.
        for _ in 0..<8 {let action=try p.action();if action.0==0 {break}}
        try p.input("agent");try p.frame();let turn=try p.action();XCTAssertEqual(turn.0,524);XCTAssertTrue(turn.1.hasPrefix("world.turn.v1 splice dev 7"))
        try p.event(8,0,"world.plan.v1 splice 7\nmove 1 420 -75\n");try p.frame();let edit=try p.action();XCTAssertEqual(edit.0,520);XCTAssertTrue(edit.1.contains("420"))
        try p.event(6,0,model(8));try p.frame();for _ in 0..<8 {if try p.action().0==0 {break}}
        try p.event(8,0,"world.plan.v1 splice 7\nremove 1\n");try p.frame()
        XCTAssertEqual(try p.action().0,0)
        XCTAssertTrue(p.elements().contains {$0.label.contains("world changed while the agent worked")})
    }
    func testWorldChatUsesExistingComposerAndTypedAccountRequest()throws {
        let p=try program("app");try p.event(16,0,"world.turn.v1 splice dev 7 1\nk 1001 Cargo crate\ne 1 1001 350 -1200 90\n");try p.frame()
        XCTAssertEqual(try p.action().0,0);XCTAssertTrue(p.elements().contains {$0.id=="chat.composer.frame" && $0.role=="composer"})
        try p.input("chat.composer","Move the crate");try p.frame();try p.input("chat.composer.send");try p.frame();XCTAssertEqual(try p.action().0,4)
        let thread:[String:Any]=["schema":"openagents.chat.thread.v1","id":"m_12345678901234567890","revision":1,"messages":[["role":"user","content":"Move the crate"]]]
        try p.event(104,200,String(decoding:JSONSerialization.data(withJSONObject:thread),as:UTF8.self));try p.frame()
        let inference=try p.action(),request=try object(inference.1);XCTAssertEqual(request["path"] as? String,"/api/chat/world/complete/v1")
        let body=try object(XCTUnwrap(request["body"] as? String));XCTAssertEqual(body["schema"] as? String,"openagents.world.account-turn.v1");XCTAssertTrue((body["world"] as? String ?? "").contains("splice dev 7"))
        try p.event(104,401,"{}");try p.frame();XCTAssertTrue(p.nodes.values.contains {$0.id=="chat.back"});XCTAssertFalse(p.nodes.values.contains {$0.id=="login.send"})
        try p.input("chat.back");try p.frame();XCTAssertEqual(try p.action().0,529)
    }
}
