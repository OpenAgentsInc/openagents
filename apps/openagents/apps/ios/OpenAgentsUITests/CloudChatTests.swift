import XCTest

/// The signed-out first screen (#11978): a few messages answered by
/// OpenAgents, then the sign-in sheet. Runs against a visitor server given
/// by `CLOUD_CHAT_API` / `CLOUD_CHAT_SITE` (pass them as
/// `TEST_RUNNER_CLOUD_CHAT_API` and `TEST_RUNNER_CLOUD_CHAT_SITE`) that
/// allows three messages.
final class CloudChatTests: XCTestCase {
    private func capture(_ app: XCUIApplication, _ name: String) {
        let shot = app.screenshot()
        let attachment = XCTAttachment(screenshot: shot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        if let dir = ProcessInfo.processInfo.environment["CLOUD_CHAT_SHOTS"] {
            try? shot.pngRepresentation.write(to: URL(fileURLWithPath: dir).appendingPathComponent("\(name).png"))
        }
    }

    func testSignedOutChatAnswersThenAsksToSignIn() throws {
        let env = ProcessInfo.processInfo.environment
        guard let api = env["CLOUD_CHAT_API"], let site = env["CLOUD_CHAT_SITE"] else {
            throw XCTSkip("Run with a visitor chat server.")
        }
        let app = XCUIApplication()
        app.launchArguments = ["-signedout", "-no-auth-browser", "-account-api", api, "-account-site", site]
        app.launch()
        XCTAssertTrue(app.staticTexts["cloud-chat-empty"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["account-sign-in"].exists)
        XCTAssertTrue(app.buttons["open-demo"].exists)
        capture(app, "cloud-chat-first-launch")
        let input = app.textViews["composer-input"]
        let answer = app.staticTexts.matching(identifier: "cloud-chat-answer")
        for (i, text) in ["What can you do?", "Can you fix a bug in my app?", "Thanks!"].enumerated() {
            input.tap()
            input.typeText(text)
            app.buttons["composer-send"].tap()
            let done = NSPredicate(format: "count == %d", i + 1)
            expectation(for: done, evaluatedWith: answer)
            waitForExpectations(timeout: 20)
            XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS %@", "start coding tasks")).firstMatch.waitForExistence(timeout: 20))
            if i == 1 {
                XCTAssertTrue(app.staticTexts["1 message left before you sign in"].waitForExistence(timeout: 5))
                capture(app, "cloud-chat-answered")
            }
        }
        input.tap()
        input.typeText("One more question")
        app.buttons["composer-send"].tap()
        XCTAssertTrue(app.buttons["sign-in-prompt-sign-in"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.staticTexts["Sign in to keep chatting. It's free."].exists)
        capture(app, "cloud-chat-sign-in-sheet")
        app.buttons["sign-in-prompt-later"].tap()
        XCTAssertEqual(input.value as? String, "One more question")
    }
}
