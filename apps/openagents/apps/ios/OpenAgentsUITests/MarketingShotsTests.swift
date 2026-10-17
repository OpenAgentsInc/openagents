import XCTest

/// Re-takeable marketing screenshots (#11984), on the offline demo
/// (`-demo`: the Rust `DemoHost` and the demo's chat answered by
/// OpenAgents). Nothing signs in, so the shots never show a real account;
/// every file name says `demo`. Shots go to the folder in
/// `MARKETING_SHOTS_DIR` (pass it as `TEST_RUNNER_MARKETING_SHOTS_DIR`) and
/// into the test result. The command is in the app README.
final class MarketingShotsTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    private func launch(_ args: [String]) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-demo", "-fast"] + args
        app.launch()
        return app
    }

    private func shot(_ app: XCUIApplication, _ name: String) {
        let screenshot = app.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        guard let dir = ProcessInfo.processInfo.environment["MARKETING_SHOTS_DIR"], !dir.isEmpty else { return }
        let folder = URL(fileURLWithPath: dir, isDirectory: true)
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        XCTAssertNoThrow(try screenshot.pngRepresentation.write(to: folder.appendingPathComponent("\(name).png")))
    }

    /// Sends `text` and waits for the `count`th answer to finish.
    private func ask(_ app: XCUIApplication, _ text: String, answers count: Int) {
        let input = app.textViews["composer-input"]
        XCTAssertTrue(input.waitForExistence(timeout: 15))
        input.tap()
        if app.buttons["Continue"].waitForExistence(timeout: 1) { app.buttons["Continue"].tap() }
        input.typeText(text)
        app.buttons["composer-send"].tap()
        let answers = app.staticTexts.matching(identifier: "cloud-chat-answer")
        expectation(for: NSPredicate(format: "count == %d", count), evaluatedWith: answers)
        waitForExpectations(timeout: 20)
        // Finished: the composer's stop button turns back into send.
        XCTAssertTrue(app.buttons["composer-send"].waitForExistence(timeout: 20))
    }

    /// Drags the messages down into the keyboard (it dismisses
    /// interactively), past the simulator's first-run typing tip.
    private func dismissKeyboard(_ app: XCUIApplication) {
        let tip = app.buttons["Continue"]
        if tip.exists { tip.tap() }
        let window = app.windows.firstMatch
        app.tables["cloud-chat-lines"].coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            .press(forDuration: 0.05, thenDragTo: window.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.98)))
        _ = app.keyboards.firstMatch.waitForNonExistence(timeout: 3)
    }

    /// The landing chat, its first answer, and the
    /// "Run this on your own computer" card.
    func testFirstAnswerAndOwnComputerCard() {
        let app = launch(["-route", "welcome"])
        XCTAssertTrue(app.staticTexts["cloud-chat-empty"].waitForExistence(timeout: 15))
        XCTAssertFalse(app.buttons["account-sign-in"].exists)
        shot(app, "demo-01-first-launch")

        ask(app, "What can you do?", answers: 1)
        dismissKeyboard(app)
        let card = app.otherElements["own-computer-card"]
        XCTAssertTrue(card.waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["own-computer-command"].exists)
        shot(app, "demo-02-own-computer-card")

        app.buttons["own-computer-close"].tap()
        XCTAssertTrue(card.waitForNonExistence(timeout: 5))
        shot(app, "demo-03-first-answer")
    }

    /// A longer chat answered by OpenAgents, in the signed-in shell.
    func testCloudChat() {
        let app = launch(["-route", "cloud"])
        XCTAssertTrue(app.otherElements["cloud-chat"].waitForExistence(timeout: 15))
        ask(app, "What can you do?", answers: 1)
        ask(app, "Can you fix a bug in my app?", answers: 2)
        dismissKeyboard(app)
        // The demo has computers, so the card stays away.
        XCTAssertFalse(app.otherElements["own-computer-card"].exists)
        shot(app, "demo-04-cloud-chat")
    }

    /// Home, then the session list in the drawer.
    func testHomeAndSessions() {
        let app = launch([])
        XCTAssertTrue(app.buttons["drawer-menu"].waitForExistence(timeout: 15))
        // Let Home's widgets fill in before the shot.
        _ = app.buttons["home-sessions"].waitForExistence(timeout: 5)
        shot(app, "demo-05-home")
        XCTAssertTrue(app.openDrawer())
        XCTAssertTrue(app.cells["section-pinned"].waitForExistence(timeout: 5))
        shot(app, "demo-06-sessions")
    }
}
