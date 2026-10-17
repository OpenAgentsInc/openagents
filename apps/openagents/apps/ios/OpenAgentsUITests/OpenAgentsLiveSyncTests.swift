import XCTest

/// Real staging/production sync, a dedicated SDK test account, and an isolated mock engine.
final class OpenAgentsLiveSyncTests: XCTestCase {
    private var control: String = ""
    private var ownedApp: XCUIApplication?
    override func tearDownWithError() throws {
        if let app = ownedApp {
            app.terminate()
            app.launchArguments = ["-account-fixture", "-signedout", "-no-auth-browser"]
            app.launch()
            XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 10))
        }
        ownedApp = nil
        try super.tearDownWithError()
    }
    private func post(_ path: String) throws {
        let done = expectation(description: path)
        var result: Int?
        var problem: Error?
        var request = URLRequest(url: try XCTUnwrap(URL(string: control + path)))
        request.httpMethod = "POST"
        request.timeoutInterval = 100
        URLSession.shared.dataTask(with: request) { _, response, error in
            result = (response as? HTTPURLResponse)?.statusCode
            problem = error
            done.fulfill()
        }.resume()
        wait(for: [done], timeout: 110)
        if let problem { throw problem }
        XCTAssertEqual(result, 200, "Dedicated host check: \(path)")
    }
    private func capture(_ app: XCUIApplication, _ name: String) {
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = name
        shot.lifetime = .keepAlways
        add(shot)
    }
    func testDesktopPhoneSessionAndRecovery() throws {
        continueAfterFailure = false
        let env = ProcessInfo.processInfo.environment
        guard let raw = env["CODER_MOBILE_SYNC_CONFIG"], let data = raw.data(using: .utf8),
              let config = try JSONSerialization.jsonObject(with: data) as? [String: String] else {
            throw XCTSkip("Run with the dedicated live-sync coordinator.")
        }
        control = try XCTUnwrap(config["control"])
        let app = XCUIApplication()
        app.launchArguments = ["-no-auth-browser", "-account-fixture", "-account-test-credential-url", try XCTUnwrap(config["credential_url"]), "-edge", try XCTUnwrap(config["sync"]), "-harness", "mock"]
        app.launch()
        ownedApp = app
        let session = app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", try XCTUnwrap(config["title"]))).firstMatch
        XCTAssertTrue(session.waitForExistence(timeout: 90), "The desktop-created session appears")
        session.tap()
        let input = app.textViews["composer-input"]
        let option = app.buttons["question-option-1"]
        XCTAssertTrue(option.waitForExistence(timeout: 90), "Desktop questions reach the phone")
        capture(app, "live-desktop-question")
        option.tap()
        XCTAssertTrue(app.staticTexts["Which suites should gate the merge?"].waitForExistence(timeout: 10))
        app.buttons["question-option-0"].tap()
        app.buttons["question-next"].tap()
        try post("/check/answers")
        XCTAssertTrue(app.buttons["Send message"].waitForExistence(timeout: 30))
        input.tap()
        input.typeText("Phone live message")
        app.buttons["composer-send"].tap()
        try post("/check/phone")
        input.tap()
        input.typeText("Phone queued message")
        XCTAssertTrue(app.buttons["Queue message"].waitForExistence(timeout: 5))
        app.buttons["Queue message"].tap()
        try post("/check/queue")
        XCTAssertTrue(app.buttons["More queue actions"].waitForExistence(timeout: 30))
        capture(app, "live-phone-queue")
        app.buttons["More queue actions"].tap()
        app.buttons["Remove"].tap()
        XCTAssertTrue(app.buttons["More queue actions"].waitForNonExistence(timeout: 10))
        XCTAssertTrue(app.buttons["Stop response"].waitForExistence(timeout: 5))
        app.buttons["Stop response"].tap()
        try post("/check/stop")
        XCTAssertTrue(app.buttons["Send message"].waitForExistence(timeout: 20))
        XCUIDevice.shared.press(.home)
        sleep(2)
        app.activate()
        XCTAssertTrue(input.waitForExistence(timeout: 15))
        try post("/check/foreground")
        capture(app, "live-phone-foreground")
        try post("/offline")
        input.tap()
        input.typeText("Phone reconnected message")
        app.buttons["composer-send"].tap()
        try post("/online")
        try post("/check/reconnect")
        XCTAssertTrue(app.buttons["Stop response"].waitForExistence(timeout: 30))
        app.buttons["Stop response"].tap()
        capture(app, "live-host-reconnected")
        // Leave the conversation and remove this test app's saved credential.
        app.navigationBars.buttons.firstMatch.tap()
        XCTAssertTrue(app.tabBars.buttons["Settings"].waitForExistence(timeout: 10))
        app.tabBars.buttons["Settings"].tap()
        let signOut = app.cells["settings-signout"]
        for _ in 0..<6 where !signOut.isHittable { app.swipeUp() }
        signOut.tap()
        app.buttons["Sign Out"].tap()
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 10))
        XCTAssertFalse(session.exists)
    }
}
