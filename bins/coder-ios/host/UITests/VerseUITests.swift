// These checks mount the actual Metal surface with a synthetic, offline world.
import XCTest

final class VerseUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--synthetic"]
        app.launch()
        XCTAssertTrue(app.tabBars.buttons["Verse"].waitForExistence(timeout: 20))
        app.tabBars.buttons["Verse"].tap()
        XCTAssertTrue(app.otherElements["verse-surface"].waitForExistence(timeout: 30))
        XCTAssertTrue(waitForFrames(after: 0), app.staticTexts["verse-error"].exists ? app.staticTexts["verse-error"].label : "The world presented no frames.")
    }

    func testMetalWorldRendersAndTouchMovementChangesPosition() throws {
        let position = app.staticTexts["verse-position"]
        let before = coordinates(position.label)
        let surface = app.otherElements["verse-surface"]
        let origin = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.2, dy: 0.7))
        let forward = surface.coordinate(withNormalizedOffset: CGVector(dx: 0.2, dy: 0.4))
        origin.press(forDuration: 0.1, thenDragTo: forward, withVelocity: .slow, thenHoldForDuration: 0.8)
        let moved = XCTNSPredicateExpectation(predicate: NSPredicate { [weak self] _, _ in
            guard let self else { return false }
            let after = self.coordinates(position.label)
            return abs(after[0] - before[0]) + abs(after[2] - before[2]) > 0.1
        }, object: position)
        XCTAssertEqual(XCTWaiter.wait(for: [moved], timeout: 5), .completed)
        let frames = frameCount()
        app.buttons["verse-jump"].tap()
        XCTAssertTrue(waitForFrames(after: frames))
        attach("Synthetic Verse Metal world")
    }

    func testTabsAndBackgroundResumeWithoutBreakingChats() throws {
        app.tabBars.buttons["Chats"].tap()
        XCTAssertTrue(app.buttons["chat-0"].waitForExistence(timeout: 10))
        app.buttons["chat-0"].tap()
        XCTAssertTrue(app.buttons["back"].waitForExistence(timeout: 10))
        app.tabBars.buttons["Verse"].tap()
        XCTAssertTrue(waitForFrames(after: 0))
        let before = frameCount()
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(waitForFrames(after: before))
        XCTAssertFalse(app.staticTexts["verse-error"].exists)
        attach("Synthetic Verse after background resume")
    }

    private func coordinates(_ label: String) -> [Double] {
        let values = label.split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespaces)) }
        XCTAssertEqual(values.count, 3)
        return values.count == 3 ? values : [0, 0, 0]
    }

    private func frameCount() -> UInt64 {
        let label = app.staticTexts["verse-frames"].label
        return UInt64(label.split(separator: " ").last ?? "0") ?? 0
    }

    private func waitForFrames(after previous: UInt64) -> Bool {
        let match = XCTNSPredicateExpectation(predicate: NSPredicate { [weak self] _, _ in
            guard let self, self.app.staticTexts["verse-frames"].exists else { return false }
            return self.frameCount() > previous
        }, object: app)
        return XCTWaiter.wait(for: [match], timeout: 30) == .completed
    }

    private func attach(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
