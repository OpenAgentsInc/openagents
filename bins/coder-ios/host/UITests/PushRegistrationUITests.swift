// Push token handoff in a build configured for push. It runs only when the
// test runner sets CODER_PUSH_TEST=1 (pass TEST_RUNNER_CODER_PUSH_TEST=1 to
// xcodebuild) against an app built with CODER_IOS_PUSH=development and
// loopback push settings, as bins/coder-ios/README.md describes. Default
// builds skip it. Nothing here contacts APNs's delivery path or a gateway.
import XCTest

final class PushRegistrationUITests: XCTestCase {
    func testConfiguredBuildPassesTheTokenToRust() throws {
        guard ProcessInfo.processInfo.environment["CODER_PUSH_TEST"] == "1" else {
            throw XCTSkip("Set TEST_RUNNER_CODER_PUSH_TEST=1 with a push-configured build.")
        }
        continueAfterFailure = false
        let app = XCUIApplication()
        // A synthetic world with loopback push settings.
        app.launchArguments = ["--synthetic", "--loopback-test"]
        app.launch()
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let allow = springboard.buttons["Allow"]
        if allow.waitForExistence(timeout: 15) { allow.tap() }
        app.openWorldComputer()
        app.buttons["computer-settings"].tap()
        let details = app.buttons["Device details"]
        XCTAssertTrue(details.waitForExistence(timeout: 10))
        for _ in 0..<6 where !details.isHittable { app.scrollViews.firstMatch.swipeUp() }
        details.tap()
        let status = app.staticTexts["push-status"]
        XCTAssertTrue(status.waitForExistence(timeout: 20), "A push-configured build shows its wake status.")
        // The loopback gateway isn't running, so Rust's registration fails
        // after it receives the token. A native failure would say so instead.
        let handedOver = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "label BEGINSWITH %@", "Wakes unavailable:"), object: status)
        XCTAssertEqual(XCTWaiter.wait(for: [handedOver], timeout: 45), .completed, status.label)
        XCTAssertFalse(status.label.contains("Couldn't register"), status.label)
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = "Push token passed to Rust"
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
