import XCTest

/// The coordinator supplies a dedicated account and an isolated host with a Rust runner.
final class OpenAgentsAgentLabTests: XCTestCase {
    func testSaveRunReviewAndUseAgent() throws {
        continueAfterFailure = false
        guard let raw = ProcessInfo.processInfo.environment["CODER_MOBILE_SYNC_CONFIG"],
              let data = raw.data(using: .utf8), let config = try JSONSerialization.jsonObject(with: data) as? [String: String] else { throw XCTSkip("Run with the Agent Lab coordinator.") }
        let app = XCUIApplication()
        app.launchArguments = ["-no-auth-browser", "-account-fixture", "-account-test-credential-url", try XCTUnwrap(config["credential_url"]), "-edge", try XCTUnwrap(config["sync"])]
        app.launch()
        defer { app.terminate(); app.launchArguments = ["-account-fixture", "-signedout", "-no-auth-browser"]; app.launch() }
        XCTAssertTrue(app.buttons["new-session"].waitForExistence(timeout: 90))
        app.buttons["new-session"].tap()
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.25)).tap()
        _ = app.keyboards.firstMatch.waitForNonExistence(timeout: 5)
        app.buttons["composer-chip-model"].tap()
        app.buttons["Agent Lab…"].tap()
        let name = app.textViews["lab-name"]
        XCTAssertTrue(name.waitForExistence(timeout: 15))
        name.tap(); name.typeText(" phone fixture")
        func reveal(_ element: XCUIElement, up: Bool = true) {
            for _ in 0..<25 where !element.isHittable { if up { app.swipeUp() } else { app.swipeDown() } }
            XCTAssertTrue(element.isHittable)
        }
        let sdk = app.buttons["lab-runtime:sdk"]
        reveal(sdk); sdk.tap()
        let namedProject = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", try XCTUnwrap(config["project"]))).firstMatch
        reveal(namedProject); namedProject.tap()
        let runner = app.buttons["lab-runner:fixture"]
        reveal(runner); runner.tap()
        let save = app.buttons["lab-save"]
        reveal(save); save.tap()
        let run = app.buttons["lab-start"]
        XCTAssertTrue(run.waitForExistence(timeout: 30)); reveal(run); run.tap()
        let passed = app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "1 of 1 passed")).firstMatch
        XCTAssertTrue(passed.waitForExistence(timeout: 90)); reveal(passed)
        let result = XCTAttachment(screenshot: app.screenshot()); result.name = "agent-lab-result"; result.lifetime = .keepAlways; add(result)
        let use = app.buttons["lab-promote"]
        reveal(use, up: false); use.tap()
        let ready = app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "ready in your agent picker")).firstMatch
        XCTAssertTrue(ready.waitForExistence(timeout: 30))
        app.navigationBars.buttons.firstMatch.tap()
        app.buttons["composer-chip-model"].tap()
        XCTAssertTrue(app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "phone fixture,")).firstMatch.waitForExistence(timeout: 30))
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.95)).tap()
        app.navigationBars.buttons.firstMatch.tap()
        app.tabBars.buttons["Settings"].tap()
        let signOut = app.cells["settings-signout"]
        reveal(signOut); signOut.tap(); app.buttons["Sign Out"].tap()
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 10))
    }
}
