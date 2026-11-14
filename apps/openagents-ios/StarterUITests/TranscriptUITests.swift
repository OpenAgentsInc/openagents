// Firm reason: 6 (installed UIKit app against the independent local HTTP oracle).
import XCTest

final class TranscriptUITests:XCTestCase {
    func testComputerTranscriptUsesCompactToolRowsAndHeaderStatus() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13493","-OAUpdatesOrigin","http://127.0.0.1:13492"];app.launch()
        let email=app.textFields["Email address"]
        if !email.waitForExistence(timeout:5),app.buttons["Log out"].waitForExistence(timeout:5) {app.buttons["Log out"].tap()}
        if email.waitForExistence(timeout:5) {
            email.tap();email.typeText("ada@example.com");app.buttons["Send code"].tap()
            let code=app.textFields["Email code"];XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926");app.buttons["Log in"].tap()
        }
        let computers=app.buttons["Your computers"];XCTAssertTrue(computers.waitForExistence(timeout:20));computers.tap()
        let mac=app.buttons["Mac"];XCTAssertTrue(mac.waitForExistence(timeout:10));mac.tap()
        let thread=app.buttons["Transcript fixture"];XCTAssertTrue(thread.waitForExistence(timeout:10));thread.tap()
        let read=app.buttons["Read"];XCTAssertTrue(read.waitForExistence(timeout:10));XCTAssertLessThanOrEqual(read.frame.height,32)
        let output=app.textViews.matching(NSPredicate(format:"value BEGINSWITH %@","## Tool output is literal")).firstMatch
        XCTAssertFalse(output.exists);XCTAssertFalse(app.buttons["Refresh conversation"].exists)
        let refresh=app.buttons["env.refresh"];XCTAssertTrue(refresh.exists);XCTAssertTrue(refresh.isEnabled);XCTAssertLessThan(refresh.frame.minY,read.frame.minY)
        let collapsed=XCTAttachment(screenshot:app.screenshot());collapsed.name="Installed compact T3 work log";collapsed.lifetime = .keepAlways;add(collapsed)
        read.tap();XCTAssertTrue(output.waitForExistence(timeout:5))
        let expanded=XCTAttachment(screenshot:app.screenshot());expanded.name="Installed expanded literal tool output";expanded.lifetime = .keepAlways;add(expanded)
        read.tap();XCTAssertFalse(output.exists);XCTAssertLessThanOrEqual(read.frame.height,32)
        let model=app.buttons["env.model"],effort=app.buttons["env.effort"];XCTAssertTrue(model.exists);XCTAssertTrue(effort.exists)
        refresh.tap();XCTAssertTrue(refresh.isEnabled)
        XCTAssertEqual(app.webViews.count,0)
    }
}
