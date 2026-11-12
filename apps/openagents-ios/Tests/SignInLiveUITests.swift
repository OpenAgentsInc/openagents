import XCTest

/// The real flows against a real server (#13229), run by hand. Each is skipped without its input.
///
/// The app's own login, with this phone's key (no inbox needed):
///   TEST_RUNNER_OA_LIVE_ORIGIN=https://staging.openagents.com \
///   xcodebuild … -only-testing:OpenAgentsUITests/SignInLiveUITests/testKeyLoginLive test
/// It opens Log in, logs out if it was logged in, taps "Log in with this phone's key", waits for
/// "Logged in." (the server bound the login to the phone's device key), and logs out.
///
/// The /login page's QR, approved by the phone, with a fresh code from that page:
///   TEST_RUNNER_OA_SIGNIN_CODE='https://staging.openagents.com/login#c=…' \
///   [TEST_RUNNER_OA_SIGNIN_EXPECT='Logged in. Look at your computer.'] \
///   xcodebuild … -only-testing:OpenAgentsUITests/SignInLiveUITests/testApproveLoginPageLive test
/// It opens the code as a link through the system (tapping its "Open in" prompt), approves, and
/// checks what the phone says.
final class SignInLiveUITests: XCTestCase {
    private func wait(_ timeout: TimeInterval, _ test: @escaping () -> Bool) -> Bool {
        XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in test() }, object: nil)], timeout: timeout) == .completed
    }
    private func shot(_ app: XCUIApplication, _ name: String) {
        let a = XCTAttachment(screenshot: app.screenshot()); a.name = name; a.lifetime = .keepAlways; add(a)
    }

    func testKeyLoginLive() throws {
        let env = ProcessInfo.processInfo.environment
        guard let origin = env["OA_LIVE_ORIGIN"], !origin.isEmpty else { throw XCTSkip("no OA_LIVE_ORIGIN") }
        let app = XCUIApplication()
        app.launchArguments += ["-OAGridSeen", "NO", "-OANativeHome", "YES", "-OAAPIOrigin", origin]
        app.launch()
        XCTAssertTrue(app.buttons["signin.open"].waitForExistence(timeout: 30))
        app.buttons["signin.open"].tap()
        let state = app.staticTexts["login.state"]
        XCTAssertTrue(state.waitForExistence(timeout: 10))
        if app.buttons["login.logout"].exists && app.buttons["login.logout"].isHittable {
            app.buttons["login.logout"].tap()
            XCTAssertTrue(wait(30) { app.buttons["login.key"].exists && app.buttons["login.key"].isHittable })
        }
        app.buttons["login.key"].tap()
        XCTAssertTrue(wait(30) { state.label == "Logged in." }, "\(state.label) / \(app.staticTexts["login.message"].label)")
        shot(app, "Live key login")
        print("OA_LIVE_RESULT: \(state.label)")
        app.buttons["login.logout"].tap()
        XCTAssertTrue(wait(30) { app.buttons["login.key"].exists && app.buttons["login.key"].isHittable })
    }

    func testApproveLoginPageLive() throws {
        let env = ProcessInfo.processInfo.environment
        guard let code = env["OA_SIGNIN_CODE"], !code.isEmpty else { throw XCTSkip("no OA_SIGNIN_CODE") }
        let app = XCUIApplication()
        app.launchArguments += ["-OAGridSeen", "NO", "-OANativeHome", "YES"]
        app.launch()
        XCTAssertTrue(app.buttons["signin.open"].waitForExistence(timeout: 30))
        // openagents://HOST/… goes through the scheme; an https link would open Safari without the app's domain file.
        let link = code.hasPrefix("https://") ? "openagents://" + code.dropFirst(8) : code
        XCUIDevice.shared.system.open(try XCTUnwrap(URL(string: link)))
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let open = springboard.buttons["Open"]
        if open.waitForExistence(timeout: 5) { open.tap() }
        let question = app.staticTexts["signin.question"]
        XCTAssertTrue(question.waitForExistence(timeout: 20))
        app.buttons["signin.approve"].tap()
        let result = app.staticTexts["signin.result"]
        XCTAssertTrue(wait(30) { !result.label.isEmpty })
        shot(app, "Live result")
        print("OA_SIGNIN_RESULT: \(result.label)")
        XCTAssertEqual(result.label, env["OA_SIGNIN_EXPECT"].flatMap { $0.isEmpty ? nil : $0 } ?? "Logged in. Look at your computer.")
    }
}
