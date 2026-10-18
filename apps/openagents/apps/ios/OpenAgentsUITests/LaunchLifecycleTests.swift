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
        // The tutorial shows once per install (#11996); a simulator that
        // already finished it lands on the chat.
        let tutorial = app.otherElements["onboarding"]
        guard tutorial.waitForExistence(timeout: 15) else {
            XCTAssertTrue(app.staticTexts["cloud-chat-empty"].exists)
            XCTAssertFalse(app.buttons["account-sign-in"].exists)
            return
        }
        backgroundAndReturn(app)
        XCTAssertTrue(tutorial.waitForExistence(timeout: 5))
        app.buttons["onboarding-skip"].tap()
        XCTAssertTrue(app.buttons["drawer-menu"].waitForExistence(timeout: 15))
        capture(app, "release-tutorial-finished")
        backgroundAndReturn(app)
        XCTAssertTrue(app.buttons["drawer-menu"].waitForExistence(timeout: 5))
        capture(app, "release-tutorial-restored")
    }

    func testDemoSurvivesRepeatedSceneTransitionsAndRelaunch() {
        let app = XCUIApplication()
        app.launchArguments = ["-demo"]
        app.launch()
        XCTAssertTrue(app.buttons["drawer-menu"].waitForExistence(timeout: 15))
        for _ in 0..<3 {
            backgroundAndReturn(app)
            XCTAssertTrue(app.buttons["drawer-menu"].waitForExistence(timeout: 5))
        }
        app.terminate()
        app.launch()
        XCTAssertTrue(app.buttons["drawer-menu"].waitForExistence(timeout: 15))
        capture(app, "release-demo-relaunched")
    }
}
