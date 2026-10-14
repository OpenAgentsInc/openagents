// The release gate before a TestFlight build goes to testers (#11093): a
// fresh install, signed out, against the live chat. It asks the questions
// testers and Apple's reviewer start with, opens the Verse, Wallet, and Settings from
// the drawer (#11126), and opens every Settings row, keeping a screenshot and the screen's text for each step so
// a person can check them. It needs the network, so it runs only when
// `OPENAGENTS_UITEST_LIVE` is set (pass `TEST_RUNNER_OPENAGENTS_UITEST_LIVE=1`
// and `TEST_RUNNER_OPENAGENTS_UITEST_SHOTS=DIR` to `xcodebuild test
// -only-testing:OpenAgentsUITests/ReleaseGateUITests`); see
// docs/mobile/testflight-1.0.md.
import XCTest

final class ReleaseGateUITests: XCTestCase {
    private var directory: URL? {
        ProcessInfo.processInfo.environment["OPENAGENTS_UITEST_SHOTS"].map { URL(fileURLWithPath: $0) }
    }

    private func record(_ app: XCUIApplication, _ name: String) {
        let screenshot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        guard let directory else { return }
        try? screenshot.pngRepresentation.write(to: directory.appendingPathComponent("\(name).png"))
        let words = app.staticTexts.allElementsBoundByIndex.map(\.label).filter { !$0.isEmpty }
        let buttons = app.buttons.allElementsBoundByIndex.map { "[\($0.identifier)] \($0.label)" }
        let text = (words + ["--- buttons"] + buttons).joined(separator: "\n")
        try? text.write(to: directory.appendingPathComponent("\(name).txt"), atomically: true, encoding: .utf8)
    }

    /// A new chat, wherever the app opened (#11557): the welcome's Start a
    /// chat, a chat's New chat, or the list's; nothing while one shows.
    private func newChat(_ app: XCUIApplication) {
        let field = app.textViews["Ask OpenAgents"]
        let ways = ["shell-welcome-start", "shell-new-chat", "coder-new"].map { app.buttons[$0].firstMatch }
        let shown = NSPredicate { _, _ in field.exists || ways.contains { $0.exists } }
        wait(for: [expectation(for: shown, evaluatedWith: nil)], timeout: 30)
        if !field.exists { ways.first { $0.exists }?.tap() }
    }

    private func ask(_ app: XCUIApplication, _ question: String, _ name: String) {
        newChat(app)
        let field = app.textViews["Ask OpenAgents"]
        XCTAssertTrue(field.waitForExistence(timeout: 30))
        field.tap()
        field.typeText(question)
        app.buttons["Send"].firstMatch.tap()
        let cleared = expectation(for: NSPredicate(format: "value == ''"), evaluatedWith: field)
        wait(for: [cleared], timeout: 15)
        // The reply streams in; give it time to finish.
        sleep(35)
        record(app, name)
        // The start of a long reply.
        app.swipeDown()
        app.swipeDown()
        sleep(1)
        record(app, name + "-top")
    }

    /// Open a place from the drawer.
    private func open(_ app: XCUIApplication, _ place: String) {
        let menu = app.buttons["shell-menu"].firstMatch
        XCTAssertTrue(menu.waitForExistence(timeout: 10))
        menu.tap()
        let row = app.buttons[place].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 5))
        row.tap()
        sleep(1)
    }

    func testFreshInstallChatWalletAndAccount() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["OPENAGENTS_UITEST_LIVE"] != nil,
                          "Set OPENAGENTS_UITEST_LIVE to run the live release gate.")
        if let directory {
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        }
        let app = XCUIApplication()
        // A test run (#11601): the app's calls carry X-OpenAgents-Test: 1, so
        // the site lists none of its chats, deletes them within an hour, and
        // records no computer for this phone.
        app.launchEnvironment["OPENAGENTS_TEST_RUN"] = "1"
        app.launch()
        record(app, "00-launch")
        let questions = [
            "What is OpenAgents?",
            "What models do you use?",
            "How do I connect my codebase?",
            "What are plugins?",
            "Can you help me write a short note to my team about a deadline moving to Friday?",
        ]
        for (index, question) in questions.enumerated() {
            if index > 0, app.buttons["shell-new-chat"].exists { app.buttons["shell-new-chat"].tap() }
            ask(app, question, String(format: "%02d-ask", index + 1))
        }

        // Put the keyboard away, then the Verse from the drawer: the Grid,
        // and back to Coder with the switch.
        app.swipeDown()
        open(app, "shell-place-verse")
        // A simulator build's world takes a while to load.
        sleep(20)
        record(app, "08-verse")
        let coder = app.buttons["shell-switch-coder"].firstMatch
        XCTAssertTrue(coder.waitForExistence(timeout: 10))
        coder.tap()
        sleep(1)
        XCTAssertTrue(app.textViews["Ask OpenAgents"].waitForExistence(timeout: 10))
        record(app, "09-back-to-coder")

        // The drawer for Wallet.
        open(app, "shell-place-wallet")
        sleep(6)
        record(app, "10-wallet")

        open(app, "shell-place-settings")
        sleep(3)
        record(app, "20-account")
        let rows = app.buttons.allElementsBoundByIndex
            .filter { $0.isHittable && !$0.label.isEmpty && $0.identifier != "shell-menu" }
            .map(\.label)
        for (index, row) in rows.enumerated() {
            if app.state != .runningForeground { app.activate(); sleep(2) }
            if !app.navigationBars["Settings"].exists { open(app, "shell-place-settings"); sleep(1) }
            let button = app.buttons[row].firstMatch
            guard button.waitForExistence(timeout: 5) else { continue }
            if !button.isHittable { app.swipeUp() }
            button.tap()
            sleep(3)
            record(app, String(format: "%02d-account-%@", 21 + index, row.replacingOccurrences(of: " ", with: "-")))
            // Back to Settings: from another app, a pushed screen, or a sheet.
            if app.state != .runningForeground {
                app.activate()
                sleep(2)
            } else if app.navigationBars.buttons.firstMatch.exists, app.navigationBars.buttons.firstMatch.isHittable,
                      app.navigationBars.buttons.firstMatch.identifier != "shell-menu" {
                app.navigationBars.buttons.firstMatch.tap()
            } else {
                app.swipeDown(velocity: .fast)
            }
            sleep(1)
        }
        record(app, "90-account-end")
    }
}
