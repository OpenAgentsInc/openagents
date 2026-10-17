import XCTest

final class LaunchLifecycleTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func backgroundAndReturn(_ app: XCUIApplication) {
        XCUIDevice.shared.press(.home)
        let backgrounded = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            app.state == .runningBackground || app.state == .runningBackgroundSuspended
        }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [backgrounded], timeout: 10), .completed,
            "The scene must survive saving its restoration state; state=\(app.state.rawValue).")
        app.activate()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10))
    }

    func testFirstLaunchCanOpenDemoAndRestore() {
        let app = XCUIApplication()
        app.launch()
        capture(app, "release-first-launch")
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 15))
        XCTAssertTrue(app.staticTexts["OpenAgents"].exists)
        backgroundAndReturn(app)
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 5))
        app.buttons["open-demo"].tap()
        XCTAssertTrue(app.buttons["edit"].waitForExistence(timeout: 15))
        capture(app, "release-demo-opened")
        backgroundAndReturn(app)
        XCTAssertTrue(app.buttons["edit"].waitForExistence(timeout: 5))
        capture(app, "release-demo-restored")
    }

    func testDemoSurvivesRepeatedSceneTransitionsAndRelaunch() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo"]
        app.launch()
        XCTAssertTrue(app.buttons["edit"].waitForExistence(timeout: 15))
        for _ in 0..<3 {
            backgroundAndReturn(app)
            XCTAssertTrue(app.buttons["edit"].waitForExistence(timeout: 5))
        }
        app.terminate()
        app.launch()
        XCTAssertTrue(app.buttons["edit"].waitForExistence(timeout: 15))
        capture(app, "release-demo-relaunched")
    }
}
