import XCTest

/// The real flow against a real server, run by hand with a fresh code:
///   TEST_RUNNER_OA_SIGNIN_CODE='https://staging.openagents.com/signin#c=…' \
///   [TEST_RUNNER_OA_SIGNIN_EXPECT='Signed in. Look at your computer.'] \
///   xcodebuild … -only-testing:OpenAgentsUITests/SignInLiveUITests test
/// It opens the code as a link through the system (tapping its "Open in"
/// prompt), approves, and checks what the phone says. Skipped without a code.
final class SignInLiveUITests: XCTestCase {
    func testOpenApproveLive() throws {
        let env = ProcessInfo.processInfo.environment
        let code = try XCTUnwrap(env["OA_SIGNIN_CODE"].flatMap { $0.isEmpty ? nil : $0 }, "no OA_SIGNIN_CODE")
        let app = XCUIApplication()
        app.launchArguments += ["-OAGridSeen", "NO"]
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
        let done = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in !result.label.isEmpty }, object: nil)
        XCTAssertEqual(XCTWaiter.wait(for: [done], timeout: 30), .completed)
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "Live result"; shot.lifetime = .keepAlways; add(shot)
        print("OA_SIGNIN_RESULT: \(result.label)")
        if let want = env["OA_SIGNIN_EXPECT"], !want.isEmpty { XCTAssertEqual(result.label, want) }
        else { XCTAssertTrue(result.label.hasPrefix("Signed in") || result.label.hasPrefix("This phone is linked"), result.label) }
    }
}
