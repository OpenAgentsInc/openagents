import XCTest

/// The coordinator supplies a dedicated account and an isolated host with a Rust runner.
final class OpenAgentsAgentLabTests: XCTestCase {
    func testSaveRunInspectAndUseAgent() throws {
        continueAfterFailure = false
        guard let raw = ProcessInfo.processInfo.environment["CODER_MOBILE_SYNC_CONFIG"],
              let data = raw.data(using: .utf8), let config = try JSONSerialization.jsonObject(with: data) as? [String: String] else { throw XCTSkip("Run with the Agent Lab coordinator.") }
        let app = XCUIApplication()
        app.launchArguments = ["-no-auth-browser", "-account-fixture", "-account-test-credential-url", try XCTUnwrap(config["credential_url"]), "-edge", try XCTUnwrap(config["sync"])]
        app.launch()
        defer { app.terminate(); app.launchArguments = ["-account-fixture", "-signedout", "-no-auth-browser"]; app.launch() }
        func chooseProject() {
            app.buttons["composer-chip-project"].tap()
            let project = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", (config["project"] ?? "Mobile fixture") + ",")).firstMatch
            XCTAssertTrue(project.waitForExistence(timeout: 15))
            project.tap()
        }
        XCTAssertTrue(app.buttons["new-session"].waitForExistence(timeout: 90))
        app.buttons["new-session"].tap()
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.25)).tap()
        _ = app.keyboards.firstMatch.waitForNonExistence(timeout: 5)
        chooseProject()
        app.buttons["composer-chip-model"].tap()
        app.buttons["Agent Lab…"].tap()
        let name = app.textViews["lab-name"]
        XCTAssertTrue(name.waitForExistence(timeout: 15))
        let suffix = String(try XCTUnwrap(config["device"]).prefix(8))
        name.tap(); name.typeText(" phone fixture " + suffix)
        let title = app.textViews["lab-title"]
        title.tap(); title.typeText(" phone " + String(try XCTUnwrap(config["device"]).prefix(8)))
        app.toolbars.buttons["Done"].tap()
        func reveal(_ element: XCUIElement) {
            let form = app.scrollViews["lab-form-scroll"]
            let surface = form.exists ? form : app
            for _ in 0..<25 {
                if element.isHittable { return }
                surface.swipeUp()
            }
            XCTAssertTrue(element.isHittable)
        }
        let sdk = app.buttons["lab-runtime:sdk"]
        reveal(sdk); sdk.tap()
        let computer = app.buttons["lab-host:" + (try XCTUnwrap(config["device"]))]
        reveal(computer); computer.tap()
        let namedProject = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", try XCTUnwrap(config["project"]))).firstMatch
        reveal(namedProject); namedProject.tap()
        let runner = app.buttons["lab-runner:fixture"]
        reveal(runner); runner.tap()
        let guidance = app.buttons["lab-add-guidance"]
        reveal(guidance); guidance.tap()
        let reference = app.textViews["lab-component:0"]
        reveal(reference); reference.tap(); reference.typeText("Reply briefly.")
        app.toolbars.buttons["Done"].tap()
        let save = app.buttons["lab-save"]
        reveal(save); save.tap()
        let run = app.buttons["lab-start"]
        XCTAssertTrue(run.waitForExistence(timeout: 30)); reveal(run); run.tap()
        let passed = app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "1 of 1 passed")).firstMatch
        XCTAssertTrue(passed.waitForExistence(timeout: 90)); reveal(passed)
        let result = XCTAttachment(screenshot: app.screenshot()); result.name = "agent-lab-result"; result.lifetime = .keepAlways; add(result)
        let task = app.buttons["Open task"]
        reveal(task); task.tap()
        let reply = app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", "received:")).firstMatch
        XCTAssertTrue(reply.waitForExistence(timeout: 90))
        app.navigationBars.buttons.firstMatch.tap()
        if app.buttons["new-session"].exists {
            app.buttons["new-session"].tap()
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.25)).tap()
            chooseProject()
        }
        app.buttons["composer-chip-model"].tap(); app.buttons["Agent Lab…"].tap()
        XCTAssertTrue(app.buttons["lab-promote"].waitForExistence(timeout: 10))
        let use = app.buttons["lab-promote"]
        reveal(use); use.tap()
        let ready = app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "ready in your agent picker")).firstMatch
        XCTAssertTrue(ready.waitForExistence(timeout: 30))
        app.navigationBars.buttons.firstMatch.tap()
        let saved = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "phone fixture " + suffix)).firstMatch
        for _ in 0..<3 {
            if !app.buttons["Agent Lab…"].exists { app.buttons["composer-chip-model"].tap() }
            if saved.waitForExistence(timeout: 10) { break }
        }
        XCTAssertTrue(saved.exists)
        saved.tap()
        XCTAssertTrue(app.buttons["composer-chip-model"].label.contains("phone fixture " + suffix))
        app.navigationBars.buttons.firstMatch.tap()
        app.openSettings()
        let signOut = app.cells["settings-signout"]
        reveal(signOut); signOut.tap(); app.buttons["Sign Out"].tap()
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 10))
    }
}
