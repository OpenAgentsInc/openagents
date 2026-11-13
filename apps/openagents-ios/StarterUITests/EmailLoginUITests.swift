import XCTest

/// Uses build 1069's retained simulator app and an independently signed OTA.
/// Tests/email_fixture.py supplies a fake mailbox/API, while the real native Keychain,
/// device-key, HTTP and update-verification services run unchanged.
final class EmailLoginUITests:XCTestCase {
    func testEmailCodeLoginAndAccountHistoryThroughOTA() {
        let app=XCUIApplication()
        app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13491"]
        app.launch()
        let apply=app.alerts.buttons["Apply update"]
        XCTAssertTrue(apply.waitForExistence(timeout:90));apply.tap()
        let email=app.webViews.textFields["Email address"]
        XCTAssertTrue(email.waitForExistence(timeout:20));email.tap();email.typeText("ada@example.com")
        app.webViews.buttons["SEND CODE"].tap()
        let code=app.webViews.textFields["Email code"]
        XCTAssertTrue(code.waitForExistence(timeout:10))
        app.webViews.buttons["SEND A NEW CODE"].tap()
        XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("123456")
        app.webViews.buttons["LOG IN"].tap()
        XCTAssertTrue(app.webViews.staticTexts["That code isn't right. 4 tries left."].waitForExistence(timeout:10))
        // A new email also clears the field through the real DOM value command.
        app.webViews.buttons["SEND A NEW CODE"].tap()
        XCTAssertTrue(code.waitForExistence(timeout:10));code.tap();code.typeText("415926")
        app.webViews.buttons["LOG IN"].tap()
        XCTAssertTrue(app.webViews.staticTexts["My first saved chat"].waitForExistence(timeout:15))
        XCTAssertTrue(app.webViews.staticTexts["Another saved chat"].exists)
        let shot=XCTAttachment(screenshot:app.screenshot());shot.name="Email login and account chats via OTA";shot.lifetime = .keepAlways;add(shot)
        // Let the signed package complete its unchanged foreground health window.
        let health=expectation(description:"foreground health")
        DispatchQueue.main.asyncAfter(deadline:.now()+31) {health.fulfill()}
        wait(for:[health],timeout:35)
        app.terminate();app.launch()
        XCTAssertTrue(app.webViews.staticTexts["My first saved chat"].waitForExistence(timeout:15))
        app.webViews.buttons["LOG OUT"].tap()
        XCTAssertTrue(email.waitForExistence(timeout:10))
        app.terminate();app.launch()
        XCTAssertTrue(email.waitForExistence(timeout:15))
    }
}
