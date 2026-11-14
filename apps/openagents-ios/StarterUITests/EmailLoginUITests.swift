import XCTest

/// Uses a retained simulator app and an independently signed OTA.
/// Tests/email_fixture.py supplies a fake mailbox/API, while the real native Keychain,
/// device-key, HTTP and update-verification services run unchanged.
final class EmailLoginUITests:XCTestCase {
    func testEmailCodeLoginAndAccountHistoryThroughOTA() {
        let app=XCUIApplication()
        app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13491"]
        app.launch()
        XCTAssertEqual(app.webViews.count,0)
        let apply=app.alerts.buttons["Apply update"]
        XCTAssertTrue(apply.waitForExistence(timeout:90));apply.tap()
        let email=app.textFields["Email address"]
        XCTAssertTrue(email.waitForExistence(timeout:20));email.tap();email.typeText("ada@example.com")
        app.buttons["Send code"].tap()
        let code=app.textFields["Email code"]
        XCTAssertTrue(code.waitForExistence(timeout:10))
        app.buttons["Send a new code"].tap()
        XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("123456")
        app.buttons["Log in"].tap()
        XCTAssertTrue(app.staticTexts["That code isn't right. 4 tries left."].waitForExistence(timeout:10))
        // A new email also clears the field through the shared component value command.
        app.buttons["Send a new code"].tap()
        XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926")
        app.buttons["Log in"].tap()
        XCTAssertTrue(app.staticTexts["My first saved chat"].waitForExistence(timeout:15))
        XCTAssertTrue(app.staticTexts["Another saved chat"].exists)
        // Exercise real UIKit gestures on the OTA surface, not a browser mock.
        let title=app.staticTexts["My first saved chat"]
        let frame=title.frame
        app.firstMatch.pinch(withScale:2,velocity:1)
        XCTAssertEqual(title.frame.width,frame.width,accuracy:1)
        XCTAssertEqual(title.frame.minX,frame.minX,accuracy:1)
        app.firstMatch.pinch(withScale:0.5,velocity:-1)
        XCTAssertEqual(title.frame.width,frame.width,accuracy:1)
        XCTAssertEqual(title.frame.minX,frame.minX,accuracy:1)
        title.doubleTap()
        XCTAssertEqual(title.frame.width,frame.width,accuracy:1)
        XCTAssertEqual(title.frame.minX,frame.minX,accuracy:1)
        let shot=XCTAttachment(screenshot:app.screenshot());shot.name="Email login and account chats via OTA";shot.lifetime = .keepAlways;add(shot)
        // Let the signed package complete its unchanged foreground health window.
        let health=expectation(description:"foreground health")
        DispatchQueue.main.asyncAfter(deadline:.now()+31) {health.fulfill()}
        wait(for:[health],timeout:35)
        app.terminate();app.launch()
        XCTAssertTrue(app.staticTexts["My first saved chat"].waitForExistence(timeout:15))
        app.buttons["Log out"].tap()
        XCTAssertTrue(email.waitForExistence(timeout:10))
        app.terminate();app.launch()
        XCTAssertTrue(email.waitForExistence(timeout:15))
    }

    // Run after the OTA flow, on the healthy package with the keyboard closed.
    func testViewportDoesNotZoom() {
        let app=XCUIApplication()
        app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13491"]
        app.launch()
        let title=app.staticTexts["Log in"]
        XCTAssertTrue(title.waitForExistence(timeout:20))
        let frame=title.frame
        app.firstMatch.pinch(withScale:2,velocity:1)
        XCTAssertEqual(title.frame.width,frame.width,accuracy:1)
        XCTAssertEqual(title.frame.minX,frame.minX,accuracy:1)
        app.firstMatch.pinch(withScale:0.5,velocity:-1)
        XCTAssertEqual(title.frame.width,frame.width,accuracy:1)
        XCTAssertEqual(title.frame.minX,frame.minX,accuracy:1)
        title.doubleTap()
        XCTAssertEqual(title.frame.width,frame.width,accuracy:1)
        XCTAssertEqual(title.frame.minX,frame.minX,accuracy:1)
    }
}
