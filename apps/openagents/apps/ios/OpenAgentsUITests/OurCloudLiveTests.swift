import XCTest

/// Our cloud (#11852), live: a dedicated test account starts the engine on a
/// cloud computer from the phone, runs a session on it with no desktop,
/// follows the reply, and stops it. Run with OUR_CLOUD_CREDENTIAL_URL set to
/// a loopback URL that serves the test account's credential once.
final class OurCloudLiveTests: XCTestCase {
    private func capture(_ app: XCUIApplication, _ name: String) {
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = name
        shot.lifetime = .keepAlways
        add(shot)
    }

    private func openSettings(_ app: XCUIApplication) {
        app.openSettings(timeout: 30)
    }

    func testLocalCloudControls() throws {
        continueAfterFailure = false
        guard let base = ProcessInfo.processInfo.environment["OUR_CLOUD_FIXTURE_URL"] else {
            throw XCTSkip("Run scripts/ios/check-cloud-controls.py.")
        }
        let app = XCUIApplication()
        app.launchArguments = ["-no-auth-browser", "-account-fixture", "-account-api", base + "/v1",
                               "-account-site", base, "-account-test-credential-url", base + "/credential", "-edge", base]
        app.launch()
        openSettings(app)
        let row = app.cells["settings-cloudhost"]
        for _ in 0..<4 where !row.isHittable { app.swipeUp() }
        XCTAssertTrue(row.waitForExistence(timeout: 20))
        XCTAssertTrue(app.staticTexts["Start Our cloud"].exists)
        row.tap()
        XCTAssertTrue(app.staticTexts["Stop Our cloud"].waitForExistence(timeout: 20))
        row.tap()
        XCTAssertTrue(app.staticTexts["Start Our cloud"].waitForExistence(timeout: 20))
        capture(app, "our-cloud-controls")
        let signOut = app.cells["settings-signout"]
        for _ in 0..<8 where !signOut.isHittable { app.swipeUp() }
        signOut.tap()
        app.buttons["Sign Out"].tap()
        XCTAssertTrue(app.otherElements["cloud-chat"].waitForExistence(timeout: 15))
        XCTAssertFalse(app.staticTexts["Stop Our cloud"].exists)
    }

    func testStartRunAndStopOurCloud() throws {
        continueAfterFailure = false
        guard let url = ProcessInfo.processInfo.environment["OUR_CLOUD_CREDENTIAL_URL"] else {
            throw XCTSkip("Run with a dedicated test account's credential URL.")
        }
        let app = XCUIApplication()
        app.launchArguments = ["-no-auth-browser", "-account-fixture", "-account-test-credential-url", url]
        app.launch()

        // Start Our cloud from Settings.
        openSettings(app)
        let row = app.cells["settings-cloudhost"]
        for _ in 0..<4 where !row.isHittable { app.swipeUp() }
        XCTAssertTrue(row.waitForExistence(timeout: 30), "Settings offers Our cloud")
        capture(app, "our-cloud-offered")
        row.tap()
        XCTAssertTrue(app.staticTexts["Stop Our cloud"].waitForExistence(timeout: 120), "Our cloud starts")
        capture(app, "our-cloud-starting")

        // Its engine joins as a device.
        let device = app.cells.containing(NSPredicate(format: "label CONTAINS %@", "Our cloud")).matching(NSPredicate(format: "label CONTAINS %@", "Online")).firstMatch
        XCTAssertTrue(device.waitForExistence(timeout: 600), "Our cloud's engine comes online")
        capture(app, "our-cloud-online")

        // A session on it, with no desktop anywhere.
        XCTAssertTrue(app.openDrawer())
        app.buttons["new-session"].tap()
        let host = app.buttons["composer-chip-host"]
        XCTAssertTrue(host.waitForExistence(timeout: 15))
        if host.label != "Our cloud" {
            host.tap()
            let choice = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "Our cloud")).firstMatch
            XCTAssertTrue(choice.waitForExistence(timeout: 10))
            choice.tap()
        }
        app.buttons["composer-chip-model"].tap()
        let model = app.buttons["OpenAgents Fast"].firstMatch
        XCTAssertTrue(model.waitForExistence(timeout: 30), "Our cloud offers OpenAgents inference")
        model.tap()
        let input = app.textViews["composer-input"]
        XCTAssertTrue(input.waitForExistence(timeout: 10))
        input.tap()
        input.typeText("Reply with exactly the words: our cloud works")
        app.buttons["composer-send"].tap()
        let reply = app.staticTexts.matching(identifier: "row-markdown").matching(NSPredicate(format: "label CONTAINS[c] %@", "our cloud works")).firstMatch
        XCTAssertTrue(reply.waitForExistence(timeout: 300), "The cloud engine's reply reaches the phone")
        XCTAssertTrue(app.buttons["Send message"].waitForExistence(timeout: 120), "The turn finishes")
        capture(app, "our-cloud-reply")

        // Stop it.
        app.navigationBars.buttons.firstMatch.tap()
        openSettings(app)
        for _ in 0..<4 where !row.isHittable { app.swipeUp() }
        row.tap()
        XCTAssertTrue(app.staticTexts["Start Our cloud"].waitForExistence(timeout: 120), "Our cloud stops")
        capture(app, "our-cloud-stopped")
    }
}
