// Exercise native storage and panel lifetime without publishing world events.
import XCTest

final class WorldConnectionUITests: XCTestCase {
    func testWorldRelaySurvivesPanelsAndRelaunchAndLeaveForgetsIt() throws {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = ["--synthetic"]
        app.launch()
        app.openWorldComputer()
        app.buttons["computer-settings"].tap()
        if app.buttons["world-leave"].exists { app.buttons["world-leave"].tap() }
        let before = try XCTUnwrap(app.verseObservation()).position
        let relay = app.textFields["world-relay"]
        XCTAssertEqual(relay.value as? String, "wss://relay.openagents.com")
        app.buttons["world-join"].tap()
        XCTAssertTrue(wait { app.staticTexts["world-connection-status"].label == "Preview" })
        XCTAssertTrue(app.buttons["computer-close"].exists, "Joining keeps the computer panel open.")
        let after = try XCTUnwrap(app.verseObservation()).position
        XCTAssertEqual(after[0], before[0], accuracy: 0.01)
        XCTAssertEqual(after[2], before[2], accuracy: 0.01)
        attach(app, "Saved relay with concise connection state")
        app.buttons["computer-close"].tap()
        app.openWorldComputer()
        app.buttons["computer-settings"].tap()
        XCTAssertEqual(app.textFields["world-relay"].value as? String, "wss://relay.openagents.com")
        XCTAssertEqual(app.staticTexts["world-connection-status"].label, "Preview")
        app.terminate()
        app.launch()
        app.openWorldComputer()
        app.buttons["computer-settings"].tap()
        XCTAssertEqual(app.textFields["world-relay"].value as? String, "wss://relay.openagents.com")
        XCTAssertEqual(app.staticTexts["world-connection-status"].label, "Preview")
        app.buttons["world-leave"].tap()
        XCTAssertTrue(wait { app.staticTexts["world-connection-status"].label == "Offline" })
        XCTAssertFalse(app.buttons["world-leave"].exists)
        app.terminate()
        app.launch()
        app.openWorldComputer()
        app.buttons["computer-settings"].tap()
        XCTAssertEqual(app.staticTexts["world-connection-status"].label, "Offline")
        XCTAssertFalse(app.buttons["world-leave"].exists)
    }

    private func wait(_ condition: @escaping () -> Bool) -> Bool {
        let expectation = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in condition() }, object: nil)
        return XCTWaiter.wait(for: [expectation], timeout: 10) == .completed
    }

    private func attach(_ app: XCUIApplication, _ name: String) {
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = name
        shot.lifetime = .keepAlways
        add(shot)
    }
}
