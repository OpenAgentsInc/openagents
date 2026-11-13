import XCTest
final class BalanceUITests:XCTestCase {
    func testBalanceIsOnlyInExtrasAndNoPaymentActionsAreExposed() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OABalanceFixture","12345"];app.launch()
        let more=app.webViews.buttons["MORE"];XCTAssertTrue(more.waitForExistence(timeout:20))
        XCTAssertFalse(app.webViews.staticTexts["12345 sats"].exists)
        more.tap();app.webViews.buttons["BITCOIN BALANCE"].tap()
        XCTAssertTrue(app.webViews.staticTexts["12345 sats"].waitForExistence(timeout:10))
        XCTAssertFalse(app.webViews.buttons["SEND"].exists);XCTAssertFalse(app.webViews.buttons["RECEIVE"].exists)
        app.webViews.buttons["REFRESH BALANCE"].tap()
        XCTAssertTrue(app.webViews.staticTexts["12345 sats"].waitForExistence(timeout:5))
        let shot=XCTAttachment(screenshot:app.screenshot());shot.name="Read-only Spark balance fixture";shot.lifetime = .keepAlways;add(shot)
        app.webViews.buttons["CLOSE"].tap();XCTAssertFalse(app.webViews.staticTexts["12345 sats"].exists)
    }
    func testMissingWalletIsNotDisplayedAsZero() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OABalanceFixture","missing"];app.launch()
        XCTAssertTrue(app.webViews.buttons["MORE"].waitForExistence(timeout:20))
        app.webViews.buttons["MORE"].tap();app.webViews.buttons["BITCOIN BALANCE"].tap()
        XCTAssertTrue(app.webViews.staticTexts["No existing wallet was found on this phone."].waitForExistence(timeout:10))
        XCTAssertFalse(app.webViews.staticTexts["0 sats"].exists)
    }
}
