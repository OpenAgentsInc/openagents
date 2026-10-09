// The Coder / Verse switch (#11184): a new chat's feature cards, the
// drawer's Verse row into the plain Grid, back with the switch, and
// Explore the Verse's Try it straight into the Grid. Offline (the chat
// fixture); the world runs without a relay. `OPENAGENTS_UITEST_SHOTS` names
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

    /// Puts the keyboard away with a tap above the composer, clear of the cards.
    private func hideKeyboard(_ app: XCUIApplication) {
        if app.keyboards.firstMatch.exists {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.12)).tap()
            sleep(1)
        }
    }

    func testTheSwitchTheDrawerAndTryItOpenTheGridAndComeBack() throws {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = ["--chat-fixture", "1", "--appearance", appearance]
        app.launch()
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

        // Explore the Verse's Try it goes straight to the Grid. The cards
        // move on by themselves, so tap the Verse card first.
        hideKeyboard(app)
        _ = app.buttons["Explore the Verse"].firstMatch.waitForExistence(timeout: 5)
        app.buttons.matching(NSPredicate(format: "label == %@", "Explore the Verse")).allElementsBoundByIndex
            .first { $0.isHittable }?.tap()
        let tryIt = app.buttons["shell-try-verse"].firstMatch
        XCTAssertTrue(tryIt.waitForExistence(timeout: 10))
        tryIt.tap()
        XCTAssertTrue(app.buttons["shell-switch-coder"].firstMatch.waitForExistence(timeout: 10))
        sleep(3)
        shot("try-verse")
        app.buttons["shell-switch-coder"].firstMatch.tap()
        XCTAssertTrue(app.textViews["Ask OpenAgents"].waitForExistence(timeout: 10))
    }
}
