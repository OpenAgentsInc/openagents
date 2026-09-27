// Exercise shared map hit testing through real native touches in an offline world.
import XCTest

final class MapUITests: XCTestCase {
    private var app: XCUIApplication!
    private var surface: XCUIElement { app.otherElements["verse-surface"] }

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--synthetic"]
        app.launch()
        XCTAssertTrue(surface.waitForExistence(timeout: 30))
        waitFor { $0.frames > 0 && $0.map.visible }
    }

    func testMapTapStartsWalkingAndManualMovementStopsIt() throws {
        let initial = try XCTUnwrap(app.verseObservation())
        let map = initial.map
        XCTAssertFalse(map.expanded)
        XCTAssertGreaterThanOrEqual(map.frame[1], 1, "The map stays below the top safe area.")
        XCTAssertGreaterThanOrEqual(map.frame[0], 0)
        XCTAssertLessThanOrEqual(map.frame[0] + map.frame[2], surface.frame.width)
        XCTAssertLessThanOrEqual(map.frame[1] + map.frame[3], surface.frame.height)
        if app.statusBars.firstMatch.exists {
            XCTAssertGreaterThanOrEqual(map.frame[1] + surface.frame.minY, app.statusBars.firstMatch.frame.maxY)
        }
        tap(map.frame[0] + map.frame[2] / 2, map.frame[1] + 12)
        waitFor { $0.map.expanded }
        let expanded = try XCTUnwrap(app.verseObservation()).map
        let gym = try XCTUnwrap(expanded.landmarks.first { $0.id == "gym" })
        // Project the Rust-supplied landmark into the Rust-supplied map bounds.
        // This tests the real plot, not the accessibility command adapter.
        let x = expanded.plot[0] + ((gym.x - expanded.center[0]) / expanded.half_extent + 1) * expanded.plot[2] / 2
        let y = expanded.plot[1] + (1 - (gym.z - expanded.center[1]) / expanded.half_extent) * expanded.plot[3] / 2
        tap(x, y)
        waitFor { $0.map.state == "Walking" && !$0.map.expanded }
        let walking = try XCTUnwrap(app.verseObservation())
        XCTAssertEqual(try XCTUnwrap(walking.map.destination)[0], gym.x, accuracy: 1)
        XCTAssertEqual(try XCTUnwrap(walking.map.destination)[1], gym.z, accuracy: 1)
        waitFor { hypot($0.position[0] - initial.position[0], $0.position[2] - initial.position[2]) > 0.3 }
        // The route passes Halo, whose item strip can appear above the
        // bottom controls. Begin below it so this is a world movement gesture.
        let startOffset = CGVector(dx: 0.15, dy: 0.88)
        let hud = try XCTUnwrap(app.verseObservation()).doors.hud
        if hud.visible {
            let x = Double(surface.frame.width * startOffset.dx)
            let y = Double(surface.frame.height * startOffset.dy)
            let inside = x >= hud.frame[0] && x <= hud.frame[0] + hud.frame[2]
                && y >= hud.frame[1] && y <= hud.frame[1] + hud.frame[3]
            XCTAssertFalse(inside, "Manual movement must start outside the gate item strip.")
        }
        let start = surface.coordinate(withNormalizedOffset: startOffset)
        let end = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.15, dy: 0.78))
        start.press(forDuration: 0.1, thenDragTo: end, withVelocity: .slow, thenHoldForDuration: 0.2)
        waitFor { $0.map.state == "Walk stopped" }
        XCTAssertFalse(app.staticTexts["verse-error"].exists)
        attach("Map destination and manual walking cancellation")
    }

    func testDraggingTheMapCannotMoveThePlayerOrTurnTheCamera() throws {
        let compact = try XCTUnwrap(app.verseObservation()).map
        tap(compact.frame[0] + compact.frame[2] / 2, compact.frame[1] + 12)
        waitFor { $0.map.expanded }
        let before = try XCTUnwrap(app.verseObservation())
        let plot = before.map.plot
        let start = point(plot[0] + plot[2] * 0.25, plot[1] + plot[3] * 0.5)
        let end = point(plot[0] + plot[2] * 0.75, plot[1] + plot[3] * 0.5)
        start.press(forDuration: 0.1, thenDragTo: end, withVelocity: .slow, thenHoldForDuration: 0.1)
        waitFor { $0.frames > before.frames && $0.map.captured_pointers.isEmpty }
        let after = try XCTUnwrap(app.verseObservation())
        XCTAssertTrue(after.map.expanded)
        XCTAssertNil(after.map.destination)
        XCTAssertEqual(after.position[0], before.position[0], accuracy: 0.02)
        XCTAssertEqual(after.position[2], before.position[2], accuracy: 0.02)
        XCTAssertEqual(after.camera[0], before.camera[0], accuracy: 0.02)
        XCTAssertEqual(after.camera[1], before.camera[1], accuracy: 0.02)
        XCTAssertEqual(after.camera_distance, before.camera_distance, accuracy: 0.02)
        attach("Map drag remains a HUD gesture")
    }

    private func point(_ x: Double, _ y: Double) -> XCUICoordinate {
        surface.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x, dy: y))
    }
    private func tap(_ x: Double, _ y: Double) { point(x, y).tap() }
    private func waitFor(_ predicate: @escaping (VerseTestObservation) -> Bool) {
        let expectation = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            self.app.verseObservation().map(predicate) ?? false
        }, object: app)
        XCTAssertEqual(XCTWaiter.wait(for: [expectation], timeout: 15), .completed)
    }
    private func attach(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
