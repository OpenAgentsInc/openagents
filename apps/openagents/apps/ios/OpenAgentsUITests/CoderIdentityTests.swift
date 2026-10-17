import XCTest

final class CoderIdentityTests: XCTestCase {
    private func openSessions(_ app: XCUIApplication) {
        XCTAssertTrue(app.openDrawer(timeout: 15))
    }

    func testSignedOutCanOpenDemo() {
        let app = XCUIApplication()
        app.launchArguments = ["-signedout"]
        app.launch()
        XCTAssertTrue(app.staticTexts["OpenAgents"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["signed-out"].exists)
        XCTAssertTrue(app.buttons["account-sign-in"].exists)
        app.buttons["open-demo"].tap()
        openSessions(app)
        XCTAssertTrue(app.cells["section-pinned"].waitForExistence(timeout: 10))
    }

    func testDemoLaunch() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo"]
        app.launch()
        openSessions(app)
        XCTAssertTrue(app.cells["section-pinned"].exists)
    }
}
