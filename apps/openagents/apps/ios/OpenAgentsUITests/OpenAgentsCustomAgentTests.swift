import XCTest

/// Dedicated-account fixture only. The coordinator removes its chats and definitions.
final class OpenAgentsCustomAgentTests: XCTestCase {
    func testCreateRelaunchEditAndAssignAgent() throws {
        continueAfterFailure = false
        guard let raw = ProcessInfo.processInfo.environment["CODER_MOBILE_SYNC_CONFIG"],
              let data = raw.data(using: .utf8),
              let config = try JSONSerialization.jsonObject(with: data) as? [String: String] else {
            throw XCTSkip("Run with the dedicated custom-agent coordinator.")
        }
        let app = XCUIApplication()
        app.launchArguments = ["-no-auth-browser", "-account-fixture", "-account-test-credential-url", try XCTUnwrap(config["credential_url"]), "-edge", try XCTUnwrap(config["sync"])]
        app.launch()
        defer {
            app.terminate()
            app.launchArguments = ["-account-fixture", "-signedout", "-no-auth-browser"]
            app.launch()
        }
        XCTAssertTrue(app.buttons["new-session"].waitForExistence(timeout: 90))
        app.buttons["new-session"].tap()
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.25)).tap()
        _ = app.keyboards.firstMatch.waitForNonExistence(timeout: 5)
        app.buttons["composer-chip-project"].tap()
        app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", (config["project"] ?? "Mobile fixture") + ",")).firstMatch.tap()
        app.buttons["composer-chip-model"].tap()
        app.buttons["Create agent…"].tap()
        let name = app.textViews["agent-name"]
        XCTAssertTrue(name.waitForExistence(timeout: 10))
        name.tap()
        name.typeText("Phone custom agent fixture")
        let instructions = app.textViews["agent-instructions"]
        instructions.tap()
        instructions.typeText("Reply with the assigned task.")
        let runtime = app.buttons["agent-runtime-sdk"]
        for _ in 0..<8 where !runtime.isHittable { app.swipeUp() }
        runtime.tap()
        let runner = app.textViews["agent-runner"]
        for _ in 0..<6 where !runner.isHittable { app.swipeUp() }
        runner.tap()
        runner.typeText("fixture")
        let save = app.buttons["agent-save"]
        for _ in 0..<8 where !save.isHittable { app.swipeUp() }
        let form = XCTAttachment(screenshot: app.screenshot())
        form.name = "custom-agent-rust-native-form"
        form.lifetime = .keepAlways
        add(form)
        save.tap()
        XCTAssertTrue(name.waitForNonExistence(timeout: 30))
        XCTAssertTrue(app.buttons["composer-chip-model"].waitForExistence(timeout: 10))
        XCTAssertEqual(app.buttons["composer-chip-model"].label, "Phone custom agent fixture")
        // The saved definition survives a process restart without saving work yet.
        app.terminate()
        app.launchArguments = ["-no-auth-browser", "-account-fixture", "-edge", try XCTUnwrap(config["sync"])]
        app.launch()
        XCTAssertTrue(app.buttons["new-session"].waitForExistence(timeout: 60))
        app.buttons["new-session"].tap()
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.25)).tap()
        _ = app.keyboards.firstMatch.waitForNonExistence(timeout: 5)
        app.buttons["composer-chip-model"].tap()
        let saved = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "Phone custom agent fixture,")).firstMatch
        XCTAssertTrue(saved.waitForExistence(timeout: 30))
        saved.tap()
        app.buttons["composer-chip-model"].tap()
        app.buttons["Edit agent…"].tap()
        let edited = app.textViews["agent-instructions"]
        XCTAssertTrue(edited.waitForExistence(timeout: 10))
        edited.tap()
        edited.typeText(" Keep the reply short.")
        let update = app.buttons["agent-save"]
        for _ in 0..<8 where !update.isHittable { app.swipeUp() }
        update.tap()
        XCTAssertTrue(edited.waitForNonExistence(timeout: 30))
        let composer = app.textViews["composer-input"]
        composer.tap()
        composer.typeText("Phone custom agent task")
        app.buttons["composer-send"].tap()
        let reply = app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", "received: Phone custom agent task")).firstMatch
        XCTAssertTrue(reply.waitForExistence(timeout: 90))
        let result = XCTAttachment(screenshot: app.screenshot())
        result.name = "custom-agent-live-result"
        result.lifetime = .keepAlways
        add(result)
        app.navigationBars.buttons.firstMatch.tap()
        XCTAssertTrue(app.tabBars.buttons["Settings"].waitForExistence(timeout: 10))
        app.tabBars.buttons["Settings"].tap()
        let signOut = app.cells["settings-signout"]
        for _ in 0..<6 where !signOut.isHittable { app.swipeUp() }
        signOut.tap()
        app.buttons["Sign Out"].tap()
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 10))
    }
}
