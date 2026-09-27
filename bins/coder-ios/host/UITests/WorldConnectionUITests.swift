// Exercise native storage and panel lifetime without publishing world events.
import XCTest

final class WorldConnectionUITests: XCTestCase {
    func testAbsentChoiceCustomRelayAndExplicitOfflineSurviveRelaunch() throws {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = ["--synthetic", "--reset-world-preference"]
        app.launch()
        app.launchArguments = ["--synthetic"]
        app.openWorldComputer()
        app.buttons["computer-settings"].tap()
        let initial = try XCTUnwrap(app.verseObservation())
        XCTAssertEqual(initial.world_preference, "unconfigured")
        XCTAssertNil(initial.world_configuration.world_relay)
        XCTAssertNil(initial.world_configuration.world_offline)
        XCTAssertEqual(initial.world_connection.state, "offline", "A preview never starts public networking.")
        XCTAssertFalse(app.buttons["world-leave"].exists)
        let before = try XCTUnwrap(app.verseObservation()).position
        let relay = app.textFields["world-relay"]
        XCTAssertEqual(relay.value as? String, "wss://relay.openagents.com")
        let custom = "wss://world.example.test"
        let end = relay.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5))
        end.tap()
        end.press(forDuration: 1)
        let selectAll = app.menuItems["Select All"]
        XCTAssertTrue(selectAll.waitForExistence(timeout: 5))
        selectAll.tap()
        relay.typeText(custom)
        XCTAssertEqual(relay.value as? String, custom)
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
        XCTAssertEqual(app.textFields["world-relay"].value as? String, custom)
        XCTAssertEqual(app.staticTexts["world-connection-status"].label, "Preview")
        app.terminate()
        app.launch()
        app.openWorldComputer()
        app.buttons["computer-settings"].tap()
        XCTAssertEqual(app.textFields["world-relay"].value as? String, custom)
        XCTAssertEqual(app.staticTexts["world-connection-status"].label, "Preview")
        let remembered = try XCTUnwrap(app.verseObservation())
        XCTAssertEqual(remembered.world_preference, "relay")
        XCTAssertEqual(remembered.world_configuration.world_relay, custom)
        XCTAssertNil(remembered.world_configuration.world_offline)
        app.buttons["world-leave"].tap()
        XCTAssertTrue(wait { app.staticTexts["world-connection-status"].label == "Offline" })
        XCTAssertFalse(app.buttons["world-leave"].exists)
        app.terminate()
        app.launch()
        app.openWorldComputer()
        app.buttons["computer-settings"].tap()
        XCTAssertEqual(app.staticTexts["world-connection-status"].label, "Offline")
        XCTAssertFalse(app.buttons["world-leave"].exists)
        let left = try XCTUnwrap(app.verseObservation())
        XCTAssertEqual(left.world_preference, "offline")
        XCTAssertEqual(left.world_configuration.world_offline, true)
        XCTAssertNil(left.world_configuration.world_relay)
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
