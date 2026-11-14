import XCTest
final class BalanceUITests:XCTestCase {
    func testBalanceIsOnlyInExtrasAndNoPaymentActionsAreExposed() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OABalanceFixture","12345"];app.launch()
        let more=app.buttons["More"];XCTAssertTrue(more.waitForExistence(timeout:20))
        XCTAssertFalse(app.staticTexts["12345 sats"].exists)
        more.tap();app.buttons["Bitcoin balance"].tap()
        XCTAssertTrue(app.staticTexts["12345 sats"].waitForExistence(timeout:10))
        XCTAssertFalse(app.buttons["SEND"].exists);XCTAssertFalse(app.buttons["RECEIVE"].exists)
        app.buttons["Refresh balance"].tap()
        XCTAssertTrue(app.staticTexts["12345 sats"].waitForExistence(timeout:5))
        let shot=XCTAttachment(screenshot:app.screenshot());shot.name="Read-only Spark balance fixture";shot.lifetime = .keepAlways;add(shot)
        app.buttons["Close"].tap();XCTAssertFalse(app.staticTexts["12345 sats"].exists)
    }
    func testMissingWalletIsNotDisplayedAsZero() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OABalanceFixture","missing"];app.launch()
        XCTAssertTrue(app.buttons["More"].waitForExistence(timeout:20))
        app.buttons["More"].tap();app.buttons["Bitcoin balance"].tap()
        XCTAssertTrue(app.staticTexts["No existing wallet was found on this phone."].waitForExistence(timeout:10))
        XCTAssertFalse(app.staticTexts["0 sats"].exists)
    }
}
