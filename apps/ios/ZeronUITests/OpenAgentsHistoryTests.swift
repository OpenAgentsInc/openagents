import XCTest

/// Native list, paging, full text and file controls against an isolated account.
final class OpenAgentsHistoryTests: XCTestCase {
    func testSavedAccountHistory() throws {
        let env = ProcessInfo.processInfo.environment
        guard let base = env["CODER_HISTORY_FIXTURE"] else { throw XCTSkip("Run scripts/ios/check-history.py") }
        let app = XCUIApplication()
        app.launchArguments = ["-account-fixture", "-account-api", base + "/v1", "-account-site", base,
                               "-account-test-credential-url", base + "/fixture-key", "-edge", base]
        app.launch()
        func cell(_ id: String) -> XCUIElement { app.descendants(matching: .any).matching(identifier: id).firstMatch }
        func tap(_ id: String) {
            let element = cell(id)
            XCTAssertTrue(element.waitForExistence(timeout: 20), id)
            if !element.isHittable { app.swipeUp() }
            element.tap()
        }
        tap("saved-more")
        // Pinned chat 204 arrives on the second page and sorts above the other chats.
        let pinned = cell("saved-chat-000000cc-1234-4234-8234-123456789abc")
        XCTAssertTrue(pinned.waitForExistence(timeout: 20))
        app.swipeDown()
        pinned.tap()
        XCTAssertTrue(cell("saved-message-count").waitForExistence(timeout: 15))
        XCTAssertTrue(app.staticTexts["200 messages loaded"].exists)
        tap("saved-messages-more")
        XCTAssertTrue(app.staticTexts["400 messages loaded"].waitForExistence(timeout: 15))
        tap("saved-messages-more")
        XCTAssertTrue(app.staticTexts["405 messages loaded"].waitForExistence(timeout: 15))
        tap("saved-message-msg_0")
        let body = app.textViews["saved-message-body"]
        XCTAssertTrue(body.waitForExistence(timeout: 10))
        XCTAssertTrue((body.value as? String)?.contains("Saved question") == true)
        tap("saved-file-file:msg_0:0123456789abcdef0123456789abcdef")
        let contents = app.textViews["saved-message-body"]
        XCTAssertTrue(contents.waitForExistence(timeout: 15))
        XCTAssertTrue((contents.value as? String)?.contains("fixture file contents") == true)
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "saved-file"; shot.lifetime = .keepAlways; add(shot)
        // Close the saved view, show archives and shared projects, then sign out.
        for _ in 0..<3 { app.navigationBars.buttons.element(boundBy: 0).tap() }
        tap("saved-archived")
        XCTAssertTrue(pinned.waitForNonExistence(timeout: 10))
        tap("saved-projects")
        XCTAssertTrue(app.staticTexts["Shared project"].waitForExistence(timeout: 10))
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.tabBars.buttons["Settings"].tap()
        let signOut = app.cells["settings-signout"]
        for _ in 0..<8 where !signOut.isHittable { app.swipeUp() }
        signOut.tap(); app.buttons["Sign Out"].tap()
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 15))
        XCTAssertFalse(pinned.exists)
    }
    func testPublicSavedAccountHistory() throws {
        let env = ProcessInfo.processInfo.environment
        guard let raw = env["CODER_PUBLIC_HISTORY_CONFIG"], let bytes = raw.data(using: .utf8),
              let config = try JSONSerialization.jsonObject(with: bytes) as? [String: String],
              let keyURL = config["key_url"], let chat = config["chat"] else { throw XCTSkip("Dedicated-account check only") }
        let app = XCUIApplication()
        app.launchArguments = ["-account-fixture", "-account-test-credential-url", keyURL, "-edge", "https://coder-sync-157437760789.us-central1.run.app"]
        app.launch()
        let row = app.descendants(matching: .any).matching(identifier: "saved-chat-" + chat).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 30)); row.tap()
        XCTAssertTrue(app.staticTexts["200 messages loaded"].waitForExistence(timeout: 20))
        app.descendants(matching: .any).matching(identifier: "saved-messages-more").firstMatch.tap()
        XCTAssertTrue(app.staticTexts["205 messages loaded"].waitForExistence(timeout: 20))
        let shot = XCTAttachment(screenshot: app.screenshot()); shot.name = "public-account-history"; shot.lifetime = .keepAlways; add(shot)
        app.navigationBars.buttons.element(boundBy: 0).tap()
        app.tabBars.buttons["Settings"].tap()
        let signOut = app.cells["settings-signout"]
        for _ in 0..<8 where !signOut.isHittable { app.swipeUp() }
        signOut.tap(); app.buttons["Sign Out"].tap()
        XCTAssertTrue(app.buttons["account-sign-in"].waitForExistence(timeout: 15))
    }

}
