import XCTest
final class BalanceUITests:XCTestCase {
    func testBalanceIsOnlyInExtrasAndNoPaymentActionsAreExposed() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13567","-OABalanceFixture","12345"];app.launch()
        XCTAssertTrue(app.wait(for:.runningForeground,timeout:20))
        XCTAssertFalse(app.staticTexts["12345 sats"].exists)
        app.openAppExtras();app.buttons["extras.bitcoin"].tap()
        XCTAssertTrue(app.staticTexts["12345 sats"].waitForExistence(timeout:10))
        XCTAssertFalse(app.buttons["SEND"].exists);XCTAssertFalse(app.buttons["RECEIVE"].exists)
        app.buttons["Refresh balance"].tap()
        XCTAssertTrue(app.staticTexts["12345 sats"].waitForExistence(timeout:5))
        let shot=XCTAttachment(screenshot:app.screenshot());shot.name="Read-only Spark balance fixture";shot.lifetime = .keepAlways;add(shot)
        app.buttons["Close"].tap();XCTAssertFalse(app.staticTexts["12345 sats"].exists)
    }
    func testMissingWalletIsNotDisplayedAsZero() {
        let app=XCUIApplication();app.launchArguments=["-OAAutomation","YES","-OAAPIOrigin","http://127.0.0.1:13567","-OABalanceFixture","missing"];app.launch()
        app.openAppExtras();app.buttons["extras.bitcoin"].tap()
        XCTAssertTrue(app.staticTexts["No existing wallet was found on this phone."].waitForExistence(timeout:10))
        XCTAssertFalse(app.staticTexts["0 sats"].exists)
    }
}
