import XCTest

/// The offline demo's way out (#12011): the chip at the top of the shell and
/// the Settings row both leave the demo without restarting the app, and
/// none of the demo's sessions stay behind. `-signedout` keeps the phone
/// signed out after the demo, never on a saved login.
final class DemoExitTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    private func launchDemo() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-demo", "-fast", "-signedout"]
        app.launch()
        XCTAssertTrue(app.buttons["demo-exit"].waitForExistence(timeout: 15))
        XCTAssertTrue(app.openDrawer())
        XCTAssertTrue(app.cells["section-pinned"].exists, "The demo's sessions show in the drawer.")
        app.buttons["sidebar-chat"].tap()
        return app
    }

    /// Back in the real app: no chip, the chat landing, no demo sessions.
    private func assertLeftDemo(_ app: XCUIApplication) {
        let chip = app.buttons["demo-exit"]
        let gone = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: chip)
        XCTAssertEqual(XCTWaiter.wait(for: [gone], timeout: 10), .completed, "The chip goes away with the demo.")
        XCTAssertTrue(app.staticTexts["cloud-chat-empty"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.openDrawer())
        XCTAssertFalse(app.cells["section-pinned"].exists, "No demo session stays behind.")
        app.buttons["sidebar-settings"].tap()
        XCTAssertTrue(app.cells["settings-account"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.cells["settings-exit-demo"].exists)
    }

    func testChipLeavesTheDemo() {
        let app = launchDemo()
        app.buttons["demo-exit"].tap()
        assertLeftDemo(app)
    }

    func testSettingsRowLeavesTheDemo() {
        let app = launchDemo()
        XCTAssertTrue(app.openDrawer())
        app.buttons["sidebar-settings"].tap()
        let row = app.cells["settings-exit-demo"]
        XCTAssertTrue(row.waitForExistence(timeout: 5))
        XCTAssertFalse(app.cells["settings-signout"].exists, "The demo has nothing to sign out of.")
        row.tap()
        assertLeftDemo(app)
    }
}
