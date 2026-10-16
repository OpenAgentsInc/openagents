import XCTest

final class CoderIdentityTests: XCTestCase {
    func testSignedOutCanOpenDemo() {
        let app = XCUIApplication()
        app.launchArguments = ["-signedout"]
        app.launch()
        XCTAssertTrue(app.staticTexts["OpenAgents"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["signed-out"].exists)
        XCTAssertFalse(app.buttons["sign-in"].exists)
        app.buttons["open-demo"].tap()
        XCTAssertTrue(app.cells["section-pinned"].waitForExistence(timeout: 10))
    }

    func testDemoLaunch() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo"]
        app.launch()
        XCTAssertTrue(app.staticTexts["Sessions"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.cells["section-pinned"].exists)
    }
}
