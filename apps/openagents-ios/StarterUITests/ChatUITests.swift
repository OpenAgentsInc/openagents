// Firm reason: 6 (XCTest exercises UIKit, native account transport and Keychain).
import XCTest

final class ChatUITests:XCTestCase {
    func testEmailAccountSendsAndReopensConversationWithNativeComposer() {
        let app=XCUIApplication()
        app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13491","-OAUpdatesOrigin","http://127.0.0.1:13492"]
        app.launch()
        let email=app.textFields["Email address"]
        if !email.waitForExistence(timeout:5) {
            if app.buttons["chat.back"].exists {app.buttons["chat.back"].tap()}
            let logout=app.buttons["Log out"];XCTAssertTrue(logout.waitForExistence(timeout:15));logout.tap()
        }
        XCTAssertTrue(email.waitForExistence(timeout:15));email.tap();email.typeText("ada@example.com");app.buttons["Send code"].tap()
        let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926");app.buttons["Log in"].tap()
        let newChat=app.buttons["New chat"];XCTAssertTrue(newChat.waitForExistence(timeout:15));newChat.tap()
        let editor=app.textViews["chat.composer"],send=app.buttons["Send message"]
        XCTAssertTrue(editor.waitForExistence(timeout:10));XCTAssertFalse(send.isEnabled)
        XCTAssertLessThan(app.buttons["chat.back"].frame.minX,app.buttons["More"].frame.minX)
        XCTAssertEqual(app.webViews.count,0)
        editor.tap();editor.typeText("Hello from iPhone")
        XCTAssertTrue(send.isEnabled)
        let compact=XCTAttachment(screenshot:app.screenshot());compact.name="T3 compact native composer";compact.lifetime = .keepAlways;add(compact)
        send.tap()
        let reply=app.textViews.matching(NSPredicate(format:"value == %@","Hello from the account agent.")).firstMatch
        XCTAssertTrue(reply.waitForExistence(timeout:20));XCTAssertEqual(editor.value as? String,"");XCTAssertTrue(app.keyboards.firstMatch.exists)
        app.scrollViews.firstMatch.coordinate(withNormalizedOffset:CGVector(dx:0.02,dy:0.4)).tap()
        let dismissed=XCTNSPredicateExpectation(predicate:NSPredicate(format:"exists == false"),object:app.keyboards.firstMatch)
        XCTAssertEqual(XCTWaiter.wait(for:[dismissed],timeout:5),.completed)
        let copy=app.buttons["message.1.copy"];XCTAssertTrue(copy.exists);copy.tap()
        XCTAssertTrue(app.buttons.matching(identifier:"message.1.copy").matching(NSPredicate(format:"label == %@","Copied")).firstMatch.waitForExistence(timeout:5))
        let rich=XCTAttachment(screenshot:app.screenshot());rich.name="Coax Markdown native conversation";rich.lifetime = .keepAlways;add(rich)
        app.buttons["chat.back"].tap()
        let saved=app.buttons["Hello from iPhone"];XCTAssertTrue(saved.waitForExistence(timeout:15))
        app.terminate();app.launch();XCTAssertTrue(saved.waitForExistence(timeout:15));saved.tap()
        XCTAssertTrue(reply.waitForExistence(timeout:15))
        editor.tap();editor.typeText("First line\nSecond line\nThird line\nFourth line\nFifth line")
        XCTAssertLessThanOrEqual(editor.frame.height,105)
        XCTAssertGreaterThan(editor.frame.height,70)
        XCTAssertLessThanOrEqual(send.frame.maxY,app.keyboards.firstMatch.frame.minY+1)
        let expanded=XCTAttachment(screenshot:app.screenshot());expanded.name="T3 expanded keyboard-docked composer";expanded.lifetime = .keepAlways;add(expanded)
        app.firstMatch.pinch(withScale:2,velocity:1)
        XCTAssertLessThanOrEqual(editor.frame.height,105)
    }
}
