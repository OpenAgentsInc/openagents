import XCTest

/// Live check: connect this phone to an OpenAgents desktop app by its code,
/// as Paste in the scanner does. Runs only when `PAIR_CODE_FILE` names a file
/// holding a connect code (`TEST_RUNNER_PAIR_CODE_FILE` to xcodebuild); the
/// result's title and message go to `PAIR_RESULT_FILE`. Use test identities
/// only, never the owner's computer.
final class DesktopPairingTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    func testConnectComputerByPastedCode() throws {
        let env = ProcessInfo.processInfo.environment
        guard let codeFile = env["PAIR_CODE_FILE"], !codeFile.isEmpty else {
            throw XCTSkip("Set PAIR_CODE_FILE to a desktop connect code.")
        }
        let code = try String(contentsOfFile: codeFile, encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let app = XCUIApplication()
        app.launchArguments = Self.launchArguments(env)
        app.launch()
        addUIInterruptionMonitor(withDescription: "system alerts") { alert in
            for label in ["Allow Paste", "Don’t Allow", "Don't Allow", "OK"] where alert.buttons[label].exists {
                alert.buttons[label].tap()
                return true
            }
            return false
        }
        app.openSettings(timeout: 60)
        let row = app.cells["settings-scan-code"]
        XCTAssertTrue(row.waitForExistence(timeout: 60), "Connect a computer row")
        row.tap()
        XCTAssertTrue(app.otherElements["code-scanner"].waitForExistence(timeout: 10), "scanner")
        UIPasteboard.general.string = code
        app.navigationBars.buttons["Paste"].tap()
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let allow = springboard.buttons["Allow Paste"]
        if allow.waitForExistence(timeout: 3) { allow.tap() }
        app.tap()
        let done = NSPredicate(format: "label CONTAINS 'is connected' OR label CONTAINS[c] 'couldn'")
        let result = app.alerts.element(matching: done)
        XCTAssertTrue(result.waitForExistence(timeout: 120), "pairing finished")
        let text = ([result.label] + result.staticTexts.allElementsBoundByIndex.map(\.label)).joined(separator: "\n")
        if let out = env["PAIR_RESULT_FILE"], !out.isEmpty {
            try text.write(toFile: out, atomically: true, encoding: .utf8)
        }
        UIPasteboard.general.string = ""
        let connected = result.label.contains("is connected")
        result.buttons["OK"].tap()
        XCTAssertTrue(connected, text)
    }

    /// Live check after pairing: the desktop's chat `SESSION_TITLE` shows on
    /// the phone, and `SESSION_MESSAGE` sent from the phone gets the reply
    /// `SESSION_REPLY` from the desktop's agent. Settings lists the
    /// computer. Captures go to `PAIR_CAPTURE_DIR`.
    func testDesktopSessionRoundTrip() throws {
        let env = ProcessInfo.processInfo.environment
        guard let title = env["SESSION_TITLE"], !title.isEmpty,
              let message = env["SESSION_MESSAGE"], let reply = env["SESSION_REPLY"] else {
            throw XCTSkip("Set SESSION_TITLE, SESSION_MESSAGE and SESSION_REPLY.")
        }
        let app = XCUIApplication()
        app.launchArguments = Self.launchArguments(env)
        app.launch()
        if let computer = env["COMPUTER_NAME"], !computer.isEmpty {
            app.openSettings(timeout: 60)
            let row = app.cells.matching(NSPredicate(format: "label CONTAINS %@", computer)).firstMatch
            XCTAssertTrue(row.waitForExistence(timeout: 60), "Settings lists the computer")
            capture(app, env, "settings-computers")
            app.navigationBars.buttons.firstMatch.tap()
        }
        XCTAssertTrue(app.openDrawer(timeout: 30), "drawer")
        let session = app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", title)).firstMatch
        XCTAssertTrue(session.waitForExistence(timeout: 120), "the desktop's session shows on the phone")
        capture(app, env, "sessions")
        session.tap()
        let input = app.textViews["composer-input"]
        XCTAssertTrue(input.waitForExistence(timeout: 60), "composer")
        input.tap()
        input.typeText(message)
        app.buttons["composer-send"].tap()
        let answer = app.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", reply)).firstMatch
        XCTAssertTrue(answer.waitForExistence(timeout: 240), "the desktop's reply reaches the phone")
        capture(app, env, "reply")
    }

    /// `APP_ARGS`: launch arguments, space-separated (the account fixture's
    /// `-account-fixture -account-test-credential-url URL` signs in as a
    /// dedicated test account).
    private static func launchArguments(_ env: [String: String]) -> [String] {
        (env["APP_ARGS"] ?? "").split(separator: " ").map(String.init)
    }

    private func capture(_ app: XCUIApplication, _ env: [String: String], _ name: String) {
        guard let dir = env["PAIR_CAPTURE_DIR"], !dir.isEmpty else { return }
        try? app.screenshot().pngRepresentation.write(to: URL(fileURLWithPath: dir).appendingPathComponent("\(name).png"))
    }
}
