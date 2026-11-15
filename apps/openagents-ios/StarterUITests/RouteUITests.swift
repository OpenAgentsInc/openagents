// Firm reason: 6 (XCTest exercises UIKit, native account transport and Keychain).
import XCTest

// A message in the account chat is routed before it is answered (docs/router/README.md): the
// line under the answer says what would run, and the answer was told it. The independent oracle
// is Tests/email_fixture.py --environment-transcript, which has two computers and one decision.
final class RouteUITests:XCTestCase {
    func testAccountChatShowsWhereTheMessageWouldRun() {
        let app=XCUIApplication()
        app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13491","-OAUpdatesOrigin","http://127.0.0.1:13492"]
        app.launch()
        let email=app.textFields["Email address"]
        if email.waitForExistence(timeout:8) {
            email.tap();email.typeText("ada@example.com");app.buttons["Send code"].tap()
            let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926");app.buttons["Log in"].tap()
        }
        // the Chat home has the account composer; the menu's history loads the computers
        let editor=app.textViews["chat.composer"],send=app.buttons["Send message"]
        XCTAssertTrue(editor.waitForExistence(timeout:20))
        XCTAssertTrue(app.buttons["nav.toggle"].waitForExistence(timeout:10))
        sleep(4)
        editor.tap();editor.typeText("On the Mac, fix the failing test")
        send.tap()
        let reply=app.textViews.matching(NSPredicate(format:"value == %@","I'm about to hand this to Claude Code on Mac.")).firstMatch
        XCTAssertTrue(reply.waitForExistence(timeout:20))
        let line=app.descendants(matching:.any).matching(NSPredicate(format:"label BEGINSWITH %@ OR value BEGINSWITH %@","Claude Code · Opus · high · on Mac","Claude Code · Opus · high · on Mac")).firstMatch
        XCTAssertTrue(line.waitForExistence(timeout:10))
        let shot=XCTAttachment(screenshot:app.screenshot());shot.name="Where the message would run";shot.lifetime = .keepAlways;add(shot)
    }
}
