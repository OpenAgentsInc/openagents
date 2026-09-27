// The companion is picked in Rust from a real surface tap, without a model call.
import XCTest

final class CompanionUITests: XCTestCase {
    func testProjectedCompanionTapReactsWithoutMovingThePlayer() throws {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = ["--synthetic"]
        app.launch()
        let surface = app.otherElements["verse-surface"]
        XCTAssertTrue(surface.waitForExistence(timeout: 30))
        let ready = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            guard let value = app.verseObservation() else { return false }
            return value.frames > 0 && value.companion.near && value.companion.visible
        }, object: app)
        XCTAssertEqual(XCTWaiter.wait(for: [ready], timeout: 15), .completed)
        let before = try XCTUnwrap(app.verseObservation())
        attach(app, "Companion visible in the shared world")
        // Refresh the moving anchor after the screenshot before sending a real tap.
        let target = try XCTUnwrap(app.verseObservation()).companion
        surface.coordinate(withNormalizedOffset: CGVector(dx: target.screen_x, dy: target.screen_y)).tap()
        let reacted = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            app.verseObservation()?.companion.pet_count == before.companion.pet_count + 1
        }, object: app)
        XCTAssertEqual(XCTWaiter.wait(for: [reacted], timeout: 10), .completed)
        let after = try XCTUnwrap(app.verseObservation())
        XCTAssertEqual(after.position[0], before.position[0], accuracy: 0.02)
        XCTAssertEqual(after.position[2], before.position[2], accuracy: 0.02)
        XCTAssertEqual(after.highest_observed_y, before.highest_observed_y, accuracy: 0.02)
        XCTAssertEqual(after.camera[0], before.camera[0], accuracy: 0.02)
        XCTAssertEqual(after.camera[1], before.camera[1], accuracy: 0.02)
        XCTAssertFalse(app.buttons["computer-close"].exists)
        XCTAssertFalse(app.staticTexts["verse-error"].exists)
        attach(app, "Companion after a projected surface tap")
        let completed = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            guard let value = app.verseObservation() else { return false }
            return !value.companion.reacting && value.companion.cooldown_seconds == 0
        }, object: app)
        XCTAssertEqual(XCTWaiter.wait(for: [completed], timeout: 10), .completed)
        XCTAssertEqual(app.verseObservation()?.companion.pet_count, before.companion.pet_count + 1)
    }

    private func attach(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
