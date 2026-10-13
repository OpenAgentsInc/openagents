// The Coder / Verse switch (#11184): a new chat (from the welcome, the last
// chat, or every chat: the app opens where it was left, #11557), the
// drawer's Verse row into the plain Grid, and back with the switch.
// Offline (the chat fixture); the world runs without a relay. `OPENAGENTS_UITEST_SHOTS` names
// a directory for the screenshots and `OPENAGENTS_UITEST_APPEARANCE`
// (light or dark) the theme.
import XCTest

final class VerseShellUITests: XCTestCase {
    private var appearance: String {
        ProcessInfo.processInfo.environment["OPENAGENTS_UITEST_APPEARANCE"] ?? "dark"
    }

    private func shot(_ name: String) {
        let screenshot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        if let directory = ProcessInfo.processInfo.environment["OPENAGENTS_UITEST_SHOTS"] {
            try? screenshot.pngRepresentation.write(to: URL(fileURLWithPath: directory)
                .appendingPathComponent("\(name)-\(appearance).png"))
        }
    }

    /// Puts the keyboard away with a tap above the composer.
    private func hideKeyboard(_ app: XCUIApplication) {
        if app.keyboards.firstMatch.exists {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.12)).tap()
            sleep(1)
        }
    }

    /// A new chat, wherever the app opened: the welcome's Start a chat, a
    /// chat's New chat, or the list's.
    private func newChat(_ app: XCUIApplication) {
        let field = app.textViews["Ask OpenAgents"]
        let ways = ["shell-welcome-start", "shell-new-chat", "coder-new"].map { app.buttons[$0].firstMatch }
        let shown = NSPredicate { _, _ in field.exists || ways.contains { $0.exists } }
        wait(for: [expectation(for: shown, evaluatedWith: nil)], timeout: 30)
        ways.first { $0.exists }?.tap()
        XCTAssertTrue(field.waitForExistence(timeout: 10))
    }

    func testTheSwitchAndTheDrawerOpenTheGridAndComeBack() throws {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = ["--chat-fixture", "1", "--appearance", appearance]
        app.launch()
        newChat(app)
        XCTAssertTrue(app.buttons["shell-switch-verse"].waitForExistence(timeout: 30))
        hideKeyboard(app)
        sleep(2)
        shot("new")

        // The drawer lists the Verse.
        app.buttons["shell-menu"].firstMatch.tap()
        let verse = app.buttons["shell-place-verse"].firstMatch
        XCTAssertTrue(verse.waitForExistence(timeout: 5))
        sleep(1)
        shot("drawer")
        verse.tap()

        // The Grid, with the switch on Verse; a debug build's world takes a
        // while to load.
        let coder = app.buttons["shell-switch-coder"].firstMatch
        XCTAssertTrue(coder.waitForExistence(timeout: 10))
        XCTAssertFalse(app.keyboards.firstMatch.exists)
        sleep(20)
        shot("verse")
        coder.tap()
        XCTAssertTrue(app.textViews["Ask OpenAgents"].waitForExistence(timeout: 10))
    }
}
