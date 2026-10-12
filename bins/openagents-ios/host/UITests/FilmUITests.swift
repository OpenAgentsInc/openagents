// The launch film's phone shot (#11432): opens Running, signed in with a
// saved session, and stays on it while scripts/film/footage/phone-app.mjs
// records the simulator and runs Coder on the computer. Opt-in:
// `TEST_RUNNER_OPENAGENTS_UITEST_FILM=DIR` (the test writes `DIR/ready` once
// Running shows and returns when `DIR/finish` appears),
// `TEST_RUNNER_OPENAGENTS_UITEST_ORIGIN`, and
// `TEST_RUNNER_OPENAGENTS_UITEST_SESSION_FILE` as in AccountGateUITests.
import XCTest

final class FilmUITests: XCTestCase {
    func testRunningForTheFilm() throws {
        let environment = ProcessInfo.processInfo.environment
        guard let folder = environment["OPENAGENTS_UITEST_FILM"] else {
            throw XCTSkip("Set OPENAGENTS_UITEST_FILM to record the film's phone shot.")
        }
        let directory = URL(fileURLWithPath: folder)
        let app = XCUIApplication()
        app.launchArguments += ["--appearance", "dark"]
        if let origin = environment["OPENAGENTS_UITEST_ORIGIN"] {
            app.launchArguments += ["--account-origin", origin]
        }
        if let path = environment["OPENAGENTS_UITEST_SESSION_FILE"],
           let session = try? String(contentsOfFile: path, encoding: .utf8) {
            app.launchEnvironment["OPENAGENTS_UITEST_ACCOUNT_SESSION"] = session
        }
        app.launch()

        let menu = app.buttons["shell-menu"].firstMatch
        XCTAssertTrue(menu.waitForExistence(timeout: 20))
        menu.tap()
        let settings = app.buttons["shell-place-settings"].firstMatch
        XCTAssertTrue(settings.waitForExistence(timeout: 10))
        settings.tap()
        let account = app.buttons["account-link"].firstMatch
        XCTAssertTrue(account.waitForExistence(timeout: 10))
        account.tap()
        let running = app.buttons["link-open-running"].firstMatch
        XCTAssertTrue(running.waitForExistence(timeout: 30), "Not signed in.")
        running.tap()
        sleep(3)
        try? "ready".write(to: directory.appendingPathComponent("ready"), atomically: true, encoding: .utf8)

        let finish = directory.appendingPathComponent("finish")
        let deadline = Date().addingTimeInterval(15 * 60)
        while !FileManager.default.fileExists(atPath: finish.path) && Date() < deadline {
            // Touch nothing; the app reads Running again by itself.
            Thread.sleep(forTimeInterval: 0.5)
        }
    }

    /// The film's sync shot: Your chats, signed in. Driven by files in
    /// `OPENAGENTS_UITEST_FILM` (scripts/film/footage/sync-shoot.mjs):
    /// `open` opens the first chat as soon as it lists, `reply` (its text)
    /// types and sends a reply in the open chat, and `finish` ends. The test
    /// writes `ready`, `seen`, `opened`, and `replied` with the Unix time.
    func testSyncForTheFilm() throws {
        let environment = ProcessInfo.processInfo.environment
        guard let folder = environment["OPENAGENTS_UITEST_FILM_SYNC"] else {
            throw XCTSkip("Set OPENAGENTS_UITEST_FILM_SYNC to record the film's sync shot.")
        }
        let directory = URL(fileURLWithPath: folder)
        func stamp(_ name: String) {
            try? String(Date().timeIntervalSince1970)
                .write(to: directory.appendingPathComponent(name), atomically: true, encoding: .utf8)
        }
        func asked(_ name: String) -> String? {
            try? String(contentsOf: directory.appendingPathComponent(name), encoding: .utf8)
        }
        let app = XCUIApplication()
        app.launchArguments += ["--appearance", "dark"]
        if let origin = environment["OPENAGENTS_UITEST_ORIGIN"] {
            app.launchArguments += ["--account-origin", origin]
        }
        if let path = environment["OPENAGENTS_UITEST_SESSION_FILE"],
           let session = try? String(contentsOfFile: path, encoding: .utf8) {
            app.launchEnvironment["OPENAGENTS_UITEST_ACCOUNT_SESSION"] = session
        }
        app.launch()
        let menu = app.buttons["shell-menu"].firstMatch
        XCTAssertTrue(menu.waitForExistence(timeout: 20))
        menu.tap()
        let settings = app.buttons["shell-place-settings"].firstMatch
        XCTAssertTrue(settings.waitForExistence(timeout: 10))
        settings.tap()
        let account = app.buttons["account-link"].firstMatch
        XCTAssertTrue(account.waitForExistence(timeout: 10))
        account.tap()
        let chats = app.buttons["link-open-chats"].firstMatch
        XCTAssertTrue(chats.waitForExistence(timeout: 30), "Not signed in.")
        chats.tap()
        sleep(2)
        stamp("ready")

        let thread = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'link-thread-'")).firstMatch
        var opened = false
        var replied = false
        let deadline = Date().addingTimeInterval(20 * 60)
        while asked("finish") == nil && Date() < deadline {
            if !opened && asked("open") != nil && thread.exists {
                stamp("seen")
                thread.tap()
                stamp("opened")
                opened = true
            }
            if opened && !replied, let text = asked("reply") {
                let field = app.textViews.firstMatch.exists ? app.textViews.firstMatch : app.textFields.firstMatch
                if field.exists && field.isEnabled {
                    field.tap()
                    field.typeText(text)
                    let send = app.buttons["link-composer-send"].firstMatch
                    if send.waitForExistence(timeout: 5) { send.tap() }
                    stamp("replied")
                    replied = true
                }
            }
            Thread.sleep(forTimeInterval: 0.2)
        }
    }
}
