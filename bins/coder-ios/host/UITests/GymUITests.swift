// The fixture starts outside the Gym. Walking across its actual doorway is
// required before synthetic data appears; confirmation never starts real work.
import XCTest

final class GymUITests: XCTestCase {
    private var app: XCUIApplication!
    private let runID = "gym-run-" + String(repeating: "1", count: 64)

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--synthetic", "--gym-preview"]
        app.launch()
        XCTAssertTrue(app.otherElements["verse-surface"].waitForExistence(timeout: 30))
        XCTAssertTrue(label("gym-interest", becomes: "Gym idle"))
        XCTAssertFalse(app.buttons[runID].exists)
        XCTAssertFalse(app.buttons["gym-close"].exists)
        move(forward: true, hold: 3.5)
        XCTAssertTrue(label("gym-interest", becomes: "Gym listening"))
        let open = app.buttons["gym-interact"]
        XCTAssertTrue(open.waitForExistence(timeout: 5))
        XCTAssertTrue(open.isEnabled)
        open.tap()
        XCTAssertTrue(app.buttons[runID].waitForExistence(timeout: 10))
    }

    func testInteriorBoardShowsChartsAndStopsWhenPlayerLeaves() throws {
        app.buttons[runID].tap()
        XCTAssertTrue(app.staticTexts["gym-run-title"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["Cost unavailable · 125 s"].exists)
        let values = app.buttons["Recorded values"]
        reveal(values)
        values.tap()
        XCTAssertTrue(app.staticTexts["Step 24: 8 checks"].exists)
        attach("Synthetic Gym Microcoder chart and exact values")
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(label("gym-interest", becomes: "Gym listening"))
        XCTAssertTrue(app.buttons[runID].waitForExistence(timeout: 10))
        app.buttons["gym-close"].tap()
        move(forward: false, hold: 6.0)
        XCTAssertTrue(label("gym-interest", becomes: "Gym idle"))
        XCTAssertFalse(app.buttons["gym-close"].exists)
        XCTAssertFalse(app.buttons[runID].exists)
        attach("Synthetic Gym after leaving its interior")
    }

    func testRecipeReviewRequiresExplicitConfirmationAndPreviewRefusesExecution() throws {
        attach("Synthetic Gym recent and ongoing runs")
        XCTAssertFalse(app.staticTexts["gym-launch-status"].exists)
        let recipe = app.buttons["gym-recipe-preview-recipe"]
        reveal(recipe)
        recipe.tap()
        let confirm = app.buttons["gym-confirm-launch"]
        reveal(confirm)
        XCTAssertTrue(confirm.isEnabled)
        XCTAssertFalse(app.staticTexts["gym-launch-status"].exists)
        XCTAssertTrue(app.staticTexts["No dollar limit is enforced for this recipe."].exists)
        attach("Synthetic Gym explicit recipe review")
        confirm.tap()
        let status = app.staticTexts["gym-launch-status"]
        reveal(status)
        XCTAssertTrue(label("gym-launch-status", becomes: "Request: rejected"))
        XCTAssertFalse(confirm.exists, "Another launch requires a fresh recipe review.")
        XCTAssertTrue(app.staticTexts["Preview only. No training or evaluation was started."].exists)
        attach("Synthetic Gym preview refusal")
    }

    private func move(forward: Bool, hold: TimeInterval) {
        let surface = app.otherElements["verse-surface"]
        let origin = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.17, dy: forward ? 0.66 : 0.42))
        let target = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.17, dy: forward ? 0.42 : 0.66))
        origin.press(forDuration: 0.1, thenDragTo: target, withVelocity: .slow, thenHoldForDuration: hold)
    }

    private func reveal(_ element: XCUIElement) {
        for _ in 0..<4 where !element.isHittable { app.scrollViews.firstMatch.swipeUp() }
        XCTAssertTrue(element.isHittable)
    }

    private func label(_ identifier: String, becomes expected: String) -> Bool {
        let match = XCTNSPredicateExpectation(predicate: NSPredicate(format: "label == %@", expected),
                                              object: app.staticTexts[identifier])
        return XCTWaiter.wait(for: [match], timeout: 10) == .completed
    }

    private func attach(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
