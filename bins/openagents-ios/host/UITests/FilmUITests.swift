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
}
