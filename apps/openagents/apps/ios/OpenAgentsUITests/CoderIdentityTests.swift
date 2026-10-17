import XCTest

final class CoderIdentityTests: XCTestCase {
    private func openSessions(_ app: XCUIApplication) {
        XCTAssertTrue(app.openDrawer(timeout: 15))
    }

    /// Without an account the app opens on the chat, and nothing asks the
    /// person to sign in (#11996).
    func testWithoutAccountLandsOnChat() {
        let app = XCUIApplication()
        app.launchArguments = ["-signedout"]
        app.launch()
        XCTAssertTrue(app.staticTexts["cloud-chat-empty"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["drawer-menu"].exists)
        XCTAssertFalse(app.buttons["account-sign-in"].exists)
    }

    func testDemoLaunch() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo"]
        app.launch()
        openSessions(app)
        XCTAssertTrue(app.cells["section-pinned"].exists)
    }
}
