// The signed-in gate (build 57): sign in to an openagents.com account with
// a device code, then capture the account's screens: Your account, Running
// (each computer's and OpenAgents Cloud's agents, with their links and
// pause choices), the account's chats, the drawer, Settings, and a new
// chat. Live and opt-in, like ReleaseGateUITests: set
// `TEST_RUNNER_OPENAGENTS_UITEST_LIVE=1` and
// `TEST_RUNNER_OPENAGENTS_UITEST_SHOTS=DIR`. The sign-in code is written to
// `DIR/code.txt`; approve it on the site (the test waits up to five
// minutes). `TEST_RUNNER_OPENAGENTS_UITEST_ORIGIN=https://staging.openagents.com`
// signs in to staging (a debug Rust library only), and
// `TEST_RUNNER_OPENAGENTS_UITEST_SIGN_OUT=1` signs out at the end.
// `TEST_RUNNER_OPENAGENTS_UITEST_SESSION_FILE=PATH` starts signed in with a
// saved session instead (simulator only; never sign out a shared one).
import XCTest

final class AccountGateUITests: XCTestCase {
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

    private func open(_ app: XCUIApplication, _ place: String) {
        let menu = app.buttons["shell-menu"].firstMatch
        XCTAssertTrue(menu.waitForExistence(timeout: 10))
        menu.tap()
        let row = app.buttons[place].firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 10))
        row.tap()
        sleep(2)
    }

    func testSignedInAccountScreens() throws {
        let environment = ProcessInfo.processInfo.environment
        try XCTSkipUnless(environment["OPENAGENTS_UITEST_LIVE"] != nil,
                          "Set OPENAGENTS_UITEST_LIVE to run the live account gate.")
        if let directory {
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        }
        let app = XCUIApplication()
        if let origin = environment["OPENAGENTS_UITEST_ORIGIN"] {
            app.launchArguments += ["--account-origin", origin]
        }
        // `OPENAGENTS_UITEST_SESSION_FILE`: a saved session (JSON: origin,
        // account, label, expires_at, token) to start signed in with, on a
        // simulator, instead of approving a code.
        if let path = environment["OPENAGENTS_UITEST_SESSION_FILE"],
           let session = try? String(contentsOfFile: path, encoding: .utf8) {
            app.launchEnvironment["OPENAGENTS_UITEST_ACCOUNT_SESSION"] = session
        }
        app.launch()
        sleep(3)
        record(app, "00-new-chat")

        open(app, "shell-place-settings")
        record(app, "10-settings")
        let accountRow = app.buttons["account-link"].firstMatch
        XCTAssertTrue(accountRow.waitForExistence(timeout: 10))
        accountRow.tap()
        sleep(2)
        let signIn = app.buttons["link-sign-in"].firstMatch
        if signIn.waitForExistence(timeout: 5) {
            signIn.tap()
            let code = app.staticTexts["link-code"].firstMatch
            XCTAssertTrue(code.waitForExistence(timeout: 30))
            record(app, "11-sign-in-code")
            if let directory {
                try? code.label.write(to: directory.appendingPathComponent("code.txt"), atomically: true, encoding: .utf8)
            }
            let signedIn = app.staticTexts["link-signed-in"].firstMatch
            XCTAssertTrue(signedIn.waitForExistence(timeout: 300), "The code was not approved.")
        }
        sleep(3)
        record(app, "12-your-account")

        let running = app.buttons["link-open-running"].firstMatch
        XCTAssertTrue(running.waitForExistence(timeout: 10))
        running.tap()
        sleep(8)
        record(app, "20-running")
        app.swipeUp()
        sleep(1)
        record(app, "21-running-more")
        app.swipeUp()
        sleep(1)
        record(app, "22-running-end")

        // Back to Your account, then the account's chats.
        let back = app.buttons["link-running-header-back"].firstMatch
        if back.waitForExistence(timeout: 5) { back.tap(); sleep(2) }
        let chats = app.buttons["link-open-chats"].firstMatch
        if chats.waitForExistence(timeout: 10) {
            chats.tap()
            sleep(6)
            record(app, "31-account-chats")
            // The list reads itself again every few seconds (#11263).
            sleep(12)
            record(app, "32-account-chats-later")
        }

        let menu = app.buttons["shell-menu"].firstMatch
        XCTAssertTrue(menu.waitForExistence(timeout: 10))
        menu.tap()
        sleep(6)
        record(app, "30-drawer")
        let settings = app.buttons["shell-place-settings"].firstMatch
        XCTAssertTrue(settings.waitForExistence(timeout: 10))
        settings.tap()
        sleep(2)
        record(app, "40-settings-signed-in")

        // `OPENAGENTS_UITEST_SIGN_OUT`: end this phone's session after.
        if environment["OPENAGENTS_UITEST_SIGN_OUT"] != nil {
            app.buttons["account-link"].firstMatch.tap()
            let signOut = app.buttons["link-sign-out"].firstMatch
            XCTAssertTrue(signOut.waitForExistence(timeout: 10))
            signOut.tap()
            XCTAssertTrue(app.buttons["link-sign-in"].firstMatch.waitForExistence(timeout: 30))
            record(app, "50-signed-out")
        }
    }
}
